//! # waclay-wasmtime
//!
//! [wasmtime](https://wasmtime.dev) backend — the Cranelift JIT engine for
//! near-native execution. Implements waclay's engine traits on top of
//! wasmtime's `Engine`/`Module`/`Store`/`Caller` API.

#![forbid(unsafe_code)]

use std::any::Any;
use std::sync::{Arc, Mutex, PoisonError};

use wasmtime::{Engine as WtEngine, Module as WtModule, Store as WtStore};

use waclay::engine::{
    CallerView, CompiledModule, ExportType, Extern, ExternType, Func, FuncHandle, FuncType, HostFn,
    ImportType, Imports, Instance, InstanceHandle, Memory, MemoryHandle, MemoryType, Store,
    StoreHandle, Val, ValType, WasmEngine,
};
use waclay::{Error, Result};

/// Engine name reported by this backend.
pub const ENGINE_NAME: &str = "wasmtime";

/// Version of the wasmtime crate this backend is built against.
pub const WASMTIME_VERSION: &str = "50.0.0";

/// Map a wasmtime error into a waclay error, inlining the *whole* anyhow
/// chain so host trap messages survive `Display` (wasmtime otherwise only
/// shows "error while executing at wasm backtrace" and buries the cause).
fn wt_err(e: wasmtime::Error) -> Error {
    // A host callback's own `waclay::Error` survives the trip through the
    // engine unchanged, so callers can still match on its variant.
    if let Some(inner) = e.downcast_ref::<Error>() {
        return inner.clone();
    }
    Error::engine(ENGINE_NAME, format!("{e:#}"))
}

/// Host data inside the wasmtime store.
struct StoreData {
    user: Box<dyn Any + Send>,
}

impl Default for StoreData {
    fn default() -> Self {
        StoreData { user: Box::new(()) }
    }
}

/// The wasmtime engine.
#[derive(Clone)]
pub struct WasmtimeEngine {
    engine: WtEngine,
}

impl Default for WasmtimeEngine {
    fn default() -> Self {
        Self::new(wasmtime::Config::new())
    }
}

impl WasmtimeEngine {
    /// Create from an explicit wasmtime configuration.
    pub fn new(mut config: wasmtime::Config) -> Self {
        config.wasm_component_model(true);
        Self {
            engine: WtEngine::new(&config).expect("wasmtime engine init"),
        }
    }
}

impl WasmEngine for WasmtimeEngine {
    fn name(&self) -> &'static str {
        ENGINE_NAME
    }

    fn version(&self) -> &'static str {
        WASMTIME_VERSION
    }

    fn compile(&self, wasm: &[u8]) -> Result<Box<dyn CompiledModule>> {
        let module = WtModule::new(&self.engine, wasm).map_err(wt_err)?;

        let mut imports = Vec::new();
        for imp in module.imports() {
            imports.push(ImportType {
                module: imp.module().to_string(),
                name: imp.name().to_string(),
                ty: map_extern_type(imp.ty())?,
            });
        }
        let mut exports = Vec::new();
        for exp in module.exports() {
            exports.push(ExportType {
                name: exp.name().to_string(),
                ty: map_extern_type(exp.ty())?,
            });
        }

        Ok(Box::new(WasmtimeModule {
            engine: self.engine.clone(),
            module,
            imports,
            exports,
        }))
    }

    fn new_store(&self, engine: Arc<dyn WasmEngine>) -> Result<Store> {
        let store = WtStore::new(&self.engine, StoreData::default());
        Ok(Store::from_handle(
            engine,
            Box::new(WasmtimeStore { store }),
        ))
    }
}

fn map_extern_type(ty: wasmtime::ExternType) -> Result<ExternType> {
    Ok(match ty {
        wasmtime::ExternType::Func(ft) => ExternType::Func(FuncType {
            params: ft.params().map(map_val_type).collect::<Result<_>>()?,
            results: ft.results().map(map_val_type).collect::<Result<_>>()?,
        }),
        wasmtime::ExternType::Memory(mt) => ExternType::Memory(MemoryType {
            min: mt.minimum().try_into().unwrap_or(0),
            max: mt.maximum().map(|m| m.try_into().unwrap_or(u32::MAX)),
        }),
        other => {
            return Err(Error::engine(
                ENGINE_NAME,
                format!("unsupported extern type {other:?}"),
            ))
        }
    })
}

fn map_val_type(ty: wasmtime::ValType) -> Result<ValType> {
    match ty {
        wasmtime::ValType::I32 => Ok(ValType::I32),
        wasmtime::ValType::I64 => Ok(ValType::I64),
        wasmtime::ValType::F32 => Ok(ValType::F32),
        wasmtime::ValType::F64 => Ok(ValType::F64),
        other => Err(Error::unsupported(format!(
            "core value type {other:?} (waclay supports i32/i64/f32/f64)"
        ))),
    }
}

