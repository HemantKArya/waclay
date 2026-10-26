//! The cross-engine abstraction: one object-safe trait set implemented by
//! every backend (`waclay-wasmi`, `waclay-wasmtime`, `waclay-wasmer`).
//!
//! The layer mirrors wasmtime's `Engine`/`Module`/`Store`/`Instance` model:
//!
//! * [`WasmEngine`] – compilation state (Cranelift / wasmi interpreter / ...).
//! * [`CompiledModule`] – a validated, compiled core wasm module.
//! * [`Store`] – runtime state plus the user's host data.
//! * [`Func`], [`Memory`], [`Instance`] – opaque handles living in a store.
//!
//! Everything is object-safe so backends stay out-of-tree and `waclay` itself
//! never depends on an engine.

use crate::error::{Error, Result};
pub mod value;

pub use value::{ExportType, ExternType, FuncType, ImportType, MemoryType, Val, ValType};

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Opaque backend handles
// ---------------------------------------------------------------------------

/// Backend-internal function handle (never inspected by waclay itself).
pub trait FuncHandle: Any + Send + Sync {
    /// Downcast support so each backend can recover its own type.
    fn as_any(&self) -> &dyn Any;
}

/// Backend-internal memory handle.
pub trait MemoryHandle: Any + Send + Sync {
    /// Downcast support so each backend can recover its own type.
    fn as_any(&self) -> &dyn Any;
}

/// Backend-internal instance handle.
pub trait InstanceHandle: Any + Send + Sync {
    /// Downcast support so each backend can recover its own type.
    fn as_any(&self) -> &dyn Any;
}

