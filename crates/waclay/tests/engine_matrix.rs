//! Engine-matrix tests: the same waclay program must behave identically on
//! every backend (`wasmi`, `wasmtime`, `wasmer`). This is the contract that
//! makes waclay a real abstraction layer rather than a wasmi wrapper.

use waclay::abi::StringEncoding;
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport, LoweredImport};
use waclay::component::types::{FuncSig, Primitive, TypeRef, Val, WorldImport};
use waclay::engine::Val as CoreVal;
use waclay::{Component, Engine, Linker, Store};

// ---------------------------------------------------------------------------
// Engine matrix
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

// ---------------------------------------------------------------------------
// 1. Core wasm: arithmetic + host callbacks + memory on every engine
// ---------------------------------------------------------------------------

fn core_module_wasm() -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
            (import "env" "mul" (func $mul (param i32 i32) (result i32)))
            {SUPPORT}
            (func (export "add") (param i32 i32) (result i32)
                (i32.add (local.get 0) (local.get 1)))
            (func (export "host-mul") (param i32 i32) (result i32)
                (call $mul (local.get 0) (local.get 1)))
            (func (export "store-at") (param $p i32) (param $v i64)
                (i64.store (local.get $p) (local.get $v)))
            (func (export "load-at") (param $p i32) (result i64)
                (i64.load (local.get $p)))
        )
        "#
    ))
    .unwrap()
}

#[test]
fn core_wasm_identical_on_all_engines() {
    let wasm = core_module_wasm();
    for (name, engine) in engines() {
        let component = Component::from_core_module(&engine, &wasm)
            .unwrap_or_else(|e| panic!("[{name}] compile failed: {e}"));

        let mut linker = Linker::new();
        linker.raw_func(
            "env",
            "mul",
            Box::new(|_caller, args, results| {
                let a = args[0].i32().unwrap() as i64;
                let b = args[1].i32().unwrap() as i64;
                results.push(CoreVal::I32((a * b) as i32));
                Ok(())
            }),
        );

        let mut store = Store::new(&engine, ()).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate failed: {e}"));

        // pure arithmetic
        let mut res = [CoreVal::I32(0)];
        instance
            .call_core(
                &mut store,
                "add",
                &[CoreVal::I32(40), CoreVal::I32(2)],
                &mut res,
            )
            .unwrap_or_else(|e| panic!("[{name}] add failed: {e}"));
        assert_eq!(res[0].i32().unwrap(), 42, "[{name}] add");

        // guest -> host callback
        let mut res = [CoreVal::I32(0)];
        instance
            .call_core(
                &mut store,
                "host-mul",
                &[CoreVal::I32(6), CoreVal::I32(7)],
                &mut res,
            )
            .unwrap_or_else(|e| panic!("[{name}] host-mul failed: {e}"));
        assert_eq!(res[0].i32().unwrap(), 42, "[{name}] host-mul");

        // memory roundtrip through the raw core export
        let memory = match instance.get_core_export(&mut store, "memory") {
            Some(waclay::engine::Extern::Memory(m)) => m,
            other => panic!("[{name}] expected memory export, got {other:?}"),
        };
        store
            .memory_write(&memory, 128, &7i64.to_le_bytes())
            .unwrap_or_else(|e| panic!("[{name}] host write failed: {e}"));
        let mut res = [CoreVal::I64(0)];
        instance
            .call_core(&mut store, "load-at", &[CoreVal::I32(128)], &mut res)
            .unwrap_or_else(|e| panic!("[{name}] load-at failed: {e}"));
        assert_eq!(
            res[0].i64().unwrap(),
            7,
            "[{name}] host-written i64 visible to guest"
        );

        let mut res = [CoreVal::I64(0)];
        instance
            .call_core(
                &mut store,
                "store-at",
                &[CoreVal::I32(136), CoreVal::I64(-11)],
                &mut res,
            )
            .unwrap_or_else(|e| panic!("[{name}] store-at failed: {e}"));
        let mut buf = [0u8; 8];
        store
            .memory_read(&memory, 136, &mut buf)
            .unwrap_or_else(|e| panic!("[{name}] host read failed: {e}"));
        assert_eq!(
            i64::from_le_bytes(buf),
            -11,
            "[{name}] guest-written i64 visible to host"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Component path: lifted export with realloc + lowered host import
// ---------------------------------------------------------------------------

fn echo_plan() -> ComponentPlan {
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "test:echo/host@1.0.0" "shout" (func $shout (param i32 i32) (result i32)))
            {SUPPORT}
            ;; echo: copies input bytes to a fresh area, returns (ptr,len)
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
            ;; forwards the string length to the host import
            (func (export "announce") (param $p i32) (param $l i32) (result i32)
                (call $shout (local.get $p) (local.get $l)))
        )
        "#
    ))
    .unwrap();

    let realloc = CoreFuncRef::Export {
        instance: 0,
        name: "cabi_realloc".into(),
    };

    ComponentPlan {
        core_module: wasm,
        types: vec![],
        imports: vec![WorldImport {
            name: "test:echo/host@1.0.0".into(),
            funcs: vec![(
                "shout".into(),
                FuncSig {
                    params: vec![("msg".into(), TypeRef::Primitive(Primitive::String))],
                    result: Some(TypeRef::Primitive(Primitive::U32)),
                },
            )],
        }],
        lowers: vec![LoweredImport {
            sig: FuncSig {
                params: vec![("msg".into(), TypeRef::Primitive(Primitive::String))],
                result: Some(TypeRef::Primitive(Primitive::U32)),
            },
            interface: Some("test:echo/host@1.0.0".into()),
            func_name: "shout".into(),
            core_module_name: "test:echo/host@1.0.0".into(),
            core_func_name: "shout".into(),
            realloc: Some(realloc.clone()),
            string_enc: StringEncoding::Utf8,
            async_lower: false,
        }],
        lifts: vec![
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
                realloc: Some(realloc.clone()),
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "announce".into(),
                func_name: "announce".into(),
                sig: FuncSig {
                    params: vec![("msg".into(), TypeRef::Primitive(Primitive::String))],
                    result: Some(TypeRef::Primitive(Primitive::U32)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "announce".into(),
                },
                post_return: None,
                realloc: Some(realloc),
                string_enc: StringEncoding::Utf8,
            },
        ],
        ..Default::default()
    }
}

