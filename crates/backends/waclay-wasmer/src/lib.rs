//! # waclay-wasmer
//!
//! [wasmer](https://wasmer.io) backend — Cranelift-backed JIT behind
//! waclay's engine traits.
//!
//! Not available on Windows: wasmer 7.3+ removed its native compilers there.
//! Use `waclay-wasmtime` or `waclay-wasmi` on that platform.
#![cfg(not(windows))]

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use wasmer::{
    Extern as WExtern, Function as WFunction, FunctionEnv as WEnv, FunctionEnvMut as WEnvMut,
    Imports as WImports, Instance as WInstance, Memory as WMemory, Module as WModule,
    Store as WStore, Value as WValue,
};

use waclay::engine::{
    CallerView, CompiledModule, ExportType, Extern, ExternType, Func, FuncHandle, FuncType, HostFn,
    ImportType, Imports, Instance, InstanceHandle, Memory, MemoryHandle, MemoryType, Store,
    StoreHandle, Val, ValType, WasmEngine,
};
use waclay::{Error, Result};

/// Engine name reported by this backend.
pub const ENGINE_NAME: &str = "wasmer";

/// Version of the wasmer crate this backend is built against.
pub const WASMER_VERSION: &str = "7.5.0";

/// Raw pointer wrapper claiming `Send + Sync`.
///
/// Soundness: waclay stores are single-threaded by construction — host data
/// is only ever reached through `&mut Store` or an in-flight guest call on
/// one thread, exactly like the wasmi/wasmtime backends.
struct SyncPtr<T: ?Sized> {
    ptr: *mut T,
}

unsafe impl<T: ?Sized> Send for SyncPtr<T> {}
unsafe impl<T: ?Sized> Sync for SyncPtr<T> {}

/// Data visible to wasmer host functions (lives inside the store objects).
struct EnvData {
    /// Non-owning pointer to the heap contents of `WasmerStore::user`.
    user: SyncPtr<dyn Any + Send>,
    /// Exports of the instantiated module, keyed by name.
    exports: Mutex<HashMap<String, WExtern>>,
}

/// The wasmer engine (one wasmer `Engine` shared by its modules & stores).
#[derive(Clone, Default)]
pub struct WasmerEngine {
    engine: wasmer::Engine,
}

impl WasmEngine for WasmerEngine {
    fn name(&self) -> &'static str {
        ENGINE_NAME
    }

    fn version(&self) -> &'static str {
        WASMER_VERSION
    }

    fn compile(&self, wasm: &[u8]) -> Result<Box<dyn CompiledModule>> {
        let store = WStore::new(self.engine.clone());
        let module = WModule::new(&store, wasm).map_err(|e| Error::engine(ENGINE_NAME, e))?;

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

        Ok(Box::new(WasmerModule {
            engine: self.engine.clone(),
            module,
            imports,
            exports,
        }))
    }

    fn new_store(&self, engine: Arc<dyn WasmEngine>) -> Result<Store> {
        let mut store = WStore::new(self.engine.clone());
        let user: Box<dyn Any + Send> = Box::new(());
        let heap: *mut (dyn Any + Send) =
            user.as_ref() as *const (dyn Any + Send) as *mut (dyn Any + Send);
        let env = WEnv::new(
            &mut store,
            EnvData {
                user: SyncPtr { ptr: heap },
                exports: Mutex::new(HashMap::new()),
            },
        );
        Ok(Store::from_handle(
            engine,
            Box::new(WasmerStore {
                store,
                env: Arc::new(env),
                user,
            }),
        ))
    }
}

fn map_extern_type(ty: &wasmer::ExternType) -> Result<ExternType> {
    match ty {
        wasmer::ExternType::Function(ft) => Ok(ExternType::Func(FuncType {
            params: ft
                .params()
                .iter()
                .map(map_val_type)
                .collect::<Result<_>>()?,
            results: ft
                .results()
                .iter()
                .map(map_val_type)
                .collect::<Result<_>>()?,
        })),
        wasmer::ExternType::Memory(_) => Ok(ExternType::Memory(MemoryType { min: 0, max: None })),
        other => Err(Error::engine(
            ENGINE_NAME,
            format!("unsupported extern type {other:?}"),
        )),
    }
}

fn map_val_type(ty: &wasmer::Type) -> Result<ValType> {
    match ty {
        wasmer::Type::I32 => Ok(ValType::I32),
        wasmer::Type::I64 => Ok(ValType::I64),
        wasmer::Type::F32 => Ok(ValType::F32),
        wasmer::Type::F64 => Ok(ValType::F64),
        other => Err(Error::unsupported(format!(
            "core value type {other:?} (waclay supports i32/i64/f32/f64)"
        ))),
    }
}

fn to_wasmer_val_type(ty: ValType) -> wasmer::Type {
    match ty {
        ValType::I32 => wasmer::Type::I32,
        ValType::I64 => wasmer::Type::I64,
        ValType::F32 => wasmer::Type::F32,
        ValType::F64 => wasmer::Type::F64,
    }
}

