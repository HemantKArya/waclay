//! The component facade: decoding and exported-call machinery.
//!
//! `Component::from_binary` performs the full decode and
//! precomputes flat layouts for every lifted export, so per-call overhead is
//! a couple of `Vec` pushes.

pub mod asyncio;
pub mod decode;
pub mod types;

pub use decode::{ComponentPlan, CoreFuncRef, CoreMemRef, LiftedExport, LoweredImport};
pub use types::{
    FuncSig, Handle, Primitive, ResourceInfo, TypeDef, TypeRef, Val, WorldExport, WorldImport,
};

use crate::abi::layout::{alignment, size, FuncLayout};
use crate::abi::{
    lift, load, lower, store as store_val, FlatCursor, FlatSink, MemAccess, StoreMem,
    StringEncoding,
};
use crate::engine::{CompiledModule, Engine, Func, Memory, Val as CoreVal};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// A decoded component (or plain core module) bound to an engine.
pub struct Component {
    engine: Engine,
    core: Arc<dyn CompiledModule>,
    plan: ComponentPlan,
}

impl Component {
    /// Decode a component binary (or core module) and precompute layouts.
    pub fn from_binary(engine: &Engine, binary: &[u8]) -> Result<Component> {
        let plan = decode::decode(binary)?;
        Self::from_plan(engine, plan)
    }

    /// Read a file and decode it.
    pub fn from_file(engine: &Engine, path: impl AsRef<std::path::Path>) -> Result<Component> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::MalformedBinary(format!("cannot read component file: {e}")))?;
        Self::from_binary(engine, &bytes)
    }

    /// Wrap a raw core module as a core-only component.
    pub fn from_core_module(engine: &Engine, wasm: &[u8]) -> Result<Component> {
        Self::from_plan(
            engine,
            ComponentPlan {
                core_module: wasm.to_vec(),
                ..Default::default()
            },
        )
    }

    /// Build from an explicit plan (used by tests and advanced embedders).
    pub fn from_plan(engine: &Engine, plan: ComponentPlan) -> Result<Component> {
        let core: Arc<dyn CompiledModule> = Arc::from(engine.compile(&plan.core_module)?);
        Ok(Component {
            engine: engine.clone(),
            core,
            plan,
        })
    }

    /// The engine this component is compiled for.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Component imports (interfaces the host must provide).
    pub fn imports(&self) -> &[WorldImport] {
        &self.plan.imports
    }

    /// Component exports (functions the host can call).
    pub fn exports(&self) -> &[WorldExport] {
        &self.plan.exports
    }

    /// Declared resources with their owning interfaces.
    pub fn resources(&self) -> &[ResourceInfo] {
        &self.plan.resources
    }

    /// True when this is a plain core module (no component sections).
    pub fn is_core_only(&self) -> bool {
        self.plan.lifts.is_empty() && self.plan.lowers.is_empty() && self.plan.imports.is_empty()
    }

    pub(crate) fn plan(&self) -> &ComponentPlan {
        &self.plan
    }

    pub(crate) fn core_module(&self) -> &Arc<dyn CompiledModule> {
        &self.core
    }

    #[allow(dead_code)] // reserved for multi-instance composition
    pub(crate) fn engine_handle(&self) -> Engine {
        self.engine.clone()
    }
}

// ---------------------------------------------------------------------------
// Exported functions
// ---------------------------------------------------------------------------

/// One callable component export (a canon-lifted function).
#[derive(Clone)]
pub struct ExportedFunc {
    pub(crate) core_func: Func,
    pub(crate) post_return: Option<Func>,
    pub(crate) realloc: Option<Func>,
    pub(crate) memory: Memory,
    pub(crate) layout: Arc<FuncLayout>,
    pub(crate) sig: FuncSig,
    pub(crate) string_enc: StringEncoding,
    pub(crate) types: Arc<Vec<TypeDef>>,
    /// Async-lifted export: results arrive via `task.return` (and the
    /// callback event loop when a callback is present).
    pub(crate) async_lift: bool,
    pub(crate) callback: Option<Func>,
    pub(crate) asyncio: crate::component::asyncio::SharedAsync,
}

