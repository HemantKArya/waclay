//! Code generation for wasm_component_layer host bindings
//!
//! This crate generates Rust bindings from WIT files to enable runtime-agnostic
//! host-side interaction with WebAssembly components.

use anyhow::Result;
use std::collections::HashMap;
use std::fmt::Write as _;
use wit_parser::*;

mod rust;
mod source;
mod types;

pub use types::TypeInfo;
use rust::{TypeMode, to_rust_ident, to_rust_upper_camel_case};
use source::Source;

/// Configuration options for code generation
#[derive(Default, Clone)]
pub struct Opts {
    /// Whether to generate bindings only for imports
    pub imports_only: bool,
    /// Whether to generate bindings only for exports
    pub exports_only: bool,
    /// Additional derive attributes to add to generated types
    pub additional_derive_attributes: Vec<String>,
}

impl Opts {
    /// Generate Rust bindings for the given WIT world
    pub fn generate(&self, resolve: &Resolve, world: WorldId) -> Result<String> {
        let mut gen = Generator::new(self.clone(), resolve, world);
        gen.generate()
    }
}

struct Generator {
    opts: Opts,
    resolve: Resolve,
    world: WorldId,
    src: Source,
    type_info: HashMap<TypeId, TypeInfo>,
    interface_last_seen_as_import: HashMap<InterfaceId, bool>,
}

impl Generator {
    fn new(opts: Opts, resolve: &Resolve, world: WorldId) -> Self {
        Self {
            opts,
            resolve: resolve.clone(),
            world,
            src: Source::default(),
            type_info: HashMap::new(),
            interface_last_seen_as_import: HashMap::new(),
        }
    }

    fn generate(&mut self) -> Result<String> {
        self.analyze_types();
        self.generate_type_definitions()?;
        self.generate_world_struct()?;
        self.generate_imports()?;
        self.generate_exports()?;
        self.generate_add_to_linker()?;
        
        Ok(self.src.to_string())
    }

