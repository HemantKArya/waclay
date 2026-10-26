//! Criterion benchmarks: waclay's Canonical-ABI call overhead vs the raw
//! engines, plus compute (fib) and string-echo through the component path.
//!
//! Run with:
//! ```sh
//! cargo bench -p waclay
//! ```
//! The output is summarized in `docs/BENCHMARKS.md`.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use waclay::abi::StringEncoding;
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport};
use waclay::component::types::{FuncSig, Primitive, TypeRef};
use waclay::{Component, Engine, Linker, Store, Val};

// ---------------------------------------------------------------------------
// Shared fixture: guest module with add/fib/echo exports + cabi_realloc
// ---------------------------------------------------------------------------

const SUPPORT: &str = r#"
    (global $heap (mut i32) (i32.const 4096))
    ;; bump allocator with wraparound (benchmarks allocate repeatedly)
    (func $realloc (export "cabi_realloc")
        (param $old i32) (param $old_size i32) (param $align i32) (param $new i32) (result i32)
        (local $ret i32)
        (if (i32.gt_u (i32.add (global.get $heap) (local.get $new)) (i32.const 65000))
            (then (global.set $heap (i32.const 4096))))
        (global.set $heap
            (i32.mul
                (i32.div_u (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
                           (local.get $align))
                (local.get $align)))
        (local.set $ret (global.get $heap))
        (global.set $heap (i32.add (global.get $heap) (local.get $new)))
        (if (i32.ne (local.get $old) (i32.const 0))
            (then (memory.copy (local.get $ret) (local.get $old) (local.get $old_size))))
        (local.get $ret))
    (memory (export "memory") 1)
"#;

fn bench_wasm() -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
            {SUPPORT}
            (func (export "add") (param i32 i32) (result i32)
                (i32.add (local.get 0) (local.get 1)))
            (func $fib (export "fib") (param $n i32) (result i32)
                (if (result i32) (i32.lt_u (local.get $n) (i32.const 2))
                    (then (local.get $n))
                    (else
                        (i32.add
                            (call $fib (i32.sub (local.get $n) (i32.const 1)))
                            (call $fib (i32.sub (local.get $n) (i32.const 2)))))))
            ;; echo: copy input bytes to a fresh area, return (ptr,len) via retptr
            (func (export "echo") (param $p i32) (param $l i32) (result i32)
                (local $sp i32) (local $dst i32)
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (local.set $dst (call $realloc (i32.const 0) (i32.const 0)
                                               (i32.const 1) (local.get $l)))
                (memory.copy (local.get $dst) (local.get $p) (local.get $l))
                (i32.store (local.get $sp) (local.get $dst))
                (i32.store offset=4 (local.get $sp) (local.get $l))
                (local.get $sp))
        )
        "#
    ))
    .unwrap()
}

