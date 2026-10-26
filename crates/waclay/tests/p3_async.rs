//! Preview-3 async spec-conformance tests: streams, futures, waitable sets,
//! error contexts and the task model, exercised through real component
//! binaries (hand-encoded with `wasm-encoder`) on all three engines.
//!
//! The fixture's core module imports every canon builtin and re-exports it
//! as a plain guest function, so each test drives the exact core signatures
//! defined by CanonicalABI.md ("Builtin functions") and the packed payload
//! encodings (`result | (progress << 4)`, `BLOCKED = -1`).

use waclay::component::asyncio::{CopyResult, EventCode};
use waclay::component::types::Val;
use waclay::{Component, Engine, Extern, Linker, Store};
use wasm_encoder::{
    CanonicalOption, ComponentExportKind, ComponentValType, ExportKind, ModuleArg,
    PrimitiveValType as PV, ValType,
};

// ---------------------------------------------------------------------------
// Engines
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Fixture: core module with pass-throughs over every builtin
// ---------------------------------------------------------------------------

/// The guest core module. Import names match the wasm-tools convention
/// (`[stream-new]` etc.) to prove the argmap binding is name-agnostic.
const GUEST: &str = r#"
(module
  (import "p3" "[stream-new]" (func $snew (result i64)))
  (import "p3" "[stream-read]" (func $sread (param i32 i32 i32) (result i32)))
  (import "p3" "[stream-read-async]" (func $sreada (param i32 i32 i32) (result i32)))
  (import "p3" "[stream-write]" (func $swrite (param i32 i32 i32) (result i32)))
  (import "p3" "[stream-cancel-read]" (func $scread (param i32) (result i32)))
  (import "p3" "[stream-cancel-write]" (func $scwrite (param i32) (result i32)))
  (import "p3" "[stream-drop-readable]" (func $sdropr (param i32)))
  (import "p3" "[stream-drop-writable]" (func $sdropw (param i32)))
  (import "p3" "[stream-forward]" (func $sfwd (param i32 i32)))
  (import "p3" "[future-new]" (func $fnew (result i64)))
  (import "p3" "[future-read]" (func $fread (param i32 i32) (result i32)))
  (import "p3" "[future-read-async]" (func $freada (param i32 i32) (result i32)))
  (import "p3" "[future-write]" (func $fwrite (param i32 i32) (result i32)))
  (import "p3" "[future-drop-readable]" (func $fdropr (param i32)))
  (import "p3" "[future-drop-writable]" (func $fdropw (param i32)))
  (import "p3" "[future-forward]" (func $ffwd (param i32 i32)))
  (import "p3" "[waitable-set-new]" (func $wsnew (result i32)))
  (import "p3" "[waitable-set-wait]" (func $wswait (param i32 i32) (result i32)))
  (import "p3" "[waitable-set-poll]" (func $wspoll (param i32 i32) (result i32)))
  (import "p3" "[waitable-set-drop]" (func $wsdrop (param i32)))
  (import "p3" "[waitable-join]" (func $wjoin (param i32 i32)))
  (import "p3" "[error-context-new]" (func $ecnew (param i32 i32) (result i32)))
  (import "p3" "[error-context-debug-message]" (func $ecmsg (param i32 i32)))
  (import "p3" "[error-context-drop]" (func $ecdrop (param i32)))
  (import "p3" "[backpressure-inc]" (func $binc))
  (import "p3" "[backpressure-dec]" (func $bdec))
  (import "p3" "[thread-yield]" (func $yld (result i32)))
  (import "p3" "[context-get]" (func $cget (result i32)))
  (import "p3" "[context-set]" (func $cset (param i32)))
  (memory (export "memory") 1)

  (global $heap (mut i32) (i32.const 8192))
  (func (export "cabi_realloc") (param $old i32) (param $oldsz i32)
        (param $align i32) (param $newsz i32) (result i32)
    (local $p i32)
    (global.set $heap
      (i32.and
        (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
        (i32.xor (i32.sub (local.get $align) (i32.const 1)) (i32.const -1))))
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $newsz)))
    (local.get $p))

  (func (export "snew") (result i64) call $snew)
  (func (export "sread") (param i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 call $sread)
  (func (export "sreada") (param i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 call $sreada)
  (func (export "swrite") (param i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 call $swrite)
  (func (export "scread") (param i32) (result i32) local.get 0 call $scread)
  (func (export "scwrite") (param i32) (result i32) local.get 0 call $scwrite)
  (func (export "sdropr") (param i32) local.get 0 call $sdropr)
  (func (export "sdropw") (param i32) local.get 0 call $sdropw)
  (func (export "sfwd") (param i32 i32) local.get 0 local.get 1 call $sfwd)

  (func (export "fnew") (result i64) call $fnew)
  (func (export "fread") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $fread)
  (func (export "freada") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $freada)
  (func (export "fwrite") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $fwrite)
  (func (export "fdropr") (param i32) local.get 0 call $fdropr)
  (func (export "fdropw") (param i32) local.get 0 call $fdropw)
  (func (export "ffwd") (param i32 i32) local.get 0 local.get 1 call $ffwd)

  (func (export "wsnew") (result i32) call $wsnew)
  (func (export "wswait") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $wswait)
  (func (export "wspoll") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $wspoll)
  (func (export "wsdrop") (param i32) local.get 0 call $wsdrop)
  (func (export "wjoin") (param i32 i32) local.get 0 local.get 1 call $wjoin)

  (func (export "ecnew") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $ecnew)
  (func (export "ecmsg") (param i32 i32) local.get 0 local.get 1 call $ecmsg)
  (func (export "ecdrop") (param i32) local.get 0 call $ecdrop)

  (func (export "binc") call $binc)
  (func (export "bdec") call $bdec)
  (func (export "yld") (result i32) call $yld)
  (func (export "cget") (result i32) call $cget)
  (func (export "cset") (param i32) local.get 0 call $cset)

  ;; pump: one sync read from $r, then write whatever was read into $w,
  ;; returning the packed read payload
  (func (export "pump") (param $r i32) (param $w i32) (param $buf i32)
        (param $cap i32) (result i32)
    (local $n i32)
    (local.set $n (call $sread (local.get $r) (local.get $buf) (local.get $cap)))
    (if (i32.eq (i32.and (local.get $n) (i32.const 15)) (i32.const 1))
      (then (call $sdropw (local.get $w)) (return (local.get $n))))
    (drop (call $swrite (local.get $w) (local.get $buf)
                        (i32.shr_u (local.get $n) (i32.const 4))))
    (local.get $n))

  ;; async read, then join+poll a waitable set; returns the poll event code
  ;; and stores (p1, p2) at 1024
  (func (export "pread_async") (param $r i32) (param $ptr i32) (param $n i32)
        (param $wset i32) (result i32)
    (drop (call $sreada (local.get $r) (local.get $ptr) (local.get $n)))
    (call $wjoin (local.get $r) (local.get $wset))
    (call $wspoll (local.get $wset) (i32.const 1024)))

  ;; same for futures
  (func (export "fread_async") (param $r i32) (param $ptr i32)
        (param $wset i32) (result i32)
    (drop (call $freada (local.get $r) (local.get $ptr)))
    (call $wjoin (local.get $r) (local.get $wset))
    (call $wspoll (local.get $wset) (i32.const 1024)))
)
"#;

// ---------------------------------------------------------------------------
// Component assembly
// ---------------------------------------------------------------------------

/// Encode a component function type from primitive params/result.
fn ftype(b: &mut wasm_encoder::ComponentBuilder, params: &[PV], result: Option<PV>) -> u32 {
    let (idx, mut enc) = b.type_function(None);
    let ps: Vec<(&str, ComponentValType)> = params
        .iter()
        .map(|p| ("_", ComponentValType::Primitive(*p)))
        .collect();
    enc.params(ps);
    enc.result(result.map(ComponentValType::Primitive));
    idx
}

/// Assemble the full P3 fixture component.
fn p3_component() -> Vec<u8> {
    let guest = wat::parse_str(GUEST).expect("guest wat");
    let mut b = wasm_encoder::ComponentBuilder::default();

    // -- types ---------------------------------------------------------------
    let (t_stream, e) = b.type_defined(None);
    e.stream(Some(ComponentValType::Primitive(PV::U8)));
    let (t_future, e) = b.type_defined(None);
    e.future(Some(ComponentValType::Primitive(PV::U32)));

    let t_snew = ftype(&mut b, &[], Some(PV::U64));
    let t_rw3 = ftype(&mut b, &[PV::U32, PV::U32, PV::U32], Some(PV::U32));
    let t_r2 = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    let t_u32_u32 = ftype(&mut b, &[PV::U32], Some(PV::U32));
    let t_u32_void = ftype(&mut b, &[PV::U32], None);
    let t_2void = ftype(&mut b, &[PV::U32, PV::U32], None);
    let t_void_u32 = ftype(&mut b, &[], Some(PV::U32));
    let t_3u32 = ftype(&mut b, &[PV::U32, PV::U32, PV::U32], Some(PV::U32));
    let t_4u32 = ftype(&mut b, &[PV::U32, PV::U32, PV::U32, PV::U32], Some(PV::U32));
    let t_void = ftype(&mut b, &[], None);

    // -- core module ----------------------------------------------------------
    let m = b.core_module_raw(None, &guest);

    // -- canon builtins ---------------------------------------------------------
    // (memory options are recorded but resolved lazily at call time; the
    // debug-message realloc is resolved by name after instantiation)
    let utf8 = [CanonicalOption::Memory(0), CanonicalOption::UTF8];
    let utf8_async = [
        CanonicalOption::Memory(0),
        CanonicalOption::UTF8,
        CanonicalOption::Async,
    ];

    let canon: Vec<(&'static str, u32)> = vec![
        ("[stream-new]", b.stream_new(t_stream)),
        ("[stream-read]", b.stream_read(t_stream, utf8)),
        ("[stream-read-async]", b.stream_read(t_stream, utf8_async)),
        ("[stream-write]", b.stream_write(t_stream, utf8)),
        (
            "[stream-cancel-read]",
            b.stream_cancel_read(t_stream, false),
        ),
        (
            "[stream-cancel-write]",
            b.stream_cancel_write(t_stream, false),
        ),
        ("[stream-drop-readable]", b.stream_drop_readable(t_stream)),
        ("[stream-drop-writable]", b.stream_drop_writable(t_stream)),
        ("[stream-forward]", b.stream_forward(t_stream)),
        ("[future-new]", b.future_new(t_future)),
        ("[future-read]", b.future_read(t_future, utf8)),
        ("[future-read-async]", b.future_read(t_future, utf8_async)),
        ("[future-write]", b.future_write(t_future, utf8)),
        ("[future-drop-readable]", b.future_drop_readable(t_future)),
        ("[future-drop-writable]", b.future_drop_writable(t_future)),
        ("[future-forward]", b.future_forward(t_future)),
        ("[waitable-set-new]", b.waitable_set_new()),
        ("[waitable-set-wait]", b.waitable_set_wait(0)),
        ("[waitable-set-poll]", b.waitable_set_poll(0)),
        ("[waitable-set-drop]", b.waitable_set_drop()),
        ("[waitable-join]", b.waitable_join()),
        ("[error-context-new]", b.error_context_new(utf8)),
        (
            "[error-context-debug-message]",
            b.error_context_debug_message(utf8),
        ),
        ("[error-context-drop]", b.error_context_drop()),
        ("[backpressure-inc]", b.backpressure_inc()),
        ("[backpressure-dec]", b.backpressure_dec()),
        ("[thread-yield]", b.thread_yield()),
        ("[context-get]", b.context_get(ValType::I32, 0)),
        ("[context-set]", b.context_set(ValType::I32, 0)),
    ];

    // -- wrapper instance + guest instantiation ---------------------------------
    let shim = b.core_instantiate_exports(
        Some("p3-shim"),
        canon
            .iter()
            .map(|(n, f)| (*n, ExportKind::Func, *f))
            .collect::<Vec<_>>(),
    );
    let inst = b.core_instantiate(Some("guest"), m, [("p3", ModuleArg::Instance(shim))]);

    // -- lifts + component exports ------------------------------------------------
    let lifted: &[(&str, u32)] = &[
        ("snew", t_snew),
        ("sread", t_rw3),
        ("sreada", t_rw3),
        ("swrite", t_rw3),
        ("scread", t_u32_u32),
        ("scwrite", t_u32_u32),
        ("sdropr", t_u32_void),
        ("sdropw", t_u32_void),
        ("sfwd", t_2void),
        ("fnew", t_snew),
        ("fread", t_r2),
        ("freada", t_r2),
        ("fwrite", t_r2),
        ("fdropr", t_u32_void),
        ("fdropw", t_u32_void),
        ("ffwd", t_2void),
        ("wsnew", t_void_u32),
        ("wswait", t_r2),
        ("wspoll", t_r2),
        ("wsdrop", t_u32_void),
        ("wjoin", t_2void),
        ("ecnew", t_r2),
        ("ecmsg", t_2void),
        ("ecdrop", t_u32_void),
        ("binc", t_void),
        ("bdec", t_void),
        ("yld", t_void_u32),
        ("cget", t_void_u32),
        ("cset", t_u32_void),
        ("pump", t_4u32),
        ("pread_async", t_4u32),
        ("fread_async", t_3u32),
    ];
    for (name, ty) in lifted {
        let cf = b.core_alias_export(None, inst, name, ExportKind::Func);
        let f = b.lift_func(None, cf, *ty, []);
        b.export(*name, ComponentExportKind::Func, f, None);
    }

    b.finish()
}

/// The async-lift fixture: `answer() -> u32` returns via `task.return` and
/// loops the callback once (YIELD, then EXIT); `seven() -> u32` completes
/// without a callback.
fn async_lift_component() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (import "p3" "[task-return]" (func $tret (param i32)))
  (memory (export "memory") 1)
  (func $entry (result i32) (call $tret (i32.const 42)) (i32.const 1))
  (func $cb (param i32 i32 i32) (result i32) (i32.const 0))
  (func $seven (call $tret (i32.const 7)))
  (export "answer" (func $entry))
  (export "callback" (func $cb))
  (export "seven" (func $seven)))
"#,
    )
    .expect("wat");
    let mut b = wasm_encoder::ComponentBuilder::default();

    let t_ret_u32 = ftype(&mut b, &[], Some(PV::U32));
    let m = b.core_module_raw(None, &guest);
    let c_tret = b.task_return(Some(ComponentValType::Primitive(PV::U32)), []);

    let shim = b.core_instantiate_exports(
        Some("p3-shim"),
        [("[task-return]", ExportKind::Func, c_tret)],
    );
    let inst = b.core_instantiate(Some("guest"), m, [("p3", ModuleArg::Instance(shim))]);

    let answer = b.core_alias_export(None, inst, "answer", ExportKind::Func);
    let cb = b.core_alias_export(None, inst, "callback", ExportKind::Func);
    let seven = b.core_alias_export(None, inst, "seven", ExportKind::Func);

    let f = b.lift_func(
        None,
        answer,
        t_ret_u32,
        [CanonicalOption::Async, CanonicalOption::Callback(cb)],
    );
    b.export("answer", ComponentExportKind::Func, f, None);
    let f = b.lift_func(None, seven, t_ret_u32, [CanonicalOption::Async]);
    b.export("seven", ComponentExportKind::Func, f, None);

    b.finish()
}

