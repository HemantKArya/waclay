//! Type information tracking

/// Information about a type used during code generation
#[derive(Default, Debug, Clone)]
pub struct TypeInfo {
    /// Whether this type contains a list somewhere in its structure
    pub has_list: bool,
}
