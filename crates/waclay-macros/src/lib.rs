//! `waclay-macros`: the `bindgen!` procedural macro.
//!
//! Generates typed Rust bindings from WIT world definitions:
//!
//! * Rust types for every record / variant / enum / flags type
//! * a `trait {World}Imports` for host-side implementations
//! * `add_{interface}_to_linker` helpers wiring those impls into a
//!   [`waclay::Linker`](https://docs.rs/waclay)
//! * a `{World}Exports` struct with typed accessors for guest exports

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};

mod wit;

use wit::{FuncDecl, Item, Package, TypeDecl, WitType, World, WorldItem};

// ---------------------------------------------------------------------------
// Macro entry
// ---------------------------------------------------------------------------

struct BindgenInput {
    world: Option<String>,
    path: String,
}

fn parse_input(input: TokenStream) -> BindgenInput {
    let trees: Vec<proc_macro::TokenTree> = input.into_iter().collect();
    // form 1: "path/to/world.wit"
    if let Some(proc_macro::TokenTree::Literal(lit)) = trees.first() {
        let s = literal_str(lit);
        return BindgenInput {
            world: None,
            path: s,
        };
    }
    // form 2: world-name in "dir"
    if trees.len() >= 3 {
        let world = match &trees[0] {
            proc_macro::TokenTree::Ident(i) => i.to_string(),
            _ => panic!("bindgen!: expected world name"),
        };
        let dir = match &trees[2] {
            proc_macro::TokenTree::Literal(lit) => literal_str(lit),
            other => panic!("bindgen!: expected directory string, found {other}"),
        };
        return BindgenInput {
            world: Some(world),
            path: dir,
        };
    }
    panic!("bindgen!: usage: bindgen!(\"wit/world.wit\") or bindgen!(world in \"wit/\")")
}

fn literal_str(lit: &proc_macro::Literal) -> String {
    let s = lit.to_string();
    s.trim_matches('"').to_string()
}

fn resolve_path(path: &str) -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&manifest).join(path)
}

/// Read either a single .wit file or every .wit file in a directory.
fn read_sources(path: &str) -> Vec<(String, String)> {
    let p = resolve_path(path);
    if p.is_file() {
        let src = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("bindgen!: cannot read {}: {e}", p.display()));
        return vec![(p.display().to_string(), src)];
    }
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&p)
        .unwrap_or_else(|e| panic!("bindgen!: cannot read directory {}: {e}", p.display()));
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "wit").unwrap_or(false))
        .collect();
    paths.sort();
    for file in paths {
        let src = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("bindgen!: cannot read {}: {e}", file.display()));
        out.push((file.display().to_string(), src));
    }
    if out.is_empty() {
        panic!("bindgen!: no .wit files found in {}", p.display());
    }
    out
}

#[proc_macro]
pub fn bindgen(input: TokenStream) -> TokenStream {
    let args = parse_input(input);
    let sources = read_sources(&args.path);
    let mut packages = Vec::new();
    for (file, src) in &sources {
        let pkg = wit::parse_wit(src).unwrap_or_else(|e| panic!("bindgen!: {file}: {e}"));
        packages.push(pkg);
    }

    let world = select_world(&packages, args.world.as_deref());
    let generated = generate(&world, &packages);
    generated.into()
}

fn select_world(packages: &[Package], name: Option<&str>) -> World {
    let mut found = Vec::new();
    for p in packages {
        for w in &p.worlds {
            if name.map(|n| w.name == n).unwrap_or(true) {
                found.push(w.clone());
            }
        }
    }
    match (name, found.len()) {
        (Some(n), 0) => panic!("bindgen!: world `{n}` not found"),
        (_, 1) => found.remove(0),
        (Some(n), _) => panic!("bindgen!: multiple worlds named `{n}`"),
        (None, 0) => panic!("bindgen!: no worlds defined"),
        (None, k) => panic!("bindgen!: {k} worlds found, specify one"),
    }
}

// ---------------------------------------------------------------------------
// Naming helpers
// ---------------------------------------------------------------------------