/// The async-lower fixture: a host import `add` is lowered with the `async`
/// option (out-pointer + `RETURNED` completion flag) and the guest consumes it.
fn async_lower_component() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (import "" "add" (func $add (param i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "run") (param $a i32) (param $b i32) (result i32)
    (local $r i32)
    (local.set $r (call $add (local.get $a) (local.get $b) (i32.const 2048)))
    (if (i32.ne (local.get $r) (i32.const 2)) (then unreachable))
    (i32.load (i32.const 2048))))
"#,
    )
    .expect("wat");
    let mut b = wasm_encoder::ComponentBuilder::default();

    let t_add = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    let t_run = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));

    let comp_add = b.import("add", wasm_encoder::ComponentTypeRef::Func(t_add));
    let m = b.core_module_raw(None, &guest);
    let lowered = b.lower_func(
        None,
        comp_add,
        [CanonicalOption::Memory(0), CanonicalOption::Async],
    );

    let shim = b.core_instantiate_exports(Some("host-shim"), [("add", ExportKind::Func, lowered)]);
    let inst = b.core_instantiate(Some("guest"), m, [("", ModuleArg::Instance(shim))]);

    let run = b.core_alias_export(None, inst, "run", ExportKind::Func);
    let f = b.lift_func(None, run, t_run, []);
    b.export("run", ComponentExportKind::Func, f, None);

    b.finish()
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Fixture {
    name: &'static str,
    store: Store,
    instance: waclay::ComponentInstance,
}

