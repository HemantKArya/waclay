//! # Waclay 2.0
//!
//! **One Component Model runtime, three engines.**
//!
//! Waclay provides a single, engine-agnostic WebAssembly runtime API on top of
//! wasmtime, wasmer and wasmi — with first-class Component Model support
//! (WIT worlds, Canonical ABI, WASI) and no lock-in.

#![forbid(unsafe_code)]
#![allow(clippy::type_complexity)]

pub mod abi;
pub mod component;
pub mod engine;
pub mod error;
pub mod func;
pub mod linker;
pub mod resource;
pub mod wasi;

pub use component::asyncio::{
    AsyncState, CallbackCode, CopyResult, EventCode, SubtaskState, BLOCKED,
};
pub use component::{Component, ComponentPlan, Val};
pub use engine::{
    CallerView, CompiledModule, Engine, ExportType, Extern, ExternType, Func, FuncType, HostFn,
    ImportType, Imports, Instance, Memory, MemoryType, Store, ValType, WasmEngine,
};
pub use error::{Error, Result};
pub use func::{
    ComponentParams, ComponentResults, ComponentValue, FutureHandle, StreamHandle, TypedFunc,
};
pub use linker::{ComponentInstance, HostCtx, Linker};

/// Version string of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
