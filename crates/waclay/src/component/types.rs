//! Component-model value & type model (the WIT level, above core wasm).

use std::fmt;

/// WIT primitive value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    Bool,
    S8,
    U8,
    S16,
    U16,
    S32,
    U32,
    S64,
    U64,
    F32,
    F64,
    Char,
    String,
}

impl Primitive {
    /// kebab-case name as it appears in WIT.
    pub fn name(&self) -> &'static str {
        match self {
            Primitive::Bool => "bool",
            Primitive::S8 => "s8",
            Primitive::U8 => "u8",
            Primitive::S16 => "s16",
            Primitive::U16 => "u16",
            Primitive::S32 => "s32",
            Primitive::U32 => "u32",
            Primitive::S64 => "s64",
            Primitive::U64 => "u64",
            Primitive::F32 => "f32",
            Primitive::F64 => "f64",
            Primitive::Char => "char",
            Primitive::String => "string",
        }
    }
}

impl fmt::Display for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Reference to a type: either a primitive or an index into the
/// component's type table (see [`Component::types`](super::Component)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeRef {
    Primitive(Primitive),
    Def(u32),
}

/// A defined (non-primitive) component type.
#[derive(Debug, Clone)]
pub enum TypeDef {
    /// A bare primitive (used when a defined type aliases a primitive).
    Primitive(Primitive),
    Record(Vec<(String, TypeRef)>),
    Variant(Vec<(String, Option<TypeRef>)>),
    List(TypeRef),
    Tuple(Vec<TypeRef>),
    Flags(Vec<String>),
    Enum(Vec<String>),
    Option(TypeRef),
    Result {
        ok: Option<TypeRef>,
        err: Option<TypeRef>,
    },
    /// Owned handle to a resource (resource type ids live in the same table).
    Own(u32),
    /// Borrowed handle to a resource.
    Borrow(u32),
    /// `stream<T>`: one of the two ends of a value pipe. The payload type
    /// (when present) determines what the stream carries; the runtime value
    /// is always an opaque handle ([`Val::Stream`]).
    Stream(Option<TypeRef>),
    /// `future<T>`: one of the two ends of a single-shot value cell.
    Future(Option<TypeRef>),
}

impl TypeDef {
    /// True when this dense type is the synthetic resource-type marker the
    /// decoder uses for `(resource (rep ..))` declarations.
    pub fn is_resource_marker(&self) -> bool {
        matches!(self,
            TypeDef::Flags(v) if v.first().map(|s| s.as_str()) == Some("\u{0}resource"))
    }

    /// Short human-readable description for error messages.
    pub fn describe(&self, types: &[TypeDef]) -> String {
        match self {
            TypeDef::Primitive(p) => p.name().to_string(),
            TypeDef::Record(fields) => {
                let fs: Vec<String> = fields
                    .iter()
                    .map(|(n, t)| format!("{n}: {}", describe_ref(t, types)))
                    .collect();
                format!("record {{ {} }}", fs.join(", "))
            }
            TypeDef::Variant(cases) => {
                let cs: Vec<String> = cases
                    .iter()
                    .map(|(n, t)| match t {
                        Some(t) => format!("{n}({})", describe_ref(t, types)),
                        None => n.clone(),
                    })
                    .collect();
                format!("variant {{ {} }}", cs.join(", "))
            }
            TypeDef::List(t) => format!("list<{}>", describe_ref(t, types)),
            TypeDef::Tuple(ts) => format!(
                "tuple<{}>",
                ts.iter()
                    .map(|t| describe_ref(t, types))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TypeDef::Flags(names) => format!("flags({})", names.join("|")),
            TypeDef::Enum(names) => format!("enum({})", names.join("|")),
            TypeDef::Option(t) => format!("option<{}>", describe_ref(t, types)),
            TypeDef::Result { ok, err } => format!(
                "result<_, _>({}/{})",
                ok.as_ref()
                    .map(|t| describe_ref(t, types))
                    .unwrap_or_default(),
                err.as_ref()
                    .map(|t| describe_ref(t, types))
                    .unwrap_or_default(),
            ),
            TypeDef::Own(r) => format!("own<resource#{r}>"),
            TypeDef::Borrow(r) => format!("borrow<resource#{r}>"),
            TypeDef::Stream(t) => match t {
                Some(t) => format!("stream<{}>", describe_ref(t, types)),
                None => "stream".to_string(),
            },
            TypeDef::Future(t) => match t {
                Some(t) => format!("future<{}>", describe_ref(t, types)),
                None => "future".to_string(),
            },
        }
    }
}

fn describe_ref(t: &TypeRef, types: &[TypeDef]) -> String {
    match t {
        TypeRef::Primitive(p) => p.name().to_string(),
        TypeRef::Def(idx) => types
            .get(*idx as usize)
            .map(|d| d.describe(types))
            .unwrap_or_else(|| format!("type#{idx}")),
    }
}

/// A component-level (WIT) function signature.
#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params: Vec<(String, TypeRef)>,
    pub result: Option<TypeRef>,
}