impl ExportedFunc {
    /// Human-readable signature of this export.
    pub fn signature(&self) -> String {
        self.sig.describe(&self.types)
    }

    /// Call with dynamic component values; returns the result value (if any).
    pub fn call_dynamic(&self, store: &mut crate::Store, params: &[Val]) -> Result<Option<Val>> {
        if params.len() != self.layout.params.len() {
            return Err(Error::value(format!(
                "expected {} params, got {}",
                self.layout.params.len(),
                params.len()
            )));
        }
        let types = self.types.as_slice();
        let enc = self.string_enc;

        // --- lower parameters (scoped: mem borrows store) ---------------
        let (core_args, param_ptr) = {
            let mut mem = StoreMem {
                store,
                memory: &self.memory,
                realloc_fn: self.realloc.as_ref(),
            };
            if self.layout.params_indirect {
                let area = self.layout.param_area_size;
                let align = self
                    .layout
                    .params
                    .iter()
                    .map(|s| alignment(&s.ty, types))
                    .max()
                    .unwrap_or(1);
                let ptr = mem.realloc(0, 0, align, area)?;
                for (i, (slot, val)) in self.layout.params.iter().zip(params).enumerate() {
                    store_val(
                        &slot.ty,
                        types,
                        val,
                        ptr as u64 + slot.offset,
                        &mut mem,
                        enc,
                    )
                    .map_err(|e| Error::value(format!("param #{i}: {e}")))?;
                }
                (vec![CoreVal::I32(ptr as i32)], ptr)
            } else {
                let mut sink = FlatSink::default();
                for (i, (slot, val)) in self.layout.params.iter().zip(params).enumerate() {
                    lower(&slot.ty, types, val, &mut sink, &mut mem, enc)
                        .map_err(|e| Error::value(format!("param #{i}: {e}")))?;
                }
                (sink.vals, 0u32)
            }
        };

        // --- core result buffer -----------------------------------------
        let lift_ctx = crate::abi::layout::Context::Lift;
        let result_types = if self.async_lift {
            // async-lifted entries return the callback packing word (or
            // nothing when no callback is present)
            if self.callback.is_some() {
                vec![crate::abi::layout::CoreTy::I32]
            } else {
                vec![]
            }
        } else {
            self.layout.core_results(lift_ctx)
        };
        let mut core_results: Vec<CoreVal> = result_types
            .iter()
            .map(|t| match t {
                crate::abi::layout::CoreTy::I32 => CoreVal::I32(0),
                crate::abi::layout::CoreTy::I64 => CoreVal::I64(0),
                crate::abi::layout::CoreTy::F32 => CoreVal::F32(0.0),
                crate::abi::layout::CoreTy::F64 => CoreVal::F64(0.0),
            })
            .collect();

        // --- call ---------------------------------------------------------
        store.call(&self.core_func, &core_args, &mut core_results)?;

        // --- async lift: callback loop + `task.return` result -------------
        if self.async_lift {
            if let Some(cb) = &self.callback {
                let mut packed = core_results.first().and_then(|v| v.i32().ok()).unwrap_or(0);
                let mut rounds: u64 = 0;
                loop {
                    let (code, si) = asyncio::unpack_callback_result(packed)?;
                    if code == asyncio::CallbackCode::Exit {
                        break;
                    }
                    rounds += 1;
                    if rounds > 1_000_000 {
                        return Err(Error::trap(
                            "async callback event loop did not converge (1M rounds)",
                        ));
                    }
                    let (ec, p1, p2) = match code {
                        asyncio::CallbackCode::Yield => (0u32, 0u32, 0u32),
                        asyncio::CallbackCode::Wait => {
                            let ev = self.asyncio.lock().unwrap().set_poll(si)?;
                            match ev {
                                Some((c, p1, p2)) => (c.raw(), p1, p2),
                                None => (0, 0, 0),
                            }
                        }
                        asyncio::CallbackCode::Exit => unreachable!(),
                    };
                    let mut out = [CoreVal::I32(0)];
                    store.call(
                        cb,
                        &[
                            CoreVal::I32(ec as i32),
                            CoreVal::I32(p1 as i32),
                            CoreVal::I32(p2 as i32),
                        ],
                        &mut out,
                    )?;
                    packed = out[0].i32()?;
                }
            }
            let res = self
                .asyncio
                .lock()
                .unwrap()
                .task_result
                .take()
                .ok_or_else(|| {
                    Error::trap("async export finished without calling `task.return`")
                })?;
            return Ok(match (&self.layout.result, res.into_iter().next()) {
                (Some(_), v) => v,
                (None, _) => None,
            });
        }

        // --- lift result (fresh mem borrow) -------------------------------
        let out = match (&self.layout.result, self.layout.results_indirect) {
            (Some(rt), true) => {
                let retptr = core_results
                    .first()
                    .map(|v| v.i32().unwrap_or(0) as u64)
                    .unwrap_or(0);
                let v = {
                    let mut mem = StoreMem {
                        store,
                        memory: &self.memory,
                        realloc_fn: self.realloc.as_ref(),
                    };
                    load(&rt.ty, types, retptr, &mut mem, enc)?
                };
                if let Some(pr) = &self.post_return {
                    store.call(pr, &[CoreVal::I32(retptr as i32)], &mut [])?;
                }
                Some(v)
            }
            (Some(rt), false) => {
                let v = {
                    let mut mem = StoreMem {
                        store,
                        memory: &self.memory,
                        realloc_fn: self.realloc.as_ref(),
                    };
                    let mut cur = FlatCursor::new(core_results.clone());
                    lift(&rt.ty, types, &mut cur, &mut mem, enc)?
                };
                if let Some(pr) = &self.post_return {
                    store.call(pr, &[CoreVal::I32(param_ptr as i32)], &mut [])?;
                }
                Some(v)
            }
            (None, _) => {
                if let Some(pr) = &self.post_return {
                    store.call(pr, &[CoreVal::I32(param_ptr as i32)], &mut [])?;
                }
                None
            }
        };
        Ok(out)
    }

