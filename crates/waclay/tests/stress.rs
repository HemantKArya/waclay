//! Stress tests: the production-readiness gauntlet. Everything here is
//! deterministic (seeded) so failures are reproducible, and everything must
//! *error cleanly* or *succeed correctly* — never panic, hang or corrupt.

use waclay::abi::layout::{CoreTy, FuncLayout};
use waclay::abi::{lift, load, lower, store, FlatCursor, FlatSink, MemAccess, StringEncoding};
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport};
use waclay::component::types::{FuncSig, Primitive, TypeDef, TypeRef, Val};
use waclay::{Component, Engine, Linker, Store};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Minimal core-module support block: heap allocator + memory.
const SUPPORT: &str = r#"
    (global $heap (mut i32) (i32.const 4096))
    (func $realloc (export "cabi_realloc")
        (param $old i32) (param $oldsz i32) (param $align i32) (param $newsz i32)
        (result i32)
        (local $p i32)
        (global.set $heap
            (i32.and
                (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
                (i32.xor (i32.sub (local.get $align) (i32.const 1)) (i32.const -1))))
        (local.set $p (global.get $heap))
        (global.set $heap (i32.add (global.get $heap) (local.get $newsz)))
        (local.get $p))
    (memory (export "memory") 5)
"#;

fn string_echo_component() -> ComponentPlan {
    let wasm = wat::parse_str(format!(
        r#"(module {SUPPORT}
            (func (export "echo") (param $p i32) (param $l i32) (result i32)
                (local $np i32) (local $sp i32)
                (local.set $np (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 1) (local.get $l)))
                (memory.copy (local.get $np) (local.get $p) (local.get $l))
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (i32.store (local.get $sp) (local.get $np))
                (i32.store offset=4 (local.get $sp) (local.get $l))
                (local.get $sp))
            (func (export "cabi_post_echo") (param $retptr i32) (nop)))
        "#
    ))
    .unwrap();

    ComponentPlan {
        core_module: wasm,
        types: vec![TypeDef::Flags(vec!["\u{0}resource".into()])],
        lifts: vec![LiftedExport {
            async_lift: false,
            callback: None,
            key: "echo".into(),
            func_name: "echo".into(),
            sig: FuncSig {
                params: vec![("s".into(), TypeRef::Primitive(Primitive::String))],
                result: Some(TypeRef::Primitive(Primitive::String)),
            },
            core_func: CoreFuncRef::Export {
                instance: 0,
                name: "echo".into(),
            },
            post_return: Some(CoreFuncRef::Export {
                instance: 0,
                name: "cabi_post_echo".into(),
            }),
            realloc: Some(CoreFuncRef::Export {
                instance: 0,
                name: "cabi_realloc".into(),
            }),
            string_enc: StringEncoding::Utf8,
        }],
        ..Default::default()
    }
}

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
// 1. Decoder fuzz: mutated binaries must never panic
// ---------------------------------------------------------------------------

/// Deterministic xorshift PRNG (no external deps, stable across runs).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn stress_decoder_fuzz_never_panics() {
    // seed component: a real component binary with canonical ops
    let base = include_bytes!("p3_component.bin");

    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    let mut rng = Lcg(0x9E3779B97F4A7C15);
    let mut decoded_ok = 0;
    let mut decoded_err = 0;

    for round in 0..2000u32 {
        let mut bytes = base.to_vec();
        let mutations = 1 + (rng.next() % 8) as usize;
        for _ in 0..mutations {
            match rng.next() % 4 {
                0 => {
                    // flip a byte
                    let i = (rng.next() as usize) % bytes.len();
                    bytes[i] ^= 1 << (rng.next() % 8);
                }
                1 => {
                    // set a byte to a random value
                    let i = (rng.next() as usize) % bytes.len();
                    bytes[i] = (rng.next() & 0xff) as u8;
                }
                2 => {
                    // truncate
                    let keep = (rng.next() as usize) % bytes.len();
                    bytes.truncate(keep.max(8));
                }
                _ => {
                    // splice: duplicate a chunk
                    if bytes.len() > 32 {
                        let from = (rng.next() as usize) % (bytes.len() - 16);
                        let len = 1 + (rng.next() as usize) % 16;
                        let end = (from + len).min(bytes.len());
                        let chunk: Vec<u8> = bytes[from..end].to_vec();
                        let at = (rng.next() as usize) % bytes.len();
                        bytes.splice(at..at, chunk);
                    }
                }
            }
        }
        match Component::from_binary(&engine, &bytes) {
            Ok(_) => decoded_ok += 1,
            Err(_) => decoded_err += 1,
        }
        let _ = round;
    }
    // both outcomes are acceptable; panics are not (the test harness catches them)
    assert!(
        decoded_err > 0,
        "fuzzing should reject most mutations (ok={decoded_ok})"
    );
}

