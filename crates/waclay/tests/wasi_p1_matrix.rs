//! WASI Preview 1 (`wasi_snapshot_preview1`) engine matrix.
//!
//! The same hand-written P1 guest programs must behave identically on every
//! backend (`wasmi`, `wasmtime`, `wasmer`). Covers fd_write/fd_read, args,
//! environ, clocks, random and proc_exit — i.e. the classic P1 surface that
//! real wasm32-wasip1 binaries use.

use std::io::Write;
use std::sync::{Arc, Mutex};

use waclay::engine::Val as CoreVal;
use waclay::wasi::{add_p1_to_linker, run, WasiCtx};
use waclay::{Component, Engine, Linker, Store};

fn engines() -> Vec<(&'static str, Engine)> {
    #[allow(unused_mut)]
    let mut engines = vec![
        ("wasmi", Engine::new(waclay_wasmi::WasmiEngine::default())),
        (
            "wasmtime",
            Engine::new(waclay_wasmtime::WasmtimeEngine::default()),
        ),
    ];
    #[cfg(not(windows))]
    engines.push((
        "wasmer",
        Engine::new(waclay_wasmer::WasmerEngine::default()),
    ));
    engines
}

/// Shared in-memory `Write` sink for capturing guest stdio.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Capture {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

// ---------------------------------------------------------------------------
// hello world through fd_write
// ---------------------------------------------------------------------------

const HELLO: &str = r#"(module
    (import "wasi_snapshot_preview1" "fd_write"
        (func $fd_write (param i32 i32 i32 i32) (result i32)))
    (memory (export "memory") 1)
    (data (i32.const 16) "Hello from waclay P1!\n")
    (func (export "_start")
        ;; iovec at 0: {ptr=16, len=22}
        (i32.store (i32.const 0) (i32.const 16))
        (i32.store (i32.const 4) (i32.const 22))
        (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8))))
)"#;

#[test]
fn p1_hello_stdout_on_all_engines() {
    let wasm = wat::parse_str(HELLO).unwrap();
    for (name, engine) in engines() {
        let out = Capture::default();
        let component = Component::from_core_module(&engine, &wasm)
            .unwrap_or_else(|e| panic!("[{name}] compile: {e}"));
        let mut linker = Linker::new();
        add_p1_to_linker(&mut linker);
        let ctx = WasiCtx::builder().stdout(Box::new(out.clone())).build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        instance
            .call_core(&mut store, "_start", &[], &mut [])
            .unwrap_or_else(|e| panic!("[{name}] _start: {e}"));

        assert_eq!(
            out.text(),
            "Hello from waclay P1!\n",
            "[{name}] fd_write output"
        );
    }
}

// ---------------------------------------------------------------------------
// args + environ
// ---------------------------------------------------------------------------

