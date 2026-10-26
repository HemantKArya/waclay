//! WASI P2/P3 stdio through REAL resource shims, on every engine.
//!
//! This is the flagship Component-Model test: the guest module follows the
//! exact shim pattern `wit-component` generates for real wasm32-wasip2
//! guests — it receives `own<output-stream>` from `wasi:cli/stdout`,
//! wraps the host rep via the `[resource-new]output-stream` intrinsic,
//! calls the `[method]output-stream.write` lowered import (with a
//! retptr for the indirect `result<_, stream-error>`), and drops the
//! handle through `[resource-drop]output-stream`.
//!
//! Must behave identically on wasmi, wasmtime and wasmer, at both the
//! 0.2.0 (P2) and 0.3.0 (P3) interface versions.

use std::io::Write;
use std::sync::{Arc, Mutex};
use waclay::abi::StringEncoding;
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport, LoweredImport};
use waclay::component::types::{FuncSig, Primitive, TypeDef, TypeRef, Val, WorldImport};
use waclay::wasi::{add_to_linker, WasiCtx};
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

fn stdio_plan(version: &str, stream: &str) -> ComponentPlan {
    // stream: "stdout" (rep 2) or "stderr" (rep 3)
    let v = version;
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "wasi:cli/{stream}@{v}" "get-{stream}" (func $get_stream (result i32)))
            (import "wasi:io/streams@{v}" "[resource-new]output-stream"
                (func $res_new (param i32) (result i32)))
            (import "wasi:io/streams@{v}" "[resource-drop]output-stream"
                (func $res_drop (param i32)))
            (import "wasi:io/streams@{v}" "[method]output-stream.write"
                (func $write (param i32 i32 i32 i32)))
            {SUPPORT}
            (data (i32.const 128) "hi from P2")
            (func (export "run") (result i32)
                (local $rep i32) (local $h i32) (local $rt i32)
                (local.set $rep (call $get_stream))
                (local.set $h (call $res_new (local.get $rep)))
                (local.set $rt (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (call $write (local.get $h) (i32.const 128) (i32.const 10) (local.get $rt))
                (call $res_drop (local.get $h))
                (i32.load (local.get $rt))))
        "#
    ))
    .unwrap();

    let realloc = CoreFuncRef::Export {
        instance: 0,
        name: "cabi_realloc".into(),
    };

    // type table (indices matter):
    //   0: resource placeholder, 1: own<0>, 2: borrow<0>,
    //   3: stream-error variant, 4: result<_, stream-error>, 5: list<u8>
    let types = vec![
        TypeDef::Flags(vec!["\u{0}resource".into()]),
        TypeDef::Own(0),
        TypeDef::Borrow(0),
        TypeDef::Variant(vec![
            ("last-operation-failed".into(), Some(TypeRef::Def(1))),
            ("closed".into(), None),
        ]),
        TypeDef::Result {
            ok: None,
            err: Some(TypeRef::Def(3)),
        },
        TypeDef::List(TypeRef::Primitive(Primitive::U8)),
    ];

    let streams = format!("wasi:io/streams@{v}");
    let cli_stream = format!("wasi:cli/{stream}@{v}");
    let getter = format!("get-{stream}");

    ComponentPlan {
        core_module: wasm,
        types,
        imports: vec![
            WorldImport {
                name: cli_stream.clone(),
                funcs: vec![(
                    getter.clone(),
                    FuncSig {
                        params: vec![],
                        result: Some(TypeRef::Def(1)),
                    },
                )],
            },
            WorldImport {
                name: streams.clone(),
                funcs: vec![(
                    "[method]output-stream.write".into(),
                    FuncSig {
                        params: vec![
                            ("self".into(), TypeRef::Def(2)),
                            ("contents".into(), TypeRef::Def(5)),
                        ],
                        result: Some(TypeRef::Def(4)),
                    },
                )],
            },
        ],
        lowers: vec![
            LoweredImport {
                async_lower: false,
                sig: FuncSig {
                    params: vec![],
                    result: Some(TypeRef::Def(1)),
                },
                interface: Some(cli_stream.clone()),
                func_name: getter,
                core_module_name: cli_stream,
                core_func_name: format!("get-{stream}"),
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LoweredImport {
                async_lower: false,
                sig: FuncSig {
                    params: vec![
                        ("self".into(), TypeRef::Def(2)),
                        ("contents".into(), TypeRef::Def(5)),
                    ],
                    result: Some(TypeRef::Def(4)),
                },
                interface: Some(streams.clone()),
                func_name: "[method]output-stream.write".into(),
                core_module_name: streams,
                core_func_name: "[method]output-stream.write".into(),
                realloc: Some(realloc),
                string_enc: StringEncoding::Utf8,
            },
        ],
        resources: vec![waclay::component::types::ResourceInfo {
            type_idx: 0,
            interface: Some(format!("wasi:io/streams@{v}")),
            name: "output-stream".into(),
        }],
        lifts: vec![LiftedExport {
            async_lift: false,
            callback: None,
            key: "run".into(),
            func_name: "run".into(),
            sig: FuncSig {
                params: vec![],
                result: Some(TypeRef::Primitive(Primitive::U32)),
            },
            core_func: CoreFuncRef::Export {
                instance: 0,
                name: "run".into(),
            },
            post_return: None,
            realloc: None,
            string_enc: StringEncoding::Utf8,
        }],
        ..Default::default()
    }
}

