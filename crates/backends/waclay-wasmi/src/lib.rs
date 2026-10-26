//! # waclay-wasmi
//!
//! [wasmi](https://github.com/wasmi-labs/wasmi) interpreter backend —
//! the reference implementation of waclay's [`WasmEngine`] trait.
//!
//! wasmi is a pure interpreter with excellent determinism and embeddability,
//! making it the ideal engine to develop and debug the abstraction layer
//! against before porting to optimizing compilers like wasmtime.

#![forbid(unsafe_code)]

use std::any::Any;
use std::sync::{Arc, Mutex, PoisonError};

use waclay::engine::{
    CallerView, CompiledModule, ExportType, Extern, ExternType, Func, FuncHandle, FuncType, HostFn,
    ImportType, Imports, Instance, InstanceHandle, Memory, MemoryHandle, MemoryType, Store,
    StoreHandle, Val, ValType, WasmEngine,
};
use waclay::{Error, Result};

/// Engine name reported by this backend.
pub const ENGINE_NAME: &str = "wasmi";

/// Version of the wasmi crate this backend is built against.
pub const WASMI_VERSION: &str = "2.0.0";

/// Host data kept inside the wasmi store.
struct StoreData {
    user: Box<dyn Any + Send>,
}

impl Default for StoreData {
    fn default() -> Self {
        StoreData { user: Box::new(()) }
    }
}

/// The wasmi engine: wraps `wasmi::Engine` configuration.
#[derive(Clone)]
pub struct WasmiEngine {
    engine: wasmi::Engine,
}

impl Default for WasmiEngine {
    fn default() -> Self {
        Self::new(wasmi::Config::default())
    }
}

impl WasmiEngine {
    /// Create a wasmi engine from an explicit configuration.
    pub fn new(config: wasmi::Config) -> Self {
        Self {
            engine: wasmi::Engine::new(&config),
        }
    }
}

impl WasmEngine for WasmiEngine {
    fn name(&self) -> &'static str {
        ENGINE_NAME
    }

    fn version(&self) -> &'static str {
        WASMI_VERSION
    }

    fn compile(&self, wasm: &[u8]) -> Result<Box<dyn CompiledModule>> {
        let module =
            wasmi::Module::new(&self.engine, wasm).map_err(|e| Error::engine(ENGINE_NAME, e))?;

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

        Ok(Box::new(WasmiModule {
            engine: self.engine.clone(),
            module,
            imports,
            exports,
        }))
    }

    fn new_store(&self, engine: Arc<dyn WasmEngine>) -> Result<Store> {
        let store = wasmi::Store::new(&self.engine, StoreData::default());
        Ok(Store::from_handle(engine, Box::new(WasmiStore { store })))
    }
}

fn map_extern_type(ty: &wasmi::ExternType) -> Result<ExternType> {
    match ty {
        wasmi::ExternType::Func(ft) => Ok(ExternType::Func(map_func_type(ft)?)),
        wasmi::ExternType::Memory(mt) => Ok(ExternType::Memory(MemoryType {
            min: u32::try_from(mt.minimum()).unwrap_or(u32::MAX),
            max: mt.maximum().and_then(|m| u32::try_from(m).ok()),
        })),
        other => Err(Error::engine(
            ENGINE_NAME,
            format!("unsupported extern type: {other:?}"),
        )),
    }
}

fn map_func_type(ft: &wasmi::FuncType) -> Result<FuncType> {
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

fn map_val_type(ty: &wasmi::ValType) -> Result<ValType> {
    match ty {
        wasmi::ValType::I32 => Ok(ValType::I32),
        wasmi::ValType::I64 => Ok(ValType::I64),
        wasmi::ValType::F32 => Ok(ValType::F32),
        wasmi::ValType::F64 => Ok(ValType::F64),
        other => Err(Error::unsupported(format!(
            "core value type {other:?} (waclay supports i32/i64/f32/f64)"
        ))),
    }
}

fn to_wasmi_val_type(ty: ValType) -> wasmi::ValType {
    match ty {
        ValType::I32 => wasmi::ValType::I32,
        ValType::I64 => wasmi::ValType::I64,
        ValType::F32 => wasmi::ValType::F32,
        ValType::F64 => wasmi::ValType::F64,
    }
}

fn map_val(v: &wasmi::Val) -> Result<Val> {
    match v {
        wasmi::Val::I32(x) => Ok(Val::I32(*x)),
        wasmi::Val::I64(x) => Ok(Val::I64(*x)),
        wasmi::Val::F32(x) => Ok(Val::F32(f32::from(*x))),
        wasmi::Val::F64(x) => Ok(Val::F64(f64::from(*x))),
        other => Err(Error::unsupported(format!("core value {other:?}"))),
    }
}

fn unmap_val(v: &Val) -> wasmi::Val {
    match v {
        Val::I32(x) => wasmi::Val::I32(*x),
        Val::I64(x) => wasmi::Val::I64(*x),
        Val::F32(x) => wasmi::Val::F32((*x).into()),
        Val::F64(x) => wasmi::Val::F64((*x).into()),
    }
}

// ---------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct WasmiFunc(wasmi::Func);

impl FuncHandle for WasmiFunc {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Copy)]
struct WasmiMem(wasmi::Memory);