    /// Call with dynamic component values from *inside* a host callback.
    ///
    /// Identical to [`ExportedFunc::call_dynamic`], but driven by a
    /// [`CallerView`] instead of a `&mut Store` — this is what lets a
    /// parent component's lowered import call into a nested component
    /// (cross-instance composition) while the parent's guest is running.
    pub(crate) fn call_dynamic_via(
        &self,
        caller: &mut dyn crate::engine::CallerView,
        params: &[Val],
    ) -> Result<Option<Val>> {
        if params.len() != self.layout.params.len() {
            return Err(Error::value(format!(
                "expected {} params, got {}",
                self.layout.params.len(),
                params.len()
            )));
        }
        let types = self.types.as_slice();
        let enc = self.string_enc;

        // --- lower parameters (into this function's own memory) ----------
        let (core_args, param_ptr) = {
            let mut mem = CallerHandleMem {
                caller,
                memory: &self.memory,
                realloc_fn: self.realloc.as_ref(),
            };
            if self.layout.params_indirect {
                let area = self.layout.param_area_size;
                let align = self
                    .layout
                    .params
                    .iter()
                    .map(|s| alignment(&s.ty, types))
                    .max()
                    .unwrap_or(1);
                let ptr = mem.realloc(0, 0, align, area)?;
                for (i, (slot, val)) in self.layout.params.iter().zip(params).enumerate() {
                    store_val(
                        &slot.ty,
                        types,
                        val,
                        ptr as u64 + slot.offset,
                        &mut mem,
                        enc,
                    )
                    .map_err(|e| Error::value(format!("param #{i}: {e}")))?;
                }
                (vec![CoreVal::I32(ptr as i32)], ptr)
            } else {
                let mut sink = FlatSink::default();
                for (i, (slot, val)) in self.layout.params.iter().zip(params).enumerate() {
                    lower(&slot.ty, types, val, &mut sink, &mut mem, enc)
                        .map_err(|e| Error::value(format!("param #{i}: {e}")))?;
                }
                (sink.vals, 0u32)
            }
        };

        // --- core result buffer -----------------------------------------
        let lift_ctx = crate::abi::layout::Context::Lift;
        let result_types = if self.async_lift {
            if self.callback.is_some() {
                vec![crate::abi::layout::CoreTy::I32]
            } else {
                vec![]
            }
        } else {
            self.layout.core_results(lift_ctx)
        };
        let mut core_results: Vec<CoreVal> = result_types
            .iter()
            .map(|t| match t {
                crate::abi::layout::CoreTy::I32 => CoreVal::I32(0),
                crate::abi::layout::CoreTy::I64 => CoreVal::I64(0),
                crate::abi::layout::CoreTy::F32 => CoreVal::F32(0.0),
                crate::abi::layout::CoreTy::F64 => CoreVal::F64(0.0),
            })
            .collect();

        // --- call ---------------------------------------------------------
        caller.call(&self.core_func, &core_args, &mut core_results)?;

        // --- async lift: callback loop + `task.return` result -------------
        if self.async_lift {
            if let Some(cb) = &self.callback {
                let mut packed = core_results.first().and_then(|v| v.i32().ok()).unwrap_or(0);
                let mut rounds: u64 = 0;
                loop {
                    let (code, si) = asyncio::unpack_callback_result(packed)?;
                    if code == asyncio::CallbackCode::Exit {
                        break;
                    }
                    rounds += 1;
                    if rounds > 1_000_000 {
                        return Err(Error::trap(
                            "async callback event loop did not converge (1M rounds)",
                        ));
                    }
                    let (ec, p1, p2) = match code {
                        asyncio::CallbackCode::Yield => (0u32, 0u32, 0u32),
                        asyncio::CallbackCode::Wait => {
                            let ev = self.asyncio.lock().unwrap().set_poll(si)?;
                            match ev {
                                Some((c, p1, p2)) => (c.raw(), p1, p2),
                                None => (0, 0, 0),
                            }
                        }
                        asyncio::CallbackCode::Exit => unreachable!(),
                    };
                    let mut out = [CoreVal::I32(0)];
                    caller.call(
                        cb,
                        &[
                            CoreVal::I32(ec as i32),
                            CoreVal::I32(p1 as i32),
                            CoreVal::I32(p2 as i32),
                        ],
                        &mut out,
                    )?;
                    packed = out[0].i32()?;
                }
            }
            let res = self
                .asyncio
                .lock()
                .unwrap()
                .task_result
                .take()
                .ok_or_else(|| {
                    Error::trap("async export finished without calling `task.return`")
                })?;
            return Ok(match (&self.layout.result, res.into_iter().next()) {
                (Some(_), v) => v,
                (None, _) => None,
            });
        }

        // --- lift result (fresh mem borrow) -------------------------------
        let out = match (&self.layout.result, self.layout.results_indirect) {
            (Some(rt), true) => {
                let retptr = core_results
                    .first()
                    .map(|v| v.i32().unwrap_or(0) as u64)
                    .unwrap_or(0);
                let v = {
                    let mut mem = CallerHandleMem {
                        caller,
                        memory: &self.memory,
                        realloc_fn: self.realloc.as_ref(),
                    };
                    load(&rt.ty, types, retptr, &mut mem, enc)?
                };
                if let Some(pr) = &self.post_return {
                    caller.call(pr, &[CoreVal::I32(retptr as i32)], &mut [])?;
                }
                Some(v)
            }
            (Some(rt), false) => {
                let v = {
                    let mut mem = CallerHandleMem {
                        caller,
                        memory: &self.memory,
                        realloc_fn: self.realloc.as_ref(),
                    };
                    let mut cur = FlatCursor::new(core_results.clone());
                    lift(&rt.ty, types, &mut cur, &mut mem, enc)?
                };
                if let Some(pr) = &self.post_return {
                    caller.call(pr, &[CoreVal::I32(param_ptr as i32)], &mut [])?;
                }
                Some(v)
            }
            (None, _) => {
                if let Some(pr) = &self.post_return {
                    caller.call(pr, &[CoreVal::I32(param_ptr as i32)], &mut [])?;
                }
                None
            }
        };
        Ok(out)
    }
}