fn map_val(v: &WValue) -> Result<Val> {
    match v {
        WValue::I32(x) => Ok(Val::I32(*x)),
        WValue::I64(x) => Ok(Val::I64(*x)),
        WValue::F32(x) => Ok(Val::F32(*x)),
        WValue::F64(x) => Ok(Val::F64(*x)),
        other => Err(Error::unsupported(format!("core value {other:?}"))),
    }
}

/// Map a wasmer call error back to waclay, recovering a host callback's own
/// `waclay::Error` when that is what trapped.
fn rt_err(e: wasmer::RuntimeError) -> Error {
    match e.downcast::<Error>() {
        Ok(inner) => inner,
        Err(e) => Error::engine(ENGINE_NAME, e),
    }
}

fn unmap_val(v: &Val) -> WValue {
    match v {
        Val::I32(x) => WValue::I32(*x),
        Val::I64(x) => WValue::I64(*x),
        Val::F32(x) => WValue::F32(*x),
        Val::F64(x) => WValue::F64(*x),
    }
}

// ---------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct WasmerFunc(WFunction);

impl FuncHandle for WasmerFunc {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
struct WasmerMem(WMemory);

impl MemoryHandle for WasmerMem {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
struct WasmerInstance(WInstance);

impl InstanceHandle for WasmerInstance {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

struct WasmerStore {
    store: WStore,
    env: Arc<WEnv<EnvData>>,
    /// Owner of the user data; `EnvData.user` points at its heap contents.
    user: Box<dyn Any + Send>,
}

// Soundness (wasmer >= 7): wasmer's store/env types stopped being `Send`,
// but waclay stores are single-threaded by construction — host data is only
// reached through `&mut Store` or an in-flight guest call on one thread,
// exactly like the SyncPtr wrapper above. No waclay handle escapes to
// another thread while a store is live.
unsafe impl Send for WasmerStore {}

impl StoreHandle for WasmerStore {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn data(&self) -> &dyn Any {
        let env: &WEnv<EnvData> = &self.env;
        unsafe { &*env.as_ref(&self.store).user.ptr }
    }

    fn data_mut(&mut self) -> &mut dyn Any {
        let env: &WEnv<EnvData> = &self.env;
        unsafe { &mut *env.as_ref(&self.store).user.ptr }
    }

    fn set_data(&mut self, data: Box<dyn Any + Send>) {
        self.user = data;
        let heap: *mut (dyn Any + Send) =
            self.user.as_ref() as *const (dyn Any + Send) as *mut (dyn Any + Send);
        let env: &WEnv<EnvData> = &self.env;
        env.as_mut(&mut self.store).user.ptr = heap;
    }

    fn new_func(&mut self, ty: FuncType, func: HostFn) -> Result<Func> {
        let fty = wasmer::FunctionType::new(
            ty.params
                .iter()
                .map(|t| to_wasmer_val_type(*t))
                .collect::<Vec<_>>(),
            ty.results
                .iter()
                .map(|t| to_wasmer_val_type(*t))
                .collect::<Vec<_>>(),
        );
        let n_results = ty.results.len();
        let cb = Arc::new(Mutex::new(func));
        let f = WFunction::new_with_env(
            &mut self.store,
            &self.env,
            fty,
            move |mut env_mut: WEnvMut<'_, EnvData>, args: &[WValue]| {
                let wargs = args
                    .iter()
                    .map(map_val)
                    .collect::<Result<Vec<Val>>>()
                    .map_err(|e| wasmer::RuntimeError::user(Box::new(e)))?;
                let mut wresults: Vec<Val> = Vec::with_capacity(n_results);
                let mut view = WasmerCaller {
                    env_mut: &mut env_mut,
                };
                let mut guard = cb.lock().unwrap_or_else(PoisonError::into_inner);
                match (guard)(&mut view, &wargs, &mut wresults) {
                    Ok(()) => Ok(wresults.iter().map(unmap_val).collect()),
                    Err(e) => Err(wasmer::RuntimeError::user(Box::new(e))),
                }
            },
        );
        Ok(Func::new(Arc::new(WasmerFunc(f))))
    }

    fn new_memory(&mut self, ty: MemoryType) -> Result<Memory> {
        let mty = wasmer::MemoryType::new(ty.min, ty.max, false);
        let mem = WMemory::new(&mut self.store, mty).map_err(|e| Error::engine(ENGINE_NAME, e))?;
        Ok(Memory::new(Arc::new(WasmerMem(mem))))
    }

    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        let f = &func.downcast::<WasmerFunc>()?.0;
        let params: Vec<WValue> = args.iter().map(unmap_val).collect();
        let out = f.call(&mut self.store, &params).map_err(rt_err)?;
        for (slot, v) in results.iter_mut().zip(out.iter()) {
            *slot = map_val(v)?;
        }
        Ok(())
    }

    fn func_type(&self, func: &Func) -> Result<FuncType> {
        let f = &func.downcast::<WasmerFunc>()?.0;
        let ft = f.ty(&self.store);
        Ok(FuncType {
            params: ft
                .params()
                .iter()
                .map(map_val_type)
                .collect::<Result<_>>()?,
            results: ft
                .results()
                .iter()
                .map(map_val_type)
                .collect::<Result<_>>()?,
        })
    }