impl Fixture {
    fn new(name: &'static str, engine: &Engine, binary: &[u8]) -> Fixture {
        let mut store = Store::new(engine, ()).unwrap();
        let component = Component::from_binary(engine, binary)
            .unwrap_or_else(|e| panic!("[{name}] decode failed: {e}"));
        let mut linker = Linker::new();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate failed: {e}"));
        Fixture {
            name,
            store,
            instance,
        }
    }

    fn call(&mut self, name: &str, args: &[Val]) -> Option<Val> {
        self.instance
            .call_func(&mut self.store, name, args)
            .unwrap_or_else(|e| panic!("[{}:{name}] call failed: {e}", self.name))
    }

    fn try_call(&mut self, name: &str, args: &[Val]) -> waclay::Result<Option<Val>> {
        self.instance.call_func(&mut self.store, name, args)
    }

    fn mem_write(&mut self, offset: usize, bytes: &[u8]) {
        let mem = match self.instance.get_core_export(&mut self.store, "memory") {
            Some(Extern::Memory(m)) => m,
            _ => panic!("no memory"),
        };
        self.store.memory_write(&mem, offset, bytes).unwrap();
    }

    fn mem_read(&mut self, offset: usize, len: usize) -> Vec<u8> {
        let mem = match self.instance.get_core_export(&mut self.store, "memory") {
            Some(Extern::Memory(m)) => m,
            _ => panic!("no memory"),
        };
        let mut buf = vec![0u8; len];
        self.store.memory_read(&mem, offset, &mut buf).unwrap();
        buf
    }

