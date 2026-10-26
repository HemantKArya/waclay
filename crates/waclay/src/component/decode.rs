//! Component binary decoder: translates a component-model binary into a
//! [`ComponentPlan`] the runtime can instantiate on any engine.
//!
//! This replaces the old `wasmtime-environ` dependency: it walks the binary
//! with `wasmparser`, maintains every index space, resolves canonical
//! `lift`/`lower` operations to core functions of the embedded shim module,
//! and precomputes everything the Canonical ABI needs at instantiation time.

use std::collections::HashMap;

use wasmparser::{
    CanonicalFunction, CanonicalOption, ComponentAlias, ComponentDefinedType,
    ComponentExternalKind, ComponentFuncType, ComponentType, ComponentTypeRef, ComponentValType,
    InstanceTypeDeclaration, Parser, Payload, PrimitiveValType, TypeBounds, ValType as CoreValType,
};

use crate::abi::StringEncoding;
use crate::component::types::{
    FuncSig, Primitive, ResourceInfo, TypeDef, TypeRef, WorldExport, WorldImport,
};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Plan structures
// ---------------------------------------------------------------------------

/// Where a core function comes from (resolved at instantiation time).
#[derive(Debug, Clone)]
pub enum CoreFuncRef {
    /// Export `name` of core instance `instance`.
    Export { instance: u32, name: String },
    /// A canon-lowered import (index into `ComponentPlan::lowers`); resolved
    /// by the linker when it wires the shim module's imports.
    Lower(usize),
    /// A canon builtin (stream/future/waitable/task op) — index into
    /// `ComponentPlan::builtins`; bound by the linker to the runtime.
    Builtin(usize),
    /// A canon-lowered export of a nested component — the linker wires a
    /// trampoline that calls the child instance.
    ChildLower(usize),
    /// A re-exported nested-component function — the linker binds the
    /// child's already-built [`crate::component::ExportedFunc`].
    ChildExport { instantiation: u32, name: String },
}

/// Where a core memory comes from.
#[derive(Debug, Clone)]
pub enum CoreMemRef {
    Export { instance: u32, name: String },
}

/// A host function the component imports (canon lower).
#[derive(Debug, Clone)]
pub struct LoweredImport {
    /// Component-level signature.
    pub sig: FuncSig,
    /// Owning interface import name (`None` for root-level imports).
    pub interface: Option<String>,
    /// Raw function name (e.g. `get-random-bytes`, `[method]stream.write`).
    pub func_name: String,
    /// Core module import name to bind this to.
    pub core_module_name: String,
    /// Core function import name to bind this to.
    pub core_func_name: String,
    /// Realloc core func (needed when results contain heap types).
    pub realloc: Option<CoreFuncRef>,
    pub string_enc: StringEncoding,
    /// The import was lowered with the `async` canon option: results travel
    /// through an out-pointer and the core func returns a completion flag.
    pub async_lower: bool,
}

/// An exported component function (canon lift).
#[derive(Debug, Clone)]
pub struct LiftedExport {
    /// Fully-qualified lookup key (`"add"` or `"iface@1.0|func"`).
    pub key: String,
    /// Function name inside its interface (or the bare name).
    pub func_name: String,
    pub sig: FuncSig,
    pub core_func: CoreFuncRef,
    pub post_return: Option<CoreFuncRef>,
    pub realloc: Option<CoreFuncRef>,
    pub string_enc: StringEncoding,
    /// The export was lifted with the `async` canon option: the guest
    /// returns its value through `task.return` instead of the core result.
    pub async_lift: bool,
    /// Callback core function (async lift with callback event loop).
    pub callback: Option<CoreFuncRef>,
}

/// A canon builtin operation (preview-3 async surface: streams, futures,
/// waitable sets, error contexts and the task model).
#[derive(Debug, Clone)]
pub enum BuiltinOp {
    StreamNew(u32),
    StreamRead {
        ty: u32,
        enc: StringEncoding,
        async_: bool,
    },
    StreamWrite {
        ty: u32,
        enc: StringEncoding,
        async_: bool,
    },
    StreamForward(u32),
    StreamCancelRead {
        ty: u32,
        async_: bool,
    },
    StreamCancelWrite {
        ty: u32,
        async_: bool,
    },
    StreamDropReadable(u32),
    StreamDropWritable(u32),
    FutureNew(u32),
    FutureRead {
        ty: u32,
        enc: StringEncoding,
        async_: bool,
    },
    FutureWrite {
        ty: u32,
        enc: StringEncoding,
        async_: bool,
    },
    FutureForward(u32),
    FutureCancelRead {
        ty: u32,
        async_: bool,
    },
    FutureCancelWrite {
        ty: u32,
        async_: bool,
    },
    FutureDropReadable(u32),
    FutureDropWritable(u32),
    ErrorContextNew(StringEncoding),
    ErrorContextDebugMessage {
        enc: StringEncoding,
        realloc: Option<CoreFuncRef>,
    },
    ErrorContextDrop,
    WaitableSetNew,
    WaitableSetWait,
    WaitableSetPoll,
    WaitableSetDrop,
    WaitableJoin,
    SubtaskDrop,
    SubtaskCancel {
        async_: bool,
    },
    TaskReturn {
        result: Option<TypeRef>,
        enc: StringEncoding,
    },
    TaskCancel,
    BackpressureInc,
    BackpressureDec,
    ThreadYield,
    ThreadIndex,
    ThreadSuspend,
    ThreadResumeLater,
    ThreadAvailableParallelism,
    ContextGet {
        ty: CoreValType,
        slot: u32,
    },
    ContextSet {
        ty: CoreValType,
        slot: u32,
    },
    /// Stack-switching builtins that a synchronous runtime cannot host;
    /// bound to core functions that trap with a clear message.
    Unsupported(String),
}

/// A component-level value import (`(import "x" (value (ty)))`).
/// Values are constants the host provides before instantiation; the
/// component's start function receives them as arguments.
#[derive(Debug, Clone)]
pub struct ValueImport {
    /// Import name.
    pub name: String,
    /// Value type.
    pub ty: TypeRef,
}

/// The component start function (start section). It runs once at the end
/// of instantiation with the imported values as arguments; its result (if
/// any) joins the component value index space.
#[derive(Debug, Clone)]
pub struct StartFunc {
    /// Index into [`ComponentPlan::lifts`] of the canon-lifted start func.
    pub lift: usize,
    /// Value index space indices of the arguments (all value imports).
    pub args: Vec<u32>,
}

/// Where an argument of a nested-component instantiation comes from.
#[derive(Debug, Clone)]
pub enum ArgSource {
    /// The parent's imported interface instance (by import name).
    ParentImport(String),
    /// The parent's imported root-level function (by import name).
    ParentFunc(String),
    /// An export of an earlier nested instantiation (by export name).
    ChildExport { instantiation: u32, name: String },
}

/// One nested-component instantiation op (component instance section):
/// instantiate `nested` wiring each child import name to its source.
#[derive(Debug, Clone)]
pub struct NestedInstantiate {
    /// Index into [`ComponentPlan::nested`].
    pub nested: u32,
    /// `(child import name, source)` pairs in argument order.
    pub args: Vec<(String, ArgSource)>,
}

/// A canon `lower` of a nested component's exported function: the parent's
/// core module calls into the child through a linker trampoline that lifts
/// the parent's flat args, invokes the child, and lowers the result back.
#[derive(Debug, Clone)]
pub struct ChildLower {
    /// Index into [`ComponentPlan::component_instantiations`].
    pub instantiation: u32,
    /// Export name inside the child.
    pub name: String,
    pub sig: FuncSig,
    pub realloc: Option<CoreFuncRef>,
    pub string_enc: StringEncoding,
    pub async_lower: bool,
}