fn snake(s: &str) -> String {
    let mut out = String::new();
    let mut prev_upper = false;
    for (i, c) in s.chars().enumerate() {
        if c == '-' {
            out.push('_');
            prev_upper = false;
        } else if c.is_uppercase() {
            if i > 0 && !prev_upper {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_upper = true;
        } else {
            out.push(c);
            prev_upper = false;
        }
    }
    out
}

fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
    for c in s.chars() {
        if c == '-' || c == '_' {
            upper_next = true;
        } else if upper_next {
            out.push(c.to_ascii_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "async", "await", "box", "gen", "try",
];

fn ident(s: &str) -> proc_macro2::Ident {
    let name = if RUST_KEYWORDS.contains(&s) {
        format!("{s}_")
    } else {
        s.to_string()
    };
    proc_macro2::Ident::new(&name, Span::call_site())
}

// ---------------------------------------------------------------------------
// Type collection & naming
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TypeRegistry {
    /// wit name -> rust ident
    names: std::collections::HashMap<String, String>,
    decls: Vec<(String, TypeDecl)>,
}

fn collect_types(world: &World, packages: &[Package]) -> TypeRegistry {
    let mut reg = TypeRegistry::default();

    let push = |reg: &mut TypeRegistry, prefix: &str, d: &TypeDecl| {
        let wit_name = match d {
            TypeDecl::Record { name, .. }
            | TypeDecl::Variant { name, .. }
            | TypeDecl::Enum { name, .. }
            | TypeDecl::Flags { name, .. }
            | TypeDecl::Resource { name } => name.clone(),
            TypeDecl::Alias { name, .. } => name.clone(),
        };
        let rust = if reg.names.contains_key(&wit_name) {
            format!("{}{}", camel(prefix), camel(&wit_name))
        } else {
            camel(&wit_name)
        };
        reg.names.insert(wit_name, rust);
        reg.decls.push((prefix.to_string(), d.clone()));
    };

    for pkg in packages {
        for iface in &pkg.interfaces {
            for item in &iface.items {
                if let Item::Type(d) = item {
                    push(&mut reg, &iface.name, d);
                }
            }
        }
    }
    for item in &world.items {
        if let WorldItem::Type(d) = item {
            push(&mut reg, "", d);
        }
    }
    reg
}

fn resolve_named(reg: &TypeRegistry, name: &str) -> String {
    reg.names.get(name).cloned().unwrap_or_else(|| camel(name))
}

fn rust_type(reg: &TypeRegistry, ty: &WitType) -> TokenStream2 {
    match ty {
        WitType::Bool => quote!(bool),
        WitType::S8 => quote!(i8),
        WitType::U8 => quote!(u8),
        WitType::S16 => quote!(i16),
        WitType::U16 => quote!(u16),
        WitType::S32 => quote!(i32),
        WitType::U32 => quote!(u32),
        WitType::S64 => quote!(i64),
        WitType::U64 => quote!(u64),
        WitType::F32 => quote!(f32),
        WitType::F64 => quote!(f64),
        WitType::Char => quote!(char),
        WitType::String => quote!(String),
        WitType::Named(n) => {
            let id = ident(&resolve_named(reg, n));
            quote!(#id)
        }
        WitType::List(t) => {
            let inner = rust_type(reg, t);
            quote!(Vec<#inner>)
        }
        WitType::Option(t) => {
            let inner = rust_type(reg, t);
            quote!(Option<#inner>)
        }
        WitType::Result(ok, err) => {
            let ok_t = match ok.as_ref() {
                Some(t) => rust_type(reg, t),
                None => quote!(),
            };
            let err_t = match err.as_ref() {
                Some(t) => rust_type(reg, t),
                None => quote!(),
            };
            quote!(std::result::Result<#ok_t, #err_t>)
        }
        WitType::Tuple(ts) => {
            let items: Vec<_> = ts.iter().map(|t| rust_type(reg, t)).collect();
            quote!((#(#items,)*))
        }
        // streams/futures cross the boundary as opaque waitable handles
        WitType::Stream(_) => quote!(::waclay::StreamHandle),
        WitType::Future(_) => quote!(::waclay::FutureHandle),
        WitType::Own(_) | WitType::Borrow(_) => quote!(u32),
    }
}

// ---------------------------------------------------------------------------
// Codegen
// ---------------------------------------------------------------------------

fn generate(world: &World, packages: &[Package]) -> TokenStream2 {
    let reg = collect_types(world, packages);

    let types_code = generate_types(&reg);
    let (imports_code, _import_trait_names) = generate_imports(world, packages, &reg);
    let exports_code = generate_exports(world, packages, &reg);
    let world_name = camel(&world.name);

    quote! {
        #types_code
        #imports_code
        #exports_code

        /// Marker module documenting which world these bindings came from.
        #[doc(hidden)]
        pub mod __waclay_world {
            pub const WORLD: &str = #world_name;
        }
    }
}

fn generate_types(reg: &TypeRegistry) -> TokenStream2 {
    let mut out = TokenStream2::new();
    for (prefix, decl) in &reg.decls {
        let _ = prefix;
        match decl {
            TypeDecl::Record { name, fields } => {
                let id = ident(&resolve_named(reg, name));
                let f_names: Vec<_> = fields.iter().map(|(n, _)| ident(&snake(n))).collect();
                let f_types: Vec<_> = fields.iter().map(|(_, t)| rust_type(reg, t)).collect();
                let f_wit: Vec<_> = fields.iter().map(|(n, _)| n.clone()).collect();
                out.extend(quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    pub struct #id { #(pub #f_names: #f_types,)* }

                    impl waclay::ComponentValue for #id {
                        fn into_val(self) -> waclay::Val {
                            let mut fields: Vec<(String, waclay::Val)> = Vec::new();
                            #(
                                fields.push((#f_wit.to_string(), self.#f_names.into_val()));
                            )*
                            waclay::Val::Record(fields)
                        }
                        fn from_val(v: &waclay::Val) -> waclay::Result<Self> {
                            match v {
                                waclay::Val::Record(fields) => {
                                    let mut it = fields.iter();
                                    Ok(#id {
                                        #(
                                            #f_names: {
                                                let found = it.clone().find(|(k, _)| k == #f_wit)
                                                    .ok_or_else(|| waclay::Error::value(concat!("missing field `", #f_wit, "`")))?;
                                                waclay::ComponentValue::from_val(&found.1)?
                                            },
                                        )*
                                    })
                                }
                                other => Err(waclay::Error::value(format!(
                                    "expected record, found {}", other.type_name()))),
                            }
                        }
                    }
                });
            }
            TypeDecl::Variant { name, cases } => {
                let id = ident(&resolve_named(reg, name));
                let c_ids: Vec<_> = cases.iter().map(|(n, _)| ident(&camel(n))).collect();
                let c_wit: Vec<_> = cases.iter().map(|(n, _)| n.clone()).collect();
                // payload-less cases become unit variants, payload cases keep
                // their tuple wrapper
                let mut variant_items = TokenStream2::new();
                for (n, t) in cases.iter() {
                    let cid = ident(&camel(n));
                    match t {
                        Some(t) => {
                            let ty = rust_type(reg, t);
                            variant_items.extend(quote! { #cid(#ty), });
                        }
                        None => variant_items.extend(quote! { #cid, }),
                    }
                }
                let has_payload: Vec<bool> = cases.iter().map(|(_, t)| t.is_some()).collect();

                let mut into_arms = TokenStream2::new();
                let mut from_arms = TokenStream2::new();
                for (i, _) in cases.iter().enumerate() {
                    let cid = &c_ids[i];
                    let cwit = &c_wit[i];
                    if has_payload[i] {
                        into_arms.extend(quote! {
                            #id::#cid(v) => waclay::Val::Variant {
                                case: #cwit.to_string(),
                                payload: Some(Box::new(waclay::ComponentValue::into_val(v))),
                            },
                        });
                        from_arms.extend(quote! {
                            #cwit => {
                                let p = payload.as_deref()
                                    .ok_or_else(|| waclay::Error::value("missing variant payload"))?;
                                Ok(#id::#cid(waclay::ComponentValue::from_val(p)?))
                            }
                        });
                    } else {
                        into_arms.extend(quote! {
                            #id::#cid => waclay::Val::Variant { case: #cwit.to_string(), payload: None },
                        });
                        from_arms.extend(quote! {
                            #cwit => Ok(#id::#cid),
                        });
                    }
                }

                out.extend(quote! {
                    #[derive(Debug, Clone, PartialEq)]
                    pub enum #id { #variant_items }

                    impl waclay::ComponentValue for #id {
                        fn into_val(self) -> waclay::Val {
                            match self { #into_arms }
                        }
                        fn from_val(v: &waclay::Val) -> waclay::Result<Self> {
                            match v {
                                waclay::Val::Variant { case, payload } => match case.as_str() {
                                    #from_arms
                                    _ => Err(waclay::Error::value(format!("unknown variant case `{case}`"))),
                                },
                                other => Err(waclay::Error::value(format!(
                                    "expected variant, found {}", other.type_name()))),
                            }
                        }
                    }
                });
            }
            TypeDecl::Enum { name, cases } => {
                let id = ident(&resolve_named(reg, name));
                let c_ids: Vec<_> = cases.iter().map(|n| ident(&camel(n))).collect();
                let c_wit: Vec<_> = cases.to_vec();
                out.extend(quote! {
                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    pub enum #id { #(#c_ids,)* }

                    impl waclay::ComponentValue for #id {
                        fn into_val(self) -> waclay::Val {
                            match self {
                                #( #id::#c_ids => waclay::Val::Enum(#c_wit.to_string()), )*
                            }
                        }
                        fn from_val(v: &waclay::Val) -> waclay::Result<Self> {
                            match v {
                                waclay::Val::Enum(s) => match s.as_str() {
                                    #( #c_wit => Ok(#id::#c_ids), )*
                                    _ => Err(waclay::Error::value(format!("unknown enum case `{s}`"))),
                                },
                                other => Err(waclay::Error::value(format!(
                                    "expected enum, found {}", other.type_name()))),
                            }
                        }
                    }
                });
            }
            TypeDecl::Flags { name, cases } => {
                let id = ident(&resolve_named(reg, name));
                let c_ids: Vec<_> = cases
                    .iter()
                    .map(|n| ident(&format!("{}_flag", snake(n))))
                    .collect();
                let c_wit: Vec<_> = cases.to_vec();
                out.extend(quote! {
                    #[derive(Debug, Clone, Default, PartialEq)]
                    pub struct #id(pub Vec<String>);

                    impl #id {
                        #( pub const #c_ids: &'static str = #c_wit; )*

                        pub fn contains(&self, name: &str) -> bool {
                            self.0.iter().any(|s| s == name)
                        }
                        pub fn insert(&mut self, name: &str) {
                            if !self.contains(name) {
                                self.0.push(name.to_string());
                            }
                        }
                    }

                    impl waclay::ComponentValue for #id {
                        fn into_val(self) -> waclay::Val {
                            waclay::Val::Flags(self.0)
                        }
                        fn from_val(v: &waclay::Val) -> waclay::Result<Self> {
                            match v {
                                waclay::Val::Flags(set) => Ok(#id(set.clone())),
                                other => Err(waclay::Error::value(format!(
                                    "expected flags, found {}", other.type_name()))),
                            }
                        }
                    }
                });
            }
            TypeDecl::Resource { name } => {
                let id = ident(&resolve_named(reg, name));
                out.extend(quote! {
                    /// Resource handle. The `u32` is the table index; the
                    /// representation lives in the host's resource table.
                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    pub struct #id(pub u32);

                    impl waclay::ComponentValue for #id {
                        fn into_val(self) -> waclay::Val {
                            waclay::Val::Handle(waclay::Handle::Own { resource: 0, index: self.0 })
                        }
                        fn from_val(v: &waclay::Val) -> waclay::Result<Self> {
                            match v {
                                waclay::Val::Handle(h) => Ok(#id(h.index())),
                                other => Err(waclay::Error::value(format!(
                                    "expected resource handle, found {}", other.type_name()))),
                            }
                        }
                    }
                });
            }
            TypeDecl::Alias { name, ty } => {
                let id = ident(&resolve_named(reg, name));
                let t = rust_type(reg, ty);
                out.extend(quote! {
                    pub type #id = #t;
                });
            }
        }
    }
    out
}

fn interface_func_items(iface: &wit::Interface) -> Vec<FuncDecl> {
    iface
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Func(f) => Some(f.clone()),
            _ => None,
        })
        .collect()
}

fn find_interface<'a>(packages: &'a [Package], name: &str) -> Option<&'a wit::Interface> {
    for p in packages {
        for i in &p.interfaces {
            if i.name == name
                || i.name.ends_with(&format!("/{name}"))
                || name.ends_with(&format!("/{}", i.name))
            {
                return Some(i);
            }
        }
    }
    None
}

fn generate_imports(
    world: &World,
    packages: &[Package],
    reg: &TypeRegistry,
) -> (TokenStream2, Vec<String>) {
    let mut out = TokenStream2::new();
    let mut trait_names = Vec::new();

    for item in &world.items {
        if let WorldItem::ImportInterface(name) = item {
            let iface = match find_interface(packages, name) {
                Some(i) => i.clone(),
                None => {
                    // unknown interface: generate a no-op marker
                    continue;
                }
            };
            let funcs = interface_func_items(&iface);
            let trait_id = format_ident!("{}Imports", camel(&iface.name));
            let add_id = format_ident!("add_{}_to_linker", snake(&iface.name));
            trait_names.push(trait_id.to_string());

            let iface_name_lit = name.clone();

            // trait methods
            let mut trait_methods = TokenStream2::new();
            let mut linker_arms = TokenStream2::new();
            for f in &funcs {
                let m_id = ident(&snake(&f.name));
                let f_name_lit = f.name.clone();
                let p_types: Vec<_> = f.params.iter().map(|(_, t)| rust_type(reg, t)).collect();
                let p_names: Vec<_> = f.params.iter().map(|(n, _)| ident(&snake(n))).collect();
                let ret = match &f.result {
                    Some(t) => rust_type(reg, t),
                    None => quote!(()),
                };
                trait_methods.extend(quote! {
                    fn #m_id(&mut self, #(#p_names: #p_types),*) -> #ret;
                });

                let param_count = f.params.len();
                let call_args: Vec<_> = (0..param_count)
                    .map(|i| {
                        let idx = proc_macro2::Literal::usize_unsuffixed(i);
                        quote!(args.#idx)
                    })
                    .collect();

                let result_handle = match &f.result {
                    Some(_) => quote! {
                        let result = host.#m_id(#(#call_args),*);
                        let vals = waclay::ComponentResults::to_result(result)?;
                        if let Some(v) = vals {
                            out.push(v);
                        }
                    },
                    None => quote! {
                        host.#m_id(#(#call_args),*);
                    },
                };

                linker_arms.extend(quote! {
                    linker.func(Some(#iface_name_lit), #f_name_lit, {
                        let get = get;
                        move |ctx: &mut waclay::HostCtx<'_>, vals: &[waclay::Val], out: &mut Vec<waclay::Val>| {
                            let host: &mut dyn HostImpl = (get)(ctx.caller.data_mut());
                            let args = <(#(#p_types,)*) as waclay::ComponentParams>::from_values(vals)?;
                            #result_handle
                            Ok(())
                        }
                    });
                });
            }

            out.extend(quote! {
                /// Host-side implementation of the imported interface
                /// `#iface_name_lit`.
                pub trait #trait_id {
                    #trait_methods
                }

                /// Register every function of `#iface_name_lit` on a linker.
                pub fn #add_id<T, G>(linker: &mut waclay::Linker, get: G)
                where
                    T: #trait_id + 'static,
                    G: Fn(&mut dyn std::any::Any) -> &mut T + Send + Sync + Copy + 'static,
                {
                    trait HostImpl: #trait_id {}
                    impl<T2: #trait_id> HostImpl for T2 {}

                    // The closure downcasts the store's user data to T and
                    // dispatches through the generated trait object.
                    #linker_arms
                }
            });
        }
    }
    (out, trait_names)
}

fn generate_exports(world: &World, packages: &[Package], reg: &TypeRegistry) -> TokenStream2 {
    let world_id = format_ident!("{}Exports", camel(&world.name));
    let mut methods = TokenStream2::new();

    for item in &world.items {
        match item {
            WorldItem::ExportFunc(f) => {
                methods.extend(export_method(&f.name, &f.name, f, reg));
            }
            WorldItem::ExportInterface(name) => {
                if let Some(iface) = find_interface(packages, name) {
                    for f in interface_func_items(iface) {
                        methods.extend(export_method(name, &f.name, &f, reg));
                    }
                }
            }
            _ => {}
        }
    }

    quote! {
        /// Typed accessors for the world's guest exports.
        pub struct #world_id<'a> {
            pub instance: &'a waclay::ComponentInstance,
        }

        impl<'a> #world_id<'a> {
            #methods
        }

        /// Shorthand: build the exports wrapper from an instance.
        pub fn exports(instance: &waclay::ComponentInstance) -> #world_id<'_> {
            #world_id { instance }
        }
    }
}

fn export_method(interface: &str, func: &str, f: &FuncDecl, reg: &TypeRegistry) -> TokenStream2 {
    let m_id = ident(&snake(func));
    let key = format!("{interface}|{func}");
    let p_types: Vec<_> = f.params.iter().map(|(_, t)| rust_type(reg, t)).collect();
    let p_names: Vec<_> = f.params.iter().map(|(n, _)| ident(&snake(n))).collect();
    let ret = match &f.result {
        Some(t) => rust_type(reg, t),
        None => quote!(),
    };
    let call = quote! {
        self.instance
            .typed_func::<(#(#p_types,)*), #ret>(#key)?
            .call(store, (#(#p_names,)*))
    };
    match &f.result {
        Some(_) => quote! {
            pub fn #m_id(&self, store: &mut waclay::Store, #(#p_names: #p_types),*) -> waclay::Result<#ret> {
                #call
            }
        },
        None => quote! {
            pub fn #m_id(&self, store: &mut waclay::Store, #(#p_names: #p_types),*) -> waclay::Result<()> {
                #call
            }
        },
    }
}