    fn u32_word(&mut self, offset: usize) -> u32 {
        u32::from_le_bytes(self.mem_read(offset, 4).try_into().unwrap())
    }
}

fn unpack_pair(v: Option<&Val>) -> (u32, u32) {
    match v {
        Some(Val::U64(p)) => ((*p & 0xffff_ffff) as u32, (*p >> 32) as u32),
        other => panic!("expected u64 handle pair, got {other:?}"),
    }
}

fn u32_of(v: Option<&Val>) -> u32 {
    match v {
        Some(Val::U32(n)) => *n,
        other => panic!("expected u32, got {other:?}"),
    }
}

fn i32_of(v: Option<&Val>) -> i32 {
    match v {
        Some(Val::U32(n)) => *n as i32,
        other => panic!("expected i32, got {other:?}"),
    }
}

const STREAM_TY: u32 = 0;
const FUTURE_TY: u32 = 1;

// ---------------------------------------------------------------------------
// Streams
// ---------------------------------------------------------------------------

#[test]
fn p3_stream_sync_roundtrip_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());

        let (r, w) = unpack_pair(f.call("snew", &[]).as_ref());

        // host writes 5 bytes into guest memory, guest pushes them
        f.mem_write(64, &[10, 20, 30, 40, 50]);
        let packed = u32_of(
            f.call("swrite", &[Val::U32(w), Val::U32(64), Val::U32(5)])
                .as_ref(),
        );
        assert_eq!(
            packed,
            (CopyResult::Completed as u32) | (5 << 4),
            "[{ename}] write payload"
        );

        // guest pulls them back out into another buffer
        let packed = u32_of(
            f.call("sread", &[Val::U32(r), Val::U32(128), Val::U32(5)])
                .as_ref(),
        );
        assert_eq!(packed, (CopyResult::Completed as u32) | (5 << 4));
        assert_eq!(f.mem_read(128, 5), vec![10, 20, 30, 40, 50], "[{ename}]");

        // empty buffer + open writer: sync read must trap (not hang)
        let err = f
            .try_call("sread", &[Val::U32(r), Val::U32(128), Val::U32(5)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("would block"), "[{ename}] {err}");

        // writer dropped: read reports DROPPED
        f.call("sdropw", &[Val::U32(w)]);
        let packed = u32_of(
            f.call("sread", &[Val::U32(r), Val::U32(128), Val::U32(5)])
                .as_ref(),
        );
        assert_eq!(packed, CopyResult::Dropped as u32, "[{ename}]");
        f.call("sdropr", &[Val::U32(r)]);
    }
}

