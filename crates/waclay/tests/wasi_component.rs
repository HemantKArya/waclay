//! WASI 0.3 component interfaces driven end-to-end through the Canonical ABI.
//!
//! A hand-written core module imports the lowered WASI functions
//! (`wasi:random/random`, `wasi:clocks/monotonic-clock`,
//! `wasi:cli/environment`) exactly like a real wasm32-wasip2 guest would,
//! and the test drives them through lifted component exports.

use waclay::abi::StringEncoding;
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport, LoweredImport};
use waclay::component::types::{FuncSig, Primitive, TypeDef, TypeRef, WorldImport};
use waclay::engine::Val as CoreVal;
use waclay::wasi::WasiCtx;
use waclay::{Component, Engine, Linker, Store, Val};

const SUPPORT: &str = r#"
    (global $heap (mut i32) (i32.const 4096))
    (func $realloc (export "cabi_realloc")
        (param $old i32) (param $old_size i32) (param $align i32) (param $new i32) (result i32)
        (local $ret i32)
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

/// Every backend must serve WASI guests identically.
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

fn wasi_plan(version: &str) -> ComponentPlan {
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "wasi:random/random@{version}" "get-random-u64"
                (func $rand (result i64)))
            (import "wasi:clocks/monotonic-clock@{version}" "now"
                (func $now (result i64)))
            ;; list<tuple<string,string>> result: retptr passed by the guest
            (import "wasi:cli/environment@{version}" "get-environment"
                (func $env (param i32)))
            {SUPPORT}

            (func (export "run-random") (result i64)
                (call $rand))

            (func (export "run-clock") (result i64)
                (call $now))

            ;; allocate a retptr, call get-environment into it, return the ptr
            (func (export "run-env") (result i32)
                (local $sp i32)
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (call $env (local.get $sp))
                (local.get $sp))
        )
        "#
    ))
    .unwrap();

    let realloc = CoreFuncRef::Export {
        instance: 0,
        name: "cabi_realloc".into(),
    };

    let types = vec![
        // 0: tuple<string, string>
        TypeDef::Tuple(vec![
            TypeRef::Primitive(Primitive::String),
            TypeRef::Primitive(Primitive::String),
        ]),
        // 1: list<tuple<string, string>>
        TypeDef::List(TypeRef::Def(0)),
    ];

    let iface = |ns: &str| format!("{ns}@{version}");

    ComponentPlan {
        core_module: wasm,
        types,
        imports: vec![
            WorldImport {
                name: iface("wasi:random/random"),
                funcs: vec![(
                    "get-random-u64".into(),
                    FuncSig {
                        params: vec![],
                        result: Some(TypeRef::Primitive(Primitive::U64)),
                    },
                )],
            },
            WorldImport {
                name: iface("wasi:clocks/monotonic-clock"),
                funcs: vec![(
                    "now".into(),
                    FuncSig {
                        params: vec![],
                        result: Some(TypeRef::Primitive(Primitive::U64)),
                    },
                )],
            },
            WorldImport {
                name: iface("wasi:cli/environment"),
                funcs: vec![(
                    "get-environment".into(),
                    FuncSig {
                        params: vec![],
                        result: Some(TypeRef::Def(1)),
                    },
                )],
            },
        ],
        lowers: vec![
            LoweredImport {
                async_lower: false,
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Primitive(Primitive::U64)),
                },
                interface: Some(iface("wasi:random/random")),
                func_name: "get-random-u64".into(),
                core_module_name: iface("wasi:random/random"),
                core_func_name: "get-random-u64".into(),
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LoweredImport {
                async_lower: false,
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Primitive(Primitive::U64)),
                },
                interface: Some(iface("wasi:clocks/monotonic-clock")),
                func_name: "now".into(),
                core_module_name: iface("wasi:clocks/monotonic-clock"),
                core_func_name: "now".into(),
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LoweredImport {
                async_lower: false,
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Def(1)),
                },
                interface: Some(iface("wasi:cli/environment")),
                func_name: "get-environment".into(),
                core_module_name: iface("wasi:cli/environment"),
                core_func_name: "get-environment".into(),
                realloc: Some(realloc),
                string_enc: StringEncoding::Utf8,
            },
        ],
        lifts: vec![
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "run-random".into(),
                func_name: "run-random".into(),
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Primitive(Primitive::U64)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "run-random".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "run-clock".into(),
                func_name: "run-clock".into(),
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Primitive(Primitive::U64)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "run-clock".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "run-env".into(),
                func_name: "run-env".into(),
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Def(1)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "run-env".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
        ],
        ..Default::default()
    }
}

