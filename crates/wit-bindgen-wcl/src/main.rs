use anyhow::{bail, Context, Result};
use heck::ToUpperCamelCase;
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::Path;
use wit_parser::{Function, Resolve, Results, Type, TypeDefKind, WorldId, WorldItem};

mod codegen;
use codegen::*;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <wit-file-or-dir> <output-file>", args[0]);
        eprintln!("Example: {} ./my.wit ./bindings.rs", args[0]);
        std::process::exit(1);
    }

    let wit_path = &args[1];
    let output_path = &args[2];

    println!("Parsing WIT from: {}", wit_path);

    let mut resolve = Resolve::default();
    let (package_id, _source_map) = resolve
        .push_path(Path::new(wit_path))
        .context("Failed to parse WIT file")?;

    println!("Parsed package: {:?}", resolve.packages[package_id].name);

    // Find the world to generate bindings for
    let world_id = find_default_world(&resolve, package_id)?;
    let world = &resolve.worlds[world_id];
    println!("Generating bindings for world: {}", world.name);

    // Generate bindings
    let bindings = generate_bindings(&resolve, world_id)?;

    // Write to file
    fs::write(output_path, bindings).context("Failed to write bindings file")?;

    println!("Generated bindings: {}", output_path);
    Ok(())
}

fn find_default_world(resolve: &Resolve, package_id: wit_parser::PackageId) -> Result<WorldId> {
    let package = &resolve.packages[package_id];

    // Find the first world in the package
    if let Some((_name, world_id)) = package.worlds.iter().next() {
        return Ok(*world_id);
    }

    bail!("No worlds found in package")
}

fn generate_bindings(resolve: &Resolve, world_id: WorldId) -> Result<String> {
    let mut output = String::new();

    // Header
    writeln!(
        output,
        "// AUTO-GENERATED WIT BINDINGS for wasm-component-layer"
    )?;
    writeln!(
        output,
        "// DO NOT EDIT - Regenerate using wit-bindgen-wcl\n"
    )?;
    writeln!(
        output,
        "#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]"
    )?;
    writeln!(output)?;
    writeln!(output, "use anyhow::*;")?;
    writeln!(output, "use waclay::*;")?;
    writeln!(output, "use wasm_runtime_layer::{{backend}};")?;
    writeln!(output)?;

    // Check if we need bitflags
    let world = &resolve.worlds[world_id];
    let needs_bitflags = world
        .imports
        .iter()
        .chain(world.exports.iter())
        .any(|(_, item)| {
            if let WorldItem::Interface { id, .. } = item {
                let iface = &resolve.interfaces[*id];
                iface
                    .types
                    .values()
                    .any(|type_id| matches!(&resolve.types[*type_id].kind, TypeDefKind::Flags(_)))
            } else {
                false
            }
        });

    if needs_bitflags {
        writeln!(
            output,
            "// Note: If using flags types, add to your Cargo.toml:"
        )?;
        writeln!(output, "// bitflags = \"2.0\"")?;
        writeln!(output)?;
    }
    writeln!(output)?;
    let mut generator = BindingsGenerator::new(resolve, world_id);

    // Collect all types used in this world
    generator.collect_types();

    // Generate type definitions
    generator.generate_types(&mut output)?;

    // Generate imports (host functions)
    generator.generate_imports(&mut output)?;

    // Generate exports (guest functions)
    generator.generate_exports(&mut output)?;

    Ok(output)
}

struct BindingsGenerator<'a> {
    resolve: &'a Resolve,
    world_id: WorldId,
    types_to_generate: BTreeMap<wit_parser::TypeId, String>,
}

impl<'a> BindingsGenerator<'a> {
    fn new(resolve: &'a Resolve, world_id: WorldId) -> Self {
        Self {
            resolve,
            world_id,
            types_to_generate: BTreeMap::new(),
        }
    }

    fn collect_types(&mut self) {
        let world = &self.resolve.worlds[self.world_id];

        // Collect types from imports
        for (_name, item) in &world.imports {
            self.collect_types_from_item(item);
        }

        // Collect types from exports
        for (_name, item) in &world.exports {
            self.collect_types_from_item(item);
        }
    }

    fn collect_types_from_item(&mut self, item: &WorldItem) {
        match item {
            WorldItem::Function(func) => {
                self.collect_types_from_function(func);
            }
            WorldItem::Interface { id: iface_id, .. } => {
                let iface = &self.resolve.interfaces[*iface_id];
                for (_name, func) in &iface.functions {
                    self.collect_types_from_function(func);
                }
                for (_name, type_id) in &iface.types {
                    self.collect_type(*type_id);
                }
            }
            WorldItem::Type(type_id) => {
                self.collect_type(*type_id);
            }
        }
    }

    fn collect_types_from_function(&mut self, func: &Function) {
        for (_name, ty) in func.params.iter() {
            self.collect_types_from_type(ty);
        }
        match &func.results {
            Results::Named(params) => {
                for (_name, ty) in params.iter() {
                    self.collect_types_from_type(ty);
                }
            }
            Results::Anon(ty) => {
                self.collect_types_from_type(ty);
            }
        }
    }