/// [`MemAccess`] over an arbitrary memory handle, driven by a
/// [`CallerView`] — used to run another component's Canonical ABI while a
/// (parent) guest is executing.
struct CallerHandleMem<'a> {
    caller: &'a mut dyn crate::engine::CallerView,
    memory: &'a Memory,
    realloc_fn: Option<&'a Func>,
}

impl MemAccess for CallerHandleMem<'_> {
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.caller.memory_read(self.memory, offset, buf)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<()> {
        self.caller.memory_write(self.memory, offset, buf)
    }

    fn len(&self) -> usize {
        self.caller.memory_size(self.memory)
    }

    fn realloc(&mut self, old_ptr: u32, old_size: u64, align: u64, new_size: u64) -> Result<u32> {
        let f = self.realloc_fn.ok_or_else(|| {
            Error::unsupported(
                "component does not export `cabi_realloc`; cannot pass heap parameters",
            )
        })?;
        let mut out = [CoreVal::I32(0)];
        self.caller.call(
            f,
            &[
                CoreVal::I32(old_ptr as i32),
                CoreVal::I32(old_size as i32),
                CoreVal::I32(align as i32),
                CoreVal::I32(new_size as i32),
            ],
            &mut out,
        )?;
        Ok(out[0].i32()? as u32)
    }
}