    fn get_export(&mut self, instance: &Instance, name: &str) -> Option<Extern> {
        let inst = &instance.downcast::<WasmerInstance>().ok()?.0;
        map_extern(inst.exports.get(name).ok()?)
    }

    fn memory_size(&self, mem: &Memory) -> Result<usize> {
        let m = &mem.downcast::<WasmerMem>()?.0;
        Ok(m.view(&self.store).data_size() as usize)
    }

    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()> {
        let m = &mem.downcast::<WasmerMem>()?.0;
        m.view(&self.store)
            .read(offset as u64, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        let m = &mem.downcast::<WasmerMem>()?.0;
        m.view(&self.store)
            .write(offset as u64, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_grow(&mut self, mem: &Memory, delta_pages: u32) -> Result<u32> {
        let m = &mem.downcast::<WasmerMem>()?.0;
        m.grow(&mut self.store, delta_pages)
            .map(|old| old.0)
            .map_err(|e| Error::engine(ENGINE_NAME, e))
    }
}

fn map_extern(ext: &WExtern) -> Option<Extern> {
    match ext {
        WExtern::Function(f) => Some(Extern::Func(Func::new(Arc::new(WasmerFunc(f.clone()))))),
        WExtern::Memory(m) => Some(Extern::Memory(Memory::new(Arc::new(WasmerMem(m.clone()))))),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Caller view
// ---------------------------------------------------------------------------

struct WasmerCaller<'a, 'b> {
    env_mut: &'b mut WEnvMut<'a, EnvData>,
}

impl CallerView for WasmerCaller<'_, '_> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.env_mut.data_mut()
    }

    fn data(&self) -> &dyn Any {
        unsafe { &*self.env_mut.data().user.ptr }
    }

    fn data_mut(&mut self) -> &mut dyn Any {
        unsafe { &mut *self.env_mut.data_mut().user.ptr }
    }

    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()> {
        let m = &mem.downcast::<WasmerMem>()?.0;
        m.view(&*self.env_mut)
            .read(offset as u64, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        let m = &mem.downcast::<WasmerMem>()?.0;
        m.view(&*self.env_mut)
            .write(offset as u64, buf)
            .map_err(|e| Error::trap(e.to_string()))
    }

    fn memory_size(&self, mem: &Memory) -> usize {
        match mem.downcast::<WasmerMem>() {
            Ok(m) => m.0.view(&*self.env_mut).data_size() as usize,
            Err(_) => 0,
        }
    }

    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        let f = &func.downcast::<WasmerFunc>()?.0;
        let params: Vec<WValue> = args.iter().map(unmap_val).collect();
        let out = f.call(&mut *self.env_mut, &params).map_err(rt_err)?;
        for (slot, v) in results.iter_mut().zip(out.iter()) {
            *slot = map_val(v)?;
        }
        Ok(())
    }

    fn get_export(&mut self, name: &str) -> Option<Extern> {
        let exports = self
            .env_mut
            .data()
            .exports
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        map_extern(exports.get(name)?)
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

struct WasmerModule {
    #[allow(dead_code)] // kept for future multi-engine composition
    engine: wasmer::Engine,
    module: WModule,
    imports: Vec<ImportType>,
    exports: Vec<ExportType>,
}

impl CompiledModule for WasmerModule {
    fn imports(&self) -> &[ImportType] {
        &self.imports
    }

    fn exports(&self) -> &[ExportType] {
        &self.exports
    }

    fn instantiate(&self, store: &mut Store, imports: &Imports) -> Result<Instance> {
        let ws = store
            .as_any_mut()
            .downcast_mut::<WasmerStore>()
            .ok_or_else(|| Error::TypeMismatch("store does not belong to wasmer engine".into()))?;

        let mut wimports = WImports::new();
        for imp in &self.imports {
            let ext =
                imports
                    .resolve(&imp.module, &imp.name)
                    .ok_or_else(|| Error::MissingImport {
                        module: imp.module.clone(),
                        name: imp.name.clone(),
                    })?;
            let wext = match ext {
                Extern::Func(f) => {
                    let wf = f.downcast::<WasmerFunc>()?;
                    WExtern::Function(wf.0.clone())
                }
                Extern::Memory(m) => {
                    let wm = m.downcast::<WasmerMem>()?;
                    WExtern::Memory(wm.0.clone())
                }
            };
            wimports.define(&imp.module, &imp.name, wext);
        }

        let instance = WInstance::new(&mut ws.store, &self.module, &wimports)
            .map_err(|e| Error::engine(ENGINE_NAME, e))?;

        // publish exports for CallerView::get_export
        {
            let env_data = ws.env.as_mut(&mut ws.store);
            let mut map = env_data.exports.lock().unwrap();
            for (name, ext) in instance.exports.iter() {
                map.insert(name.clone(), ext.clone());
            }
        }

        Ok(Instance::new(Arc::new(WasmerInstance(instance))))
    }
}