#[test]
fn p2_stdio_resource_shim_on_all_engines() {
    for version in ["0.2.0", "0.3.0"] {
        for (name, engine) in engines() {
            let tag = format!("[{name} @ {version}]");
            let plan = stdio_plan(version, "stdout");
            let component = Component::from_plan(&engine, plan)
                .unwrap_or_else(|e| panic!("{tag} plan compile: {e}"));

            let out = Capture::default();
            let mut linker = Linker::new();
            add_to_linker(&mut linker, version);

            let ctx = WasiCtx::builder().stdout(Box::new(out.clone())).build();
            let mut store = Store::new(&engine, ctx).unwrap();
            let instance = linker
                .instantiate(&mut store, &component)
                .unwrap_or_else(|e| panic!("{tag} instantiate: {e}"));

            let res = instance
                .call_func(&mut store, "run", &[])
                .unwrap_or_else(|e| panic!("{tag} run: {e}"));
            match res {
                Some(Val::U32(0)) => {} // result::ok discriminant
                other => panic!("{tag} expected ok result, got {other:?}"),
            }

            assert_eq!(
                out.text(),
                "hi from P2",
                "{tag} guest bytes must reach the host stdout sink"
            );
        }
    }
}

/// The stderr path is the same machinery with rep 3 (get-stderr).
#[test]
fn p2_stderr_write_on_all_engines() {
    for (name, engine) in engines() {
        let tag = format!("[{name}]");
        let plan = stdio_plan("0.2.0", "stderr");
        let component = Component::from_plan(&engine, plan)
            .unwrap_or_else(|e| panic!("{tag} plan compile: {e}"));

        let sink = Capture::default();
        let mut linker = Linker::new();
        add_to_linker(&mut linker, "0.2.0");

        let ctx = WasiCtx::builder().stderr(Box::new(sink.clone())).build();
        let mut store = Store::new(&engine, ctx).unwrap();
        let instance = linker
            .instantiate(&mut store, &component)
            .unwrap_or_else(|e| panic!("{tag} instantiate: {e}"));

        let res = instance.call_func(&mut store, "run", &[]).unwrap();
        assert!(matches!(res, Some(Val::U32(0))), "{tag} ok result");
        assert_eq!(
            sink.text(),
            "hi from P2",
            "{tag} guest bytes must reach the host stderr sink"
        );
    }
}