fn to_wt_val_type(ty: ValType) -> wasmtime::ValType {
    match ty {
        ValType::I32 => wasmtime::ValType::I32,
        ValType::I64 => wasmtime::ValType::I64,
        ValType::F32 => wasmtime::ValType::F32,
        ValType::F64 => wasmtime::ValType::F64,
    }
}

fn map_val(v: &wasmtime::Val) -> Result<Val> {
    match v {
        wasmtime::Val::I32(x) => Ok(Val::I32(*x)),
        wasmtime::Val::I64(x) => Ok(Val::I64(*x)),
        wasmtime::Val::F32(x) => Ok(Val::F32(f32::from_bits(*x))),
        wasmtime::Val::F64(x) => Ok(Val::F64(f64::from_bits(*x))),
        other => Err(Error::unsupported(format!("core value {other:?}"))),
    }
}

/// Zeroed result slots matching a function's declared results.
fn result_slots(fty: &wasmtime::FuncType) -> Vec<wasmtime::Val> {
    fty.results()
        .map(|t| match t {
            wasmtime::ValType::I32 => wasmtime::Val::I32(0),
            wasmtime::ValType::I64 => wasmtime::Val::I64(0),
            wasmtime::ValType::F32 => wasmtime::Val::F32(0),
            _ => wasmtime::Val::F64(0),
        })
        .collect()
}

fn unmap_val(v: &Val) -> wasmtime::Val {
    match v {
        Val::I32(x) => wasmtime::Val::I32(*x),
        Val::I64(x) => wasmtime::Val::I64(*x),
        Val::F32(x) => wasmtime::Val::F32(x.to_bits()),
        Val::F64(x) => wasmtime::Val::F64(x.to_bits()),
    }
}

// ---------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct WtFunc(wasmtime::Func);

impl FuncHandle for WtFunc {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
struct WtMem(wasmtime::Memory);

impl MemoryHandle for WtMem {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
struct WtInstance(wasmtime::Instance);

impl InstanceHandle for WtInstance {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

struct WasmtimeStore {
    store: WtStore<StoreData>,
}

impl StoreHandle for WasmtimeStore {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn data(&self) -> &dyn Any {
        self.store.data().user.as_ref()
    }

    fn data_mut(&mut self) -> &mut dyn Any {
        self.store.data_mut().user.as_mut()
    }

    fn set_data(&mut self, data: Box<dyn Any + Send>) {
        self.store.data_mut().user = data;
    }

    fn new_func(&mut self, ty: FuncType, func: HostFn) -> Result<Func> {
        let fty = wasmtime::FuncType::new(
            self.store.engine(),
            ty.params.iter().map(|t| to_wt_val_type(*t)),
            ty.results.iter().map(|t| to_wt_val_type(*t)),
        );
        let cb = Mutex::new(func);
        let f = wasmtime::Func::new(
            &mut self.store,
            fty,
            move |caller: wasmtime::Caller<'_, StoreData>,
                  args: &[wasmtime::Val],
                  results: &mut [wasmtime::Val]| {
                let wargs = args
                    .iter()
                    .map(map_val)
                    .collect::<Result<Vec<Val>>>()
                    .map_err(wasmtime::Error::new)?;
                let mut wresults: Vec<Val> = Vec::with_capacity(results.len());
                let mut view = WtCaller(caller);
                let mut guard = cb.lock().unwrap_or_else(PoisonError::into_inner);
                match (guard)(&mut view, &wargs, &mut wresults) {
                    Ok(()) => {
                        for (i, v) in wresults.iter().enumerate() {
                            if let Some(slot) = results.get_mut(i) {
                                *slot = unmap_val(v);
                            }
                        }
                        Ok(())
                    }
                    Err(e) => Err(wasmtime::Error::new(e)),
                }
            },
        );
        Ok(Func::new(Arc::new(WtFunc(f))))
    }

    fn new_memory(&mut self, ty: MemoryType) -> Result<Memory> {
        let mty = wasmtime::MemoryType::new(ty.min, ty.max);
        let mem = wasmtime::Memory::new(&mut self.store, mty).map_err(wt_err)?;
        Ok(Memory::new(Arc::new(WtMem(mem))))
    }

    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        let f = &func.downcast::<WtFunc>()?.0;
        let fty = f.ty(&self.store);
        let params: Vec<wasmtime::Val> = args.iter().map(unmap_val).collect();
        let mut res = result_slots(&fty);
        f.call(&mut self.store, &params, &mut res).map_err(wt_err)?;
        for (slot, v) in results.iter_mut().zip(res.iter()) {
            *slot = map_val(v)?;
        }
        Ok(())
    }