#[test]
fn p3_stream_pump_between_two_streams_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r1, w1) = unpack_pair(f.call("snew", &[]).as_ref());
        let (r2, w2) = unpack_pair(f.call("snew", &[]).as_ref());

        // host -> stream1, guest pumps stream1 -> stream2
        f.instance
            .stream_feed(
                &mut f.store,
                STREAM_TY,
                w1,
                vec![Val::U8(1), Val::U8(2), Val::U8(3)],
            )
            .unwrap();
        let packed = u32_of(
            f.call(
                "pump",
                &[Val::U32(r1), Val::U32(w2), Val::U32(64), Val::U32(8)],
            )
            .as_ref(),
        );
        assert_eq!(
            packed,
            (CopyResult::Completed as u32) | (3 << 4),
            "[{ename}]"
        );

        let (vals, done) = f.instance.stream_drain(STREAM_TY, r2, 10).unwrap();
        assert!(!done);
        assert_eq!(
            vals,
            vec![Val::U8(1), Val::U8(2), Val::U8(3)],
            "[{ename}] drained"
        );

        // EOF propagation: closing stream1's writer makes pump report DROPPED
        // and close stream2's writer
        f.instance.stream_close_writable(STREAM_TY, w1).unwrap();
        let packed = u32_of(
            f.call(
                "pump",
                &[Val::U32(r1), Val::U32(w2), Val::U32(64), Val::U32(8)],
            )
            .as_ref(),
        );
        assert_eq!(packed, CopyResult::Dropped as u32, "[{ename}]");
        let (vals, done) = f.instance.stream_drain(STREAM_TY, r2, 10).unwrap();
        assert!(vals.is_empty() && done, "[{ename}] eof after drop");
    }
}