impl FuncSig {
    /// Pretty-printed signature for diagnostics.
    pub fn describe(&self, types: &[TypeDef]) -> String {
        let ps: Vec<String> = self
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {}", describe_ref(t, types)))
            .collect();
        let r = match &self.result {
            Some(t) => describe_ref(t, types),
            None => "()".to_string(),
        };
        format!("func({}) -> {}", ps.join(", "), r)
    }
}

/// A component-model runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Bool(bool),
    S8(i8),
    U8(u8),
    S16(i16),
    U16(u16),
    S32(i32),
    U32(u32),
    S64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Char(char),
    String(String),
    List(Vec<Val>),
    Record(Vec<(String, Val)>),
    Tuple(Vec<Val>),
    Variant {
        case: String,
        payload: Option<Box<Val>>,
    },
    Enum(String),
    Flags(Vec<String>),
    Option(Option<Box<Val>>),
    Result(Result<Option<Box<Val>>, Option<Box<Val>>>),
    /// Resource handle: `own` handles own a table slot, `borrow`s do not.
    Handle(Handle),
    /// Handle to one end of a `stream<T>` (`ty` is the dense stream type
    /// index, `index` the waitable-table slot of this end).
    Stream {
        ty: u32,
        index: u32,
    },
    /// Handle to one end of a `future<T>`.
    Future {
        ty: u32,
        index: u32,
    },
}

/// A resource handle handed to host code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Index into the per-instance resource table of the given type.
    Own {
        resource: u32,
        index: u32,
    },
    Borrow {
        resource: u32,
        index: u32,
    },
}

impl Handle {
    /// Table slot index of this handle.
    pub fn index(&self) -> u32 {
        match self {
            Handle::Own { index, .. } | Handle::Borrow { index, .. } => *index,
        }
    }

    /// Resource type id of this handle.
    pub fn resource(&self) -> u32 {
        match self {
            Handle::Own { resource, .. } | Handle::Borrow { resource, .. } => *resource,
        }
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Bool(b) => write!(f, "{b}"),
            Val::String(s) => write!(f, "{s:?}"),
            Val::List(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Val::Record(fields) => {
                write!(f, "{{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
            Val::Variant { case, payload } => match payload {
                Some(p) => write!(f, "{case}({p})"),
                None => write!(f, "{case}"),
            },
            Val::Enum(name) => write!(f, "{name}"),
            Val::Flags(names) => write!(f, "{names:?}"),
            Val::Option(Some(v)) => write!(f, "some({v})"),
            Val::Option(None) => write!(f, "none"),
            Val::Result(Ok(Some(v))) => write!(f, "ok({v})"),
            Val::Result(Ok(None)) => write!(f, "ok"),
            Val::Result(Err(Some(v))) => write!(f, "err({v})"),
            Val::Result(Err(None)) => write!(f, "err"),
            Val::Handle(h) => write!(f, "handle({h:?})"),
            Val::Stream { index, .. } => write!(f, "stream#{index}"),
            Val::Future { index, .. } => write!(f, "future#{index}"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl Val {
    /// Human-readable type name of this value (best effort).
    pub fn type_name(&self) -> &'static str {
        match self {
            Val::Bool(_) => "bool",
            Val::S8(_) => "s8",
            Val::U8(_) => "u8",
            Val::S16(_) => "s16",
            Val::U16(_) => "u16",
            Val::S32(_) => "s32",
            Val::U32(_) => "u32",
            Val::S64(_) => "s64",
            Val::U64(_) => "u64",
            Val::F32(_) => "f32",
            Val::F64(_) => "f64",
            Val::Char(_) => "char",
            Val::String(_) => "string",
            Val::List(_) => "list",
            Val::Record(_) => "record",
            Val::Tuple(_) => "tuple",
            Val::Variant { .. } => "variant",
            Val::Enum(_) => "enum",
            Val::Flags(_) => "flags",
            Val::Option(_) => "option",
            Val::Result(_) => "result",
            Val::Handle(_) => "handle",
            Val::Stream { .. } => "stream",
            Val::Future { .. } => "future",
        }
    }
}

/// A named component-level import (interface instance or root function).
#[derive(Debug, Clone)]
pub struct WorldImport {
    /// Import name as written in the world (e.g. `wasi:cli/run@0.2.0` or `log`).
    pub name: String,
    /// Functions offered by this import; empty for non-instance imports.
    pub funcs: Vec<(String, FuncSig)>,
}

/// A named component-level export.
#[derive(Debug, Clone)]
pub struct WorldExport {
    /// Export name as written in the world.
    pub name: String,
    /// Functions of this export. For root func exports this has one entry.
    pub funcs: Vec<(String, FuncSig)>,
}

/// Description of a resource type declared by an interface.
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    /// Index of the resource type in the component type table.
    pub type_idx: u32,
    /// Owning interface name (None for root-level resources).
    pub interface: Option<String>,
    /// Resource name (kebab-case, e.g. `output-stream`).
    pub name: String,
}
