//! The linker: host-side registration of WIT imports and instantiation of
//! components on any engine.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::abi::layout::FuncLayout;
use crate::abi::{
    lift, load, lower, store, CallerMem, FlatCursor, FlatSink, MemAccess, StringEncoding,
};
use crate::component::decode::{ArgSource, ChildLower, CoreFuncRef};
use crate::component::types::{FuncSig, TypeDef, TypeRef, Val};
use crate::component::{Component, ExportedFunc, InstanceShared};
use crate::engine::{
    CallerView, Extern, Func as CoreFunc, FuncType, HostFn, Imports, Instance, Val as CoreVal,
};
use crate::resource::SharedTables;
use crate::{Error, Result};

/// Context handed to component host callbacks.
pub struct HostCtx<'a> {
    /// Calling guest context (memory access, host data).
    pub caller: &'a mut dyn CallerView,
    /// Per-instance resource tables.
    pub tables: &'a SharedTables,
    /// Per-instance async state (streams/futures/waitable sets).
    pub asyncio: &'a crate::component::asyncio::SharedAsync,
}

/// Untyped host callback for a component function.
///
/// Shared (`Arc<Mutex<..>>`) so one registration can serve several
/// consumers — e.g. the same interface passed to multiple nested components.
pub type ComponentHostFn =
    Arc<Mutex<dyn FnMut(&mut HostCtx<'_>, &[Val], &mut Vec<Val>) -> Result<()> + Send>>;

/// Registry of host functions + instantiation entry point.
#[derive(Default)]
pub struct Linker {
    /// Component-level host funcs, keyed `interface|func` or bare `func`.
    entries: HashMap<String, ComponentHostFn>,
    /// Raw core-module imports, keyed `(module, name)`.
    raw: HashMap<(String, String), HostFn>,
    /// Component-level values (start-function arguments), keyed by name.
    values: HashMap<String, Val>,
}

impl Linker {
    /// Empty linker.
    pub fn new() -> Linker {
        Linker::default()
    }

    /// Register a component-level host function.
    ///
    /// `interface` is the WIT interface name including version (e.g.
    /// `"wasi:random/random@0.2.0"`), or `None` for root-level functions.
    pub fn func(
        &mut self,
        interface: Option<&str>,
        name: &str,
        f: impl FnMut(&mut HostCtx<'_>, &[Val], &mut Vec<Val>) -> Result<()> + Send + 'static,
    ) {
        let key = match interface {
            Some(i) => format!("{i}|{name}"),
            None => name.to_string(),
        };
        self.entries.insert(key, Arc::new(Mutex::new(f)));
    }

    /// Provide a component-level value (a start-function argument).
    ///
    /// Components may import constants (`(import "x" (value (ty)))`) and
    /// consume them in their start function; the host registers them here.
    pub fn value(&mut self, name: &str, val: Val) {
        self.values.insert(name.to_string(), val);
    }

    /// Register a raw core-module import (`module` `name`).
    pub fn raw_func(&mut self, module: &str, name: &str, f: HostFn) {
        self.raw.insert((module.to_string(), name.to_string()), f);
    }

    /// True when a component-level function is registered.
    pub fn has(&self, interface: Option<&str>, name: &str) -> bool {
        let key = match interface {
            Some(i) => format!("{i}|{name}"),
            None => name.to_string(),
        };
        self.entries.contains_key(&key)
    }

    /// Instantiate `component` in `store`, binding every import.
    pub fn instantiate(
        &mut self,
        store: &mut crate::Store,
        component: &Component,
    ) -> Result<ComponentInstance> {
        let shared = InstanceShared::new();
        let plan = component.plan();
        let types: Arc<Vec<TypeDef>> = Arc::new(plan.types.clone());
        let mut imports = Imports::new();

        // ---- core-only path: raw imports, no canonical ABI ---------------
        if plan.lifts.is_empty() && plan.lowers.is_empty() {
            let module = component.core_module();
            for imp in module.imports() {
                let key = (imp.module.clone(), imp.name.clone());
                match &imp.ty {
                    crate::engine::ExternType::Func(fty) => {
                        if let Some(f) = self.raw.remove(&key) {
                            let func = store.new_func(fty.clone(), f)?;
                            imports.add_func(&imp.module, &imp.name, func);
                        } else {
                            return Err(Error::MissingImport {
                                module: imp.module.clone(),
                                name: imp.name.clone(),
                            });
                        }
                    }
                    crate::engine::ExternType::Memory(_) => {
                        // memory imports from the host are rare; create one
                        let mem =
                            store.new_memory(crate::engine::MemoryType { min: 1, max: None })?;
                        imports.add_memory(&imp.module, &imp.name, mem);
                    }
                }
            }
            let instance = module.instantiate(store, &imports)?;
            return Ok(ComponentInstance {
                exports: HashMap::new(),
                instance,
                tables: shared.tables.clone(),
                asyncio: shared.asyncio.clone(),
                core_only: true,
                start_result: None,
                resources: plan.resources.clone(),
            });
        }

        // ---- component path ----------------------------------------------
        let module = component.core_module();
        let mut used_lowers: HashSet<usize> = HashSet::new();

        // ---- nested components: instantiate children first ----------------
        // Children must exist before the parent's shim is wired: the parent
        // can canon-lower child functions into its own core module and
        // re-export the child's exports.
        let engine = component.engine_handle();
        let mut children: HashMap<u32, ComponentInstance> = HashMap::new();
        for (ci, inst) in plan.component_instantiations.iter().enumerate() {
            let child_plan = plan
                .nested
                .get(inst.nested as usize)
                .ok_or_else(|| Error::MalformedBinary("nested component plan missing".into()))?;
            let child_component = Component::from_plan(&engine, child_plan.clone())?;
            let mut child_linker = Linker::new();
            for (cname, src) in &inst.args {
                match src {
                    ArgSource::ParentImport(pname) => {
                        // copy every parent entry of that interface into the
                        // child linker under the child's import name
                        let copied: Vec<(String, ComponentHostFn)> = self
                            .entries
                            .iter()
                            .filter(|(k, _)| interface_key_matches(k, pname))
                            .map(|(k, v)| {
                                let fname =
                                    k.split_once('|').map(|x| x.1).unwrap_or("").to_string();
                                (format!("{cname}|{fname}"), v.clone())
                            })
                            .collect();
                        if copied.is_empty() {
                            return Err(Error::MissingImport {
                                module: pname.clone(),
                                name: format!("(needed by nested component import `{cname}`)"),
                            });
                        }
                        for (k, v) in copied {
                            child_linker.entries.insert(k, v);
                        }
                    }
                    ArgSource::ParentFunc(fname) => match self.entries.get(fname) {
                        Some(f) => {
                            child_linker.entries.insert(cname.clone(), f.clone());
                        }
                        None => {
                            return Err(Error::MissingImport {
                                module: "(root)".into(),
                                name: format!(
                                    "{fname} (needed by nested component import `{cname}`)"
                                ),
                            })
                        }
                    },
                    ArgSource::ChildExport { .. } => {
                        return Err(Error::unsupported(
                            "passing a sibling component's export to a nested component is not supported; pass a parent import instead",
                        ));
                    }
                }
            }
            let child = child_linker.instantiate(store, &child_component)?;
            children.insert(ci as u32, child);
        }

        for imp in module.imports() {
            let ty = match &imp.ty {
                crate::engine::ExternType::Func(ft) => ft.clone(),
                crate::engine::ExternType::Memory(_) => continue,
            };

            // 1) resource intrinsics ------------------------------------
            if let Some((op, res_name)) = parse_intrinsic(&imp.name) {
                let res = find_resource_id(&plan.resources, &imp.module, res_name)?;
                let f = make_resource_intrinsic(
                    shared.tables.clone(),
                    res,
                    op,
                    rep_is_i64(&ty),
                    rep_ret_is_i64(&ty),
                )?;
                let func = store.new_func(ty.clone(), f)?;
                imports.add_func(&imp.module, &imp.name, func);
                continue;
            }

            // 1b) argmap-bound canon op (builtin or lowered import): the
            //     decoder resolved the core-instantiation args, so this
            //     binding is exact regardless of naming conventions.
            if let Some(r) = plan.argmap.get(&(imp.module.clone(), imp.name.clone())) {
                match r {
                    CoreFuncRef::Builtin(i) => {
                        let op = plan.builtins.get(*i).cloned().ok_or_else(|| {
                            Error::MalformedBinary("builtin op index out of range".into())
                        })?;
                        let f = make_builtin_func(shared.clone(), op, types.clone())?;
                        let func = store.new_func(ty.clone(), f)?;
                        imports.add_func(&imp.module, &imp.name, func);
                        continue;
                    }
                    CoreFuncRef::ChildLower(i) => {
                        let cl = plan.child_lowers.get(*i).ok_or_else(|| {
                            Error::MalformedBinary("child lower index out of range".into())
                        })?;
                        let child = children.get(&cl.instantiation).ok_or_else(|| {
                            Error::MalformedBinary("nested component instance missing".into())
                        })?;
                        let (fty2, f) = make_child_lower_func(shared.clone(), child, cl, &types)?;
                        let func = store.new_func(fty2, f)?;
                        imports.add_func(&imp.module, &imp.name, func);
                        continue;
                    }
                    CoreFuncRef::Lower(idx) if !used_lowers.contains(idx) => {
                        let idx = *idx;
                        used_lowers.insert(idx);
                        let lower = &plan.lowers[idx];
                        wire_lower(self, &shared, store, lower, &types, imp, &ty, &mut imports)?;
                        continue;
                    }
                    _ => {}
                }
            }

            // 2) canon lower ----------------------------------------------
            if let Some(idx) =
                self.match_lower(plan, &imp.module, &imp.name, &ty, &used_lowers, &types)
            {
                used_lowers.insert(idx);
                let lower = &plan.lowers[idx];
                wire_lower(self, &shared, store, lower, &types, imp, &ty, &mut imports)?;
                continue;
            }

            // 3) raw fallback ---------------------------------------------
            let key = (imp.module.clone(), imp.name.clone());
            if let Some(f) = self.raw.remove(&key) {
                let func = store.new_func(ty, f)?;
                imports.add_func(&imp.module, &imp.name, func);
                continue;
            }

            // 4) graceful stub: instantiation succeeds; calling traps with a
            //    clear message (lets components with unimplemented WASI
            //    interfaces like filesystem still load)
            let module = imp.module.clone();
            let name = imp.name.clone();
            let f = store.new_func(
                ty,
                Box::new(move |_caller, _args, _results| {
                    Err(Error::unsupported(format!(
                        "import `{module}` `{name}` is not implemented by this host"
                    )))
                }),
            )?;
            imports.add_func(&imp.module, &imp.name, f);
        }

        // ---- instantiate the shim module ---------------------------------
        let instance = module.instantiate(store, &imports)?;

        // resolve memory + named core funcs
        if let Some(Extern::Memory(m)) = store.get_export(&instance, "memory") {
            *shared.memory.lock().unwrap() = Some(m);
        }
        for r in core_func_refs(plan) {
            let name = match r {
                CoreFuncRef::Export { name, .. } => name,
                // lowered imports / builtins / child refs are wired by the
                // linker directly, nothing to resolve by name
                CoreFuncRef::Lower(_)
                | CoreFuncRef::Builtin(_)
                | CoreFuncRef::ChildLower(_)
                | CoreFuncRef::ChildExport { .. } => continue,
            };
            if let Some(Extern::Func(f)) = store.get_export(&instance, &name) {
                shared
                    .funcs
                    .lock()
                    .unwrap()
                    .entry((0, name.clone()))
                    .or_insert(f);
            }
        }

        // ---- bind lifted exports ------------------------------------------
        let mut exports: HashMap<String, ExportedFunc> = HashMap::new();
        for (i, lift) in plan.lifts.iter().enumerate() {
            if lift.key.is_empty() {
                continue;
            }
            let exported = match &lift.core_func {
                CoreFuncRef::ChildExport {
                    instantiation,
                    name,
                } => {
                    // re-export of a nested component's function: the child
                    // is already fully bound — reuse its ExportedFunc
                    let child = children.get(instantiation).ok_or_else(|| {
                        Error::MalformedBinary("nested component instance missing".into())
                    })?;
                    child.find_func(name).cloned().ok_or_else(|| {
                        Error::MalformedBinary(format!(
                            "nested component export `{name}` not found"
                        ))
                    })?
                }
                _ => build_exported_func(plan, i, &shared, &instance, store, &types)?,
            };
            exports.insert(lift.key.clone(), exported);
        }

        // ---- reactor initialization ---------------------------------------
        if plan.has_initialize {
            if let Some(Extern::Func(f)) = store.get_export(&instance, "_initialize") {
                store.call(&f, &[], &mut [])?;
            }
        }

        // ---- component start section ----------------------------------------
        let mut start_result: Option<Val> = None;
        if let Some(start) = &plan.start {
            let mut args = Vec::with_capacity(start.args.len());
            for ai in &start.args {
                let vi = plan.values.get(*ai as usize).ok_or_else(|| {
                    Error::MalformedBinary("start argument value index out of range".into())
                })?;
                let v = self
                    .values
                    .remove(&vi.name)
                    .ok_or_else(|| Error::MissingImport {
                        module: "value".into(),
                        name: vi.name.clone(),
                    })?;
                args.push(v);
            }
            let f = build_exported_func(plan, start.lift, &shared, &instance, store, &types)?;
            start_result = f.call_dynamic(store, &args)?;
        }

        Ok(ComponentInstance {
            exports,
            instance,
            tables: shared.tables.clone(),
            asyncio: shared.asyncio.clone(),
            core_only: false,
            start_result,
            resources: plan.resources.clone(),
        })
    }

    fn lookup_entry(&mut self, interface: Option<&str>, name: &str) -> Option<ComponentHostFn> {
        if let Some(i) = interface {
            // 1) exact `interface|func`
            let key = format!("{i}|{name}");
            if let Some(f) = self.entries.remove(&key) {
                return Some(f);
            }
            // 2) tolerate version suffixes and qualification differences:
            //    the plan carries full package names (`ns:pkg/iface@1.0.0`)
            //    while worlds may reference the interface by its local name.
            let base = i.split('@').next().unwrap_or(i);
            let local = base.rsplit('/').next().unwrap_or(base);
            for candidate in [base, local] {
                let key = format!("{candidate}|{name}");
                if let Some(f) = self.entries.remove(&key) {
                    return Some(f);
                }
            }
            // 3) loose suffix match: any registered interface that denotes the
            //    same WIT interface (after version stripping / local naming).
            let suffix = format!("|{name}");
            let matches = self.entries.keys().find(|k| {
                if !k.ends_with(&suffix) {
                    return false;
                }
                let ki = k.split('|').next().unwrap_or("");
                let kibase = ki.split('@').next().unwrap_or(ki);
                kibase == base
                    || kibase.ends_with(&format!("/{local}"))
                    || base.ends_with(&format!("/{kibase}"))
                    || kibase == local
            });
            if let Some(k) = matches.cloned() {
                if let Some(f) = self.entries.remove(&k) {
                    return Some(f);
                }
            }
        }
        // 4) bare name (convenience for single-interface worlds)
        self.entries.remove(name)
    }

    fn match_lower(
        &self,
        plan: &crate::component::ComponentPlan,
        module: &str,
        name: &str,
        ty: &FuncType,
        used: &HashSet<usize>,
        types: &[TypeDef],
    ) -> Option<usize> {
        // 1) exact core names
        for (i, l) in plan.lowers.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }
            if l.core_module_name == module && l.core_func_name == name {
                return Some(i);
            }
        }
        // 2) name-based with interface fallback
        for (i, l) in plan.lowers.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }
            let iface_ok = match &l.interface {
                Some(iface) => iface == module,
                None => true,
            };
            if iface_ok && l.func_name == name {
                return Some(i);
            }
        }
        // 3) type-based fallback (flat signature comparison)
        for (i, l) in plan.lowers.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }
            let layout = FuncLayout::new(&l.sig.params, l.sig.result, types);
            let params_match = layout
                .core_params(crate::abi::layout::Context::Lower)
                .iter()
                .map(core_ty_of)
                .eq(ty.params.iter().copied());
            let results_match = layout
                .core_results(crate::abi::layout::Context::Lower)
                .iter()
                .map(core_ty_of)
                .eq(ty.results.iter().copied());
            if params_match && results_match {
                return Some(i);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Import binding helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum ResourceIntrinsic {
    New,
    Drop,
    Rep,
}

fn parse_intrinsic(name: &str) -> Option<(ResourceIntrinsic, &str)> {
    if let Some(r) = name.strip_prefix("[resource-new]") {
        Some((ResourceIntrinsic::New, r))
    } else if let Some(r) = name.strip_prefix("[resource-drop]") {
        Some((ResourceIntrinsic::Drop, r))
    } else if let Some(r) = name.strip_prefix("[resource-rep]") {
        Some((ResourceIntrinsic::Rep, r))
    } else {
        None
    }
}

fn rep_is_i64(ty: &FuncType) -> bool {
    ty.params.first() == Some(&crate::engine::ValType::I64)
}

fn rep_ret_is_i64(ty: &FuncType) -> bool {
    ty.results.first() == Some(&crate::engine::ValType::I64)
}

fn find_resource_id(
    resources: &[crate::component::ResourceInfo],
    interface: &str,
    resource: &str,
) -> Result<u32> {
    for r in resources {
        if r.name == resource {
            match &r.interface {
                Some(i) if i == interface => return Ok(r.type_idx),
                Some(_) => continue,
                None => return Ok(r.type_idx),
            }
        }
    }
    // fallback: unique resource name across interfaces
    let mut found = None;
    for r in resources {
        if r.name == resource {
            match found {
                None => found = Some(r.type_idx),
                Some(_) => {
                    return Err(Error::Resource(format!(
                        "ambiguous resource `{resource}` in `{interface}`"
                    )))
                }
            }
        }
    }
    found.ok_or_else(|| Error::Resource(format!("unknown resource `{resource}` in `{interface}`")))
}

fn make_resource_intrinsic(
    tables: SharedTables,
    resource: u32,
    op: ResourceIntrinsic,
    rep_is_i64: bool,
    ret_is_i64: bool,
) -> Result<HostFn> {
    Ok(match op {
        ResourceIntrinsic::New => Box::new(move |_caller, args, results| {
            let rep = if rep_is_i64 {
                args[0].i64()? as u64
            } else {
                args[0].i32()? as u64
            };
            let h = tables.lock().unwrap().resource_new(resource, rep);
            if ret_is_i64 {
                results.push(CoreVal::I64(h as i64));
            } else {
                results.push(CoreVal::I32(h as i32));
            }
            Ok(())
        }),
        ResourceIntrinsic::Drop => Box::new(move |_caller, args, _results| {
            let h = args[0].i32()? as u32;
            tables
                .lock()
                .unwrap()
                .resource_drop(resource, h)
                .map_err(|e| Error::trap(e.to_string()))?;
            Ok(())
        }),
        ResourceIntrinsic::Rep => Box::new(move |_caller, args, results| {
            let h = args[0].i32()? as u32;
            let rep = tables.lock().unwrap().resource_rep(resource, h)?;
            if ret_is_i64 {
                results.push(CoreVal::I64(rep as i64));
            } else {
                results.push(CoreVal::I32(rep as i32));
            }
            Ok(())
        }),
    })
}

/// Create the core host function implementing one canon-lowered import.
#[allow(clippy::too_many_arguments)]
fn make_lowered_func(
    shared: Arc<InstanceShared>,
    layout: Arc<FuncLayout>,
    sig: Arc<FuncSig>,
    types: Arc<Vec<TypeDef>>,
    enc: StringEncoding,
    realloc_ref: Option<CoreFuncRef>,
    user: ComponentHostFn,
    async_lower: bool,
) -> HostFn {
    Box::new(move |caller, args, results| {
        let memory = shared.memory()?;
        let realloc = match &realloc_ref {
            Some(r) => shared.realloc(r)?,
            None => None,
        };
        let ctx = if async_lower {
            crate::abi::layout::Context::AsyncLower
        } else {
            crate::abi::layout::Context::Lower
        };

        // out-pointer for async lowering (present whenever a result exists)
        let outptr: Option<u32> = if async_lower && !layout.result_flat.is_empty() {
            Some(
                args.last()
                    .and_then(|v| v.i32().ok())
                    .ok_or_else(|| Error::value("missing out-pointer argument"))?
                    as u32,
            )
        } else {
            None
        };

        // -- lift params -------------------------------------------------
        let (params, retptr) = {
            let mut mem = CallerMem {
                caller,
                memory: &memory,
                realloc_fn: realloc.as_ref(),
            };
            let mut cursor = FlatCursor::new(args.to_vec());
            let mut params = Vec::with_capacity(layout.params.len());
            if layout.params_indirect_for(ctx) {
                let base = cursor.vals.first().and_then(|v| v.i32().ok()).unwrap_or(0) as u64;
                for slot in &layout.params {
                    params.push(load(&slot.ty, &types, base + slot.offset, &mut mem, enc)?);
                }
                (params, None)
            } else {
                for slot in &layout.params {
                    params.push(lift(&slot.ty, &types, &mut cursor, &mut mem, enc)?);
                }
                let retptr = match ctx {
                    crate::abi::layout::Context::Lower if layout.results_indirect => Some(
                        args.last()
                            .and_then(|v| v.i32().ok())
                            .ok_or_else(|| Error::value("missing retptr argument"))?
                            as u32,
                    ),
                    _ => None,
                };
                (params, retptr)
            }
        };

        // -- call the user ------------------------------------------------
        let mut out_vals: Vec<Val> = Vec::new();
        {
            let mut ctx = HostCtx {
                caller: &mut *caller,
                tables: &shared.tables,
                asyncio: &shared.asyncio,
            };
            (user.lock().unwrap())(&mut ctx, &params, &mut out_vals)?;
        }

        // -- lower the result ---------------------------------------------
        if let Some(rt) = &layout.result {
            let v = out_vals
                .into_iter()
                .next()
                .ok_or_else(|| Error::Host("host function produced no result".into()))?;
            let mut mem = CallerMem {
                caller,
                memory: &memory,
                realloc_fn: realloc.as_ref(),
            };
            let indirect = match ctx {
                crate::abi::layout::Context::Lower => layout.results_indirect,
                crate::abi::layout::Context::AsyncLower => true,
                _ => false,
            };
            if indirect {
                let ptr = if async_lower {
                    outptr.ok_or_else(|| Error::value("missing out-pointer"))? as u64
                } else {
                    retptr.ok_or_else(|| Error::value("missing retptr"))? as u64
                };
                store(&rt.ty, &types, &v, ptr, &mut mem, enc)?;
            } else {
                let mut sink = FlatSink::default();
                lower(&rt.ty, &types, &v, &mut sink, &mut mem, enc)?;
                for v in sink.vals {
                    results.push(v);
                }
            }
        }
        // async lowering: the core function returns the subtask state
        // (RETURNED = 2 — the host always completes synchronously)
        if async_lower {
            results.push(CoreVal::I32(
                crate::component::asyncio::SubtaskState::Returned as i32,
            ));
        }
        let _ = &sig;
        Ok(())
    })
}

/// Tolerant interface-name match between a linker entry key's interface part
/// and a (possibly versioned / locally named) WIT interface name.
fn interface_key_matches(key: &str, iface: &str) -> bool {
    let ki = key.split('|').next().unwrap_or("");
    if ki.is_empty() {
        return false;
    }
    let base = iface.split('@').next().unwrap_or(iface);
    let local = base.rsplit('/').next().unwrap_or(base);
    let kibase = ki.split('@').next().unwrap_or(ki);
    let kilocal = kibase.rsplit('/').next().unwrap_or(kibase);
    kibase == base
        || kibase == local
        || kilocal == base
        || kilocal == local
        || base.ends_with(&format!("/{kilocal}"))
        || kibase.ends_with(&format!("/{local}"))
}

/// Wire a canon-lowered export of a nested component into the parent's core
/// module: a trampoline that lifts the parent's flat args, calls the child
/// through its own memory/ABI, and lowers the result back into the parent.
fn make_child_lower_func(
    shared: Arc<InstanceShared>,
    child: &ComponentInstance,
    cl: &ChildLower,
    types: &Arc<Vec<TypeDef>>,
) -> Result<(FuncType, HostFn)> {
    let layout = Arc::new(FuncLayout::new(&cl.sig.params, cl.sig.result, types));
    let ctx = if cl.async_lower {
        crate::abi::layout::Context::AsyncLower
    } else {
        crate::abi::layout::Context::Lower
    };
    let fty = FuncType::new(
        layout
            .core_params(ctx)
            .iter()
            .map(core_ty_of)
            .collect::<Vec<_>>(),
        layout
            .core_results(ctx)
            .iter()
            .map(core_ty_of)
            .collect::<Vec<_>>(),
    );
    let func = child.find_func(&cl.name).cloned().ok_or_else(|| {
        Error::MalformedBinary(format!("nested component export `{}` not found", cl.name))
    })?;
    let types = types.clone();
    let enc = cl.string_enc;
    let realloc_ref = cl.realloc.clone();
    let async_lower = cl.async_lower;
    let f = Box::new(
        move |caller: &mut dyn crate::engine::CallerView,
              args: &[CoreVal],
              results: &mut Vec<CoreVal>| {
            let memory = shared.memory()?;
            let realloc = match &realloc_ref {
                Some(r) => shared.realloc(r)?,
                None => None,
            };
            let outptr: Option<u32> = if async_lower && !layout.result_flat.is_empty() {
                Some(
                    args.last()
                        .and_then(|v| v.i32().ok())
                        .ok_or_else(|| Error::value("missing out-pointer argument"))?
                        as u32,
                )
            } else {
                None
            };

            // -- lift params out of the PARENT's memory ----------------------
            let (params, retptr) = {
                let mut mem = CallerMem {
                    caller: &mut *caller,
                    memory: &memory,
                    realloc_fn: realloc.as_ref(),
                };
                let mut cursor = FlatCursor::new(args.to_vec());
                let mut params = Vec::with_capacity(layout.params.len());
                if layout.params_indirect_for(ctx) {
                    let base = cursor.vals.first().and_then(|v| v.i32().ok()).unwrap_or(0) as u64;
                    for slot in &layout.params {
                        params.push(load(&slot.ty, &types, base + slot.offset, &mut mem, enc)?);
                    }
                    (params, None)
                } else {
                    for slot in &layout.params {
                        params.push(lift(&slot.ty, &types, &mut cursor, &mut mem, enc)?);
                    }
                    let retptr = match ctx {
                        crate::abi::layout::Context::Lower if layout.results_indirect => Some(
                            args.last()
                                .and_then(|v| v.i32().ok())
                                .ok_or_else(|| Error::value("missing retptr argument"))?
                                as u32,
                        ),
                        _ => None,
                    };
                    (params, retptr)
                }
            };

            // -- call the child (uses the child's memory + realloc) ----------
            let res = func.call_dynamic_via(caller, &params)?;

            // -- lower the result back into the parent's space ----------------
            if let Some(rt) = &layout.result {
                let v = res.ok_or_else(|| {
                    Error::Host("nested component function produced no result".into())
                })?;
                let mut mem = CallerMem {
                    caller: &mut *caller,
                    memory: &memory,
                    realloc_fn: realloc.as_ref(),
                };
                let indirect = match ctx {
                    crate::abi::layout::Context::Lower => layout.results_indirect,
                    crate::abi::layout::Context::AsyncLower => true,
                    _ => false,
                };
                if indirect {
                    let ptr = if async_lower {
                        outptr.ok_or_else(|| Error::value("missing out-pointer"))? as u64
                    } else {
                        retptr.ok_or_else(|| Error::value("missing retptr"))? as u64
                    };
                    store(&rt.ty, &types, &v, ptr, &mut mem, enc)?;
                } else {
                    let mut sink = FlatSink::default();
                    lower(&rt.ty, &types, &v, &mut sink, &mut mem, enc)?;
                    for v in sink.vals {
                        results.push(v);
                    }
                }
            }
            if async_lower {
                results.push(CoreVal::I32(
                    crate::component::asyncio::SubtaskState::Returned as i32,
                ));
            }
            Ok(())
        },
    );
    Ok((fty, f))
}

/// Bind one canon-lowered import (shared by argmap and name-based paths).
#[allow(clippy::too_many_arguments)]
fn wire_lower(
    linker: &mut Linker,
    shared: &Arc<InstanceShared>,
    store: &mut crate::Store,
    lower: &crate::component::LoweredImport,
    types: &Arc<Vec<TypeDef>>,
    imp: &crate::engine::ImportType,
    _ty: &FuncType,
    imports: &mut Imports,
) -> Result<()> {
    let layout = Arc::new(FuncLayout::new(&lower.sig.params, lower.sig.result, types));
    let ctx = if lower.async_lower {
        crate::abi::layout::Context::AsyncLower
    } else {
        crate::abi::layout::Context::Lower
    };
    let fty = FuncType::new(
        layout
            .core_params(ctx)
            .iter()
            .map(core_ty_of)
            .collect::<Vec<_>>(),
        layout
            .core_results(ctx)
            .iter()
            .map(core_ty_of)
            .collect::<Vec<_>>(),
    );
    let user = match linker.lookup_entry(lower.interface.as_deref(), &lower.func_name) {
        Some(u) => u,
        None => {
            // The plan knows this import but the host never registered it:
            // bind a stub that traps with a clear message when called
            // (instantiation still succeeds — components with optional or
            // unimplemented interfaces remain loadable).
            let module = imp.module.clone();
            let name = imp.name.clone();
            Arc::new(Mutex::new(Box::new(
                move |_ctx: &mut HostCtx<'_>, _args: &[Val], _out: &mut Vec<Val>| {
                    Err(Error::unsupported(format!(
                        "import `{module}` `{name}` is not implemented by this host"
                    )))
                },
            )
                as Box<
                    dyn FnMut(&mut HostCtx<'_>, &[Val], &mut Vec<Val>) -> Result<()> + Send,
                >))
        }
    };
    let f = make_lowered_func(
        shared.clone(),
        layout,
        Arc::new(lower.sig.clone()),
        types.clone(),
        lower.string_enc,
        lower.realloc.clone(),
        user,
        lower.async_lower,
    );
    let f = store.new_func(fty, f)?;
    imports.add_func(&imp.module, &imp.name, f);
    Ok(())
}

// ---------------------------------------------------------------------------
// Preview-3 async builtins (streams / futures / waitables / tasks)
// ---------------------------------------------------------------------------

/// Create the core host function implementing one canon async builtin.
///
/// Core signatures follow the Component Model spec (CanonicalABI.md,
/// "Builtin functions"); value copy directions follow `canon stream.read` /
/// `canon stream.write` (`ReadableEnd`/`WritableEnd` + guest buffers).
fn make_builtin_func(
    shared: Arc<InstanceShared>,
    op: crate::component::decode::BuiltinOp,
    types: Arc<Vec<TypeDef>>,
) -> Result<HostFn> {
    use crate::abi::layout::size as ty_size;
    use crate::component::asyncio::EventCode;
    use crate::component::decode::BuiltinOp;

    /// Element type of a stream/future type definition.
    fn elem_of(types: &[TypeDef], ty: u32) -> Result<Option<TypeRef>> {
        match types.get(ty as usize) {
            Some(TypeDef::Stream(t)) | Some(TypeDef::Future(t)) => Ok(*t),
            _ => Err(Error::value(format!("type #{ty} is not a stream/future"))),
        }
    }

    Ok(match op {
        // (func (result i64)) — low 32 bits: readable, high 32: writable
        BuiltinOp::StreamNew(ty) => Box::new(move |_c, _a, results| {
            let (r, w) = shared.asyncio.lock().unwrap().stream_new(ty);
            results.push(CoreVal::I64((r as i64) | ((w as i64) << 32)));
            Ok(())
        }),
        BuiltinOp::FutureNew(ty) => Box::new(move |_c, _a, results| {
            let (r, w) = shared.asyncio.lock().unwrap().future_new(ty);
            results.push(CoreVal::I64((r as i64) | ((w as i64) << 32)));
            Ok(())
        }),

        // (func (param i32 i32 i32) (result i32))
        BuiltinOp::StreamRead { ty, enc, async_ } => Box::new(move |caller, args, results| {
            let h = args[0].i32()? as u32;
            let ptr = args[1].i32()? as u32;
            let count = args[2].i32()? as u32;
            let (packed, vals) = {
                let mut st = shared.asyncio.lock().unwrap();
                let (packed, vals) = st.stream_read(ty, h, count, async_)?;
                if packed == crate::component::asyncio::BLOCKED {
                    // remember the guest buffer; the host completes it later
                    st.register_read(
                        h,
                        crate::component::asyncio::PendingRead {
                            ptr,
                            count,
                            elem: elem_of(&types, ty)?,
                            enc,
                            types: types.clone(),
                        },
                    )?;
                }
                (packed, vals)
            };
            if !vals.is_empty() {
                let memory = shared.memory()?;
                if let Some(et) = elem_of(&types, ty)? {
                    let mut mem = CallerMem {
                        caller,
                        memory: &memory,
                        realloc_fn: None,
                    };
                    let esz = ty_size(&et, &types);
                    for (i, v) in vals.iter().enumerate() {
                        crate::abi::store(
                            &et,
                            &types,
                            v,
                            ptr as u64 + i as u64 * esz,
                            &mut mem,
                            enc,
                        )?;
                    }
                }
            }
            results.push(CoreVal::I32(packed));
            Ok(())
        }),
        BuiltinOp::FutureRead { ty, enc, async_ } => Box::new(move |caller, args, results| {
            let h = args[0].i32()? as u32;
            let ptr = args[1].i32()? as u32;
            let (packed, val) = {
                let mut st = shared.asyncio.lock().unwrap();
                let (packed, val) = st.future_read(ty, h, async_)?;
                if packed == crate::component::asyncio::BLOCKED {
                    st.register_read(
                        h,
                        crate::component::asyncio::PendingRead {
                            ptr,
                            count: 1,
                            elem: elem_of(&types, ty)?,
                            enc,
                            types: types.clone(),
                        },
                    )?;
                }
                (packed, val)
            };
            if let (Some(v), Some(et)) = (val, elem_of(&types, ty)?) {
                let memory = shared.memory()?;
                let mut mem = CallerMem {
                    caller,
                    memory: &memory,
                    realloc_fn: None,
                };
                crate::abi::store(&et, &types, &v, ptr as u64, &mut mem, enc)?;
            }
            results.push(CoreVal::I32(packed));
            Ok(())
        }),

        // (func (param i32 i32 i32) (result i32))
        BuiltinOp::StreamWrite { ty, enc, .. } => Box::new(move |caller, args, results| {
            let h = args[0].i32()? as u32;
            let ptr = args[1].i32()? as u32;
            let count = args[2].i32()? as u32;
            let memory = shared.memory()?;
            let mut vals: Vec<Val> = Vec::new();
            if count > 0 {
                if let Some(et) = elem_of(&types, ty)? {
                    let mut mem = CallerMem {
                        caller,
                        memory: &memory,
                        realloc_fn: None,
                    };
                    let esz = ty_size(&et, &types);
                    for i in 0..count {
                        vals.push(crate::abi::load(
                            &et,
                            &types,
                            ptr as u64 + i as u64 * esz,
                            &mut mem,
                            enc,
                        )?);
                    }
                }
            }
            let (packed, pending, drained) =
                shared.asyncio.lock().unwrap().stream_write(ty, h, vals)?;
            if let Some(p) = pending {
                // complete the pending async read: store into guest memory
                if let Some(et) = p.elem {
                    let memory = shared.memory()?;
                    let mut mem = CallerMem {
                        caller: &mut *caller,
                        memory: &memory,
                        realloc_fn: None,
                    };
                    let esz = ty_size(&et, &p.types);
                    for (i, v) in drained.iter().enumerate() {
                        crate::abi::store(
                            &et,
                            &p.types,
                            v,
                            p.ptr as u64 + i as u64 * esz,
                            &mut mem,
                            p.enc,
                        )?;
                    }
                }
            }
            results.push(CoreVal::I32(packed));
            Ok(())
        }),
        BuiltinOp::FutureWrite { ty, enc, .. } => Box::new(move |caller, args, results| {
            let h = args[0].i32()? as u32;
            let ptr = args[1].i32()? as u32;
            let memory = shared.memory()?;
            let val = match elem_of(&types, ty)? {
                Some(et) => {
                    let mut mem = CallerMem {
                        caller,
                        memory: &memory,
                        realloc_fn: None,
                    };
                    Some(crate::abi::load(&et, &types, ptr as u64, &mut mem, enc)?)
                }
                None => None,
            };
            let v = val.unwrap_or(Val::Bool(false));
            let (packed, pending, deliver) =
                shared.asyncio.lock().unwrap().future_write(ty, h, v)?;
            if let (Some(p), Some(v)) = (pending, deliver) {
                let memory = shared.memory()?;
                let mut mem = CallerMem {
                    caller: &mut *caller,
                    memory: &memory,
                    realloc_fn: None,
                };
                if let Some(et) = p.elem {
                    crate::abi::store(&et, &p.types, &v, p.ptr as u64, &mut mem, p.enc)?;
                }
            }
            results.push(CoreVal::I32(packed));
            Ok(())
        }),

        // (func (param i32) (result i32))
        BuiltinOp::StreamCancelRead { ty, .. } => Box::new(move |_c, args, results| {
            let h = args[0].i32()? as u32;
            let r = shared
                .asyncio
                .lock()
                .unwrap()
                .cancel_copy(ty, h, true, true)?;
            results.push(CoreVal::I32(r));
            Ok(())
        }),
        BuiltinOp::StreamCancelWrite { ty, .. } => Box::new(move |_c, args, results| {
            let h = args[0].i32()? as u32;
            let r = shared
                .asyncio
                .lock()
                .unwrap()
                .cancel_copy(ty, h, false, true)?;
            results.push(CoreVal::I32(r));
            Ok(())
        }),
        BuiltinOp::FutureCancelRead { ty, .. } => Box::new(move |_c, args, results| {
            let h = args[0].i32()? as u32;
            let r = shared
                .asyncio
                .lock()
                .unwrap()
                .cancel_copy(ty, h, true, false)?;
            results.push(CoreVal::I32(r));
            Ok(())
        }),
        BuiltinOp::FutureCancelWrite { ty, .. } => Box::new(move |_c, args, results| {
            let h = args[0].i32()? as u32;
            let r = shared
                .asyncio
                .lock()
                .unwrap()
                .cancel_copy(ty, h, false, false)?;
            results.push(CoreVal::I32(r));
            Ok(())
        }),

        // (func (param i32))
        BuiltinOp::StreamDropReadable(ty) => Box::new(move |_c, args, _r| {
            let h = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().drop_end(ty, h, true, true)
        }),
        BuiltinOp::StreamDropWritable(ty) => Box::new(move |_c, args, _r| {
            let h = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().drop_end(ty, h, false, true)
        }),
        BuiltinOp::FutureDropReadable(ty) => Box::new(move |_c, args, _r| {
            let h = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().drop_end(ty, h, true, false)
        }),
        BuiltinOp::FutureDropWritable(ty) => Box::new(move |_c, args, _r| {
            let h = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().drop_end(ty, h, false, false)
        }),

        // (func (param i32 i32))
        BuiltinOp::StreamForward(ty) => Box::new(move |_c, args, _r| {
            let ri = args[0].i32()? as u32;
            let wi = args[1].i32()? as u32;
            shared.asyncio.lock().unwrap().forward(ty, ri, wi, true)
        }),
        BuiltinOp::FutureForward(ty) => Box::new(move |_c, args, _r| {
            let ri = args[0].i32()? as u32;
            let wi = args[1].i32()? as u32;
            shared.asyncio.lock().unwrap().forward(ty, ri, wi, false)
        }),

        // error contexts -------------------------------------------------
        BuiltinOp::ErrorContextNew(enc) => Box::new(move |caller, args, results| {
            let ptr = args[0].i32()? as u32;
            let units = args[1].i32()? as u32;
            let memory = shared.memory()?;
            let mut mem = CallerMem {
                caller,
                memory: &memory,
                realloc_fn: None,
            };
            let s = match crate::abi::read_string(&mut mem, ptr, units, enc)? {
                Val::String(s) => s,
                _ => return Err(Error::value("error-context.new: string expected")),
            };
            let h = shared.asyncio.lock().unwrap().errctx_new(s);
            results.push(CoreVal::I32(h as i32));
            Ok(())
        }),
        BuiltinOp::ErrorContextDebugMessage { enc, realloc } => {
            Box::new(move |caller, args, _results| {
                let h = args[0].i32()? as u32;
                let ptr = args[1].i32()? as u32;
                let s = shared.asyncio.lock().unwrap().errctx_debug_message(h)?;
                let memory = shared.memory()?;
                let realloc_fn = match &realloc {
                    Some(r) => shared.realloc(r)?,
                    None => None,
                };
                let mut mem = CallerMem {
                    caller,
                    memory: &memory,
                    realloc_fn: realloc_fn.as_ref(),
                };
                let (sptr, units) = crate::abi::write_string(&mut mem, &s, enc)?;
                mem.write(ptr as usize, &sptr.to_le_bytes())?;
                mem.write(ptr as usize + 4, &units.to_le_bytes())?;
                Ok(())
            })
        }
        BuiltinOp::ErrorContextDrop => Box::new(move |_c, args, _r| {
            let h = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().errctx_drop(h)
        }),

        // waitable sets ----------------------------------------------------
        BuiltinOp::WaitableSetNew => Box::new(move |_c, _a, results| {
            let h = shared.asyncio.lock().unwrap().set_new();
            results.push(CoreVal::I32(h as i32));
            Ok(())
        }),
        BuiltinOp::WaitableSetWait => Box::new(move |caller, args, results| {
            let si = args[0].i32()? as u32;
            let ptr = args[1].i32()? as u32;
            let (code, p1, p2) = shared.asyncio.lock().unwrap().set_wait(si)?;
            let memory = shared.memory()?;
            let mut mem = CallerMem {
                caller,
                memory: &memory,
                realloc_fn: None,
            };
            mem.write(ptr as usize, &p1.to_le_bytes())?;
            mem.write(ptr as usize + 4, &p2.to_le_bytes())?;
            results.push(CoreVal::I32(code.raw() as i32));
            Ok(())
        }),
        BuiltinOp::WaitableSetPoll => Box::new(move |caller, args, results| {
            let si = args[0].i32()? as u32;
            let ptr = args[1].i32()? as u32;
            let ev = shared.asyncio.lock().unwrap().set_poll(si)?;
            let (code, p1, p2) = ev.unwrap_or((EventCode::None, 0, 0));
            let memory = shared.memory()?;
            let mut mem = CallerMem {
                caller,
                memory: &memory,
                realloc_fn: None,
            };
            mem.write(ptr as usize, &p1.to_le_bytes())?;
            mem.write(ptr as usize + 4, &p2.to_le_bytes())?;
            results.push(CoreVal::I32(code.raw() as i32));
            Ok(())
        }),
        BuiltinOp::WaitableSetDrop => Box::new(move |_c, args, _r| {
            let si = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().set_drop(si)
        }),
        BuiltinOp::WaitableJoin => Box::new(move |_c, args, _r| {
            let wi = args[0].i32()? as u32;
            let si = args[1].i32()? as u32;
            shared.asyncio.lock().unwrap().join(wi, si)
        }),

        // subtasks ---------------------------------------------------------
        BuiltinOp::SubtaskDrop => Box::new(move |_c, args, _r| {
            let h = args[0].i32()? as u32;
            shared.asyncio.lock().unwrap().subtask_drop(h)
        }),
        BuiltinOp::SubtaskCancel { async_ } => Box::new(move |_c, args, results| {
            let h = args[0].i32()? as u32;
            let r = shared.asyncio.lock().unwrap().subtask_cancel(h, async_)?;
            results.push(CoreVal::I32(r));
            Ok(())
        }),

        // task model ---------------------------------------------------------
        BuiltinOp::TaskReturn { result, enc } => Box::new(move |caller, args, _results| {
            let mut st = shared.asyncio.lock().unwrap();
            match result {
                Some(ty) => {
                    let layout =
                        crate::abi::layout::FuncLayout::new(&[(String::new(), ty)], None, &types);
                    let memory = shared.memory()?;
                    let mut mem = CallerMem {
                        caller,
                        memory: &memory,
                        realloc_fn: None,
                    };
                    let mut cursor = FlatCursor::new(args.to_vec());
                    let v = if layout.params_indirect {
                        let base =
                            cursor.vals.first().and_then(|v| v.i32().ok()).unwrap_or(0) as u64;
                        crate::abi::load(&ty, &types, base, &mut mem, enc)?
                    } else {
                        crate::abi::lift(&ty, &types, &mut cursor, &mut mem, enc)?
                    };
                    st.task_result = Some(vec![v]);
                }
                None => {
                    st.task_result = Some(Vec::new());
                }
            }
            Ok(())
        }),
        BuiltinOp::TaskCancel => Box::new(move |_c, _a, _r| {
            Err(Error::trap(
                "task.cancel: no cancellation was requested of this task",
            ))
        }),

        // backpressure / threads / context -----------------------------------
        BuiltinOp::BackpressureInc => {
            Box::new(move |_c, _a, _r| shared.asyncio.lock().unwrap().backpressure_inc())
        }
        BuiltinOp::BackpressureDec => {
            Box::new(move |_c, _a, _r| shared.asyncio.lock().unwrap().backpressure_dec())
        }
        BuiltinOp::ThreadYield => Box::new(move |_c, _a, results| {
            // a synchronous runtime yields trivially: nothing else can run
            results.push(CoreVal::I32(0));
            Ok(())
        }),
        BuiltinOp::ThreadIndex => Box::new(move |_c, _a, results| {
            results.push(CoreVal::I32(0));
            Ok(())
        }),
        BuiltinOp::ThreadSuspend => Box::new(move |_c, _a, results| {
            results.push(CoreVal::I32(0));
            Ok(())
        }),
        BuiltinOp::ThreadResumeLater => Box::new(move |_c, _a, _r| Ok(())),
        BuiltinOp::ThreadAvailableParallelism => Box::new(move |_c, _a, results| {
            results.push(CoreVal::I32(1));
            Ok(())
        }),
        BuiltinOp::ContextGet { ty, slot } => Box::new(move |_c, _a, results| {
            let st = shared.asyncio.lock().unwrap();
            let v = *st
                .context_slots
                .get(slot as usize)
                .ok_or_else(|| Error::value(format!("invalid context slot {slot}")))?;
            match ty {
                wasmparser::ValType::I64 => results.push(CoreVal::I64(v as i64)),
                _ => results.push(CoreVal::I32(v as i32)),
            }
            Ok(())
        }),
        BuiltinOp::ContextSet { ty, slot } => Box::new(move |_c, args, _r| {
            let mut st = shared.asyncio.lock().unwrap();
            let slot = slot as usize;
            if slot >= st.context_slots.len() {
                return Err(Error::value(format!("invalid context slot {slot}")));
            }
            st.context_slots[slot] = match ty {
                wasmparser::ValType::I64 => args[0].i64()? as u64,
                _ => args[0].i32()? as u64,
            };
            Ok(())
        }),

        // stack-switching builtins: clear diagnostic in a synchronous runtime
        BuiltinOp::Unsupported(name) => {
            let msg = format!(
                "canon builtin `{name}` requires stack switching (threads) and \
                 is not supported by this synchronous runtime"
            );
            Box::new(move |_c, _a, _r| Err(Error::trap(msg.clone())))
        }
    })
}

fn core_ty_of(t: &crate::abi::layout::CoreTy) -> crate::engine::ValType {
    match t {
        crate::abi::layout::CoreTy::I32 => crate::engine::ValType::I32,
        crate::abi::layout::CoreTy::I64 => crate::engine::ValType::I64,
        crate::abi::layout::CoreTy::F32 => crate::engine::ValType::F32,
        crate::abi::layout::CoreTy::F64 => crate::engine::ValType::F64,
    }
}

fn core_func_refs(plan: &crate::component::ComponentPlan) -> Vec<CoreFuncRef> {
    use crate::component::decode::BuiltinOp;
    let mut out = Vec::new();
    for l in &plan.lifts {
        out.push(l.core_func.clone());
        if let Some(r) = &l.post_return {
            out.push(r.clone());
        }
        if let Some(r) = &l.realloc {
            out.push(r.clone());
        }
        if let Some(r) = &l.callback {
            out.push(r.clone());
        }
    }
    for l in &plan.lowers {
        if let Some(r) = &l.realloc {
            out.push(r.clone());
        }
    }
    for b in &plan.builtins {
        if let BuiltinOp::ErrorContextDebugMessage {
            realloc: Some(r), ..
        } = b
        {
            out.push(r.clone());
        }
    }
    out
}

fn resolve_core_func(
    shared: &Arc<InstanceShared>,
    instance: &Instance,
    store: &mut crate::Store,
    r: &CoreFuncRef,
) -> Result<CoreFunc> {
    match r {
        CoreFuncRef::Export { instance: _, name } => {
            // after instantiation all named funcs live under the single core
            // instance; the shared map is authoritative
            if let Some(f) = shared.funcs.lock().unwrap().get(&(0, name.clone())) {
                return Ok(f.clone());
            }
            match store.get_export(instance, name) {
                Some(Extern::Func(f)) => Ok(f),
                _ => Err(Error::MalformedBinary(format!(
                    "core function `{name}` not found in component instance"
                ))),
            }
        }
        CoreFuncRef::Lower(_)
        | CoreFuncRef::Builtin(_)
        | CoreFuncRef::ChildLower(_)
        | CoreFuncRef::ChildExport { .. } => Err(Error::MalformedBinary(
            "cannot resolve a lowered import, canon builtin or nested-component reference as a core export; it is wired by the linker".into(),
        )),
    }
}

/// Build one bound component function from `plan.lifts[i]` (shared by the
/// export-binding loop and the start function).
fn build_exported_func(
    plan: &crate::component::ComponentPlan,
    i: usize,
    shared: &Arc<InstanceShared>,
    instance: &Instance,
    store: &mut crate::Store,
    types: &Arc<Vec<TypeDef>>,
) -> Result<ExportedFunc> {
    let lift = &plan.lifts[i];
    let layout = Arc::new(plan_lift_layout(plan, i));
    let core_func = resolve_core_func(shared, instance, store, &lift.core_func)?;
    let post_return = match &lift.post_return {
        Some(r) => Some(resolve_core_func(shared, instance, store, r)?),
        None => None,
    };
    let realloc = match &lift.realloc {
        Some(r) => resolve_core_func(shared, instance, store, r).ok(),
        None => None,
    };
    let callback = match &lift.callback {
        Some(r) => resolve_core_func(shared, instance, store, r).ok(),
        None => None,
    };
    let memory = shared.memory()?;
    Ok(ExportedFunc {
        core_func,
        post_return,
        realloc,
        memory,
        layout,
        sig: lift.sig.clone(),
        string_enc: lift.string_enc,
        types: types.clone(),
        async_lift: lift.async_lift,
        callback,
        asyncio: shared.asyncio.clone(),
    })
}

/// Layout of lift `i` (the component precomputes them parallel to lifts).
fn plan_lift_layout(
    plan: &crate::component::ComponentPlan,
    i: usize,
) -> crate::abi::layout::FuncLayout {
    let lift = &plan.lifts[i];
    FuncLayout::new(&lift.sig.params, lift.sig.result, &plan.types)
}

// ---------------------------------------------------------------------------
// Component instance
// ---------------------------------------------------------------------------

/// An instantiated component: typed/dynamic exports plus resource tables.
pub struct ComponentInstance {
    exports: HashMap<String, ExportedFunc>,
    instance: Instance,
    tables: SharedTables,
    /// Preview-3 async state (streams/futures/waitable sets/tasks).
    asyncio: crate::component::asyncio::SharedAsync,
    core_only: bool,
    /// Result value of the component start function (if it returned one).
    start_result: Option<Val>,
    /// Resource declarations (for name → id lookup by the host).
    resources: Vec<crate::component::ResourceInfo>,
}

impl ComponentInstance {
    /// Look up an exported function by name.
    ///
    /// Accepts `"func"` for root exports and `"iface|func"` for interface
    /// exports (also `"iface::func"`).
    pub fn find_func(&self, name: &str) -> Option<&ExportedFunc> {
        if let Some(f) = self.exports.get(name) {
            return Some(f);
        }
        if let Some((_, func)) = name.split_once('|') {
            for (k, f) in &self.exports {
                if k.ends_with(&format!("|{func}")) {
                    return Some(f);
                }
            }
            return None;
        }
        if let Some((_, func)) = name.split_once("::") {
            for (k, f) in &self.exports {
                if k.ends_with(&format!("|{func}")) {
                    return Some(f);
                }
            }
        }
        None
    }

    /// Get a typed handle to an export.
    pub fn typed_func<P, R>(&self, name: &str) -> Result<crate::func::TypedFunc<P, R>>
    where
        P: crate::func::ComponentParams,
        R: crate::func::ComponentResults,
    {
        let inner = self
            .find_func(name)
            .cloned()
            .ok_or_else(|| Error::MissingImport {
                module: "export".into(),
                name: name.to_string(),
            })?;
        Ok(crate::func::TypedFunc {
            inner,
            _marker: std::marker::PhantomData,
        })
    }

    /// Call an export with dynamic values.
    pub fn call_func(
        &self,
        store: &mut crate::Store,
        name: &str,
        params: &[Val],
    ) -> Result<Option<Val>> {
        let f = self.find_func(name).ok_or_else(|| Error::MissingImport {
            module: "export".into(),
            name: name.to_string(),
        })?;
        f.call_dynamic(store, params)
    }

    /// All export keys (for introspection).
    pub fn export_names(&self) -> Vec<&str> {
        self.exports.keys().map(|s| s.as_str()).collect()
    }

    /// Per-instance resource tables (for host resource implementations).
    pub fn tables(&self) -> &SharedTables {
        &self.tables
    }

    /// Result value of the component start function, when the component has
    /// a start section and the function returned a value.
    pub fn start_result(&self) -> Option<&Val> {
        self.start_result.as_ref()
    }

    /// Resolve a guest-declared resource type by (interface, name) to its
    /// dense resource id (used with [`Self::resource_new`] and friends).
    ///
    /// `interface` is the WIT interface name that exports the resource, or
    /// `None` for root-level resources.
    pub fn resource_id(&self, interface: Option<&str>, name: &str) -> Result<u32> {
        find_resource_id(&self.resources, interface.unwrap_or(""), name)
    }

    /// Create a handle for a guest-declared resource type (host side of
    /// `canon resource.new`). Returns the table handle index.
    pub fn resource_new(&self, resource: u32, rep: u64) -> u32 {
        self.tables.lock().unwrap().resource_new(resource, rep)
    }

    /// Read the representation of a resource handle (`canon resource.rep`).
    pub fn resource_rep(&self, handle: &crate::component::types::Handle) -> Result<u64> {
        self.tables
            .lock()
            .unwrap()
            .resource_rep(handle.resource(), handle.index())
    }

    /// Drop a resource handle (`canon resource.drop`), returning its rep.
    pub fn resource_drop(&self, handle: crate::component::types::Handle) -> Result<u64> {
        self.tables
            .lock()
            .unwrap()
            .resource_drop(handle.resource(), handle.index())
    }

    /// Number of live handles of one resource type (diagnostics/tests).
    pub fn live_resources(&self, resource: u32) -> usize {
        self.tables.lock().unwrap().live(resource)
    }

    /// Per-instance async state (preview 3): streams, futures, waitable
    /// sets, error contexts and the task model.
    pub fn async_state(&self) -> &crate::component::asyncio::SharedAsync {
        &self.asyncio
    }

    /// Create a host-owned stream pair (readable, writable handles).
    ///
    /// Pass either handle to a guest export as a [`Val::Stream`] parameter,
    /// or feed/drain it from the host with [`ComponentInstance::stream_feed`]
    /// / [`ComponentInstance::stream_drain`].
    pub fn stream_new(&self, stream_type: u32) -> Result<(u32, u32)> {
        Ok(self.asyncio.lock().unwrap().stream_new(stream_type))
    }

    /// Create a host-owned future pair (readable, writable handles).
    pub fn future_new(&self, future_type: u32) -> Result<(u32, u32)> {
        Ok(self.asyncio.lock().unwrap().future_new(future_type))
    }

    /// Host feed: push values into the writable end of a stream.
    ///
    /// When the guest has a pending async read, the values are written
    /// straight into the guest's buffer and its waitable-set event fires.
    pub fn stream_feed(
        &self,
        store: &mut crate::Store,
        stream_type: u32,
        writable: u32,
        vals: Vec<Val>,
    ) -> Result<usize> {
        let write = self
            .asyncio
            .lock()
            .unwrap()
            .host_write(stream_type, writable, vals, true)?;
        match write {
            crate::component::asyncio::HostWrite::Buffered => Ok(0),
            crate::component::asyncio::HostWrite::Deliver { pending, vals, end } => {
                let n = vals.len() as u32;
                self.deliver_pending(store, &pending, &vals, true)?;
                self.asyncio.lock().unwrap().confirm_read(end, n, true);
                Ok(n as usize)
            }
        }
    }

    /// Host feed for futures (single value).
    pub fn future_set(
        &self,
        store: &mut crate::Store,
        future_type: u32,
        writable: u32,
        val: Val,
    ) -> Result<()> {
        let write =
            self.asyncio
                .lock()
                .unwrap()
                .host_write(future_type, writable, vec![val], false)?;
        match write {
            crate::component::asyncio::HostWrite::Buffered => Ok(()),
            crate::component::asyncio::HostWrite::Deliver { pending, vals, end } => {
                self.deliver_pending(store, &pending, &vals, false)?;
                self.asyncio.lock().unwrap().confirm_read(end, 1, false);
                Ok(())
            }
        }
    }

    /// Host drain: pull up to `max` buffered values from a readable end.
    /// Returns the values plus `true` at end-of-stream (writer gone and
    /// nothing buffered).
    pub fn stream_drain(
        &self,
        stream_type: u32,
        readable: u32,
        max: u32,
    ) -> Result<(Vec<Val>, bool)> {
        self.asyncio
            .lock()
            .unwrap()
            .host_read(stream_type, readable, max, true)
    }

    /// Host take for futures.
    pub fn future_take(&self, future_type: u32, readable: u32) -> Result<Option<Val>> {
        let (mut vals, _) =
            self.asyncio
                .lock()
                .unwrap()
                .host_read(future_type, readable, 1, false)?;
        Ok(vals.pop())
    }

    /// Close the writable end of a host-owned stream (signals EOF).
    pub fn stream_close_writable(&self, stream_type: u32, writable: u32) -> Result<()> {
        self.asyncio
            .lock()
            .unwrap()
            .drop_end(stream_type, writable, false, true)
    }

    /// Deliver pending-read values into guest memory (shared by the host
    /// feed wrappers).
    fn deliver_pending(
        &self,
        store: &mut crate::Store,
        pending: &crate::component::asyncio::PendingRead,
        vals: &[Val],
        _is_stream: bool,
    ) -> Result<()> {
        let memory = match store.get_export(&self.instance, "memory") {
            Some(Extern::Memory(m)) => m,
            _ => return Err(Error::unsupported("component has no exported memory")),
        };
        if let Some(et) = pending.elem {
            let mut mem = crate::abi::StoreMem {
                store,
                memory: &memory,
                realloc_fn: None,
            };
            let esz = crate::abi::layout::size(&et, &pending.types);
            for (i, v) in vals.iter().enumerate() {
                crate::abi::store(
                    &et,
                    &pending.types,
                    v,
                    pending.ptr as u64 + i as u64 * esz,
                    &mut mem,
                    pending.enc,
                )?;
            }
        }
        Ok(())
    }

    /// The underlying core instance (raw modules and advanced use).
    pub fn core_instance(&self) -> &Instance {
        &self.instance
    }

    /// Whether this instance came from a plain core module.
    pub fn is_core_only(&self) -> bool {
        self.core_only
    }

    /// Resolve a raw core export (functions/memories of core-only modules).
    pub fn get_core_export(&self, store: &mut crate::Store, name: &str) -> Option<Extern> {
        store.get_export(&self.instance, name)
    }

    /// Call a raw core export function directly.
    pub fn call_core(
        &self,
        store: &mut crate::Store,
        name: &str,
        args: &[CoreVal],
        results: &mut [CoreVal],
    ) -> Result<()> {
        match store.get_export(&self.instance, name) {
            Some(Extern::Func(f)) => store.call(&f, args, results),
            _ => Err(Error::MissingImport {
                module: "export".into(),
                name: name.to_string(),
            }),
        }
    }
}