#[test]
fn p3_stream_async_read_waitable_set_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r, w) = unpack_pair(f.call("snew", &[]).as_ref());
        let wset = u32_of(f.call("wsnew", &[]).as_ref());

        // async read returns BLOCKED and registers the pending copy; the
        // first poll sees no events yet
        let ev = u32_of(
            f.call(
                "pread_async",
                &[Val::U32(r), Val::U32(64), Val::U32(4), Val::U32(wset)],
            )
            .as_ref(),
        );
        assert_eq!(ev, EventCode::None.raw(), "[{ename}] first poll empty");

        // host feeds the stream: values land in the guest buffer, event fires
        let delivered = f
            .instance
            .stream_feed(
                &mut f.store,
                STREAM_TY,
                w,
                vec![Val::U8(7), Val::U8(8), Val::U8(9), Val::U8(10)],
            )
            .unwrap();
        assert_eq!(delivered, 4, "[{ename}]");

        let ev = u32_of(f.call("wspoll", &[Val::U32(wset), Val::U32(1024)]).as_ref());
        assert_eq!(ev, EventCode::StreamRead.raw(), "[{ename}] event code");
        assert_eq!(f.u32_word(1024), r, "[{ename}] event p1 = end handle");
        assert_eq!(
            f.u32_word(1028),
            (CopyResult::Completed as u32) | (4 << 4),
            "[{ename}] event p2 = packed copy result"
        );
        assert_eq!(
            f.mem_read(64, 4),
            vec![7, 8, 9, 10],
            "[{ename}] guest buffer"
        );

        // event consumed: next poll is empty again
        let ev = u32_of(f.call("wspoll", &[Val::U32(wset), Val::U32(1024)]).as_ref());
        assert_eq!(ev, EventCode::None.raw(), "[{ename}]");
    }
}

#[test]
fn p3_stream_cancel_read_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r, _w) = unpack_pair(f.call("snew", &[]).as_ref());

        // async read -> BLOCKED
        let packed = i32_of(
            f.call("sreada", &[Val::U32(r), Val::U32(64), Val::U32(4)])
                .as_ref(),
        );
        assert_eq!(packed, waclay::BLOCKED, "[{ename}]");

        // cancel -> CANCELLED payload
        let packed = u32_of(f.call("scread", &[Val::U32(r)]).as_ref());
        assert_eq!(packed, CopyResult::Cancelled as u32, "[{ename}]");

        // the end is Idle again: a sync read traps "would block" (not
        // "already in progress"), proving the state transition
        let err = f
            .try_call("sread", &[Val::U32(r), Val::U32(64), Val::U32(4)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("would block"), "[{ename}] {err}");

        // cancelling with nothing in progress traps
        let err = f
            .try_call("scread", &[Val::U32(r)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("in-progress"), "[{ename}] {err}");
    }
}