fn bench_plan() -> ComponentPlan {
    let wasm = bench_wasm();
    let realloc = CoreFuncRef::Export {
        instance: 0,
        name: "cabi_realloc".into(),
    };
    ComponentPlan {
        core_module: wasm,
        types: vec![],
        imports: vec![],
        lowers: vec![],
        lifts: vec![
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "add".into(),
                func_name: "add".into(),
                sig: FuncSig {
                    params: vec![
                        ("a".into(), TypeRef::Primitive(Primitive::U32)),
                        ("b".into(), TypeRef::Primitive(Primitive::U32)),
                    ],
                    result: Some(TypeRef::Primitive(Primitive::U32)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "add".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
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
                post_return: None,
                realloc: Some(realloc),
                string_enc: StringEncoding::Utf8,
            },
        ],
        ..Default::default()
    }
}

fn component_engine(name: &str) -> Engine {
    match name {
        "wasmi" => Engine::new(waclay_wasmi::WasmiEngine::default()),
        "wasmtime" => Engine::new(waclay_wasmtime::WasmtimeEngine::default()),
        #[cfg(not(windows))]
        _ => Engine::new(waclay_wasmer::WasmerEngine::default()),
        #[cfg(windows)]
        other => panic!("engine `{other}` is not available on Windows"),
    }
}

// ---------------------------------------------------------------------------
// Raw-engine baselines (no waclay layer in the loop)
// ---------------------------------------------------------------------------

fn raw_wasmi_add() -> Box<dyn FnMut(u32, u32) -> u32> {
    let wasm = bench_wasm();
    let engine = wasmi::Engine::default();
    let module = wasmi::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmi::Store::new(&engine, ());
    let linker = <wasmi::Linker<()>>::new(&engine);
    let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
    let func = instance
        .get_export(&store, "add")
        .and_then(wasmi::Extern::into_func)
        .unwrap();
    Box::new(move |a: u32, b: u32| {
        let mut res = [wasmi::Val::I32(0)];
        func.call(
            &mut store,
            &[wasmi::Val::I32(a as i32), wasmi::Val::I32(b as i32)],
            &mut res,
        )
        .unwrap();
        res[0].i32().unwrap() as u32
    })
}

fn raw_wasmtime_add() -> Box<dyn FnMut(u32, u32) -> u32> {
    let wasm = bench_wasm();
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance.get_func(&mut store, "add").unwrap();
    Box::new(move |a: u32, b: u32| {
        let mut res = [wasmtime::Val::I32(0)];
        func.call(
            &mut store,
            &[wasmtime::Val::I32(a as i32), wasmtime::Val::I32(b as i32)],
            &mut res,
        )
        .unwrap();
        res[0].i32().unwrap() as u32
    })
}

// ---------------------------------------------------------------------------
// waclay component path per engine
// ---------------------------------------------------------------------------

struct WaclayBench {
    store: Store,
    instance: waclay::ComponentInstance,
    fib: waclay::engine::Func,
}

fn waclay_setup(engine: &Engine) -> WaclayBench {
    let component = Component::from_plan(engine, bench_plan()).unwrap();
    let mut linker = Linker::new();
    let mut store = Store::new(engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();
    let fib = match instance.get_core_export(&mut store, "fib") {
        Some(waclay::engine::Extern::Func(f)) => f,
        _ => panic!("fib export missing"),
    };
    WaclayBench {
        store,
        instance,
        fib,
    }
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

fn bench_add_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("add-u32-overhead");
    group.throughput(criterion::Throughput::Elements(1));

    // raw baselines
    let mut raw_wasmi = raw_wasmi_add();
    group.bench_function("raw-wasmi", |b| {
        b.iter(|| raw_wasmi(black_box(40u32), black_box(2u32)))
    });
    let mut raw_wasmtime = raw_wasmtime_add();
    group.bench_function("raw-wasmtime", |b| {
        b.iter(|| raw_wasmtime(black_box(40u32), black_box(2u32)))
    });

    // waclay component path
    for engine_name in ["wasmi", "wasmtime", "wasmer"] {
        let engine = component_engine(engine_name);
        let mut s = waclay_setup(&engine);
        group.bench_function(format!("waclay-{engine_name}"), |b| {
            b.iter(|| {
                let out = s
                    .instance
                    .call_func(
                        &mut s.store,
                        "add",
                        &[Val::U32(black_box(40)), Val::U32(black_box(2))],
                    )
                    .unwrap();
                match out {
                    Some(Val::U32(n)) => black_box(n),
                    other => panic!("unexpected {other:?}"),
                }
            })
        });
    }
    group.finish();
}

fn bench_fib_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("fib-20-compute");
    for engine_name in ["wasmi", "wasmtime", "wasmer"] {
        let engine = component_engine(engine_name);
        let mut s = waclay_setup(&engine);
        group.bench_function(format!("waclay-{engine_name}"), |b| {
            b.iter(|| {
                let mut res = [waclay::engine::Val::I32(0)];
                s.store
                    .call(&s.fib, &[waclay::engine::Val::I32(20)], &mut res)
                    .unwrap();
                black_box(res[0].i32().unwrap())
            })
        });
    }
    group.finish();
}

fn bench_string_echo(c: &mut Criterion) {
    let mut group = c.benchmark_group("string-echo-realloc");
    for engine_name in ["wasmi", "wasmtime", "wasmer"] {
        let engine = component_engine(engine_name);
        let mut s = waclay_setup(&engine);
        let msg = "waclay benchmark string with some length to exercise realloc";
        group.bench_function(format!("waclay-{engine_name}"), |b| {
            b.iter(|| {
                let out = s
                    .instance
                    .call_func(
                        &mut s.store,
                        "echo",
                        &[Val::String(black_box(msg).to_string())],
                    )
                    .unwrap();
                match out {
                    Some(Val::String(str)) => black_box(str),
                    other => panic!("unexpected {other:?}"),
                }
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_add_overhead,
    bench_fib_compute,
    bench_string_echo
);
criterion_main!(benches);