impl MemoryHandle for WasmiMem {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Copy)]
struct WasmiInstance(wasmi::Instance);

impl InstanceHandle for WasmiInstance {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

struct WasmiStore {
    store: wasmi::Store<StoreData>,
}

impl StoreHandle for WasmiStore {
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
        let fty = wasmi::FuncType::new(
            ty.params
                .iter()
                .map(|t| to_wasmi_val_type(*t))
                .collect::<Vec<_>>(),
            ty.results
                .iter()
                .map(|t| to_wasmi_val_type(*t))
                .collect::<Vec<_>>(),
        );
        let cb = Mutex::new(func);
        let f = wasmi::Func::new(
            &mut self.store,
            fty,
            move |caller: wasmi::Caller<StoreData>,
                  args: &[wasmi::Val],
                  results: &mut [wasmi::Val]| {
                let wargs = args
                    .iter()
                    .map(map_val)
                    .collect::<Result<Vec<Val>>>()
                    .map_err(|e| wasmi::Error::new(e.to_string()))?;
                let mut wresults: Vec<Val> = Vec::with_capacity(results.len());
                let mut view = WasmiCaller(caller);
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
                    Err(e) => Err(wasmi::Error::new(e.to_string())),
                }
            },
        );
        Ok(Func::new(Arc::new(WasmiFunc(f))))
    }

    fn new_memory(&mut self, ty: MemoryType) -> Result<Memory> {
        let mty = wasmi::MemoryType::new(ty.min, ty.max);
        let mem =
            wasmi::Memory::new(&mut self.store, mty).map_err(|e| Error::engine(ENGINE_NAME, e))?;
        Ok(Memory::new(Arc::new(WasmiMem(mem))))
    }

    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        let f = func.downcast::<WasmiFunc>()?.0;
        let fty = f.ty(&self.store);
        let params: Vec<wasmi::Val> = args.iter().map(unmap_val).collect();
        let mut res: Vec<wasmi::Val> = fty
            .results()
            .iter()
            .map(|t| wasmi::Val::default_for_ty(*t))
            .collect();
        f.call(&mut self.store, &params, &mut res)
            .map_err(|e| Error::engine(ENGINE_NAME, e))?;
        for (slot, v) in results.iter_mut().zip(res.iter()) {
            *slot = map_val(v)?;
        }
        Ok(())
    }

    fn func_type(&self, func: &Func) -> Result<FuncType> {
        let f = func.downcast::<WasmiFunc>()?.0;
        map_func_type(&f.ty(&self.store))
    }

    fn get_export(&mut self, instance: &Instance, name: &str) -> Option<Extern> {
        let inst = instance.downcast::<WasmiInstance>().ok()?.0;
        match inst.get_export(&self.store, name)? {
            wasmi::Extern::Func(f) => Some(Extern::Func(Func::new(Arc::new(WasmiFunc(f))))),
            wasmi::Extern::Memory(m) => Some(Extern::Memory(Memory::new(Arc::new(WasmiMem(m))))),
            _ => None,
        }
    }

    fn memory_size(&self, mem: &Memory) -> Result<usize> {
        let m = mem.downcast::<WasmiMem>()?.0;
        Ok(m.data(&self.store).len())
    }

    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()> {
        let m = mem.downcast::<WasmiMem>()?.0;
        let data = m.data(&self.store);
        let end = offset
            .checked_add(buf.len())
            .ok_or_else(|| Error::trap("memory offset overflow"))?;
        let src = data
            .get(offset..end)
            .ok_or_else(|| Error::trap("out of bounds memory read"))?;
        buf.copy_from_slice(src);
        Ok(())
    }

    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        let m = mem.downcast::<WasmiMem>()?.0;
        let data = m.data_mut(&mut self.store);
        let end = offset
            .checked_add(buf.len())
            .ok_or_else(|| Error::trap("memory offset overflow"))?;
        let dst = data
            .get_mut(offset..end)
            .ok_or_else(|| Error::trap("out of bounds memory write"))?;
        dst.copy_from_slice(buf);
        Ok(())
    }

    fn memory_grow(&mut self, mem: &Memory, delta_pages: u32) -> Result<u32> {
        let m = mem.downcast::<WasmiMem>()?.0;
        let old = m
            .grow(&mut self.store, u64::from(delta_pages))
            .map_err(|e| Error::engine(ENGINE_NAME, e))?;
        Ok(u32::try_from(old).unwrap_or(u32::MAX))
    }
}