#[test]
fn p3_stream_forward_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r1, w1) = unpack_pair(f.call("snew", &[]).as_ref());
        let (r2, w2) = unpack_pair(f.call("snew", &[]).as_ref());

        f.instance
            .stream_feed(&mut f.store, STREAM_TY, w1, vec![Val::U8(42), Val::U8(43)])
            .unwrap();
        f.call("sfwd", &[Val::U32(r1), Val::U32(w2)]);

        // data moved to the spliced pipe and the old handles are gone
        let (vals, _done) = f.instance.stream_drain(STREAM_TY, r2, 10).unwrap();
        assert_eq!(vals, vec![Val::U8(42), Val::U8(43)], "[{ename}]");
        assert!(
            f.try_call("sread", &[Val::U32(r1), Val::U32(64), Val::U32(1)])
                .is_err(),
            "[{ename}] old readable handle must be invalid"
        );
    }
}

// ---------------------------------------------------------------------------
// Futures
// ---------------------------------------------------------------------------

#[test]
fn p3_future_roundtrip_and_dropped_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r, w) = unpack_pair(f.call("fnew", &[]).as_ref());

        f.mem_write(64, &12345u32.to_le_bytes());
        let packed = u32_of(f.call("fwrite", &[Val::U32(w), Val::U32(64)]).as_ref());
        assert_eq!(packed, CopyResult::Completed as u32, "[{ename}]");

        let packed = u32_of(f.call("fread", &[Val::U32(r), Val::U32(128)]).as_ref());
        assert_eq!(packed, CopyResult::Completed as u32, "[{ename}]");
        assert_eq!(f.u32_word(128), 12345, "[{ename}]");

        // the future end is DONE: further reads trap
        let err = f
            .try_call("fread", &[Val::U32(r), Val::U32(128)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("done"), "[{ename}] {err}");

        // writing into a future whose reader was dropped -> DROPPED
        let (r2, w2) = unpack_pair(f.call("fnew", &[]).as_ref());
        f.call("fdropr", &[Val::U32(r2)]);
        let packed = u32_of(f.call("fwrite", &[Val::U32(w2), Val::U32(64)]).as_ref());
        assert_eq!(packed, CopyResult::Dropped as u32, "[{ename}]");
    }
}

#[test]
fn p3_future_async_read_host_delivery_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r, w) = unpack_pair(f.call("fnew", &[]).as_ref());
        let wset = u32_of(f.call("wsnew", &[]).as_ref());

        let ev = u32_of(
            f.call("fread_async", &[Val::U32(r), Val::U32(64), Val::U32(wset)])
                .as_ref(),
        );
        assert_eq!(ev, EventCode::None.raw(), "[{ename}]");

        // host delivers the value straight into the pending guest read
        f.instance
            .future_set(&mut f.store, FUTURE_TY, w, Val::U32(0xbeef))
            .unwrap();

        let ev = u32_of(f.call("wspoll", &[Val::U32(wset), Val::U32(1024)]).as_ref());
        assert_eq!(ev, EventCode::FutureRead.raw(), "[{ename}]");
        assert_eq!(f.u32_word(1024), r, "[{ename}] p1");
        assert_eq!(
            f.u32_word(1028),
            CopyResult::Completed as u32,
            "[{ename}] p2"
        );
        assert_eq!(f.u32_word(64), 0xbeef, "[{ename}] guest buffer");
    }
}

// ---------------------------------------------------------------------------
// Waitable sets
// ---------------------------------------------------------------------------

