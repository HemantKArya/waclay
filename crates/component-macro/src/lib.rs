//! Procedural macros for wasm_component_layer
//!
//! This crate provides the `bindgen!` macro for generating host-side bindings
//! from WIT definitions.

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod bindgen;

/// Generate host-side bindings from a WIT world
///
/// # Example
///
/// ```ignore
/// use wasm_component_layer::bindgen;
///
/// bindgen!({
///     path: "wit/world.wit",
///     world: "my-world",
/// });
/// ```
#[proc_macro]
pub fn bindgen(input: TokenStream) -> TokenStream {
    let config = parse_macro_input!(input as bindgen::Config);
    bindgen::expand(&config)
        .unwrap_or_else(|e| {
            let msg = e.to_string();
            quote::quote! {
                compile_error!(#msg);
            }
        })
        .into()
}