    fn analyze_types(&mut self) {
        // Analyze all types in the world to gather information
        let world = &self.resolve.worlds[self.world];
        
        // Collect interface IDs first to avoid borrowing issues
        let import_interfaces: Vec<_> = world.imports.iter()
            .filter_map(|(_, item)| {
                if let WorldItem::Interface { id, .. } = item {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
            
        let export_interfaces: Vec<_> = world.exports.iter()
            .filter_map(|(_, item)| {
                if let WorldItem::Interface { id, .. } = item {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        
        for id in import_interfaces {
            self.analyze_interface(id, true);
        }
        
        for id in export_interfaces {
            self.analyze_interface(id, false);
        }
    }

    fn analyze_interface(&mut self, id: InterfaceId, is_import: bool) {
        self.interface_last_seen_as_import.insert(id, is_import);
        
        // Collect type IDs first to avoid borrowing issues
        let type_ids: Vec<_> = self.resolve.interfaces[id].types.values().copied().collect();
        
        for ty_id in type_ids {
            self.analyze_type(ty_id);
        }
    }

    fn analyze_type(&mut self, id: TypeId) {
        if self.type_info.contains_key(&id) {
            return;
        }

        let ty = &self.resolve.types[id];
        let mut info = TypeInfo::default();

        // Collect nested type IDs first
        let nested_types: Vec<TypeId> = match &ty.kind {
            TypeDefKind::List(Type::Id(id)) => vec![*id],
            TypeDefKind::Record(r) => r.fields.iter()
                .filter_map(|f| if let Type::Id(id) = &f.ty { Some(*id) } else { None })
                .collect(),
            TypeDefKind::Variant(v) => v.cases.iter()
                .filter_map(|c| if let Some(Type::Id(id)) = &c.ty { Some(*id) } else { None })
                .collect(),
            TypeDefKind::Option(Type::Id(id)) => vec![*id],
            TypeDefKind::Result(r) => {
                let mut ids = Vec::new();
                if let Some(Type::Id(id)) = &r.ok {
                    ids.push(*id);
                }
                if let Some(Type::Id(id)) = &r.err {
                    ids.push(*id);
                }
                ids
            }
            TypeDefKind::Tuple(t) => t.types.iter()
                .filter_map(|ty| if let Type::Id(id) = ty { Some(*id) } else { None })
                .collect(),
            _ => Vec::new(),
        };

        if matches!(ty.kind, TypeDefKind::List(_)) {
            info.has_list = true;
        }

        self.type_info.insert(id, info);

        // Now recursively analyze nested types
        for nested_id in nested_types {
            self.analyze_type(nested_id);
        }
    }

    fn generate_type_definitions(&mut self) -> Result<()> {
        // Collect all type definitions that need to be generated
        let mut type_ids: Vec<TypeId> = Vec::new();
        
        for (id, ty) in self.resolve.types.iter() {
            if ty.name.is_some() && matches!(ty.kind, 
                TypeDefKind::Record(_) | TypeDefKind::Variant(_) | 
                TypeDefKind::Enum(_) | TypeDefKind::Flags(_)) {
                type_ids.push(id);
            }
        }

        // Generate type definitions
        for id in type_ids {
            self.generate_type_definition(id)?;
        }

        Ok(())
    }

    fn generate_type_definition(&mut self, id: TypeId) -> Result<()> {
        let ty = &self.resolve.types[id];
        let name = ty.name.as_ref().unwrap().clone();
        let kind = ty.kind.clone();
        
        match &kind {
            TypeDefKind::Record(r) => self.generate_record(&name, r)?,
            TypeDefKind::Variant(v) => self.generate_variant(&name, v)?,
            TypeDefKind::Enum(e) => self.generate_enum(&name, e)?,
            TypeDefKind::Flags(f) => self.generate_flags(&name, f)?,
            _ => {}
        }

        Ok(())
    }

    fn generate_record(&mut self, name: &str, record: &Record) -> Result<()> {
        let type_name = to_rust_upper_camel_case(name);
        
        uwriteln!(self.src, "#[derive(Debug, Clone, PartialEq)]");
        uwriteln!(self.src, "pub struct {} {{", type_name);
        
        for field in &record.fields {
            let field_name = to_rust_ident(&field.name);
            uwrite!(self.src, "    pub {}: ", field_name);
            self.print_ty(&field.ty, TypeMode::Owned);
            uwriteln!(self.src, ",");
        }
        
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");
        
        Ok(())
    }

    fn generate_variant(&mut self, name: &str, variant: &Variant) -> Result<()> {
        let type_name = to_rust_upper_camel_case(name);
        
        uwriteln!(self.src, "#[derive(Debug, Clone, PartialEq)]");
        uwriteln!(self.src, "pub enum {} {{", type_name);
        
        for case in &variant.cases {
            let case_name = to_rust_upper_camel_case(&case.name);
            if let Some(ty) = &case.ty {
                uwrite!(self.src, "    {}(", case_name);
                self.print_ty(ty, TypeMode::Owned);
                uwriteln!(self.src, "),");
            } else {
                uwriteln!(self.src, "    {},", case_name);
            }
        }
        
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");
        
        Ok(())
    }

    fn generate_enum(&mut self, name: &str, enum_: &Enum) -> Result<()> {
        let type_name = to_rust_upper_camel_case(name);
        
        uwriteln!(self.src, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
        uwriteln!(self.src, "pub enum {} {{", type_name);
        
        for case in &enum_.cases {
            let case_name = to_rust_upper_camel_case(&case.name);
            uwriteln!(self.src, "    {},", case_name);
        }
        
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");
        
        Ok(())
    }

    fn generate_flags(&mut self, name: &str, flags: &Flags) -> Result<()> {
        let type_name = to_rust_upper_camel_case(name);
        
        uwriteln!(self.src, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
        uwriteln!(self.src, "pub struct {} {{", type_name);
        uwriteln!(self.src, "    bits: u32,");
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");
        
        uwriteln!(self.src, "impl {} {{", type_name);
        
        for (i, flag) in flags.flags.iter().enumerate() {
            let flag_name = to_rust_ident(&flag.name).to_uppercase();
            uwriteln!(self.src, "    pub const {}: Self = Self {{ bits: 1 << {} }};", flag_name, i);
        }
        
        uwriteln!(self.src, "");
        uwriteln!(self.src, "    pub const fn empty() -> Self {{");
        uwriteln!(self.src, "        Self {{ bits: 0 }}");
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "");
        uwriteln!(self.src, "    pub const fn all() -> Self {{");
        let all_bits = (1u32 << flags.flags.len()) - 1;
        uwriteln!(self.src, "        Self {{ bits: {} }}", all_bits);
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "");
        uwriteln!(self.src, "    pub const fn contains(&self, other: Self) -> bool {{");
        uwriteln!(self.src, "        (self.bits & other.bits) == other.bits");
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "");
        uwriteln!(self.src, "    pub const fn insert(&mut self, other: Self) {{");
        uwriteln!(self.src, "        self.bits |= other.bits;");
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "");
        uwriteln!(self.src, "    pub const fn remove(&mut self, other: Self) {{");
        uwriteln!(self.src, "        self.bits &= !other.bits;");
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");
        
        Ok(())
    }

    fn generate_value_conversion(&mut self, param_name: &str, param_type: &Type) -> Result<()> {
        let ident = to_rust_ident(param_name);
        match param_type {
            Type::Bool => uwrite!(self.src, "wasm_component_layer::Value::Bool({})", ident),
            Type::S8 => uwrite!(self.src, "wasm_component_layer::Value::S8({})", ident),
            Type::U8 => uwrite!(self.src, "wasm_component_layer::Value::U8({})", ident),
            Type::S16 => uwrite!(self.src, "wasm_component_layer::Value::S16({})", ident),
            Type::U16 => uwrite!(self.src, "wasm_component_layer::Value::U16({})", ident),
            Type::S32 => uwrite!(self.src, "wasm_component_layer::Value::S32({})", ident),
            Type::U32 => uwrite!(self.src, "wasm_component_layer::Value::U32({})", ident),
            Type::S64 => uwrite!(self.src, "wasm_component_layer::Value::S64({})", ident),
            Type::U64 => uwrite!(self.src, "wasm_component_layer::Value::U64({})", ident),
            Type::F32 => uwrite!(self.src, "wasm_component_layer::Value::F32({})", ident),
            Type::F64 => uwrite!(self.src, "wasm_component_layer::Value::F64({})", ident),
            Type::Char => uwrite!(self.src, "wasm_component_layer::Value::Char({})", ident),
            Type::String => uwrite!(self.src, "wasm_component_layer::Value::String({}.into())", ident),
            Type::Id(id) => {
                let ty = &self.resolve.types[*id];
                match &ty.kind {
                    TypeDefKind::List(_) => uwrite!(self.src, "wasm_component_layer::Value::List({}.into())", ident),
                    TypeDefKind::Option(_) => uwrite!(self.src, "wasm_component_layer::Value::Option({}.into())", ident),
                    TypeDefKind::Result(_) => uwrite!(self.src, "wasm_component_layer::Value::Result({}.into())", ident),
                    _ => uwrite!(self.src, "{}.try_into()?", ident),
                }
            }
            _ => uwrite!(self.src, "{}.try_into()?", ident),
        }
        Ok(())
    }

    fn generate_world_struct(&mut self) -> Result<()> {
        let world = &self.resolve.worlds[self.world];
        let world_name = to_rust_upper_camel_case(&world.name);

        uwriteln!(self.src, "/// Auto-generated bindings for a component that implements the");
        uwriteln!(self.src, "/// world `{}`", world.name);
        uwriteln!(self.src, "pub struct {} {{", world_name);
        uwriteln!(self.src, "    instance: wasm_component_layer::Instance,");
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");

        Ok(())
    }

    fn generate_imports(&mut self) -> Result<()> {
        if self.opts.exports_only {
            return Ok(());
        }

        let world = &self.resolve.worlds[self.world];
        if world.imports.is_empty() {
            return Ok(());
        }

        // Collect import information first
        let imports: Vec<_> = world.imports.iter()
            .map(|(name, item)| (name.clone(), item.clone()))
            .collect();

        uwriteln!(self.src, "/// Trait representing the imports required by this component");
        uwriteln!(self.src, "pub trait HostImports {{");

        for (name, item) in imports {
            match item {
                WorldItem::Function(func) => {
                    self.generate_import_function(&name, &func)?;
                }
                WorldItem::Interface { id, .. } => {
                    self.generate_import_interface(&name, id)?;
                }
                WorldItem::Type(_) => {}
            }
        }

        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");

        Ok(())
    }

    fn generate_import_function(&mut self, name: &WorldKey, func: &Function) -> Result<()> {
        let func_name = match name {
            WorldKey::Name(s) => to_rust_ident(s),
            WorldKey::Interface(_) => to_rust_ident(&func.name),
        };

        uwrite!(self.src, "    fn {}(", func_name);
        uwrite!(self.src, "&mut self");

        for (param_name, param_type) in func.params.iter() {
            uwrite!(self.src, ", {}: ", to_rust_ident(param_name));
            self.print_ty(param_type, TypeMode::Owned);
        }

        uwrite!(self.src, ") -> anyhow::Result<");
        if let Some(result_ty) = &func.result {
            self.print_ty(result_ty, TypeMode::Owned);
        } else {
            uwrite!(self.src, "()");
        }
        uwriteln!(self.src, ">;");

        Ok(())
    }

    fn generate_import_interface(&mut self, name: &WorldKey, id: InterfaceId) -> Result<()> {
        let iface = &self.resolve.interfaces[id];
        let interface_name = match name {
            WorldKey::Name(s) => to_rust_upper_camel_case(s),
            WorldKey::Interface(_) => {
                if let Some(n) = &iface.name {
                    to_rust_upper_camel_case(n)
                } else {
                    "UnnamedInterface".to_string()
                }
            }
        };

        // Generate trait for the interface
        uwriteln!(self.src, "    /// Interface: {}", interface_name);
        
        // Collect functions from the interface
        let functions: Vec<_> = iface.functions.values().cloned().collect();
        
        for func in functions {
            uwrite!(self.src, "    fn {}(", to_rust_ident(&func.name));
            uwrite!(self.src, "&mut self");
            
            for (param_name, param_type) in func.params.iter() {
                uwrite!(self.src, ", {}: ", to_rust_ident(param_name));
                self.print_ty(param_type, TypeMode::Owned);
            }
            
            uwrite!(self.src, ") -> anyhow::Result<");
            if let Some(result_ty) = &func.result {
                self.print_ty(result_ty, TypeMode::Owned);
            } else {
                uwrite!(self.src, "()");
            }
            uwriteln!(self.src, ">;");
        }

        Ok(())
    }

    fn generate_exports(&mut self) -> Result<()> {
        if self.opts.imports_only {
            return Ok(());
        }

        let world = &self.resolve.worlds[self.world];
        if world.exports.is_empty() {
            return Ok(());
        }

        // Collect export information first
        let exports: Vec<_> = world.exports.iter()
            .map(|(name, item)| (name.clone(), item.clone()))
            .collect();

        for (name, item) in exports {
            match item {
                WorldItem::Function(func) => {
                    self.generate_export_function(&name, &func)?;
                }
                WorldItem::Interface { id, .. } => {
                    self.generate_export_interface(&name, id)?;
                }
                WorldItem::Type(_) => {}
            }
        }

        Ok(())
    }

    fn generate_export_function(&mut self, name: &WorldKey, func: &Function) -> Result<()> {
        let func_name = match name {
            WorldKey::Name(s) => to_rust_ident(s),
            WorldKey::Interface(_) => to_rust_ident(&func.name),
        };

        let world = &self.resolve.worlds[self.world];
        let world_name = to_rust_upper_camel_case(&world.name);

        uwriteln!(self.src, "impl {} {{", world_name);
        uwrite!(self.src, "    pub fn {}(&self, store: &mut impl wasm_component_layer::AsContextMut", func_name);

        for (param_name, param_type) in func.params.iter() {
            uwrite!(self.src, ", {}: ", to_rust_ident(param_name));
            self.print_ty(param_type, TypeMode::Owned);
        }

        uwrite!(self.src, ") -> anyhow::Result<");
        if let Some(result_ty) = &func.result {
            self.print_ty(result_ty, TypeMode::Owned);
        } else {
            uwrite!(self.src, "()");
        }
        uwriteln!(self.src, "> {{");
        
        // Generate actual implementation
        uwriteln!(self.src, "        let exports = self.instance.exports();");
        uwriteln!(self.src, "        let root = exports.root();");
        uwriteln!(self.src, "        let func = root.func(\"{}\").ok_or_else(|| anyhow::anyhow!(\"function not found\"))?;", func.name);
        
        // Build parameters
        if func.params.is_empty() {
            uwriteln!(self.src, "        let params = vec![];");
        } else {
            uwriteln!(self.src, "        let params = vec![");
            for (i, (param_name, param_type)) in func.params.iter().enumerate() {
                if i > 0 {
                    uwriteln!(self.src, ",");
                }
                uwrite!(self.src, "            ");
                self.generate_value_conversion(param_name, param_type)?;
            }
            uwriteln!(self.src, "");
            uwriteln!(self.src, "        ];");
        }
        
        // Call function
        if func.result.is_some() {
            uwriteln!(self.src, "        let mut results = vec![wasm_component_layer::Value::Bool(false)];");
            uwriteln!(self.src, "        func.call(store, &params, &mut results)?;");
            uwriteln!(self.src, "        wasm_component_layer::ComponentType::from_value(&results[0])");
        } else {
            uwriteln!(self.src, "        let mut results = vec![];");
            uwriteln!(self.src, "        func.call(store, &params, &mut results)?;");
            uwriteln!(self.src, "        Ok(())");
        }
        
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");

        Ok(())
    }

    fn generate_export_interface(&mut self, name: &WorldKey, id: InterfaceId) -> Result<()> {
        let iface = &self.resolve.interfaces[id];
        let interface_name = match name {
            WorldKey::Name(s) => to_rust_upper_camel_case(s),
            WorldKey::Interface(_) => {
                if let Some(n) = &iface.name {
                    to_rust_upper_camel_case(n)
                } else {
                    "UnnamedInterface".to_string()
                }
            }
        };

        // Generate a simple comment noting this is an interface
        uwriteln!(self.src, "// Interface export: {}", interface_name);
        uwriteln!(self.src, "// TODO: Implement interface '{}' export methods", interface_name);
        uwriteln!(self.src, "");

        Ok(())
    }

    fn generate_add_to_linker(&mut self) -> Result<()> {
        let world = &self.resolve.worlds[self.world];
        
        // Only generate if there are imports
        if world.imports.is_empty() {
            return Ok(());
        }
        
        uwriteln!(self.src, "/// Helper function to add all imports to a linker");
        uwriteln!(self.src, "/// Note: This is a simplified implementation - you may need to adapt it to your use case");
        uwriteln!(self.src, "pub fn add_to_linker<T: HostImports + 'static>(");
        uwriteln!(self.src, "    _linker: &mut wasm_component_layer::Linker,");
        uwriteln!(self.src, ") -> anyhow::Result<()> {{");
        uwriteln!(self.src, "    // TODO: Implement linker binding");
        uwriteln!(self.src, "    // You need to define functions in the linker that call your HostImports trait methods");
        uwriteln!(self.src, "    Ok(())");
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");

        Ok(())
    }
    
    fn print_ty(&mut self, ty: &Type, mode: TypeMode) {
        let s = self.ty(ty, mode);
        self.src.push_str(&s);
    }
    
    fn ty(&self, ty: &Type, mode: TypeMode) -> String {
        match ty {
            Type::Id(t) => self.tyid(*t, mode),
            Type::Bool => "bool".to_string(),
            Type::U8 => "u8".to_string(),
            Type::U16 => "u16".to_string(),
            Type::U32 => "u32".to_string(),
            Type::U64 => "u64".to_string(),
            Type::S8 => "i8".to_string(),
            Type::S16 => "i16".to_string(),
            Type::S32 => "i32".to_string(),
            Type::S64 => "i64".to_string(),
            Type::F32 => "f32".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Char => "char".to_string(),
            Type::String => "String".to_string(),
            // Handle ErrorContext as a placeholder type
            _ => "()".to_string(),
        }
    }
    
    fn tyid(&self, id: TypeId, mode: TypeMode) -> String {
        let ty = &self.resolve.types[id];
        let name = ty.name.as_ref().map(|s| to_rust_upper_camel_case(s));

        match &ty.kind {
            TypeDefKind::List(t) => {
                format!("Vec<{}>", self.ty(t, mode))
            }
            TypeDefKind::Option(t) => {
                format!("Option<{}>", self.ty(t, mode))
            }
            TypeDefKind::Result(r) => {
                let ok = r.ok.as_ref().map(|t| self.ty(t, mode)).unwrap_or_else(|| "()".to_string());
                let err = r.err.as_ref().map(|t| self.ty(t, mode)).unwrap_or_else(|| "()".to_string());
                format!("Result<{}, {}>", ok, err)
            }
            TypeDefKind::Tuple(t) => {
                let types = t.types.iter()
                    .map(|ty| self.ty(ty, mode))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", types)
            }
            TypeDefKind::Record(_) | TypeDefKind::Variant(_) | 
            TypeDefKind::Enum(_) | TypeDefKind::Flags(_) | 
            TypeDefKind::Resource => {
                name.unwrap_or_else(|| "UnnamedType".to_string())
            }
            TypeDefKind::Handle(_) => {
                name.unwrap_or_else(|| "Handle".to_string())
            }
            TypeDefKind::Type(t) => self.ty(t, mode),
            _ => name.unwrap_or_else(|| "UnknownType".to_string()),
        }
    }
}

macro_rules! uwrite {
    ($dst:expr, $($arg:tt)*) => {
        write!($dst, $($arg)*).unwrap()
    };
}

macro_rules! uwriteln {
    ($dst:expr, $($arg:tt)*) => {
        writeln!($dst, $($arg)*).unwrap()
    };
}

pub(crate) use {uwrite, uwriteln};