// ---------------------------------------------------------------------------
// Caller view
// ---------------------------------------------------------------------------

struct WasmiCaller<'a>(wasmi::Caller<'a, StoreData>);

impl CallerView for WasmiCaller<'_> {
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
        let m = mem.downcast::<WasmiMem>()?.0;
        let data = m.data(&self.0);
        let end = offset
            .checked_add(buf.len())
            .ok_or_else(|| Error::trap("memory offset overflow"))?;
        let src = data
            .get(offset..end)
            .ok_or_else(|| Error::trap("out of bounds memory read"))?;
        buf.copy_from_slice(src);
        Ok(())
    }

    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        let m = mem.downcast::<WasmiMem>()?.0;
        let data = m.data_mut(&mut self.0);
        let end = offset
            .checked_add(buf.len())
            .ok_or_else(|| Error::trap("memory offset overflow"))?;
        let dst = data
            .get_mut(offset..end)
            .ok_or_else(|| Error::trap("out of bounds memory write"))?;
        dst.copy_from_slice(buf);
        Ok(())
    }

    fn memory_size(&self, mem: &Memory) -> usize {
        match mem.downcast::<WasmiMem>() {
            Ok(m) => m.0.data(&self.0).len(),
            Err(_) => 0,
        }
    }

    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        let f = func.downcast::<WasmiFunc>()?.0;
        let fty = f.ty(&self.0);
        let params: Vec<wasmi::Val> = args.iter().map(unmap_val).collect();
        let mut res: Vec<wasmi::Val> = fty
            .results()
            .iter()
            .map(|t| wasmi::Val::default_for_ty(*t))
            .collect();
        f.call(&mut self.0, &params, &mut res)
            .map_err(|e| Error::engine(ENGINE_NAME, e))?;
        for (slot, v) in results.iter_mut().zip(res.iter()) {
            *slot = map_val(v)?;
        }
        Ok(())
    }

    fn get_export(&mut self, name: &str) -> Option<Extern> {
        match self.0.get_export(name)? {
            wasmi::Extern::Func(f) => Some(Extern::Func(Func::new(Arc::new(WasmiFunc(f))))),
            wasmi::Extern::Memory(m) => Some(Extern::Memory(Memory::new(Arc::new(WasmiMem(m))))),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Module
// ---------------------------------------------------------------------------

struct WasmiModule {
    engine: wasmi::Engine,
    module: wasmi::Module,
    imports: Vec<ImportType>,
    exports: Vec<ExportType>,
}

impl CompiledModule for WasmiModule {
    fn imports(&self) -> &[ImportType] {
        &self.imports
    }

    fn exports(&self) -> &[ExportType] {
        &self.exports
    }

    fn instantiate(&self, store: &mut Store, imports: &Imports) -> Result<Instance> {
        let ws = store
            .as_any_mut()
            .downcast_mut::<WasmiStore>()
            .ok_or_else(|| Error::TypeMismatch("store does not belong to wasmi engine".into()))?;

        let mut linker = wasmi::Linker::<StoreData>::new(&self.engine);
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
                    let wf = f.downcast::<WasmiFunc>()?;
                    linker
                        .define(&imp.module, &imp.name, wf.0)
                        .map_err(|e| Error::engine(ENGINE_NAME, e))?;
                }
                Extern::Memory(m) => {
                    let wm = m.downcast::<WasmiMem>()?;
                    linker
                        .define(&imp.module, &imp.name, wm.0)
                        .map_err(|e| Error::engine(ENGINE_NAME, e))?;
                }
            }
        }

        let inst = linker
            .instantiate_and_start(&mut ws.store, &self.module)
            .map_err(|e| Error::engine(ENGINE_NAME, e))?;

        Ok(Instance::new(Arc::new(WasmiInstance(inst))))
    }
}