#[test]
fn p3_waitable_set_membership_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());
        let (r, w) = unpack_pair(f.call("snew", &[]).as_ref());
        let wset = u32_of(f.call("wsnew", &[]).as_ref());

        f.call("wjoin", &[Val::U32(r), Val::U32(wset)]);

        // dropping a non-empty set traps
        let err = f
            .try_call("wsdrop", &[Val::U32(wset)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("members"), "[{ename}] {err}");

        // wait with nothing ready traps with the sync-host diagnostic
        let err = f
            .try_call("wswait", &[Val::U32(wset), Val::U32(1024)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("would block"), "[{ename}] {err}");

        // detach + drop
        f.call("wjoin", &[Val::U32(r), Val::U32(0)]);
        f.call("wsdrop", &[Val::U32(wset)]);

        // dropping a stream end detaches it from its set automatically
        let wset2 = u32_of(f.call("wsnew", &[]).as_ref());
        f.call("wjoin", &[Val::U32(w), Val::U32(wset2)]);
        f.call("sdropw", &[Val::U32(w)]);
        f.call("wsdrop", &[Val::U32(wset2)]);
    }
}

// ---------------------------------------------------------------------------
// Error contexts
// ---------------------------------------------------------------------------

#[test]
fn p3_error_context_roundtrip_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());

        f.mem_write(64, b"boom");
        let h = u32_of(f.call("ecnew", &[Val::U32(64), Val::U32(4)]).as_ref());
        assert!(h > 0, "[{ename}]");

        // debug-message writes an 8-byte (ptr, units) pair at 512 via realloc
        f.call("ecmsg", &[Val::U32(h), Val::U32(512)]);
        let sptr = f.u32_word(512);
        let units = f.u32_word(516);
        assert_eq!(units, 4, "[{ename}] units");
        assert_eq!(f.mem_read(sptr as usize, 4), b"boom".to_vec(), "[{ename}]");

        f.call("ecdrop", &[Val::U32(h)]);
        let err = f
            .try_call("ecdrop", &[Val::U32(h)])
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid error-context"), "[{ename}] {err}");
    }
}

// ---------------------------------------------------------------------------
// Task model: backpressure, yield, context, async lift/lower
// ---------------------------------------------------------------------------

#[test]
fn p3_backpressure_yield_context_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &p3_component());

        f.call("binc", &[]);
        f.call("binc", &[]);
        assert_eq!(f.instance.async_state().lock().unwrap().backpressure(), 2);
        f.call("bdec", &[]);
        f.call("bdec", &[]);
        assert_eq!(f.instance.async_state().lock().unwrap().backpressure(), 0);
        // underflow traps
        assert!(f.try_call("bdec", &[]).is_err());

        assert_eq!(u32_of(f.call("yld", &[]).as_ref()), 0, "[{ename}]");

        f.call("cset", &[Val::U32(77)]);
        assert_eq!(u32_of(f.call("cget", &[]).as_ref()), 77, "[{ename}]");
    }
}

#[test]
fn p3_async_lift_task_return_all_engines() {
    for (ename, engine) in engines() {
        let mut f = Fixture::new(ename, &engine, &async_lift_component());

        // with callback: entry returns YIELD(1), host runs the callback once
        // (EXIT), then reads the task.return value
        let v = f.call("answer", &[]);
        assert_eq!(v, Some(Val::U32(42)), "[{ename}]");

        // without callback: entry returns no core results; the value comes
        // from task.return
        let v = f.call("seven", &[]);
        assert_eq!(v, Some(Val::U32(7)), "[{ename}]");
    }
}

#[test]
fn p3_async_lower_outptr_returned_all_engines() {
    for (ename, engine) in engines() {
        // a real host implementation: the guest sees RETURNED(2) and reads
        // the sum from its out-pointer
        let mut store = Store::new(&engine, ()).unwrap();
        let component = Component::from_binary(&engine, &async_lower_component()).unwrap();
        let mut linker = Linker::new();
        linker.func(None, "add", |_ctx, args: &[Val], out: &mut Vec<Val>| {
            let a = match args[0] {
                Val::U32(x) => x,
                _ => return Err(waclay::Error::value("expected u32")),
            };
            let b = match args[1] {
                Val::U32(x) => x,
                _ => return Err(waclay::Error::value("expected u32")),
            };
            out.push(Val::U32(a.wrapping_add(b)));
            Ok(())
        });
        let instance = linker.instantiate(&mut store, &component).unwrap();
        let v = instance
            .call_func(&mut store, "run", &[Val::U32(20), Val::U32(22)])
            .unwrap();
        assert_eq!(v, Some(Val::U32(42)), "[{ename}]");
    }
}
