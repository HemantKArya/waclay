//! Rust code generation utilities

use heck::*;

/// Type mode for code generation
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TypeMode {
    /// Generate owning types
    Owned,
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