    fn func_type(&self, func: &Func) -> Result<FuncType> {
        let f = &func.downcast::<WtFunc>()?.0;
        let ft = f.ty(&self.store);
        Ok(FuncType {
            params: ft.params().map(map_val_type).collect::<Result<_>>()?,
            results: ft.results().map(map_val_type).collect::<Result<_>>()?,
        })
    }

    fn get_export(&mut self, instance: &Instance, name: &str) -> Option<Extern> {
        let inst = instance.downcast::<WtInstance>().ok()?.0;
        match inst.get_export(&mut self.store, name)? {
            wasmtime::Extern::Func(f) => Some(Extern::Func(Func::new(Arc::new(WtFunc(f))))),
            wasmtime::Extern::Memory(m) => Some(Extern::Memory(Memory::new(Arc::new(WtMem(m))))),
            _ => None,
        }
    }

    fn memory_size(&self, mem: &Memory) -> Result<usize> {
        let m = &mem.downcast::<WtMem>()?.0;
        Ok(m.size(&self.store) as usize * 64 * 1024)
    }

    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()> {
        let m = &mem.downcast::<WtMem>()?.0;
        m.read(&self.store, offset, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        let m = &mem.downcast::<WtMem>()?.0;
        m.write(&mut self.store, offset, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_grow(&mut self, mem: &Memory, delta_pages: u32) -> Result<u32> {
        let m = &mem.downcast::<WtMem>()?.0;
        m.grow(&mut self.store, delta_pages as u64)
            .map(|old| old as u32)
            .map_err(wt_err)
    }
}

// ---------------------------------------------------------------------------
// Caller view
// ---------------------------------------------------------------------------

struct WtCaller<'a>(wasmtime::Caller<'a, StoreData>);

impl CallerView for WtCaller<'_> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.0.data_mut()
    }

    fn data(&self) -> &dyn Any {
        self.0.data().user.as_ref()
    }

    fn data_mut(&mut self) -> &mut dyn Any {
        self.0.data_mut().user.as_mut()
    }

    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()> {
        let m = &mem.downcast::<WtMem>()?.0;
        m.read(&self.0, offset, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        let m = &mem.downcast::<WtMem>()?.0;
        m.write(&mut self.0, offset, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_size(&self, mem: &Memory) -> usize {
        match mem.downcast::<WtMem>() {
            Ok(m) => m.0.size(&self.0) as usize * 64 * 1024,
            Err(_) => 0,
        }
    }

    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        let f = &func.downcast::<WtFunc>()?.0;
        let fty = f.ty(&self.0);
        let params: Vec<wasmtime::Val> = args.iter().map(unmap_val).collect();
        let mut res = result_slots(&fty);
        f.call(&mut self.0, &params, &mut res).map_err(wt_err)?;
        for (slot, v) in results.iter_mut().zip(res.iter()) {
            *slot = map_val(v)?;
        }
        Ok(())
    }

    fn get_export(&mut self, name: &str) -> Option<Extern> {
        match self.0.get_export(name)? {
            wasmtime::Extern::Func(f) => Some(Extern::Func(Func::new(Arc::new(WtFunc(f))))),
            wasmtime::Extern::Memory(m) => Some(Extern::Memory(Memory::new(Arc::new(WtMem(m))))),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

struct WasmtimeModule {
    engine: WtEngine,
    module: WtModule,
    imports: Vec<ImportType>,
    exports: Vec<ExportType>,
}

impl CompiledModule for WasmtimeModule {
    fn imports(&self) -> &[ImportType] {
        &self.imports
    }

    fn exports(&self) -> &[ExportType] {
        &self.exports
    }

    fn instantiate(&self, store: &mut Store, imports: &Imports) -> Result<Instance> {
        let ws = store
            .as_any_mut()
            .downcast_mut::<WasmtimeStore>()
            .ok_or_else(|| {
                Error::TypeMismatch("store does not belong to wasmtime engine".into())
            })?;

        let mut linker = wasmtime::Linker::<StoreData>::new(&self.engine);
        for imp in &self.imports {
            let ext =
                imports
                    .resolve(&imp.module, &imp.name)
                    .ok_or_else(|| Error::MissingImport {
                        module: imp.module.clone(),
                        name: imp.name.clone(),
                    })?;
            match ext {
                Extern::Func(f) => {
                    let wf = f.downcast::<WtFunc>()?;
                    linker
                        .define(&mut ws.store, &imp.module, &imp.name, wf.0)
                        .map_err(wt_err)?;
                }
                Extern::Memory(m) => {
                    let wm = m.downcast::<WtMem>()?;
                    linker
                        .define(&mut ws.store, &imp.module, &imp.name, wm.0)
                        .map_err(wt_err)?;
                }
            }
        }

        let instance = linker
            .instantiate(&mut ws.store, &self.module)
            .map_err(wt_err)?;

        Ok(Instance::new(Arc::new(WtInstance(instance))))
    }
}
