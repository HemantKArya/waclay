//! Bindgen macro implementation

use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::path::PathBuf;
use syn::parse::{Error, Parse, ParseStream, Result as ParseResult};
use syn::{LitStr, Token, braced};
use wit_parser::{Resolve, WorldId};

/// Configuration for the bindgen macro
pub struct Config {
    opts: wasm_component_layer_bindgen::Opts,
    resolve: Resolve,
    world: WorldId,
    files: Vec<PathBuf>,
}

impl Parse for Config {
    fn parse(input: ParseStream<'_>) -> ParseResult<Self> {
        let mut opts = wasm_component_layer_bindgen::Opts::default();
        let mut path = None;
        let mut inline = None;
        let mut world = None;
        let mut files = Vec::new();

        let content;
        braced!(content in input);

        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            let _: Token![:] = content.parse()?;

            match key.to_string().as_str() {
                "path" => {
                    let value: LitStr = content.parse()?;
                    path = Some(value.value());
                }
                "inline" => {
                    let value: LitStr = content.parse()?;
                    inline = Some(value.value());
                }
                "world" => {
                    let value: LitStr = content.parse()?;
                    world = Some(value.value());
                }
                "imports_only" => {
                    let value: syn::LitBool = content.parse()?;
                    opts.imports_only = value.value;
                }
                "exports_only" => {
                    let value: syn::LitBool = content.parse()?;
                    opts.exports_only = value.value;
                }
                other => {
                    return Err(Error::new(key.span(), format!("unknown key: {}", other)));
                }
            }

            if content.peek(Token![,]) {
                let _: Token![,] = content.parse()?;
            }
        }

        // Parse WIT file
        let mut resolve = Resolve::default();
        let pkg = if let Some(inline_wit) = inline {
            resolve
                .push_str("inline.wit", &inline_wit)
                .map_err(|e| Error::new(Span::call_site(), format!("failed to parse inline WIT: {}", e)))?
        } else if let Some(path_str) = path {
            let path = PathBuf::from(path_str);
            if !path.exists() {
                return Err(Error::new(Span::call_site(), format!("path does not exist: {}", path.display())));
            }

            files.push(path.clone());

            let (pkg, _) = resolve
                .push_path(&path)
                .map_err(|e| Error::new(Span::call_site(), format!("failed to parse WIT file: {}", e)))?;
            pkg
        } else {
            return Err(Error::new(Span::call_site(), "either 'path' or 'inline' must be specified"));
        };

        // Find the world
        let world_name = world.as_ref().map(String::as_str);
        let pkg_data = &resolve.packages[pkg];
        
        let world_id = if let Some(name) = world_name {
            pkg_data.worlds.values()
                .find(|&w_id| {
                    resolve.worlds[*w_id].name == name
                })
                .ok_or_else(|| Error::new(Span::call_site(), format!("world '{}' not found", name)))?
                .clone()
        } else if pkg_data.worlds.len() == 1 {
            *pkg_data.worlds.values().next().unwrap()
        } else {
            return Err(Error::new(Span::call_site(), "multiple worlds found, please specify 'world'"));
        };

        Ok(Config {
            opts,
            resolve,
            world: world_id,
            files,
        })
    }
}

pub fn expand(config: &Config) -> syn::Result<TokenStream> {
    let src = config.opts.generate(&config.resolve, config.world)
        .map_err(|e| syn::Error::new(Span::call_site(), e.to_string()))?;

    let tokens: TokenStream = src.parse()
        .map_err(|e: proc_macro2::LexError| syn::Error::new(Span::call_site(), e.to_string()))?;

    // Note: File dependency tracking is disabled for now to avoid path issues
    // TODO: Make path tracking work correctly with workspace and example builds
    
    Ok(tokens)
}