// ---------------------------------------------------------------------------
// Shared instantiation state (used by the linker)
// ---------------------------------------------------------------------------

/// Shared per-instantiation state captured by host closures.
pub(crate) struct InstanceShared {
    /// The component's linear memory (filled right after instantiation).
    pub memory: std::sync::Mutex<Option<Memory>>,
    /// Realloc / post-return core functions resolved from instance exports.
    pub funcs: std::sync::Mutex<HashMap<(u32, String), Func>>,
    /// Resource tables.
    pub tables: crate::resource::SharedTables,
    /// Preview-3 async state (streams/futures/waitable-sets/tasks).
    pub asyncio: crate::component::asyncio::SharedAsync,
}

impl InstanceShared {
    pub(crate) fn new() -> Arc<InstanceShared> {
        Arc::new(InstanceShared {
            memory: std::sync::Mutex::new(None),
            funcs: std::sync::Mutex::new(HashMap::new()),
            tables: crate::resource::shared(),
            asyncio: crate::component::asyncio::shared(),
        })
    }

    pub(crate) fn memory(&self) -> Result<Memory> {
        self.memory
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Error::Unsupported("component has no exported memory".into()))
    }

    pub(crate) fn realloc(&self, r: &CoreFuncRef) -> Result<Option<Func>> {
        match r {
            CoreFuncRef::Export { instance, name } => Ok(self
                .funcs
                .lock()
                .unwrap()
                .get(&(*instance, name.clone()))
                .cloned()),
            CoreFuncRef::Lower(_)
            | CoreFuncRef::Builtin(_)
            | CoreFuncRef::ChildLower(_)
            | CoreFuncRef::ChildExport { .. } => Ok(None),
        }
    }
}

/// Size of the result area of a layout (used by lowered host functions).
#[allow(dead_code)] // used by lowered host funcs in later phases
pub(crate) fn result_area_size(layout: &FuncLayout, types: &[TypeDef]) -> u64 {
    match &layout.result {
        Some(slot) => align_up(size(&slot.ty, types), alignment(&slot.ty, types)),
        None => 0,
    }
}

fn align_up(v: u64, a: u64) -> u64 {
    if a == 0 {
        v
    } else {
        v.div_ceil(a) * a
    }
}