const ARGS_ENV: &str = r#"(module
    (import "wasi_snapshot_preview1" "args_sizes_get"
        (func $args_sizes_get (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "args_get"
        (func $args_get (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "environ_sizes_get"
        (func $env_sizes_get (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "environ_get"
        (func $environ_get (param i32 i32) (result i32)))
    (memory (export "memory") 1)
    (func (export "fetch-args")
        (drop (call $args_sizes_get (i32.const 0) (i32.const 4)))
        (drop (call $args_get (i32.const 64) (i32.const 512))))
    (func (export "fetch-env")
        (drop (call $env_sizes_get (i32.const 8) (i32.const 12)))
        (drop (call $environ_get (i32.const 1024) (i32.const 2048))))
)"#;

#[test]
fn p1_args_and_environ_on_all_engines() {
    let wasm = wat::parse_str(ARGS_ENV).unwrap();
    for (name, engine) in engines() {
        let component = Component::from_core_module(&engine, &wasm)
            .unwrap_or_else(|e| panic!("[{name}] compile: {e}"));
        let mut linker = Linker::new();
        add_p1_to_linker(&mut linker);
        let ctx = WasiCtx::builder()
            .arg("prog")
            .arg("a b")
            .env("FRUIT", "mango")
            .build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        instance
            .call_core(&mut store, "fetch-args", &[], &mut [])
            .unwrap_or_else(|e| panic!("[{name}] fetch-args: {e}"));
        instance
            .call_core(&mut store, "fetch-env", &[], &mut [])
            .unwrap_or_else(|e| panic!("[{name}] fetch-env: {e}"));

        // args_get writes the pointer array at 64 and the string bytes at 512
        let mut buf = vec![0u8; 128];
        let memory = memory_of(&instance, &mut store, name);
        store.memory_read(&memory, 512, &mut buf).unwrap();
        assert!(
            buf.windows(5).any(|w| w == b"prog\0"),
            "[{name}] argv strings must contain prog\\0, got {buf:02x?}"
        );
        assert!(
            buf.windows(4).any(|w| w == b"a b\0"),
            "[{name}] argv strings must contain `a b\\0`"
        );

        // environ_get writes the pointer array at 1024, strings at 2048
        let mut ebuf = [0u8; 64];
        let memory = memory_of(&instance, &mut store, name);
        store.memory_read(&memory, 2048, &mut ebuf).unwrap();
        assert!(
            ebuf.windows(12).any(|w| w == b"FRUIT=mango\0"),
            "[{name}] environ strings must contain FRUIT=mango, got {ebuf:02x?}"
        );
    }
}

fn memory_of(
    instance: &waclay::ComponentInstance,
    store: &mut Store,
    name: &str,
) -> waclay::engine::Memory {
    match instance.get_core_export(store, "memory") {
        Some(waclay::engine::Extern::Memory(m)) => m,
        other => panic!("[{name}] no memory export: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// clock + random
// ---------------------------------------------------------------------------

const CLOCK_RAND: &str = r#"(module
    (import "wasi_snapshot_preview1" "clock_time_get"
        (func $clock_time_get (param i32 i64 i32) (result i32)))
    (import "wasi_snapshot_preview1" "random_get"
        (func $random_get (param i32 i32) (result i32)))
    (memory (export "memory") 1)
    (func (export "tick") (result i32)
        (drop (call $clock_time_get (i32.const 1) (i64.const 0) (i32.const 0)))
        (i32.load (i32.const 0)))
    (func (export "noise") (result i32)
        (drop (call $random_get (i32.const 32) (i32.const 16)))
        (i32.load (i32.const 32)))
)"#;

#[test]
fn p1_clock_and_random_on_all_engines() {
    let wasm = wat::parse_str(CLOCK_RAND).unwrap();
    for (name, engine) in engines() {
        let component = Component::from_core_module(&engine, &wasm)
            .unwrap_or_else(|e| panic!("[{name}] compile: {e}"));
        let mut linker = Linker::new();
        add_p1_to_linker(&mut linker);
        let mut store = Store::new(&engine, WasiCtx::builder().build()).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        let mut t1 = [CoreVal::I32(0)];
        instance
            .call_core(&mut store, "tick", &[], &mut t1)
            .unwrap_or_else(|e| panic!("[{name}] tick: {e}"));
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut t2 = [CoreVal::I32(0)];
        instance
            .call_core(&mut store, "tick", &[], &mut t2)
            .unwrap_or_else(|e| panic!("[{name}] tick: {e}"));
        // low 32 bits of the monotonic clock: must move forward
        assert_ne!(
            t1[0].i32().unwrap(),
            t2[0].i32().unwrap(),
            "[{name}] monotonic clock must advance"
        );

        let mut r1 = [CoreVal::I32(0)];
        instance
            .call_core(&mut store, "noise", &[], &mut r1)
            .unwrap_or_else(|e| panic!("[{name}] noise: {e}"));
        let mut bytes = [0u8; 16];
        let memory = memory_of(&instance, &mut store, name);
        store.memory_read(&memory, 32, &mut bytes).unwrap();
        assert!(
            bytes.iter().any(|&b| b != 0),
            "[{name}] random bytes must not be all zero"
        );
    }
}

// ---------------------------------------------------------------------------
// proc_exit
// ---------------------------------------------------------------------------

const EXIT: &str = r#"(module
    (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
    (func (export "_start") (call $exit (i32.const 7)))
)"#;

#[test]
fn p1_proc_exit_on_all_engines() {
    let wasm = wat::parse_str(EXIT).unwrap();
    for (name, engine) in engines() {
        // (a) raw path: the trap carries the code and the ctx records it
        let component = Component::from_core_module(&engine, &wasm)
            .unwrap_or_else(|e| panic!("[{name}] compile: {e}"));
        let mut linker = Linker::new();
        add_p1_to_linker(&mut linker);
        let ctx = WasiCtx::builder().build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));
        let err = instance
            .call_core(&mut store, "_start", &[], &mut [])
            .expect_err(&format!("[{name}] proc_exit must trap"));
        // the host fn records the code in the ctx before trapping (the trap
        // message wrapping differs per engine, so check the recorded code)
        let recorded = store
            .data()
            .downcast_ref::<WasiCtx>()
            .and_then(|c| c.exit_code);
        assert_eq!(recorded, Some(7), "[{name}] ctx must record the exit code");
        let _ = &err;

        // (b) convenience runner: translates the trap into an exit code
        let code = run(&engine, &wasm, WasiCtx::builder().build())
            .unwrap_or_else(|e| panic!("[{name}] wasi::run: {e}"));
        assert_eq!(code, 7, "[{name}] wasi::run must return the exit code");
    }
}

// ---------------------------------------------------------------------------
// stdin via fd_read
// ---------------------------------------------------------------------------

const STDIN: &str = r#"(module
    (import "wasi_snapshot_preview1" "fd_read"
        (func $fd_read (param i32 i32 i32 i32) (result i32)))
    (memory (export "memory") 1)
    (func (export "read-some") (result i32)
        (i32.store (i32.const 16) (i32.const 32))   ;; iov ptr
        (i32.store (i32.const 20) (i32.const 8))    ;; iov len
        (drop (call $fd_read (i32.const 0) (i32.const 16) (i32.const 1) (i32.const 0)))
        (i32.load (i32.const 0)))                    ;; nread written by fd_read
)"#;

#[test]
fn p1_stdin_read_on_all_engines() {
    let wasm = wat::parse_str(STDIN).unwrap();
    for (name, engine) in engines() {
        let component = Component::from_core_module(&engine, &wasm)
            .unwrap_or_else(|e| panic!("[{name}] compile: {e}"));
        let mut linker = Linker::new();
        add_p1_to_linker(&mut linker);
        let ctx = WasiCtx::builder()
            .stdin(Box::new(std::io::Cursor::new(b"hi".to_vec())))
            .build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        let mut res = [CoreVal::I32(0)];
        instance
            .call_core(&mut store, "read-some", &[], &mut res)
            .unwrap_or_else(|e| panic!("[{name}] read-some: {e}"));
        assert_eq!(res[0].i32().unwrap(), 2, "[{name}] fd_read byte count");

        let mut buf = [0u8; 8];
        let memory = memory_of(&instance, &mut store, name);
        store.memory_read(&memory, 32, &mut buf).unwrap();
        assert_eq!(&buf[..2], b"hi", "[{name}] stdin bytes landed in memory");
    }
}