#[test]
fn component_path_identical_on_all_engines() {
    let plan = echo_plan();
    let messages: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    for (name, engine) in engines() {
        let component = Component::from_plan(&engine, plan.clone())
            .unwrap_or_else(|e| panic!("[{name}] plan compile failed: {e}"));

        let mut linker = Linker::new();
        let seen: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        linker.func(
            Some("test:echo/host@1.0.0"),
            "shout",
            move |_ctx, args, out| {
                let Val::String(s) = &args[0] else {
                    return Err(waclay::Error::value("expected string arg"));
                };
                seen2.lock().unwrap().push(s.clone());
                out.push(Val::U32(s.len() as u32));
                Ok(())
            },
        );

        let mut store = Store::new(&engine, ()).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("[{name}] instantiate failed: {e}"));

        // string through realloc, both directions
        let out = instance
            .call_func(&mut store, "echo", &[Val::String("héllo 🦀".into())])
            .unwrap_or_else(|e| panic!("[{name}] echo failed: {e}"));
        match out {
            Some(Val::String(s)) => assert_eq!(s, "héllo 🦀", "[{name}] echo roundtrip"),
            other => panic!("[{name}] expected string, got {other:?}"),
        }

        // host import via lower
        let out = instance
            .call_func(
                &mut store,
                "announce",
                &[Val::String("engine matrix".into())],
            )
            .unwrap_or_else(|e| panic!("[{name}] announce failed: {e}"));
        match out {
            Some(Val::U32(n)) => assert_eq!(n, "engine matrix".len() as u32, "[{name}] len"),
            other => panic!("[{name}] expected u32, got {other:?}"),
        }
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            ["engine matrix".to_string()],
            "[{name}] host import must receive the string"
        );
        let _ = &messages;
    }
}

#[test]
fn engine_names_are_distinct() {
    let names: Vec<&str> = engines().into_iter().map(|(n, _)| n).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(names.len(), sorted.len(), "engine names must be unique");
    for (n, e) in engines() {
        assert_eq!(e.name(), n, "engine name() must match its backend");
    }
}

/// Every backend must report a usable version of the engine it wraps.
#[test]
fn engine_versions_are_reported() {
    for (name, engine) in engines() {
        let v = engine.version();
        assert!(!v.is_empty(), "[{name}] version() must not be empty");
        assert!(
            v.split('.').count() >= 2 && v.chars().next().unwrap().is_ascii_digit(),
            "[{name}] version {v:?} must look like x.y(.z)"
        );
        let desc = engine.describe();
        assert!(
            desc.starts_with(name) && desc.contains(v),
            "[{name}] describe() = {desc:?} must embed name+version"
        );
    }
}