/// Fully decoded component, ready to instantiate.
#[derive(Debug, Clone, Default)]
pub struct ComponentPlan {
    /// Bytes of the embedded core (shim) module.
    pub core_module: Vec<u8>,
    /// Dense value-type table (indices used by `TypeRef::Def`).
    pub types: Vec<TypeDef>,
    /// Component-level imports in declaration order.
    pub imports: Vec<WorldImport>,
    /// Component-level exports.
    pub exports: Vec<WorldExport>,
    /// Host functions to be bound into the shim module.
    pub lowers: Vec<LoweredImport>,
    /// Guest functions callable by the host.
    pub lifts: Vec<LiftedExport>,
    /// Resources declared by interfaces (imports and exports).
    pub resources: Vec<ResourceInfo>,
    /// Canon builtin ops (streams/futures/waitables/tasks).
    pub builtins: Vec<BuiltinOp>,
    /// Deterministic binding map built from core-instantiation args:
    /// `(import-module, import-name) -> core func source`. Lets the linker
    /// wire canon builtins/lowers without relying on naming conventions.
    pub argmap: HashMap<(String, String), CoreFuncRef>,
    /// Whether the shim module exports `_initialize` (reactor startup).
    pub has_initialize: bool,
    /// Component-level value imports (start arguments), in declaration order.
    pub values: Vec<ValueImport>,
    /// The start function, when the binary carries a start section.
    pub start: Option<StartFunc>,
    /// Nested (child) component plans, recursively decoded.
    pub nested: Vec<ComponentPlan>,
    /// Nested-component instantiation ops in section order.
    pub component_instantiations: Vec<NestedInstantiate>,
    /// Canon lowers of nested-component exports (parent calls child).
    pub child_lowers: Vec<ChildLower>,
    /// Exported values (e.g. the start function's result), by name.
    pub exported_values: Vec<(String, TypeRef)>,
}

// ---------------------------------------------------------------------------
// Decoder state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum FuncSource {
    /// Imported root-level function.
    RootImport(usize),
    /// Canon lift result.
    Lift(usize),
    /// Alias of an export of a component instance.
    AliasExport { instance: u32, name: String },
    /// Alias of an export of a nested-component instantiation.
    ChildExport { instantiation: u32, name: String },
    /// Index-space placeholder: exports (and other index-consuming items)
    /// occupy a slot in the component func index space but carry no
    /// additional runtime behavior.
    Occupied,
}

#[derive(Debug, Clone)]
enum InstanceSource {
    /// A component-level import (index into `imported`).
    Imported(usize),
    /// A synthesized instance from exports.
    FromExports(Vec<(String, ComponentExternalKind, u32)>),
    /// A nested-component instantiation (index into
    /// `ComponentPlan::component_instantiations`).
    Child(u32),
    /// Index-space placeholder (see [`FuncSource::Occupied`]).
    Occupied,
}

#[derive(Debug, Clone)]
enum CoreExternSource {
    Func(CoreFuncRef),
    Memory(CoreMemRef),
}

#[derive(Clone)]
#[allow(dead_code)] // fields mirror the component encoding; used by later phases
struct ImportedInstance {
    name: String,
    funcs: Vec<(String, FuncSig)>,
    /// resource name -> dense resource id
    resources: Vec<(String, u32)>,
}

#[derive(Clone)]
#[allow(dead_code)] // mirrors the component encoding; used by later phases
enum CoreFuncSource {
    Lower(usize),
    ResourceNew(u32),
    ResourceDrop(u32),
    ResourceRep(u32),
    Builtin(u32),
    Export(CoreFuncRef),
    /// A canon lower of a nested component's export.
    ChildLower(u32),
}

#[derive(Clone)]
#[allow(dead_code)] // mirrors the component encoding; used by later phases
enum CoreInstanceSource {
    Module {
        module: u32,
        args: Vec<(String, u32)>,
    },
    FromExports(Vec<(String, CoreExternSource)>),
}

/// Shape of an instance type (interface): its exported funcs + resources.
#[derive(Default, Clone)]
struct InstanceShape {
    funcs: Vec<(String, FuncSig)>,
    /// resource name -> dense resource id
    resources: Vec<(String, u32)>,
}

/// Entry in a type scope. Dense indices double as resource ids.
#[derive(Clone)]
enum ScopeEntry {
    Dense(u32),
    Func(FuncSig),
}