    fn collect_types_from_type(&mut self, ty: &Type) {
        // In wit-parser 0.219, Type is simpler - just match on Id
        if let Type::Id(type_id) = ty {
            self.collect_type(*type_id);
        }
        // Other variants are primitives or handled elsewhere
    }

    fn collect_type(&mut self, type_id: wit_parser::TypeId) {
        if self.types_to_generate.contains_key(&type_id) {
            return;
        }

        let typedef = &self.resolve.types[type_id];
        
        // Skip Handle types - they're references to resources, not standalone types
        if matches!(typedef.kind, TypeDefKind::Handle(_)) {
            // But we do need to collect the underlying resource type
            if let TypeDefKind::Handle(handle) = &typedef.kind {
                let resource_id = match handle {
                    wit_parser::Handle::Own(id) | wit_parser::Handle::Borrow(id) => *id,
                };
                self.collect_type(resource_id);
            }
            return;
        }
        
        let name = typedef
            .name
            .as_ref()
            .map(|n| n.to_upper_camel_case())
            .unwrap_or_else(|| format!("Type{:?}", type_id));

        self.types_to_generate.insert(type_id, name);

        // Recursively collect nested types
        match &typedef.kind {
            TypeDefKind::Record(record) => {
                for field in &record.fields {
                    self.collect_types_from_type(&field.ty);
                }
            }
            TypeDefKind::Variant(variant) => {
                for case in &variant.cases {
                    if let Some(ty) = &case.ty {
                        self.collect_types_from_type(ty);
                    }
                }
            }
            TypeDefKind::Enum(_) => {}
            TypeDefKind::List(ty) | TypeDefKind::Option(ty) => {
                self.collect_types_from_type(ty);
            }
            TypeDefKind::Result(result) => {
                if let Some(ok) = &result.ok {
                    self.collect_types_from_type(ok);
                }
                if let Some(err) = &result.err {
                    self.collect_types_from_type(err);
                }
            }
            TypeDefKind::Tuple(tuple) => {
                for ty in &tuple.types {
                    self.collect_types_from_type(ty);
                }
            }
            TypeDefKind::Type(ty) => {
                self.collect_types_from_type(ty);
            }
            _ => {}
        }
    }

    fn generate_types(&self, output: &mut String) -> Result<()> {
        writeln!(output, "// ========== Type Definitions ==========")?;
        writeln!(output)?;

        for (type_id, rust_name) in &self.types_to_generate {
            let typedef = &self.resolve.types[*type_id];
            generate_type_definition(self.resolve, typedef, rust_name, output)?;
            writeln!(output)?;
        }

        Ok(())
    }

    fn generate_imports(&self, output: &mut String) -> Result<()> {
        let world = &self.resolve.worlds[self.world_id];
        let imports: Vec<_> = world.imports.iter().collect();

        if imports.is_empty() {
            return Ok(());
        }

        writeln!(output, "// ========== Host Imports ==========")?;
        writeln!(output)?;

        // Generate trait definitions for each interface and top-level function
        for (name, item) in &imports {
            match item {
                WorldItem::Interface { id: iface_id, .. } => {
                    let iface = &self.resolve.interfaces[*iface_id];
                    let name_str = self.resolve.name_world_key(name);
                    generate_import_trait(self.resolve, &name_str, iface, *iface_id, output)?;
                }
                WorldItem::Function(func) => {
                    // Top-level function import
                    let name_str = self.resolve.name_world_key(name);
                    generate_toplevel_import_trait(self.resolve, &name_str, func, output)?;
                }
                _ => {}
            }
        }

        // Generate a single imports module with all registration functions
        writeln!(output, "pub mod imports {{")?;
        writeln!(output, "    use super::*;")?;
        writeln!(output)?;

        for (name, item) in &imports {
            match item {
                WorldItem::Interface { id: iface_id, .. } => {
                    let iface = &self.resolve.interfaces[*iface_id];
                    let name_str = self.resolve.name_world_key(name);
                    generate_import_registration_function(
                        self.resolve,
                        &name_str,
                        iface,
                        *iface_id,
                        output,
                    )?;
                }
                WorldItem::Function(func) => {
                    // Top-level function import
                    let name_str = self.resolve.name_world_key(name);
                    generate_toplevel_import_registration(self.resolve, &name_str, func, output)?;
                }
                _ => {}
            }
        }

        writeln!(output, "}}")?;
        writeln!(output)?;

        Ok(())
    }

    fn generate_exports(&self, output: &mut String) -> Result<()> {
        let world = &self.resolve.worlds[self.world_id];
        let exports: Vec<_> = world.exports.iter().collect();

        if exports.is_empty() {
            return Ok(());
        }

        writeln!(output, "// ========== Guest Exports ==========")?;
        writeln!(output)?;

        for (name, item) in exports {
            match item {
                WorldItem::Interface { id: iface_id, .. } => {
                    let iface = &self.resolve.interfaces[*iface_id];
                    let name_str = self.resolve.name_world_key(name);
                    generate_export_interface(self.resolve, &name_str, iface, output)?;
                }
                WorldItem::Function(func) => {
                    // Top-level function export
                    let name_str = self.resolve.name_world_key(name);
                    generate_toplevel_export_helper(self.resolve, &name_str, func, output)?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
