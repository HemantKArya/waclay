//! Rust code generation utilities

use crate::{Ownership, types::TypeInfo};
use heck::*;
use wit_parser::*;

/// Type mode for code generation
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TypeMode {
    /// Generate owning types
    Owned,
    /// Generate borrowing types with lifetime
    AllBorrowed(&'static str),
}

/// Trait for generating Rust code
pub trait RustGenerator {
    fn resolve(&self) -> &Resolve;
    fn push_str(&mut self, s: &str);
    fn info(&self, ty: TypeId) -> TypeInfo;
    fn path_to_interface(&self, interface: InterfaceId) -> Option<String>;
    fn is_imported_interface(&self, interface: InterfaceId) -> bool;
    fn wasmtime_path(&self) -> String;
    fn ownership(&self) -> Ownership;
    
    fn print_ty(&mut self, ty: &Type, mode: TypeMode);
    fn ty(&self, ty: &Type, mode: TypeMode) -> String;
    fn tyid(&self, id: TypeId, mode: TypeMode) -> String;
}

/// Convert a WIT identifier to a Rust identifier
pub fn to_rust_ident(name: &str) -> String {
    let name = name.to_snake_case();
    match name.as_str() {
        // Rust keywords that need to be escaped
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern" |
        "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" |
        "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static" |
        "struct" | "super" | "trait" | "true" | "type" | "unsafe" | "use" | "where" |
        "while" | "async" | "await" | "dyn" | "abstract" | "become" | "box" | "do" |
        "final" | "macro" | "override" | "priv" | "typeof" | "unsized" | "virtual" |
        "yield" | "try" => format!("{}_", name),
        _ => name,
    }
}

/// Convert a WIT name to a Rust upper camel case identifier
pub fn to_rust_upper_camel_case(name: &str) -> String {
    name.to_upper_camel_case()
}