#[test]
fn wasi_p3_component_interfaces() {
    for (name, engine) in engines() {
        let component = Component::from_plan(&engine, wasi_plan("0.3.0"))
            .unwrap_or_else(|e| panic!("[{name}] plan: {e}"));

        let mut linker = Linker::new();
        waclay::wasi::add_to_linker(&mut linker, "0.3.0");

        let ctx = WasiCtx::builder()
            .arg("prog")
            .env("FRUIT", "mango")
            .env("LANG", "rust")
            .build();

        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        // -- random ---------------------------------------------------------
        let a = instance
            .call_func(&mut store, "run-random", &[])
            .unwrap_or_else(|e| panic!("[{name}] run-random: {e}"));
        let Val::U64(r1) = a.unwrap() else {
            panic!("[{name}] expected u64 from run-random")
        };
        let b = instance.call_func(&mut store, "run-random", &[]).unwrap();
        let Val::U64(r2) = b.unwrap() else {
            panic!("[{name}] expected u64 from run-random")
        };
        // statistically impossible for a sane RNG, instantly catches a stubbed 0
        assert_ne!((r1, r2), (0, 0), "[{name}] random must not be all-zero");
        assert_ne!(r1, r2, "[{name}] two random draws must differ");

        // -- monotonic clock --------------------------------------------------
        let c = instance
            .call_func(&mut store, "run-clock", &[])
            .unwrap_or_else(|e| panic!("[{name}] run-clock: {e}"));
        let Val::U64(t1) = c.unwrap() else {
            panic!("[{name}] expected u64 from run-clock")
        };
        assert!(t1 > 0, "[{name}] monotonic clock must be positive");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let d = instance.call_func(&mut store, "run-clock", &[]).unwrap();
        let Val::U64(t2) = d.unwrap() else {
            panic!("[{name}] expected u64 from run-clock")
        };
        assert!(t2 > t1, "[{name}] monotonic clock must move forward");

        // -- environment --------------------------------------------------------
        let e = instance
            .call_func(&mut store, "run-env", &[])
            .unwrap_or_else(|e| panic!("[{name}] run-env: {e}"));
        let Val::List(pairs) = e.unwrap() else {
            panic!("[{name}] expected list from run-env")
        };
        let mut got: Vec<(String, String)> = pairs
            .into_iter()
            .map(|v| match v {
                Val::Tuple(t) => {
                    let mut it = t.into_iter();
                    match (it.next(), it.next()) {
                        (Some(Val::String(k)), Some(Val::String(v))) => (k, v),
                        other => panic!("[{name}] bad env pair {other:?}"),
                    }
                }
                other => panic!("[{name}] expected tuple, got {other:?}"),
            })
            .collect();
        got.sort();
        assert!(
            got.contains(&("FRUIT".to_string(), "mango".to_string())),
            "[{name}] FRUIT=mango missing from {got:?}"
        );
        assert!(
            got.contains(&("LANG".to_string(), "rust".to_string())),
            "[{name}] LANG=rust missing from {got:?}"
        );
    }
}

/// WASI 0.2.0 spellings must work identically (P2 guests).
#[test]
fn wasi_p2_version_aliases() {
    for (name, engine) in engines() {
        let plan = wasi_plan("0.2.0");

        let component =
            Component::from_plan(&engine, plan).unwrap_or_else(|e| panic!("[{name}] plan: {e}"));
        let mut linker = Linker::new();
        waclay::wasi::add_to_linker(&mut linker, "0.2.0");
        let ctx = WasiCtx::builder().env("A", "B").build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        let out = instance
            .call_func(&mut store, "run-random", &[])
            .unwrap_or_else(|e| panic!("[{name}] run-random: {e}"));
        assert!(matches!(out, Some(Val::U64(_))));
        let out = instance
            .call_func(&mut store, "run-env", &[])
            .unwrap_or_else(|e| panic!("[{name}] run-env: {e}"));
        assert!(matches!(out, Some(Val::List(_))));
    }
}

/// A missing-but-nonessential WASI interface must not block instantiation:
/// calls into it trap with a clear message instead.
#[test]
fn unimplemented_wasi_traps_with_clear_message() {
    for (name, engine) in engines() {
        let component = Component::from_plan(&engine, wasi_plan("0.3.0"))
            .unwrap_or_else(|e| panic!("[{name}] plan: {e}"));
        // deliberately NO wasi registration — only the raw import fallback
        let mut linker = Linker::new();
        let ctx = WasiCtx::builder().build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));

        let err = instance
            .call_func(&mut store, "run-random", &[])
            .expect_err(&format!(
                "[{name}] random must trap without wasi registration"
            ));
        let msg = err.to_string();
        assert!(
            msg.contains("not implemented") || msg.contains("not supported"),
            "[{name}] unclear error message: {msg}"
        );
    }
}

/// Convenience re-export check: `add_all_to_linker` covers P1 + P2/P3.
#[test]
fn add_all_registers_both_layers() {
    for (name, engine) in engines() {
        let component = Component::from_plan(&engine, wasi_plan("0.3.0"))
            .unwrap_or_else(|e| panic!("[{name}] plan: {e}"));
        let mut linker = Linker::new();
        waclay::wasi::add_all_to_linker(&mut linker, "0.3.0");
        let ctx = WasiCtx::builder().build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate: {e}"));
        let out = instance
            .call_func(&mut store, "run-clock", &[])
            .unwrap_or_else(|e| panic!("[{name}] run-clock: {e}"));
        assert!(matches!(out, Some(Val::U64(_))));
    }
}

// Silence unused import when Val re-export path changes.
#[allow(dead_code)]
fn _typecheck(v: CoreVal) -> i32 {
    v.i32().unwrap_or(0)
}
