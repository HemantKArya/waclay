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
use rust::{RustGenerator, TypeMode, to_rust_ident, to_rust_upper_camel_case};
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
        self.generate_world_struct()?;
        self.generate_imports()?;
        self.generate_exports()?;
        
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

    fn generate_import_interface(&mut self, _name: &WorldKey, _id: InterfaceId) -> Result<()> {
        // TODO: Implement interface imports
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
        uwrite!(self.src, "    pub fn {}(&self", func_name);

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
        uwriteln!(self.src, "        // TODO: Implement export function call");
        uwriteln!(self.src, "        unimplemented!()");
        uwriteln!(self.src, "    }}");
        uwriteln!(self.src, "}}");
        uwriteln!(self.src, "");

        Ok(())
    }

    fn generate_export_interface(&mut self, _name: &WorldKey, _id: InterfaceId) -> Result<()> {
        // TODO: Implement interface exports
        Ok(())
    }
}

impl RustGenerator for Generator {
    fn resolve(&self) -> &Resolve {
        &self.resolve
    }

    fn push_str(&mut self, s: &str) {
        self.src.push_str(s);
    }

    fn info(&self, ty: TypeId) -> TypeInfo {
        self.type_info.get(&ty).cloned().unwrap_or_default()
    }

    fn path_to_interface(&self, _interface: InterfaceId) -> Option<String> {
        None
    }

    fn is_imported_interface(&self, interface: InterfaceId) -> bool {
        self.interface_last_seen_as_import.get(&interface).copied().unwrap_or(false)
    }

    fn wasmtime_path(&self) -> String {
        "wasm_component_layer".to_string()
    }

    fn ownership(&self) -> Ownership {
        Ownership::Owning
    }
    
    fn print_ty(&mut self, ty: &Type, mode: TypeMode) {
        let s = self.ty(ty, mode);
        self.push_str(&s);
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
            Type::String => match mode {
                TypeMode::AllBorrowed(_) => "&str".to_string(),
                TypeMode::Owned => "String".to_string(),
            },
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ownership {
    Owning,
    Borrowing,
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