macro_rules! opaque_handle {
    ($(#[$meta:meta])* $name:ident, $trait:ident, $dup:literal) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $name {
            inner: Arc<dyn $trait>,
        }

        impl $name {
            /// Backend-only constructor: wrap a native engine handle.
            pub fn new(inner: Arc<dyn $trait>) -> Self {
                Self { inner }
            }

            /// Recover the backend's native handle type.
            pub fn downcast<T: 'static>(&self) -> Result<&T> {
                self.inner.as_any().downcast_ref::<T>().ok_or_else(|| {
                    Error::TypeMismatch(format!(
                        concat!("handle does not belong to this ", $dup, " engine")
                    ))
                })
            }

            /// Cheap identity comparison within one engine.
            pub fn ptr_eq(&self, other: &$name) -> bool {
                Arc::ptr_eq(&self.inner, &other.inner)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
}

opaque_handle!(Func, FuncHandle, "function");
opaque_handle!(Memory, MemoryHandle, "memory");
opaque_handle!(Instance, InstanceHandle, "instance");

/// An external item handed to (or taken out of) an instance.
#[derive(Clone, Debug)]
pub enum Extern {
    Func(Func),
    Memory(Memory),
}

impl Extern {
    /// Type of this item; needs the owning store for function types.
    pub fn ty(&self, store: &Store) -> Result<ExternType> {
        match self {
            Extern::Func(f) => Ok(ExternType::Func(store.func_type(f)?)),
            // Reports the memory's *current* size in pages as `min`; the
            // declared maximum is not observable through the facade.
            Extern::Memory(m) => Ok(ExternType::Memory(MemoryType {
                min: u32::try_from(store.memory_size(m)? / (64 * 1024)).unwrap_or(u32::MAX),
                max: None,
            })),
        }
    }
}

// ---------------------------------------------------------------------------
// Host functions
// ---------------------------------------------------------------------------

/// Callback invoked by the engine when a guest calls an imported function.
///
/// `args`/`results` are core wasm values; heap-typed data travels through the
/// linear memory via [`CallerView`]. The host pushes exactly as many result
/// values as the declared function type expects.
pub type HostFn = Box<dyn FnMut(&mut dyn CallerView, &[Val], &mut Vec<Val>) -> Result<()> + Send>;

/// View of the calling context, handed to host function callbacks.
///
/// This is the *only* way host code touches guest state during a call; each
/// backend adapts its native `Caller` type to this trait.
pub trait CallerView {
    /// Backend-internal caller (downcast hook for advanced users).
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Shared reference to the store's user data.
    fn data(&self) -> &dyn Any;

    /// Exclusive reference to the store's user data.
    fn data_mut(&mut self) -> &mut dyn Any;

    /// Read guest memory bytes.
    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()>;

    /// Write guest memory bytes.
    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()>;

    /// Current memory size in bytes.
    fn memory_size(&self, mem: &Memory) -> usize;

    /// Call a function of this engine from *inside* a host callback
    /// (used for guest `realloc` and `post-return` hooks).
    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()>;

    /// Look up an export of the *calling* instance (e.g. its `"memory"`).
    fn get_export(&mut self, name: &str) -> Option<Extern>;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Backend implementation of a store.
pub trait StoreHandle: Send {
    /// Downcast hook for the owning backend.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// User data (wasmtime-style host state), shared view.
    fn data(&self) -> &dyn Any;

    /// User data, exclusive view.
    fn data_mut(&mut self) -> &mut dyn Any;

    /// Replace user data wholesale.
    fn set_data(&mut self, data: Box<dyn Any + Send>);

    /// Create a host function owned by this store.
    fn new_func(&mut self, ty: FuncType, func: HostFn) -> Result<Func>;

    /// Create a fresh linear memory owned by this store.
    fn new_memory(&mut self, ty: MemoryType) -> Result<Memory>;

    /// Call a function (guest export or host function) with core values.
    fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()>;

    /// Reflected type of a function handle.
    fn func_type(&self, func: &Func) -> Result<FuncType>;

    /// Resolve an export of an instance.
    fn get_export(&mut self, instance: &Instance, name: &str) -> Option<Extern>;

    /// Current memory size in bytes.
    fn memory_size(&self, mem: &Memory) -> Result<usize>;

    /// Read from guest memory.
    fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()>;

    /// Write to guest memory.
    fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()>;

    /// Grow a memory, returning the previous size in pages.
    fn memory_grow(&mut self, mem: &Memory, delta_pages: u32) -> Result<u32>;
}

/// Wasmtime-style store: holds engine state plus user host data.
pub struct Store {
    engine: Arc<dyn WasmEngine>,
    inner: Box<dyn StoreHandle>,
}

impl Store {
    /// Create a store with the given user data (wasmtime-style).
    pub fn new(engine: &Engine, data: impl Any + Send) -> Result<Store> {
        let mut store = engine.new_store()?;
        store.set_data(Box::new(data));
        Ok(store)
    }

    /// Backend-only constructor: wrap a native engine store.
    pub fn from_handle(engine: Arc<dyn WasmEngine>, inner: Box<dyn StoreHandle>) -> Self {
        Store { engine, inner }
    }

    /// The engine this store belongs to.
    pub fn engine(&self) -> &Arc<dyn WasmEngine> {
        &self.engine
    }

    /// Backend handle downcast hook.
    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self.inner.as_any_mut()
    }

    /// User data, exclusive access.
    pub fn data_mut(&mut self) -> &mut dyn Any {
        self.inner.data_mut()
    }

    /// User data, shared access.
    pub fn data(&self) -> &dyn Any {
        self.inner.data()
    }

    /// Replace user data.
    pub fn set_data(&mut self, data: Box<dyn Any + Send>) {
        self.inner.set_data(data);
    }

    /// Create a host function in this store.
    pub fn new_func(&mut self, ty: FuncType, func: HostFn) -> Result<Func> {
        self.inner.new_func(ty, func)
    }

    /// Create a linear memory in this store.
    pub fn new_memory(&mut self, ty: MemoryType) -> Result<Memory> {
        self.inner.new_memory(ty)
    }

    /// Call a function handle with core values.
    pub fn call(&mut self, func: &Func, args: &[Val], results: &mut [Val]) -> Result<()> {
        self.inner.call(func, args, results)
    }

    /// Reflected type of a function handle.
    pub fn func_type(&self, func: &Func) -> Result<FuncType> {
        self.inner.func_type(func)
    }

    /// Resolve an export of an instance living in this store.
    pub fn get_export(&mut self, instance: &Instance, name: &str) -> Option<Extern> {
        self.inner.get_export(instance, name)
    }

    /// Current memory size in bytes.
    pub fn memory_size(&self, mem: &Memory) -> Result<usize> {
        self.inner.memory_size(mem)
    }

    /// Read guest memory.
    pub fn memory_read(&self, mem: &Memory, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.inner.memory_read(mem, offset, buf)
    }

    /// Write guest memory.
    pub fn memory_write(&mut self, mem: &Memory, offset: usize, buf: &[u8]) -> Result<()> {
        self.inner.memory_write(mem, offset, buf)
    }

    /// Grow guest memory.
    pub fn memory_grow(&mut self, mem: &Memory, delta_pages: u32) -> Result<u32> {
        self.inner.memory_grow(mem, delta_pages)
    }

    #[allow(dead_code)] // backend escape hatch
    pub(crate) fn inner(&mut self) -> &mut Box<dyn StoreHandle> {
        &mut self.inner
    }
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

/// Import map used while instantiating a core module: `(module, name) -> Extern`.
#[derive(Default, Clone)]
pub struct Imports {
    entries: HashMap<(String, String), Extern>,
}

impl Imports {
    /// Empty import set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a function import.
    pub fn add_func(&mut self, module: &str, name: &str, func: Func) {
        self.entries
            .insert((module.to_string(), name.to_string()), Extern::Func(func));
    }

    /// Register a memory import.
    pub fn add_memory(&mut self, module: &str, name: &str, memory: Memory) {
        self.entries.insert(
            (module.to_string(), name.to_string()),
            Extern::Memory(memory),
        );
    }

    /// Look up a required import.
    pub fn resolve(&self, module: &str, name: &str) -> Option<&Extern> {
        self.entries.get(&(module.to_string(), name.to_string()))
    }

    /// True when nothing has been registered yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Debug for Imports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.keys().map(|(m, n)| (m, n)))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Compiled module
// ---------------------------------------------------------------------------

/// A core wasm module compiled by a specific engine.
pub trait CompiledModule: Send + Sync {
    /// Static imports of the module.
    fn imports(&self) -> &[ImportType];

    /// Static exports of the module.
    fn exports(&self) -> &[ExportType];

    /// Instantiate inside `store`, resolving every import from `imports`.
    fn instantiate(&self, store: &mut Store, imports: &Imports) -> Result<Instance>;
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The cross-engine trait. Implemented by every waclay backend.
///
/// ```no_run
/// use waclay::{Engine, engine::WasmEngine};
/// use waclay_wasmi::WasmiEngine;
///
/// let engine: Engine = Engine::new(WasmiEngine::default());
/// ```
pub trait WasmEngine: Send + Sync + 'static {
    /// Human readable engine name (`"wasmi"`, `"wasmtime"`, ...).
    fn name(&self) -> &'static str;

    /// Version of the underlying engine crate (e.g. `"2.0.0"` for wasmi,
    /// `"50.0.0"` for wasmtime, `"7.5.0"` for wasmer).
    fn version(&self) -> &'static str;

    /// Validate and compile a core wasm binary.
    fn compile(&self, wasm: &[u8]) -> Result<Box<dyn CompiledModule>>;

    /// Create a fresh store bound to this engine.
    ///
    /// The `engine` argument is the shared trait-object handle that the
    /// resulting [`Store`] should expose back to users.
    fn new_store(&self, engine: Arc<dyn WasmEngine>) -> Result<Store>;
}

/// Cheaply cloneable engine handle used by the whole facade API.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<dyn WasmEngine>,
}

impl Engine {
    /// Wrap any backend engine implementation.
    pub fn new(engine: impl WasmEngine) -> Self {
        Engine {
            inner: Arc::new(engine),
        }
    }

    /// Wrap an already-shared backend engine.
    pub fn from_arc(engine: Arc<dyn WasmEngine>) -> Self {
        Engine { inner: engine }
    }

    /// The underlying trait object.
    pub fn as_trait(&self) -> &Arc<dyn WasmEngine> {
        &self.inner
    }

    /// Compile a core wasm binary with this engine.
    pub fn compile(&self, wasm: &[u8]) -> Result<Box<dyn CompiledModule>> {
        self.inner.compile(wasm)
    }

    /// Create a store bound to this engine.
    pub fn new_store(&self) -> Result<Store> {
        self.inner.new_store(self.inner.clone())
    }

    /// Engine name (`"wasmi"`, `"wasmtime"`, `"wasmer"`).
    pub fn name(&self) -> &'static str {
        self.inner.name()
    }

    /// Version of the underlying engine crate.
    pub fn version(&self) -> &'static str {
        self.inner.version()
    }

    /// `"wasmi 2.0.0"`-style one-line engine description.
    pub fn describe(&self) -> String {
        format!("{} {}", self.inner.name(), self.inner.version())
    }
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Engine({})", self.inner.name())
    }
}