#[derive(Default)]
struct Decoder {
    plan: ComponentPlan,
    // component index spaces
    imported: Vec<ImportedInstance>,
    instances: Vec<InstanceSource>,
    funcs: Vec<FuncSource>,
    core_funcs: Vec<CoreFuncSource>,
    core_mems: Vec<CoreMemRef>,
    core_instances: Vec<CoreInstanceSource>,
    /// binary func-type idx -> sig (global scope)
    func_sigs: HashMap<u32, FuncSig>,
    /// binary type idx -> dense type idx (global scope)
    type_remap: HashMap<u32, u32>,
    /// dense idx of instance types -> their shape
    instance_shapes: HashMap<u32, InstanceShape>,
    /// running binary type index counter (sections arrive in order)
    type_counter: u32,
    /// component index space (indices into `plan.nested`)
    components: Vec<u32>,
    /// component value index space size (value imports + start results)
    values: usize,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Decode a component (or core module) binary into a plan.
pub fn decode(binary: &[u8]) -> Result<ComponentPlan> {
    // Fast path: plain core modules become "core-only components".
    if !is_component(binary) {
        return Ok(ComponentPlan {
            core_module: binary.to_vec(),
            ..Default::default()
        });
    }

    let mut d = Decoder::default();
    // `Parser::parse_all` streams NESTED module/component contents inline;
    // track nesting depth and skip inner payloads (nested modules are read
    // from their byte ranges instead).
    let mut depth: usize = 0;
    for payload in Parser::new(0).parse_all(binary) {
        match payload.map_err(binary_err)? {
            Payload::Version { encoding, .. } => {
                if depth == 0 && encoding != wasmparser::Encoding::Component {
                    return Err(Error::MalformedBinary("expected a component binary".into()));
                }
                // inner versions belong to nested modules — skipped
            }
            Payload::ModuleSection {
                unchecked_range, ..
            } => {
                let is_top = depth == 0;
                depth += 1;
                if !is_top {
                    // a module embedded inside a nested component: it belongs
                    // to the child plan (decoded from the child byte range)
                    continue;
                }
                // malformed binaries may claim ranges past the end; treat
                // them as decode errors instead of panicking
                let start = unchecked_range.start as usize;
                let end = unchecked_range.end as usize;
                if start > binary.len() || end > binary.len() || start > end {
                    return Err(Error::MalformedBinary(format!(
                        "module section range {start}..{end} out of bounds (binary is {} bytes)",
                        binary.len()
                    )));
                }
                let bytes = &binary[start..end];
                if d.plan.core_module.is_empty() {
                    d.plan.core_module = bytes.to_vec();
                }
            }
            Payload::ComponentSection {
                unchecked_range, ..
            } => {
                // nested (child) component: decode it recursively from its
                // own byte range; its sections still stream inline below, so
                // enter the depth guard like for modules
                let is_top = depth == 0;
                depth += 1;
                if !is_top {
                    continue; // grandchild: belongs to the child plan
                }
                let start = unchecked_range.start as usize;
                let end = unchecked_range.end as usize;
                if start > binary.len() || end > binary.len() || start > end {
                    return Err(Error::MalformedBinary(format!(
                        "component section range {start}..{end} out of bounds (binary is {} bytes)",
                        binary.len()
                    )));
                }
                let child = decode(&binary[start..end])?;
                let idx = d.plan.nested.len() as u32;
                d.plan.nested.push(child);
                d.components.push(idx);
            }
            Payload::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Payload::ComponentImportSection(reader) => {
                if depth > 0 {
                    continue; // belongs to a nested component (decoded above)
                }
                for import in reader {
                    d.decode_import(import.map_err(binary_err)?)?;
                }
            }
            Payload::ComponentTypeSection(reader) => {
                if depth > 0 {
                    continue;
                }
                for ty in reader {
                    d.decode_global_type(ty.map_err(binary_err)?)?;
                }
            }
            Payload::CoreTypeSection(_) => {
                // core types are only needed for lowering bookkeeping which
                // waclay derives statically from the shim module itself.
            }
            Payload::ComponentAliasSection(reader) => {
                if depth > 0 {
                    continue;
                }
                for alias in reader {
                    d.decode_alias(alias.map_err(binary_err)?)?;
                }
            }
            Payload::InstanceSection(reader) => {
                if depth > 0 {
                    continue;
                }
                for inst in reader {
                    d.decode_core_instance(inst.map_err(binary_err)?)?;
                }
            }
            Payload::ComponentInstanceSection(reader) => {
                if depth > 0 {
                    continue;
                }
                for inst in reader {
                    d.decode_component_instance(inst.map_err(binary_err)?)?;
                }
            }
            Payload::ComponentCanonicalSection(reader) => {
                if depth > 0 {
                    continue;
                }
                for canon in reader {
                    d.decode_canonical(canon.map_err(binary_err)?)?;
                }
            }
            Payload::ComponentExportSection(reader) => {
                if depth > 0 {
                    continue;
                }
                for export in reader {
                    d.decode_export(export.map_err(binary_err)?)?;
                }
            }
            Payload::ComponentStartSection { start, .. } if depth == 0 => {
                // start sections of nested components belong to the child
                // plan (decoded from its own byte range above)
                d.decode_start(start)?;
            }
            _ => {}
        }
    }

    if !d.plan.core_module.is_empty() {
        d.plan.has_initialize = core_module_has_initialize(&d.plan.core_module);
    }

    Ok(d.plan)
}

fn is_component(binary: &[u8]) -> bool {
    // component binaries use version 13 (0x0d), core modules 1 (0x01);
    // the version is a little-endian u32 right after the magic, so the
    // low byte lives at offset 4.
    binary.len() >= 8 && &binary[0..4] == b"\0asm" && binary[4] == 0x0d && binary[5] == 0x00
}

fn binary_err(e: wasmparser::BinaryReaderError) -> Error {
    Error::MalformedBinary(e.to_string())
}

fn core_module_has_initialize(module: &[u8]) -> bool {
    for payload in Parser::new(0).parse_all(module) {
        if let Ok(Payload::ExportSection(reader)) = payload {
            for exp in reader.into_iter().flatten() {
                if exp.name == "_initialize" {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

impl Decoder {
    /// Decode one entry of the top-level component type section.
    fn decode_global_type(&mut self, ty: ComponentType<'_>) -> Result<()> {
        let bin_idx = self.type_counter;
        self.type_counter += 1;
        match ty {
            ComponentType::Defined(def) => {
                let td = self.convert_defined(&def, &HashMap::new())?;
                self.push_dense(bin_idx, td);
            }
            ComponentType::Func(ft) => {
                let remap = self.type_remap.clone();
                let sig = convert_func_type(&ft, &HashMap::new(), &remap)?;
                self.func_sigs.insert(bin_idx, sig);
            }
            ComponentType::Resource { .. } => {
                // placeholder keeps dense indices aligned; its dense id is
                // used as the resource id for Own/Borrow handles
                self.push_dense(bin_idx, TypeDef::Flags(vec!["\u{0}resource".into()]));
            }
            ComponentType::Instance(decls) => {
                let shape = self.instance_type_shape(&decls)?;
                let dense = self.push_dense(bin_idx, TypeDef::Flags(vec!["\u{0}instance".into()]));
                self.instance_shapes.insert(dense, shape);
            }
            ComponentType::Component(_) => {
                return Err(Error::unsupported("nested component types"));
            }
        }
        Ok(())
    }

    fn push_dense(&mut self, bin_idx: u32, td: TypeDef) -> u32 {
        let dense = self.plan.types.len() as u32;
        self.plan.types.push(td);
        self.type_remap.insert(bin_idx, dense);
        dense
    }

    fn convert_defined(
        &mut self,
        def: &ComponentDefinedType<'_>,
        scope: &HashMap<u32, ScopeEntry>,
    ) -> Result<TypeDef> {
        Ok(match def {
            ComponentDefinedType::Primitive(p) => TypeDef::Primitive(
                prim(*p).ok_or_else(|| Error::MalformedBinary("unknown primitive type".into()))?,
            ),
            ComponentDefinedType::Record(fields) => TypeDef::Record(
                fields
                    .iter()
                    .map(|(n, t)| Ok((n.to_string(), self.valref(t, scope)?)))
                    .collect::<Result<Vec<_>>>()?,
            ),
            ComponentDefinedType::Variant(cases) => TypeDef::Variant(
                cases
                    .iter()
                    .map(|c| {
                        Ok((
                            c.name.to_string(),
                            match &c.ty {
                                Some(t) => Some(self.valref(t, scope)?),
                                None => None,
                            },
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
            ComponentDefinedType::List(elem) => TypeDef::List(self.valref(elem, scope)?),
            ComponentDefinedType::Tuple(items) => {
                let mut out = Vec::with_capacity(items.len());
                for it in items.iter() {
                    out.push(self.valref(it, scope)?);
                }
                TypeDef::Tuple(out)
            }
            ComponentDefinedType::Flags(names) => {
                TypeDef::Flags(names.iter().map(|n| n.to_string()).collect())
            }
            ComponentDefinedType::Enum(names) => {
                TypeDef::Enum(names.iter().map(|n| n.to_string()).collect())
            }
            ComponentDefinedType::Option(inner) => TypeDef::Option(self.valref(inner, scope)?),
            ComponentDefinedType::Result { ok, err } => TypeDef::Result {
                ok: match ok {
                    Some(t) => Some(self.valref(t, scope)?),
                    None => None,
                },
                err: match err {
                    Some(t) => Some(self.valref(t, scope)?),
                    None => None,
                },
            },
            ComponentDefinedType::Own(r) => TypeDef::Own(self.lookup_dense(*r, scope)?),
            ComponentDefinedType::Borrow(r) => TypeDef::Borrow(self.lookup_dense(*r, scope)?),
            ComponentDefinedType::Stream(t) => TypeDef::Stream(match t {
                Some(t) => Some(self.valref(t, scope)?),
                None => None,
            }),
            ComponentDefinedType::Future(t) => TypeDef::Future(match t {
                Some(t) => Some(self.valref(t, scope)?),
                None => None,
            }),
            other => {
                return Err(Error::unsupported(format!(
                    "component type {other:?} is not supported"
                )))
            }
        })
    }

    fn valref(
        &mut self,
        val: &ComponentValType,
        scope: &HashMap<u32, ScopeEntry>,
    ) -> Result<TypeRef> {
        Ok(match val {
            ComponentValType::Primitive(p) => TypeRef::Primitive(
                prim(*p).ok_or_else(|| Error::MalformedBinary("unknown primitive type".into()))?,
            ),
            ComponentValType::Type(idx) => TypeRef::Def(self.lookup_dense(*idx, scope)?),
        })
    }

    fn lookup_dense(&self, idx: u32, scope: &HashMap<u32, ScopeEntry>) -> Result<u32> {
        match scope.get(&idx) {
            Some(ScopeEntry::Dense(d)) => Ok(*d),
            _ => self
                .type_remap
                .get(&idx)
                .copied()
                .ok_or_else(|| Error::MalformedBinary(format!("type index {idx} not found"))),
        }
    }

    /// Decode the shape of an instance type: exported funcs and resources.
    fn instance_type_shape(
        &mut self,
        decls: &[InstanceTypeDeclaration<'_>],
    ) -> Result<InstanceShape> {
        let mut scope: HashMap<u32, ScopeEntry> = HashMap::new();
        let mut counter: u32 = 0;
        let mut shape = InstanceShape::default();
        let mut last_resource: Option<u32> = None;

        for decl in decls {
            match decl {
                InstanceTypeDeclaration::Type(ComponentType::Defined(def)) => {
                    let bin = counter;
                    counter += 1;
                    let td = self.convert_defined(def, &scope)?;
                    let dense = self.plan.types.len() as u32;
                    self.plan.types.push(td);
                    scope.insert(bin, ScopeEntry::Dense(dense));
                }
                InstanceTypeDeclaration::Type(ComponentType::Resource { .. }) => {
                    let bin = counter;
                    counter += 1;
                    let dense = self.plan.types.len() as u32;
                    self.plan
                        .types
                        .push(TypeDef::Flags(vec!["\u{0}resource".into()]));
                    scope.insert(bin, ScopeEntry::Dense(dense));
                    last_resource = Some(dense);
                }
                InstanceTypeDeclaration::Type(ComponentType::Func(ft)) => {
                    let bin = counter;
                    counter += 1;
                    let remap = self.type_remap.clone();
                    let sig = convert_func_type(ft, &scope, &remap)?;
                    scope.insert(bin, ScopeEntry::Func(sig));
                }
                InstanceTypeDeclaration::Type(_) => {}
                InstanceTypeDeclaration::Alias(ComponentAlias::Outer {
                    kind: wasmparser::ComponentOuterAliasKind::Type,
                    count,
                    index,
                }) => {
                    let bin = counter;
                    counter += 1;
                    let entry = if *count == 0 {
                        scope
                            .get(index)
                            .cloned()
                            .ok_or_else(|| Error::MalformedBinary("bad outer alias".into()))?
                    } else {
                        match self.type_remap.get(index) {
                            Some(d) => ScopeEntry::Dense(*d),
                            None => return Err(Error::MalformedBinary("bad outer alias".into())),
                        }
                    };
                    scope.insert(bin, entry);
                }
                InstanceTypeDeclaration::Export { name, ty } => {
                    let export_name = name.full_name().to_string();
                    match ty {
                        ComponentTypeRef::Func(idx) => {
                            let sig = match scope.get(idx) {
                                Some(ScopeEntry::Func(s)) => s.clone(),
                                _ => {
                                    return Err(Error::MalformedBinary(
                                        "instance export references non-func type".into(),
                                    ))
                                }
                            };
                            shape.funcs.push((export_name, sig));
                        }
                        ComponentTypeRef::Type(TypeBounds::SubResource) => {
                            let dense = last_resource.ok_or_else(|| {
                                Error::MalformedBinary(
                                    "resource export without resource type".into(),
                                )
                            })?;
                            shape.resources.push((export_name, dense));
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(shape)
    }

    // -----------------------------------------------------------------------
    // Imports
    // -----------------------------------------------------------------------

    fn decode_import(&mut self, import: wasmparser::ComponentImport<'_>) -> Result<()> {
        let name = import.name.full_name().to_string();
        match import.ty {
            ComponentTypeRef::Instance(type_idx) => {
                let dense =
                    self.type_remap.get(&type_idx).copied().ok_or_else(|| {
                        Error::MalformedBinary("import instance type missing".into())
                    })?;
                let shape =
                    self.instance_shapes.get(&dense).cloned().ok_or_else(|| {
                        Error::MalformedBinary("import is not an instance".into())
                    })?;
                for (rname, rid) in &shape.resources {
                    self.plan.resources.push(ResourceInfo {
                        type_idx: *rid,
                        interface: Some(name.clone()),
                        name: rname.clone(),
                    });
                }
                self.imported.push(ImportedInstance {
                    name: name.clone(),
                    funcs: shape.funcs.clone(),
                    resources: shape.resources,
                });
                self.instances
                    .push(InstanceSource::Imported(self.imported.len() - 1));
                self.plan.imports.push(WorldImport {
                    name,
                    funcs: shape.funcs,
                });
            }
            ComponentTypeRef::Func(sig_idx) => {
                let sig = self
                    .func_sigs
                    .get(&sig_idx)
                    .cloned()
                    .ok_or_else(|| Error::MalformedBinary("import func type missing".into()))?;
                self.funcs.push(FuncSource::RootImport(self.funcs.len()));
                self.plan.imports.push(WorldImport {
                    name: name.clone(),
                    funcs: vec![(name.clone(), sig)],
                });
            }
            ComponentTypeRef::Type(bounds) => {
                // a type import occupies a slot in the type index space
                let bin_idx = self.type_counter;
                self.type_counter += 1;
                match bounds {
                    TypeBounds::Eq(outer_idx) => {
                        let dense = self.lookup_dense(outer_idx, &HashMap::new())?;
                        self.type_remap.insert(bin_idx, dense);
                    }
                    TypeBounds::SubResource => {
                        self.push_dense(bin_idx, TypeDef::Flags(vec!["\u{0}resource".into()]));
                    }
                }
            }
            ComponentTypeRef::Value(val_ty) => {
                // component-level value import: a host-provided constant,
                // consumed by the start function
                let ty = self.valref(&val_ty, &HashMap::new())?;
                self.plan.values.push(ValueImport {
                    name: name.clone(),
                    ty,
                });
                self.values += 1;
            }
            other => {
                return Err(Error::unsupported(format!(
                    "unsupported import kind {other:?} for `{name}`"
                )))
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Aliases
    // -----------------------------------------------------------------------

    fn decode_alias(&mut self, alias: ComponentAlias<'_>) -> Result<()> {
        match alias {
            ComponentAlias::InstanceExport {
                kind: ComponentExternalKind::Func,
                instance_index,
                name,
            } => {
                // a func export of a nested-component instantiation
                if let Some(InstanceSource::Child(i)) = self.instances.get(instance_index as usize)
                {
                    let instantiation = *i;
                    let sig = self.child_func_sig(instantiation, name)?;
                    let _ = sig;
                    self.funcs.push(FuncSource::ChildExport {
                        instantiation,
                        name: name.to_string(),
                    });
                    return Ok(());
                }
                self.funcs.push(FuncSource::AliasExport {
                    instance: instance_index,
                    name: name.to_string(),
                });
            }
            ComponentAlias::InstanceExport {
                kind: ComponentExternalKind::Instance,
                instance_index,
                name,
            } => {
                // aliasing an instance export = exposing the same instance
                let src = self.instances.get(instance_index as usize).cloned();
                match src {
                    Some(InstanceSource::Imported(i)) => {
                        self.instances.push(InstanceSource::Imported(i));
                        let _ = name;
                    }
                    Some(InstanceSource::FromExports(items)) => {
                        self.instances.push(InstanceSource::FromExports(items));
                        let _ = name;
                    }
                    Some(InstanceSource::Child(i)) => {
                        self.instances.push(InstanceSource::Child(i));
                        let _ = name;
                    }
                    Some(InstanceSource::Occupied) => {
                        return Err(Error::MalformedBinary(
                            "alias of an exported instance is not supported".into(),
                        ))
                    }
                    None => return Err(Error::MalformedBinary("alias of unknown instance".into())),
                }
            }
            ComponentAlias::CoreInstanceExport {
                kind: wasmparser::ExternalKind::Func,
                instance_index,
                name,
            } => {
                // resolve through FromExports renames (e.g. cabi_realloc)
                let r = match self.core_instances.get(instance_index as usize) {
                    Some(CoreInstanceSource::FromExports(items)) => {
                        match items.iter().find(|(n, _)| n == name) {
                            Some((_, CoreExternSource::Func(r))) => r.clone(),
                            _ => {
                                return Err(Error::MalformedBinary(
                                    "core func alias does not reference a function".into(),
                                ))
                            }
                        }
                    }
                    _ => CoreFuncRef::Export {
                        instance: instance_index,
                        name: name.to_string(),
                    },
                };
                self.core_funcs.push(CoreFuncSource::Export(r));
            }
            ComponentAlias::CoreInstanceExport {
                kind: wasmparser::ExternalKind::Memory,
                instance_index,
                name,
            } => {
                let r = match self.core_instances.get(instance_index as usize) {
                    Some(CoreInstanceSource::FromExports(items)) => {
                        match items.iter().find(|(n, _)| n == name) {
                            Some((_, CoreExternSource::Memory(m))) => m.clone(),
                            _ => {
                                return Err(Error::MalformedBinary(
                                    "core memory alias does not reference a memory".into(),
                                ))
                            }
                        }
                    }
                    _ => CoreMemRef::Export {
                        instance: instance_index,
                        name: name.to_string(),
                    },
                };
                self.core_mems.push(r);
            }
            ComponentAlias::CoreInstanceExport { kind, .. } => {
                return Err(Error::unsupported(format!(
                    "core alias kind {kind:?} is not supported"
                )));
            }
            ComponentAlias::InstanceExport { kind, .. } => {
                return Err(Error::unsupported(format!(
                    "component alias kind {kind:?} is not supported"
                )));
            }
            ComponentAlias::Outer { .. } => {
                // outer aliases at the top level refer to enclosing
                // components, which waclay does not support
                return Err(Error::unsupported("outer aliases at top level"));
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Instances
    // -----------------------------------------------------------------------

    fn decode_core_instance(&mut self, inst: wasmparser::Instance<'_>) -> Result<()> {
        match inst {
            wasmparser::Instance::Instantiate { module_index, args } => {
                for a in args.iter() {
                    let name = a.name.to_string();
                    let src = self.core_instances.get(a.index as usize).cloned();
                    if let Some(CoreInstanceSource::FromExports(items)) = src {
                        // args bind whole instances; each func export of the
                        // wrapper satisfies one (module, name) import pair
                        for (ename, item) in items {
                            if let CoreExternSource::Func(r) = item {
                                self.plan.argmap.insert((name.clone(), ename.clone()), r);
                            }
                        }
                    } else if let Some(CoreFuncSource::ChildLower(i)) =
                        self.core_funcs.get(a.index as usize).cloned()
                    {
                        // a lowered nested-component func passed straight to
                        // the shim: bind (module, child-func-name) exactly
                        let fname = self
                            .plan
                            .child_lowers
                            .get(i as usize)
                            .map(|c| c.name.clone())
                            .unwrap_or_default();
                        self.plan
                            .argmap
                            .insert((name, fname), CoreFuncRef::ChildLower(i as usize));
                    }
                }
                let args = args.iter().map(|a| (a.name.to_string(), a.index)).collect();
                self.core_instances.push(CoreInstanceSource::Module {
                    module: module_index,
                    args,
                });
            }
            wasmparser::Instance::FromExports(items) => {
                let mut exports = Vec::new();
                for item in items.iter() {
                    let src = match item.kind {
                        wasmparser::ExternalKind::Func => {
                            let cf = self.resolve_core_func(item.index)?.ok_or_else(|| {
                                Error::MalformedBinary("core export references non-func".into())
                            })?;
                            CoreExternSource::Func(cf)
                        }
                        wasmparser::ExternalKind::Memory => {
                            let cm = self
                                .core_mems
                                .get(item.index as usize)
                                .cloned()
                                .ok_or_else(|| {
                                    Error::MalformedBinary(
                                        "core export references unknown memory".into(),
                                    )
                                })?;
                            CoreExternSource::Memory(cm)
                        }
                        other => {
                            return Err(Error::unsupported(format!(
                                "core FromExports kind {other:?}"
                            )))
                        }
                    };
                    exports.push((item.name.to_string(), src));
                }
                self.core_instances
                    .push(CoreInstanceSource::FromExports(exports));
            }
        }
        Ok(())
    }

    fn decode_component_instance(&mut self, inst: wasmparser::ComponentInstance<'_>) -> Result<()> {
        match inst {
            wasmparser::ComponentInstance::FromExports(items) => {
                let mut out = Vec::new();
                for item in items.iter() {
                    out.push((item.name.full_name().to_string(), item.kind, item.index));
                }
                self.instances.push(InstanceSource::FromExports(out));
            }
            wasmparser::ComponentInstance::Instantiate {
                component_index,
                args,
            } => {
                let nested = *self
                    .components
                    .get(component_index as usize)
                    .ok_or_else(|| {
                        Error::MalformedBinary(format!(
                            "component index {component_index} out of range"
                        ))
                    })?;
                let mut resolved = Vec::new();
                for a in args.iter() {
                    let name = a.name.to_string();
                    let src = match a.kind {
                        ComponentExternalKind::Instance => {
                            match self.instances.get(a.index as usize) {
                                Some(InstanceSource::Imported(i)) => {
                                    let n =
                                        self.imported.get(*i).map(|x| x.name.clone()).ok_or_else(
                                            || {
                                                Error::MalformedBinary(
                                                    "instantiation arg references unknown import"
                                                        .into(),
                                                )
                                            },
                                        )?;
                                    ArgSource::ParentImport(n)
                                }
                                other => {
                                    let what = match other {
                                        Some(_) => "a synthesized or nested instance",
                                        None => "an unknown instance",
                                    };
                                    return Err(Error::unsupported(format!(
                                        "nested instantiation argument `{name}`: passing {what} is not supported (pass a parent import instead)"
                                    )));
                                }
                            }
                        }
                        ComponentExternalKind::Func => match self.funcs.get(a.index as usize) {
                            Some(FuncSource::RootImport(i)) => {
                                let n = self
                                    .plan
                                    .imports
                                    .get(*i)
                                    .map(|x| x.name.clone())
                                    .ok_or_else(|| {
                                        Error::MalformedBinary(
                                            "instantiation func arg references unknown import"
                                                .into(),
                                        )
                                    })?;
                                ArgSource::ParentFunc(n)
                            }
                            _ => {
                                return Err(Error::unsupported(format!(
                                        "nested instantiation argument `{name}`: only imported root functions can be passed as func arguments"
                                    )));
                            }
                        },
                        other => {
                            return Err(Error::unsupported(format!(
                                "nested instantiation argument `{name}`: kind {other:?} is not supported"
                            )));
                        }
                    };
                    resolved.push((name, src));
                }
                let idx = self.plan.component_instantiations.len();
                self.plan.component_instantiations.push(NestedInstantiate {
                    nested,
                    args: resolved,
                });
                self.instances.push(InstanceSource::Child(idx as u32));
            }
        }
        Ok(())
    }

    /// Decode the component start section (top level only).
    fn decode_start(&mut self, start: wasmparser::ComponentStartFunction) -> Result<()> {
        if self.plan.start.is_some() {
            return Err(Error::MalformedBinary(
                "component cannot have more than one start function".into(),
            ));
        }
        let lift = match self.funcs.get(start.func_index as usize) {
            Some(FuncSource::Lift(l)) => *l,
            Some(_) => {
                return Err(Error::unsupported(
                    "the start function must be a canon-lifted component function",
                ))
            }
            None => {
                return Err(Error::MalformedBinary(format!(
                    "start function index {} out of range",
                    start.func_index
                )))
            }
        };
        for a in start.arguments.iter() {
            if (*a as usize) >= self.values {
                return Err(Error::MalformedBinary(format!(
                    "start argument references value index {a} out of range"
                )));
            }
        }
        self.plan.start = Some(StartFunc {
            lift,
            args: start.arguments.to_vec(),
        });
        // start results join the component value index space
        self.values += start.results as usize;
        Ok(())
    }

    /// Signature of a nested component's exported function.
    fn child_func_sig(&self, instantiation: u32, name: &str) -> Result<FuncSig> {
        let ni = self
            .plan
            .component_instantiations
            .get(instantiation as usize)
            .ok_or_else(|| {
                Error::MalformedBinary(format!("instantiation index {instantiation} out of range"))
            })?
            .nested;
        let child = self
            .plan
            .nested
            .get(ni as usize)
            .ok_or_else(|| Error::MalformedBinary("nested component plan missing".into()))?;
        for e in &child.exports {
            for (fname, sig) in &e.funcs {
                if fname == name {
                    return Ok(sig.clone());
                }
            }
        }
        Err(Error::MalformedBinary(format!(
            "nested component export `{name}` not found"
        )))
    }

    fn resolve_core_func(&self, idx: u32) -> Result<Option<CoreFuncRef>> {
        Ok(match self.core_funcs.get(idx as usize) {
            Some(CoreFuncSource::Export(r)) => Some(r.clone()),
            // a canon-lowered import used as a plain core func (FromExports
            // shim instances etc.) — the linker wires it back by plan index
            Some(CoreFuncSource::Lower(i)) => Some(CoreFuncRef::Lower(*i)),
            Some(CoreFuncSource::Builtin(i)) => Some(CoreFuncRef::Builtin(*i as usize)),
            Some(CoreFuncSource::ChildLower(i)) => Some(CoreFuncRef::ChildLower(*i as usize)),
            Some(_) => None,
            None => {
                return Err(Error::MalformedBinary(format!(
                    "core func index {idx} out of range"
                )))
            }
        })
    }

    // -----------------------------------------------------------------------
    // Canonical operations
    // -----------------------------------------------------------------------

    fn decode_canonical(&mut self, canon: CanonicalFunction) -> Result<()> {
        match canon {
            CanonicalFunction::Lift {
                core_func_index,
                type_index,
                options,
            } => {
                let sig = self.func_sigs.get(&type_index).cloned().ok_or_else(|| {
                    Error::MalformedBinary(format!("lift type {type_index} not found"))
                })?;
                let core_func = self.resolve_core_func(core_func_index)?.ok_or_else(|| {
                    Error::unsupported("lift of a non-core function is not supported")
                })?;
                let mut post_return = None;
                let mut realloc = None;
                let mut enc = StringEncoding::Utf8;
                let mut async_lift = false;
                let mut callback = None;
                for opt in options.iter() {
                    match opt {
                        CanonicalOption::UTF8 => enc = StringEncoding::Utf8,
                        CanonicalOption::UTF16 => enc = StringEncoding::Utf16,
                        CanonicalOption::CompactUTF16 => enc = StringEncoding::CompactUtf16,
                        CanonicalOption::Realloc(idx) => {
                            realloc = self.resolve_core_func(*idx)?;
                        }
                        CanonicalOption::PostReturn(idx) => {
                            post_return = self.resolve_core_func(*idx)?;
                        }
                        CanonicalOption::Async => async_lift = true,
                        CanonicalOption::Callback(idx) => {
                            callback = self.resolve_core_func(*idx)?;
                        }
                        CanonicalOption::Memory(_)
                        | CanonicalOption::Gc
                        | CanonicalOption::CoreType(_) => {}
                    }
                }
                self.funcs.push(FuncSource::Lift(self.plan.lifts.len()));
                self.plan.lifts.push(LiftedExport {
                    key: String::new(), // filled by export decoding
                    func_name: String::new(),
                    sig,
                    core_func,
                    post_return,
                    realloc,
                    string_enc: enc,
                    async_lift,
                    callback,
                });
            }
            CanonicalFunction::Lower {
                func_index,
                options,
            } => {
                // lower of a NESTED component's export: the parent's core
                // module calls into the child through a linker trampoline
                if let Some(FuncSource::ChildExport {
                    instantiation,
                    name,
                }) = self.funcs.get(func_index as usize).cloned()
                {
                    let sig = self.child_func_sig(instantiation, &name)?;
                    let mut realloc = None;
                    let mut enc = StringEncoding::Utf8;
                    let mut async_lower = false;
                    for opt in options.iter() {
                        match opt {
                            CanonicalOption::UTF8 => enc = StringEncoding::Utf8,
                            CanonicalOption::UTF16 => enc = StringEncoding::Utf16,
                            CanonicalOption::CompactUTF16 => enc = StringEncoding::CompactUtf16,
                            CanonicalOption::Realloc(idx) => {
                                realloc = self.resolve_core_func(*idx)?;
                            }
                            CanonicalOption::Async => async_lower = true,
                            _ => {}
                        }
                    }
                    let idx = self.plan.child_lowers.len();
                    self.plan.child_lowers.push(ChildLower {
                        instantiation,
                        name,
                        sig,
                        realloc,
                        string_enc: enc,
                        async_lower,
                    });
                    self.core_funcs.push(CoreFuncSource::ChildLower(idx as u32));
                    return Ok(());
                }
                let (interface, func_name, sig) = self.imported_func_info(func_index)?;
                let mut realloc = None;
                let mut enc = StringEncoding::Utf8;
                let mut async_lower = false;
                for opt in options.iter() {
                    match opt {
                        CanonicalOption::UTF8 => enc = StringEncoding::Utf8,
                        CanonicalOption::UTF16 => enc = StringEncoding::Utf16,
                        CanonicalOption::CompactUTF16 => enc = StringEncoding::CompactUtf16,
                        CanonicalOption::Realloc(idx) => {
                            realloc = self.resolve_core_func(*idx)?;
                        }
                        CanonicalOption::Async => async_lower = true,
                        _ => {}
                    }
                }
                let core_module_name = interface.clone().unwrap_or_default();
                let idx = self.plan.lowers.len();
                self.plan.lowers.push(LoweredImport {
                    sig,
                    interface,
                    func_name: func_name.clone(),
                    core_module_name,
                    core_func_name: func_name,
                    realloc,
                    string_enc: enc,
                    async_lower,
                });
                self.core_funcs.push(CoreFuncSource::Lower(idx));
            }
            CanonicalFunction::ResourceNew { resource } => {
                let dense = self.resource_dense(resource)?;
                self.core_funcs.push(CoreFuncSource::ResourceNew(dense));
            }
            CanonicalFunction::ResourceDrop { resource } => {
                let dense = self.resource_dense(resource)?;
                self.core_funcs.push(CoreFuncSource::ResourceDrop(dense));
            }
            CanonicalFunction::ResourceRep { resource } => {
                let dense = self.resource_dense(resource)?;
                self.core_funcs.push(CoreFuncSource::ResourceRep(dense));
            }
            other => self.decode_builtin(other)?,
        }
        Ok(())
    }

    /// Decode one preview-3 async canon builtin into `plan.builtins`.
    fn decode_builtin(&mut self, canon: CanonicalFunction) -> Result<()> {
        let enc_of = |options: &[CanonicalOption]| -> StringEncoding {
            let mut enc = StringEncoding::Utf8;
            for o in options {
                match o {
                    CanonicalOption::UTF8 => enc = StringEncoding::Utf8,
                    CanonicalOption::UTF16 => enc = StringEncoding::Utf16,
                    CanonicalOption::CompactUTF16 => enc = StringEncoding::CompactUtf16,
                    _ => {}
                }
            }
            enc
        };
        let has_async = |options: &[CanonicalOption]| {
            options.iter().any(|o| matches!(o, CanonicalOption::Async))
        };
        let op = match canon {
            CanonicalFunction::StreamNew { ty } => BuiltinOp::StreamNew(self.resource_dense(ty)?),
            CanonicalFunction::StreamRead { ty, options } => BuiltinOp::StreamRead {
                ty: self.resource_dense(ty)?,
                enc: enc_of(&options),
                async_: has_async(&options),
            },
            CanonicalFunction::StreamWrite { ty, options } => BuiltinOp::StreamWrite {
                ty: self.resource_dense(ty)?,
                enc: enc_of(&options),
                async_: has_async(&options),
            },
            CanonicalFunction::StreamForward { ty } => {
                BuiltinOp::StreamForward(self.resource_dense(ty)?)
            }
            CanonicalFunction::StreamCancelRead { ty, async_ } => BuiltinOp::StreamCancelRead {
                ty: self.resource_dense(ty)?,
                async_,
            },
            CanonicalFunction::StreamCancelWrite { ty, async_ } => BuiltinOp::StreamCancelWrite {
                ty: self.resource_dense(ty)?,
                async_,
            },
            CanonicalFunction::StreamDropReadable { ty } => {
                BuiltinOp::StreamDropReadable(self.resource_dense(ty)?)
            }
            CanonicalFunction::StreamDropWritable { ty } => {
                BuiltinOp::StreamDropWritable(self.resource_dense(ty)?)
            }
            CanonicalFunction::FutureNew { ty } => BuiltinOp::FutureNew(self.resource_dense(ty)?),
            CanonicalFunction::FutureRead { ty, options } => BuiltinOp::FutureRead {
                ty: self.resource_dense(ty)?,
                enc: enc_of(&options),
                async_: has_async(&options),
            },
            CanonicalFunction::FutureWrite { ty, options } => BuiltinOp::FutureWrite {
                ty: self.resource_dense(ty)?,
                enc: enc_of(&options),
                async_: has_async(&options),
            },
            CanonicalFunction::FutureForward { ty } => {
                BuiltinOp::FutureForward(self.resource_dense(ty)?)
            }
            CanonicalFunction::FutureCancelRead { ty, async_ } => BuiltinOp::FutureCancelRead {
                ty: self.resource_dense(ty)?,
                async_,
            },
            CanonicalFunction::FutureCancelWrite { ty, async_ } => BuiltinOp::FutureCancelWrite {
                ty: self.resource_dense(ty)?,
                async_,
            },
            CanonicalFunction::FutureDropReadable { ty } => {
                BuiltinOp::FutureDropReadable(self.resource_dense(ty)?)
            }
            CanonicalFunction::FutureDropWritable { ty } => {
                BuiltinOp::FutureDropWritable(self.resource_dense(ty)?)
            }
            CanonicalFunction::ErrorContextNew { options } => {
                BuiltinOp::ErrorContextNew(enc_of(&options))
            }
            CanonicalFunction::ErrorContextDebugMessage { options } => {
                let mut realloc = None;
                let mut saw_realloc = false;
                for o in options.iter() {
                    if let CanonicalOption::Realloc(idx) = o {
                        saw_realloc = true;
                        realloc = self.resolve_core_func(*idx).ok().flatten();
                    }
                }
                let _ = saw_realloc;
                if realloc.is_none() {
                    // the realloc core func lives in the guest instance which
                    // does not exist yet at decode time; resolve by name at
                    // call time (wit-components always name it `cabi_realloc`)
                    realloc = Some(CoreFuncRef::Export {
                        instance: 0,
                        name: "cabi_realloc".into(),
                    });
                }
                BuiltinOp::ErrorContextDebugMessage {
                    enc: enc_of(&options),
                    realloc,
                }
            }
            CanonicalFunction::ErrorContextDrop => BuiltinOp::ErrorContextDrop,
            CanonicalFunction::WaitableSetNew => BuiltinOp::WaitableSetNew,
            CanonicalFunction::WaitableSetWait { .. } => BuiltinOp::WaitableSetWait,
            CanonicalFunction::WaitableSetPoll { .. } => BuiltinOp::WaitableSetPoll,
            CanonicalFunction::WaitableSetDrop => BuiltinOp::WaitableSetDrop,
            CanonicalFunction::WaitableJoin => BuiltinOp::WaitableJoin,
            CanonicalFunction::SubtaskDrop => BuiltinOp::SubtaskDrop,
            CanonicalFunction::SubtaskCancel { async_ } => BuiltinOp::SubtaskCancel { async_ },
            CanonicalFunction::TaskReturn { result, options } => BuiltinOp::TaskReturn {
                result: match result {
                    Some(t) => Some(self.valref(&t, &HashMap::new())?),
                    None => None,
                },
                enc: enc_of(&options),
            },
            CanonicalFunction::TaskCancel => BuiltinOp::TaskCancel,
            CanonicalFunction::BackpressureInc => BuiltinOp::BackpressureInc,
            CanonicalFunction::BackpressureDec => BuiltinOp::BackpressureDec,
            CanonicalFunction::ThreadYield => BuiltinOp::ThreadYield,
            CanonicalFunction::ThreadIndex => BuiltinOp::ThreadIndex,
            CanonicalFunction::ThreadSuspend => BuiltinOp::ThreadSuspend,
            CanonicalFunction::ThreadResumeLater => BuiltinOp::ThreadResumeLater,
            CanonicalFunction::ThreadAvailableParallelism => BuiltinOp::ThreadAvailableParallelism,
            CanonicalFunction::ContextGet { ty, slot } => BuiltinOp::ContextGet { ty, slot },
            CanonicalFunction::ContextSet { ty, slot } => BuiltinOp::ContextSet { ty, slot },
            other => BuiltinOp::Unsupported(format!("{other:?}")),
        };
        let idx = self.plan.builtins.len() as u32;
        self.plan.builtins.push(op);
        self.core_funcs.push(CoreFuncSource::Builtin(idx));
        Ok(())
    }

    fn resource_dense(&self, bin_idx: u32) -> Result<u32> {
        self.type_remap
            .get(&bin_idx)
            .copied()
            .ok_or_else(|| Error::MalformedBinary(format!("resource type {bin_idx} not found")))
    }

    /// Resolve a component func to its imported (interface, name, sig).
    fn imported_func_info(&self, func_index: u32) -> Result<(Option<String>, String, FuncSig)> {
        match self.funcs.get(func_index as usize) {
            Some(FuncSource::ChildExport {
                instantiation,
                name,
            }) => {
                let sig = self.child_func_sig(*instantiation, name)?;
                Ok((None, name.clone(), sig))
            }
            Some(FuncSource::RootImport(import_idx)) => {
                let import = self
                    .plan
                    .imports
                    .get(*import_idx)
                    .ok_or_else(|| Error::MalformedBinary("root import missing".into()))?;
                let (name, sig) = import
                    .funcs
                    .first()
                    .cloned()
                    .ok_or_else(|| Error::MalformedBinary("root import has no func".into()))?;
                Ok((None, name, sig))
            }
            Some(FuncSource::AliasExport { instance, name }) => {
                match self.instances.get(*instance as usize) {
                    Some(InstanceSource::Imported(i)) => {
                        let inst = self.imported.get(*i).ok_or_else(|| {
                            Error::MalformedBinary("imported instance missing".into())
                        })?;
                        let sig = inst
                            .funcs
                            .iter()
                            .find(|(n, _)| n == name)
                            .map(|(_, s)| s.clone())
                            .ok_or_else(|| {
                                Error::MalformedBinary(format!(
                                    "func `{name}` not found in instance `{}`",
                                    inst.name
                                ))
                            })?;
                        Ok((Some(inst.name.clone()), name.clone(), sig))
                    }
                    _ => Err(Error::unsupported(
                        "lower of a function from a synthesized instance is not supported",
                    )),
                }
            }
            _ => Err(Error::unsupported("lower of a non-imported function")),
        }
    }

    // -----------------------------------------------------------------------
    // Exports
    // -----------------------------------------------------------------------

    fn decode_export(&mut self, export: wasmparser::ComponentExport<'_>) -> Result<()> {
        let name = export.name.full_name().to_string();
        match export.kind {
            ComponentExternalKind::Func => {
                // direct re-export of a nested component's function
                if let Some(FuncSource::ChildExport {
                    instantiation,
                    name: cname,
                }) = self.funcs.get(export.index as usize).cloned()
                {
                    let sig = self.child_func_sig(instantiation, &cname)?;
                    self.plan.lifts.push(LiftedExport {
                        key: name.clone(),
                        func_name: name.clone(),
                        sig: sig.clone(),
                        core_func: CoreFuncRef::ChildExport {
                            instantiation,
                            name: cname,
                        },
                        post_return: None,
                        realloc: None,
                        string_enc: StringEncoding::Utf8,
                        async_lift: false,
                        callback: None,
                    });
                    self.plan.exports.push(WorldExport {
                        name: name.clone(),
                        funcs: vec![(name.clone(), sig)],
                    });
                    self.funcs.push(FuncSource::Occupied);
                    return Ok(());
                }
                let lift_idx = match self.funcs.get(export.index as usize) {
                    Some(FuncSource::Lift(l)) => *l,
                    _ => {
                        return Err(Error::MalformedBinary(format!(
                            "export `{name}` does not reference a lifted function"
                        )))
                    }
                };
                self.plan.lifts[lift_idx].key = name.clone();
                self.plan.lifts[lift_idx].func_name = name.clone();
                let sig = self.plan.lifts[lift_idx].sig.clone();
                self.plan.exports.push(WorldExport {
                    name: name.clone(),
                    funcs: vec![(self.plan.lifts[lift_idx].func_name.clone(), sig)],
                });
                // the export itself occupies a component-func index
                self.funcs.push(FuncSource::Occupied);
            }
            ComponentExternalKind::Instance => {
                // re-export of a nested-component instance: surface the
                // child's exports under the parent's export name
                if let Some(InstanceSource::Child(instantiation)) =
                    self.instances.get(export.index as usize).cloned()
                {
                    let ni = self
                        .plan
                        .component_instantiations
                        .get(instantiation as usize)
                        .ok_or_else(|| {
                            Error::MalformedBinary("nested instantiation missing".into())
                        })?
                        .nested;
                    let child =
                        self.plan.nested.get(ni as usize).ok_or_else(|| {
                            Error::MalformedBinary("nested component missing".into())
                        })?;
                    let mut funcs = Vec::new();
                    for e in &child.exports {
                        for (fname, sig) in &e.funcs {
                            self.plan.lifts.push(LiftedExport {
                                key: format!("{name}|{fname}"),
                                func_name: fname.clone(),
                                sig: sig.clone(),
                                core_func: CoreFuncRef::ChildExport {
                                    instantiation,
                                    name: fname.clone(),
                                },
                                post_return: None,
                                realloc: None,
                                string_enc: StringEncoding::Utf8,
                                async_lift: false,
                                callback: None,
                            });
                            funcs.push((fname.clone(), sig.clone()));
                        }
                    }
                    self.plan.exports.push(WorldExport { name, funcs });
                    self.instances.push(InstanceSource::Occupied);
                    return Ok(());
                }
                let items = match self.instances.get(export.index as usize) {
                    Some(InstanceSource::FromExports(items)) => items.clone(),
                    Some(InstanceSource::Imported(i)) => {
                        // re-export of an imported interface: just surface it
                        let funcs = self
                            .imported
                            .get(*i)
                            .map(|inst| inst.funcs.clone())
                            .unwrap_or_default();
                        self.plan.exports.push(WorldExport { name, funcs });
                        self.instances.push(InstanceSource::Occupied);
                        return Ok(());
                    }
                    Some(InstanceSource::Child(_)) | Some(InstanceSource::Occupied) | None => {
                        return Err(Error::MalformedBinary("export of unknown instance".into()))
                    }
                };
                let mut funcs = Vec::new();
                for (fname, kind, fidx) in items {
                    // register resources exported through instance exports
                    // (guest-defined resources live in shared tables)
                    if kind == ComponentExternalKind::Type {
                        if let Some(dense) = self.type_remap.get(&fidx).copied() {
                            if is_resource_marker(&self.plan.types, dense) {
                                self.plan.resources.push(ResourceInfo {
                                    type_idx: dense,
                                    interface: Some(name.clone()),
                                    name: fname.clone(),
                                });
                            }
                        }
                        continue;
                    }
                    if kind != ComponentExternalKind::Func {
                        continue;
                    }
                    let lift_idx = match self.funcs.get(fidx as usize) {
                        Some(FuncSource::Lift(l)) => *l,
                        _ => continue,
                    };
                    self.plan.lifts[lift_idx].key = format!("{name}|{fname}");
                    self.plan.lifts[lift_idx].func_name = fname.clone();
                    funcs.push((fname, self.plan.lifts[lift_idx].sig.clone()));
                }
                self.plan.exports.push(WorldExport { name, funcs });
                // the export itself occupies a component-instance index
                self.instances.push(InstanceSource::Occupied);
            }
            ComponentExternalKind::Type => {
                // a type export occupies a slot in the component type index
                // space; remember what it aliases so later references resolve
                let bin_idx = self.type_counter;
                self.type_counter += 1;
                match export.ty {
                    Some(ComponentTypeRef::Type(TypeBounds::Eq(idx))) => {
                        if let Ok(dense) = self.lookup_dense(idx, &HashMap::new()) {
                            // guest-exported resource: register it so the
                            // resource intrinsics and host API can find it
                            if is_resource_marker(&self.plan.types, dense) {
                                self.plan.resources.push(ResourceInfo {
                                    type_idx: dense,
                                    interface: None,
                                    name: name.clone(),
                                });
                            }
                            self.type_remap.insert(bin_idx, dense);
                        } else {
                            let d = self.plan.types.len() as u32;
                            self.plan
                                .types
                                .push(TypeDef::Flags(vec!["\u{0}type-export".into()]));
                            self.type_remap.insert(bin_idx, d);
                        }
                    }
                    Some(ComponentTypeRef::Type(TypeBounds::SubResource)) => {
                        let dense =
                            self.push_dense(bin_idx, TypeDef::Flags(vec!["\u{0}resource".into()]));
                        self.plan.resources.push(ResourceInfo {
                            type_idx: dense,
                            interface: None,
                            name,
                        });
                    }
                    _ => {
                        let d = self.plan.types.len() as u32;
                        self.plan
                            .types
                            .push(TypeDef::Flags(vec!["\u{0}type-export".into()]));
                        self.type_remap.insert(bin_idx, d);
                    }
                }
            }
            ComponentExternalKind::Value => {
                // exported value (e.g. the start function's result)
                let ty = match export.ty {
                    Some(ComponentTypeRef::Value(v)) => self.valref(&v, &HashMap::new())?,
                    _ => TypeRef::Primitive(Primitive::String),
                };
                self.plan.exported_values.push((name, ty));
            }
            other => {
                return Err(Error::unsupported(format!(
                    "export kind {other:?} is not supported"
                )));
            }
        }
        Ok(())
    }
}

/// True when the dense type at `idx` is the resource-type marker.
fn is_resource_marker(types: &[TypeDef], idx: u32) -> bool {
    matches!(types.get(idx as usize),
        Some(TypeDef::Flags(v)) if v.first().map(|s| s.as_str()) == Some("\u{0}resource"))
}

fn prim(p: PrimitiveValType) -> Option<Primitive> {
    Some(match p {
        PrimitiveValType::Bool => Primitive::Bool,
        PrimitiveValType::S8 => Primitive::S8,
        PrimitiveValType::U8 => Primitive::U8,
        PrimitiveValType::S16 => Primitive::S16,
        PrimitiveValType::U16 => Primitive::U16,
        PrimitiveValType::S32 => Primitive::S32,
        PrimitiveValType::U32 => Primitive::U32,
        PrimitiveValType::S64 => Primitive::S64,
        PrimitiveValType::U64 => Primitive::U64,
        PrimitiveValType::F32 => Primitive::F32,
        PrimitiveValType::F64 => Primitive::F64,
        PrimitiveValType::Char => Primitive::Char,
        PrimitiveValType::String => Primitive::String,
        _ => return None,
    })
}

fn convert_func_type(
    ft: &ComponentFuncType<'_>,
    scope: &HashMap<u32, ScopeEntry>,
    remap: &HashMap<u32, u32>,
) -> Result<FuncSig> {
    let mut params = Vec::with_capacity(ft.params.len());
    for (name, ty) in ft.params.iter() {
        params.push((name.to_string(), val_ref(ty, scope, remap)?));
    }
    let result = match &ft.result {
        Some(t) => Some(val_ref(t, scope, remap)?),
        None => None,
    };
    Ok(FuncSig { params, result })
}

fn val_ref(
    val: &ComponentValType,
    scope: &HashMap<u32, ScopeEntry>,
    remap: &HashMap<u32, u32>,
) -> Result<TypeRef> {
    Ok(match val {
        ComponentValType::Primitive(p) => TypeRef::Primitive(
            prim(*p).ok_or_else(|| Error::MalformedBinary("unknown primitive type".into()))?,
        ),
        ComponentValType::Type(idx) => TypeRef::Def(match scope.get(idx) {
            Some(ScopeEntry::Dense(d)) => *d,
            _ => remap.get(idx).copied().ok_or_else(|| {
                Error::MalformedBinary(format!("type index {idx} not found in scope"))
            })?,
        }),
    })
}