// ---------------------------------------------------------------------------
// 2. Stream throughput: 1 MiB through the guest pump in 4 KiB chunks
// ---------------------------------------------------------------------------

const P3_COMPONENT: &[u8] = include_bytes!("p3_component.bin");

fn p3_fixture(ename: &str, engine: &Engine) -> (Store, waclay::ComponentInstance) {
    let mut store = Store::new(engine, ()).unwrap();
    let component = Component::from_binary(engine, P3_COMPONENT)
        .unwrap_or_else(|e| panic!("[{ename}] decode: {e}"));
    let mut linker = Linker::new();
    let instance = linker
        .instantiate(&mut store, &component)
        .unwrap_or_else(|e| panic!("[{ename}] instantiate: {e}"));
    (store, instance)
}

fn unpack_pair(v: Option<&Val>) -> (u32, u32) {
    match v {
        Some(Val::U64(p)) => ((*p & 0xffff_ffff) as u32, (*p >> 32) as u32),
        other => panic!("expected u64 pair, got {other:?}"),
    }
}

fn u32_of(v: Option<&Val>) -> u32 {
    match v {
        Some(Val::U32(n)) => *n,
        other => panic!("expected u32, got {other:?}"),
    }
}

#[test]
fn stress_stream_throughput_1mib_pumped_through_guest() {
    for (ename, engine) in engines() {
        let (mut store, instance) = p3_fixture(ename, &engine);
        let (r1, w1) = unpack_pair(
            instance
                .call_func(&mut store, "snew", &[])
                .unwrap()
                .as_ref(),
        );
        let (r2, w2) = unpack_pair(
            instance
                .call_func(&mut store, "snew", &[])
                .unwrap()
                .as_ref(),
        );

        const CHUNK: usize = 4096;
        const CHUNKS: usize = 256; // 1 MiB total
        let mut expected_byte: u8 = 0;

        for c in 0..CHUNKS {
            let chunk: Vec<Val> = (0..CHUNK)
                .map(|i| {
                    let b = ((c as u8).wrapping_mul(31)).wrapping_add(i as u8);
                    Val::U8(b)
                })
                .collect();
            instance
                .stream_feed(&mut store, 0, w1, chunk)
                .unwrap_or_else(|e| panic!("[{ename}] feed {c}: {e}"));
            let packed = u32_of(
                instance
                    .call_func(
                        &mut store,
                        "pump",
                        &[
                            Val::U32(r1),
                            Val::U32(w2),
                            Val::U32(64),
                            Val::U32(CHUNK as u32),
                        ],
                    )
                    .unwrap()
                    .as_ref(),
            );
            assert_eq!(packed, (CHUNK as u32) << 4, "[{ename}] chunk {c}");
            let (vals, done) = instance.stream_drain(0, r2, CHUNK as u32).unwrap();
            assert!(!done);
            assert_eq!(vals.len(), CHUNK);
            for (i, v) in vals.iter().enumerate() {
                let b = ((c as u8).wrapping_mul(31)).wrapping_add(i as u8);
                assert_eq!(*v, Val::U8(b), "[{ename}] chunk {c} byte {i}");
            }
            expected_byte = expected_byte.wrapping_add(0); // keep loop shape
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Handle storm: 10k streams + 5k futures lifecycle
// ---------------------------------------------------------------------------

#[test]
fn stress_handle_storm_streams_and_futures() {
    let (ename, engine) = ("wasmi", Engine::new(waclay_wasmi::WasmiEngine::default()));
    let (mut store, instance) = p3_fixture(ename, &engine);

    // 10k stream pairs: feed, drain, close — all from the host side
    for i in 0..10_000u32 {
        let (r, w) = unpack_pair(
            instance
                .call_func(&mut store, "snew", &[])
                .unwrap()
                .as_ref(),
        );
        instance
            .stream_feed(
                &mut store,
                0,
                w,
                vec![Val::U8(i as u8), Val::U8((i >> 8) as u8)],
            )
            .unwrap();
        let (vals, done) = instance.stream_drain(0, r, 10).unwrap();
        assert_eq!(vals.len(), 2);
        assert!(!done);
        instance.stream_close_writable(0, w).unwrap();
        let (_, done) = instance.stream_drain(0, r, 10).unwrap();
        assert!(done);
        // drop the readable end through the guest pass-through too
        instance
            .call_func(&mut store, "sdropr", &[Val::U32(r)])
            .unwrap();
    }

    // 5k future pairs: set, take, drop both ends
    for i in 0..5_000u32 {
        let (r, w) = unpack_pair(
            instance
                .call_func(&mut store, "fnew", &[])
                .unwrap()
                .as_ref(),
        );
        instance.future_set(&mut store, 1, w, Val::U32(i)).unwrap();
        assert_eq!(instance.future_take(1, r).unwrap(), Some(Val::U32(i)));
        instance
            .call_func(&mut store, "fdropr", &[Val::U32(r)])
            .unwrap();
        instance
            .call_func(&mut store, "fdropw", &[Val::U32(w)])
            .unwrap();
    }
    // every handle dropped: tables are empty again
    assert_eq!(instance.async_state().lock().unwrap().live_ends(), 0);
    assert_eq!(instance.async_state().lock().unwrap().live_sets(), 0);
}

// ---------------------------------------------------------------------------
// 4. Instance churn: 200 full instantiate -> call -> drop cycles
// ---------------------------------------------------------------------------

#[test]
fn stress_instance_churn_all_engines() {
    for (ename, engine) in engines() {
        let plan = string_echo_component();
        for i in 0..200u32 {
            let component = Component::from_plan(&engine, plan.clone()).unwrap();
            let mut store = Store::new(&engine, ()).unwrap();
            let mut linker = Linker::new();
            let instance = linker.instantiate(&mut store, &component).unwrap();
            let echo = instance
                .typed_func::<(String,), String>("echo")
                .unwrap_or_else(|e| panic!("[{ename} #{i}] {e}"));
            let msg = format!("churn-{i}-payload");
            let out = echo.call(&mut store, (msg.clone(),)).unwrap();
            assert_eq!(out, msg, "[{ename} #{i}]");
            drop(instance);
            drop(store);
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Big values: 100 KiB string echo, repeated
// ---------------------------------------------------------------------------

#[test]
fn stress_string_echo_100kib_all_engines() {
    for (ename, engine) in engines() {
        for round in 0..10u32 {
            // fresh instance per round: the guest bump allocator never frees,
            // so long-lived instances would exhaust linear memory
            let component = Component::from_plan(&engine, string_echo_component()).unwrap();
            let mut store = Store::new(&engine, ()).unwrap();
            let mut linker = Linker::new();
            let instance = linker.instantiate(&mut store, &component).unwrap();
            let echo = instance.typed_func::<(String,), String>("echo").unwrap();

            let payload: String = "abcdefghij".repeat(10 * 1024) + &round.to_string(); // 100 KiB+
            let out = echo.call(&mut store, (payload.clone(),)).unwrap();
            assert_eq!(out.len(), payload.len(), "[{ename} round {round}]");
            assert_eq!(out, payload, "[{ename} round {round}]");
        }
    }
}

// ---------------------------------------------------------------------------
// 6. Ten thousand lowered host calls (async-lowered import path)
// ---------------------------------------------------------------------------

#[test]
fn stress_ten_thousand_host_calls() {
    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    let component = Component::from_binary(&engine, &async_lower_binary()).unwrap();
    let mut store = Store::new(&engine, ()).unwrap();
    let mut linker = Linker::new();
    linker.func(None, "add", |_ctx, args: &[Val], out: &mut Vec<Val>| {
        match (&args[0], &args[1]) {
            (Val::U32(a), Val::U32(b)) => out.push(Val::U32(a.wrapping_add(*b))),
            _ => return Err(waclay::Error::value("expected u32 pair")),
        }
        Ok(())
    });
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let mut acc: u64 = 0;
    for i in 0..10_000u32 {
        let v = instance
            .call_func(&mut store, "run", &[Val::U32(i), Val::U32(1)])
            .unwrap();
        acc += u32_of(v.as_ref()) as u64;
    }
    // sum of (i + 1) for i in 0..10_000 = 50_005_000
    assert_eq!(acc, 50_005_000);
}

/// The async-lower fixture from p3_async (rebuilt here to keep stress tests
/// self-contained).
fn async_lower_binary() -> Vec<u8> {
    use wasm_encoder::{
        CanonicalOption, ComponentExportKind, ComponentTypeRef, ComponentValType, ExportKind,
        ModuleArg, PrimitiveValType as PV,
    };
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
    .unwrap();
    let mut b = wasm_encoder::ComponentBuilder::default();
    let (t_add, mut e) = b.type_function(None);
    e.params([
        ("a", ComponentValType::Primitive(PV::U32)),
        ("b", ComponentValType::Primitive(PV::U32)),
    ]);
    e.result(Some(ComponentValType::Primitive(PV::U32)));
    let (t_run, mut e) = b.type_function(None);
    e.params([
        ("a", ComponentValType::Primitive(PV::U32)),
        ("b", ComponentValType::Primitive(PV::U32)),
    ]);
    e.result(Some(ComponentValType::Primitive(PV::U32)));

    let comp_add = b.import("add", ComponentTypeRef::Func(t_add));
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
// 7. Deep type nesting through the Canonical ABI (host-side roundtrip)
// ---------------------------------------------------------------------------

/// An in-memory MemAccess for pure-ABI testing.
struct BufMem(Vec<u8>);

impl MemAccess for BufMem {
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> waclay::Result<()> {
        buf.copy_from_slice(self.0.get(offset..offset + buf.len()).ok_or_else(|| {
            waclay::Error::trap(format!(
                "memory out of bounds: offset={offset} len={} size={}",
                buf.len(),
                self.0.len()
            ))
        })?);
        Ok(())
    }
    fn write(&mut self, offset: usize, buf: &[u8]) -> waclay::Result<()> {
        let size = self.0.len();
        let dst = self.0.get_mut(offset..offset + buf.len()).ok_or_else(|| {
            waclay::Error::trap(format!(
                "memory out of bounds: offset={offset} len={} size={size}",
                buf.len()
            ))
        })?;
        dst.copy_from_slice(buf);
        Ok(())
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn realloc(
        &mut self,
        _old: u32,
        _old_size: u64,
        _align: u64,
        new_size: u64,
    ) -> waclay::Result<u32> {
        let p = self.0.len() as u32;
        self.0.resize(self.0.len() + new_size as usize, 0);
        Ok(p)
    }
}

#[test]
fn stress_deep_type_nesting_abi_roundtrip() {
    // 24 levels of option/result nesting over tuple<u32, string>
    let depth = 24u32;
    let mut types: Vec<TypeDef> = Vec::new();
    for d in 0..=depth {
        types.push(match d {
            0 => TypeDef::Tuple(vec![
                TypeRef::Primitive(Primitive::U32),
                TypeRef::Primitive(Primitive::String),
            ]),
            d if d % 3 == 0 => TypeDef::Option(TypeRef::Def(d - 1)),
            d if d % 3 == 1 => TypeDef::Result {
                ok: Some(TypeRef::Def(d - 1)),
                err: Some(TypeRef::Primitive(Primitive::String)),
            },
            _ => TypeDef::Option(TypeRef::Def(d - 1)),
        });
    }
    let top = TypeRef::Def(depth);

    let layout = FuncLayout::new(&[("v".to_string(), top)], None, &types);
    // base tuple<u32, string> = 3 flats; every option/result level adds a
    // discriminant flat: 3 + 24 = 27 -> indirect params (> MAX_FLAT_PARAMS)
    assert_eq!(layout.param_flat.len(), 3 + depth as usize);
    assert!(layout.params_indirect);
    assert!(layout
        .param_flat
        .iter()
        .all(|t| matches!(t, CoreTy::I32 | CoreTy::I64)));

    // roundtrip a deep value through flat + memory space
    let val = build_nested(depth, 42, "hello deep");
    let mut mem = BufMem(vec![0u8; 1 << 20]);
    let mut sink = FlatSink::default();
    lower(
        &top,
        &types,
        &val,
        &mut sink,
        &mut mem,
        StringEncoding::Utf8,
    )
    .unwrap();
    let mut cur = FlatCursor::new(sink.vals.clone());
    let back = lift(&top, &types, &mut cur, &mut mem, StringEncoding::Utf8).unwrap();
    assert_eq!(back, val);

    // memory-representation roundtrip too
    store(&top, &types, &val, 0, &mut mem, StringEncoding::Utf8).unwrap();
    let back = load(&top, &types, 0, &mut mem, StringEncoding::Utf8).unwrap();
    assert_eq!(back, val);
}

fn build_nested(depth: u32, seed: u32, s: &str) -> Val {
    let base = Val::Tuple(vec![Val::U32(seed), Val::String(s.to_string())]);
    let mut acc = base;
    for d in 1..=depth {
        acc = match d % 3 {
            0 => Val::Option(Some(Box::new(acc))),
            1 => Val::Result(Ok(Some(Box::new(acc)))),
            _ => Val::Option(Some(Box::new(acc))),
        };
    }
    acc
}

// ---------------------------------------------------------------------------
// 8. Threads: engines and stores are Send; parallel independent runtimes
// ---------------------------------------------------------------------------

#[test]
fn stress_parallel_threaded_instances() {
    let handles: Vec<_> = (0..4)
        .map(|t| {
            std::thread::spawn(move || {
                let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
                let component = Component::from_plan(&engine, string_echo_component()).unwrap();
                for i in 0..25u32 {
                    let mut store = Store::new(&engine, ()).unwrap();
                    let mut linker = Linker::new();
                    let instance = linker.instantiate(&mut store, &component).unwrap();
                    let echo = instance.typed_func::<(String,), String>("echo").unwrap();
                    let msg = format!("t{t}-i{i}");
                    let out = echo.call(&mut store, (msg.clone(),)).unwrap();
                    assert_eq!(out, msg);
                }
                t
            })
        })
        .collect();
    for h in handles {
        h.join().expect("thread must not panic");
    }
}

// ---------------------------------------------------------------------------
// 9. Memory storm: 100k host <-> guest memory roundtrips (core-only module)
// ---------------------------------------------------------------------------

#[test]
fn stress_memory_roundtrip_100k() {
    for (ename, engine) in engines() {
        let wasm = wat::parse_str(format!(
            r#"(module {SUPPORT}
                (func (export "sum") (param $p i32) (param $n i32) (result i32)
                    (local $acc i32) (local $i i32)
                    (block $done
                        (loop $l
                            (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
                            (local.set $acc (i32.add (local.get $acc)
                                (i32.load8_u (i32.add (local.get $p) (local.get $i)))))
                            (local.set $i (i32.add (local.get $i) (i32.const 1)))
                            (br $l)))
                    (local.get $acc)))
            "#
        ))
        .unwrap();
        let component = Component::from_core_module(&engine, &wasm).unwrap();
        let mut store = Store::new(&engine, ()).unwrap();
        let mut linker = Linker::new();
        let instance = linker.instantiate(&mut store, &component).unwrap();
        let mem = match instance.get_core_export(&mut store, "memory") {
            Some(waclay::Extern::Memory(m)) => m,
            _ => panic!("[{ename}] no memory"),
        };

        let mut acc: u64 = 0;
        for i in 0..100_000u32 {
            let byte = (i & 0xff) as u8;
            store.memory_write(&mem, 0, &[byte]).unwrap();
            let mut out = [waclay::engine::Val::I32(0)];
            instance
                .call_core(
                    &mut store,
                    "sum",
                    &[waclay::engine::Val::I32(0), waclay::engine::Val::I32(1)],
                    &mut out,
                )
                .unwrap();
            acc += out[0].i32().unwrap() as u32 as u64;
            assert_eq!(
                out[0].i32().unwrap() as u32 as u64,
                byte as u64,
                "[{ename} #{i}]"
            );
        }
        assert!(acc > 0);
    }
}
