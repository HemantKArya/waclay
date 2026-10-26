//! End-to-end component pipeline tests built from explicit plans.
//!
//! These tests drive the FULL stack — decode-free plan construction,
//! layout precompute, canonical ABI lift/lower with memory + realloc,
//! lowered host imports, resources, `_initialize` and post-return —
//! against real core modules, without depending on a component encoder.

use std::sync::{Arc, Mutex};

use waclay::abi::StringEncoding;
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport, LoweredImport};
use waclay::component::types::{FuncSig, Primitive, TypeDef, TypeRef, Val, WorldImport};
use waclay::engine::Val as CoreVal;
use waclay::wasi::WasiCtx;
use waclay::{Component, Engine, Linker, Store};
use waclay_wasmi::WasmiEngine;

/// Simple bump allocator + helpers core module.
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

fn engine() -> Engine {
    Engine::new(WasmiEngine::default())
}

/// Shared type table for the tests.
fn types() -> Vec<TypeDef> {
    vec![
        // 0: point = record { x: s32, y: s32 }
        TypeDef::Record(vec![
            ("x".into(), TypeRef::Primitive(Primitive::S32)),
            ("y".into(), TypeRef::Primitive(Primitive::S32)),
        ]),
        // 1: shape = variant { circle(f64), rect(point), none }
        TypeDef::Variant(vec![
            ("circle".into(), Some(TypeRef::Primitive(Primitive::F64))),
            ("rect".into(), Some(TypeRef::Def(0))),
            ("none".into(), None),
        ]),
    ]
}

// ---------------------------------------------------------------------------
// 1. add: the simplest possible lift
// ---------------------------------------------------------------------------

#[test]
fn add_component_roundtrip() {
    let wasm = wat::parse_str(format!(
        r#"(module {SUPPORT}
            (func (export "add") (param i32 i32) (result i32)
                (i32.add (local.get 0) (local.get 1))))
        "#
    ))
    .unwrap();

    let plan = ComponentPlan {
        core_module: wasm,
        types: types(),
        lifts: vec![LiftedExport {
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
            realloc: Some(CoreFuncRef::Export {
                instance: 0,
                name: "cabi_realloc".into(),
            }),
            string_enc: StringEncoding::Utf8,
        }],
        ..Default::default()
    };

    let engine = engine();
    let component = Component::from_plan(&engine, plan).unwrap();
    let mut linker = Linker::new();
    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let add = instance.typed_func::<(u32, u32), u32>("add").unwrap();
    assert_eq!(add.call(&mut store, (2, 3)).unwrap(), 5);
    assert_eq!(add.call(&mut store, (0, 0)).unwrap(), 0);
    assert_eq!(add.call(&mut store, (u32::MAX, 1)).unwrap(), 0); // wrapping
}

// ---------------------------------------------------------------------------
// 2. strings + realloc + post-return
// ---------------------------------------------------------------------------

#[test]
fn string_echo_with_realloc_and_post_return() {
    let wasm = wat::parse_str(format!(
        r#"(module {SUPPORT}
            ;; echo(s) -> s : copies the string into a fresh buffer
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
            ;; post-return frees the return area (just bookkeeping here)
            (func (export "cabi_post_echo") (param $retptr i32)
                (nop)))
        "#
    ))
    .unwrap();

    let plan = ComponentPlan {
        core_module: wasm,
        types: types(),
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
    };

    let engine = engine();
    let component = Component::from_plan(&engine, plan).unwrap();
    let mut linker = Linker::new();
    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let echo = instance.typed_func::<(String,), String>("echo").unwrap();
    let out = echo
        .call(&mut store, ("hello, 组件模型! 🚀".into(),))
        .unwrap();
    assert_eq!(out, "hello, 组件模型! 🚀");
}

// ---------------------------------------------------------------------------
// 3. lists, records, variants, options, results
// ---------------------------------------------------------------------------

#[test]
fn aggregate_types_roundtrip() {
    let wasm = wat::parse_str(format!(
        r#"(module {SUPPORT}
            ;; sum-list(items: list<u64>) -> u64
            (func (export "sum-list") (param $p i32) (param $l i32) (result i64)
                (local $i i32) (local $acc i64)
                (block $done
                    (loop $loop
                        (br_if $done (i32.ge_u (local.get $i) (local.get $l)))
                        (local.set $acc (i64.add (local.get $acc)
                            (i64.load (i32.add (local.get $p)
                                               (i32.mul (local.get $i) (i32.const 8))))))
                        (local.set $i (i32.add (local.get $i) (i32.const 1)))
                        (br $loop)))
                (local.get $acc))

            ;; swap(point) -> point  (record {{x: s32, y: s32}})
            ;; 2-flat result > MAX_FLAT_RESULTS=1 -> retptr returned
            (func (export "swap") (param $x i32) (param $y i32) (result i32)
                (local $sp i32)
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (i32.store (local.get $sp) (local.get $y))
                (i32.store offset=4 (local.get $sp) (local.get $x))
                (local.get $sp))

            ;; shape-area(shape) -> f64
            ;; shape = variant {{ circle(f64), rect(point), none }}
            ;; flat: [i32 disc] + joined payload [i64, i32]
            ;; (join(F64, I32) = I64 under the spec's asymmetric join)
            (func (export "shape-area") (param $disc i32) (param $a i64) (param $b i32)
                (result f64)
                (if (result f64) (i32.eq (local.get $disc) (i32.const 1))
                    (then
                        (f64.mul
                            (f64.convert_i32_s (i32.wrap_i64 (local.get $a)))
                            (f64.convert_i32_s (local.get $b))))
                    (else
                        (if (result f64) (i32.eq (local.get $disc) (i32.const 0))
                            (then (f64.mul
                                    (f64.mul (f64.reinterpret_i64 (local.get $a))
                                             (f64.reinterpret_i64 (local.get $a)))
                                    (f64.const 3.141592653589793)))
                            (else (f64.const 0))))))

            ;; maybe-double(v: option<u32>) -> option<u32> (retptr result)
            (func (export "maybe-double") (param $disc i32) (param $v i32)
                (result i32)
                (local $sp i32)
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (if (i32.eqz (local.get $disc))
                    (then (i32.store (local.get $sp) (i32.const 0)))
                    (else
                        (i32.store (local.get $sp) (i32.const 1))
                        (i32.store offset=4 (local.get $sp)
                                   (i32.mul (local.get $v) (i32.const 2)))))
                (local.get $sp))

            ;; divide(a: u32, b: u32) -> result<u32, string> (retptr result)
            ;; result memory repr: [disc u32][payload: u32 or (ptr,len)]
            (func (export "divide") (param $a i32) (param $b i32) (result i32)
                (local $sp i32) (local $msg i32)
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 12)))
                (if (i32.eqz (local.get $b))
                    (then
                        (local.set $msg (call $realloc (i32.const 0) (i32.const 0)
                                                       (i32.const 1) (i32.const 14)))
                        (i32.store8 (local.get $msg) (i32.const 100)) ;; d
                        (i32.store8 offset=1 (local.get $msg) (i32.const 105)) ;; i
                        (i32.store8 offset=2 (local.get $msg) (i32.const 118)) ;; v
                        (i32.store8 offset=3 (local.get $msg) (i32.const 105)) ;; i
                        (i32.store8 offset=4 (local.get $msg) (i32.const 100)) ;; d
                        (i32.store8 offset=5 (local.get $msg) (i32.const 101)) ;; e
                        (i32.store8 offset=6 (local.get $msg) (i32.const 32))  ;; space
                        (i32.store8 offset=7 (local.get $msg) (i32.const 98))  ;; b
                        (i32.store8 offset=8 (local.get $msg) (i32.const 121)) ;; y
                        (i32.store8 offset=9 (local.get $msg) (i32.const 32))  ;; space
                        (i32.store8 offset=10 (local.get $msg) (i32.const 122)) ;; z
                        (i32.store8 offset=11 (local.get $msg) (i32.const 101)) ;; e
                        (i32.store8 offset=12 (local.get $msg) (i32.const 114)) ;; r
                        (i32.store8 offset=13 (local.get $msg) (i32.const 111)) ;; o
                        (i32.store (local.get $sp) (i32.const 1))
                        (i32.store offset=4 (local.get $sp) (local.get $msg))
                        (i32.store offset=8 (local.get $sp) (i32.const 14)))
                    (else
                        (i32.store (local.get $sp) (i32.const 0))
                        (i32.store offset=4 (local.get $sp)
                                   (i32.div_u (local.get $a) (local.get $b)))))
                (local.get $sp)))
        "#
    ))
    .unwrap();

    let plan = ComponentPlan {
        core_module: wasm,
        types: types(),
        lifts: vec![
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "sum-list".into(),
                func_name: "sum-list".into(),
                sig: FuncSig {
                    params: vec![("items".into(), TypeRef::Def(2))],
                    result: Some(TypeRef::Primitive(Primitive::U64)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "sum-list".into(),
                },
                post_return: None,
                realloc: Some(CoreFuncRef::Export {
                    instance: 0,
                    name: "cabi_realloc".into(),
                }),
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "swap".into(),
                func_name: "swap".into(),
                sig: FuncSig {
                    params: vec![("p".into(), TypeRef::Def(0))],
                    result: Some(TypeRef::Def(0)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "swap".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "shape-area".into(),
                func_name: "shape-area".into(),
                sig: FuncSig {
                    params: vec![("shape".into(), TypeRef::Def(1))],
                    result: Some(TypeRef::Primitive(Primitive::F64)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "shape-area".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "maybe-double".into(),
                func_name: "maybe-double".into(),
                sig: FuncSig {
                    params: vec![("v".into(), TypeRef::Def(3))],
                    result: Some(TypeRef::Def(3)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "maybe-double".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "divide".into(),
                func_name: "divide".into(),
                sig: FuncSig {
                    params: vec![
                        ("a".into(), TypeRef::Primitive(Primitive::U32)),
                        ("b".into(), TypeRef::Primitive(Primitive::U32)),
                    ],
                    result: Some(TypeRef::Def(4)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "divide".into(),
                },
                post_return: None,
                realloc: Some(CoreFuncRef::Export {
                    instance: 0,
                    name: "cabi_realloc".into(),
                }),
                string_enc: StringEncoding::Utf8,
            },
        ],
        ..Default::default()
    };
    // extend the type table: 2 = list<u64>, 3 = option<u32>, 4 = result<u32,string>
    let plan = with_extra_types(
        plan,
        vec![
            TypeDef::List(TypeRef::Primitive(Primitive::U64)),
            TypeDef::Option(TypeRef::Primitive(Primitive::U32)),
            TypeDef::Result {
                ok: Some(TypeRef::Primitive(Primitive::U32)),
                err: Some(TypeRef::Primitive(Primitive::String)),
            },
        ],
    );

    let engine = engine();
    let component = Component::from_plan(&engine, plan).unwrap();
    let mut linker = Linker::new();
    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    // list<u64>
    let sum = instance.typed_func::<(Vec<u64>,), u64>("sum-list").unwrap();
    assert_eq!(sum.call(&mut store, (vec![1, 2, 3, 4],)).unwrap(), 10);
    assert_eq!(sum.call(&mut store, (vec![],)).unwrap(), 0);

    // record
    #[derive(Debug, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }
    impl waclay::ComponentValue for Point {
        fn into_val(self) -> Val {
            Val::Record(vec![
                ("x".into(), Val::S32(self.x)),
                ("y".into(), Val::S32(self.y)),
            ])
        }
        fn from_val(v: &Val) -> waclay::Result<Self> {
            match v {
                Val::Record(fields) => {
                    let get = |name: &str| -> waclay::Result<i32> {
                        fields
                            .iter()
                            .find(|(k, _)| k == name)
                            .map(|(_, v)| match v {
                                Val::S32(x) => Ok(*x),
                                _ => Err(waclay::Error::value("expected s32")),
                            })
                            .ok_or_else(|| waclay::Error::value("missing field"))?
                    };
                    Ok(Point {
                        x: get("x")?,
                        y: get("y")?,
                    })
                }
                _ => Err(waclay::Error::value("expected record")),
            }
        }
    }

    let swap = instance.typed_func::<(Point,), Point>("swap").unwrap();
    let out = swap.call(&mut store, (Point { x: 1, y: -2 },)).unwrap();
    assert_eq!(out, Point { x: -2, y: 1 });

    // variant
    let area = instance.typed_func::<(Val,), f64>("shape-area").unwrap();
    let circle = Val::Variant {
        case: "circle".into(),
        payload: Some(Box::new(Val::F64(2.0))),
    };
    let a = area.call(&mut store, (circle,)).unwrap();
    assert!((a - 12.5663706).abs() < 1e-4);
    let rect = Val::Variant {
        case: "rect".into(),
        payload: Some(Box::new(Val::Record(vec![
            ("x".into(), Val::S32(3)),
            ("y".into(), Val::S32(4)),
        ]))),
    };
    let a = area.call(&mut store, (rect,)).unwrap();
    assert_eq!(a, 12.0);
    let none = Val::Variant {
        case: "none".into(),
        payload: None,
    };
    assert_eq!(area.call(&mut store, (none,)).unwrap(), 0.0);

    // option
    let double = instance
        .typed_func::<(Option<u32>,), Option<u32>>("maybe-double")
        .unwrap();
    assert_eq!(double.call(&mut store, (Some(21),)).unwrap(), Some(42));
    assert_eq!(double.call(&mut store, (None,)).unwrap(), None);

    // result
    let divide = instance
        .typed_func::<(u32, u32), std::result::Result<u32, String>>("divide")
        .unwrap();
    assert_eq!(divide.call(&mut store, (84, 2)).unwrap(), Ok(42));
    assert_eq!(
        divide.call(&mut store, (1, 0)).unwrap(),
        Err("divide by zero".into())
    );
}

fn with_extra_types(mut plan: ComponentPlan, extra: Vec<TypeDef>) -> ComponentPlan {
    plan.types.extend(extra);
    plan
}

// ---------------------------------------------------------------------------
// 4. lowered host imports (guest -> host) with strings
// ---------------------------------------------------------------------------

#[test]
fn lowered_host_import_with_string() {
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "test:host/log@1.0.0" "log" (func $log (param i32 i32)))
            {SUPPORT}
            (func (export "announce") (param $p i32) (param $l i32) (result i32)
                (call $log (local.get $p) (local.get $l))
                (local.get $l)))
        "#
    ))
    .unwrap();

    let plan = ComponentPlan {
        core_module: wasm,
        types: types(),
        imports: vec![WorldImport {
            name: "test:host/log@1.0.0".into(),
            funcs: vec![(
                "log".into(),
                FuncSig {
                    params: vec![("msg".into(), TypeRef::Primitive(Primitive::String))],
                    result: None,
                },
            )],
        }],
        lowers: vec![LoweredImport {
            async_lower: false,
            sig: FuncSig {
                params: vec![("msg".into(), TypeRef::Primitive(Primitive::String))],
                result: None,
            },
            interface: Some("test:host/log@1.0.0".into()),
            func_name: "log".into(),
            core_module_name: "test:host/log@1.0.0".into(),
            core_func_name: "log".into(),
            realloc: None,
            string_enc: StringEncoding::Utf8,
        }],
        lifts: vec![LiftedExport {
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
            realloc: Some(CoreFuncRef::Export {
                instance: 0,
                name: "cabi_realloc".into(),
            }),
            string_enc: StringEncoding::Utf8,
        }],
        ..Default::default()
    };

    let engine = engine();
    let component = Component::from_plan(&engine, plan).unwrap();

    let mut linker = Linker::new();
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    linker.func(
        Some("test:host/log@1.0.0"),
        "log",
        move |_ctx, args, _out| {
            match &args[0] {
                Val::String(s) => log2.lock().unwrap().push(s.clone()),
                other => return Err(waclay::Error::value(format!("bad arg {other:?}"))),
            }
            Ok(())
        },
    );

    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let announce = instance.typed_func::<(String,), u32>("announce").unwrap();
    let n = announce
        .call(&mut store, ("waclay 2.0 works".into(),))
        .unwrap();
    assert_eq!(n, "waclay 2.0 works".len() as u32);
    assert_eq!(log.lock().unwrap().as_slice(), ["waclay 2.0 works"]);
}

// ---------------------------------------------------------------------------
// 5. resources end to end
// ---------------------------------------------------------------------------

#[test]
fn resource_methods_end_to_end() {
    // guest: creates a counter resource via the host's resource-new with
    // rep = 77, calls [m[method]counter.tick twice, returns sum of ticks.
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "test:res/res@1.0.0" "[resource-new]counter"
                (func $res-new (param i32) (result i32)))
            (import "test:res/res@1.0.0" "[resource-drop]counter"
                (func $res-drop (param i32)))
            (import "test:res/res@1.0.0" "[resource-rep]counter"
                (func $res-rep (param i32) (result i32)))
            (import "test:res/res@1.0.0" "[m[method]counter.tick"
                (func $tick (param i32) (result i32)))
            {SUPPORT}

            (func (export "bump") (param $rep i32) (result i32)
                (local $h i32) (local $h2 i32) (local $sum i32)
                (local.set $h (call $res-new (local.get $rep)))
                (call $res-rep (local.get $h))
                (drop)
                (local.set $sum (i32.add (call $tick (local.get $h))
                                         (call $tick (local.get $h))))
                (local.set $h2 (call $res-new (local.get $rep)))
                (call $res-drop (local.get $h2))
                (call $res-drop (local.get $h))
                (local.get $sum)))
        "#
    ))
    .unwrap();

    // type table: 0 = resource placeholder, 1 = borrow<0>
    let plan = ComponentPlan {
        core_module: wasm,
        types: vec![
            TypeDef::Flags(vec!["\u{0}resource".into()]),
            TypeDef::Borrow(0),
        ],
        imports: vec![WorldImport {
            name: "test:res/res@1.0.0".into(),
            funcs: vec![(
                "[m[method]counter.tick".into(),
                FuncSig {
                    params: vec![("self".into(), TypeRef::Def(1))],
                    result: Some(TypeRef::Primitive(Primitive::U32)),
                },
            )],
        }],
        lowers: vec![LoweredImport {
            async_lower: false,
            sig: FuncSig {
                params: vec![("self".into(), TypeRef::Def(1))],
                result: Some(TypeRef::Primitive(Primitive::U32)),
            },
            interface: Some("test:res/res@1.0.0".into()),
            func_name: "[m[method]counter.tick".into(),
            core_module_name: "test:res/res@1.0.0".into(),
            core_func_name: "[m[method]counter.tick".into(),
            realloc: None,
            string_enc: StringEncoding::Utf8,
        }],
        resources: vec![waclay::component::types::ResourceInfo {
            type_idx: 0,
            interface: Some("test:res/res@1.0.0".into()),
            name: "counter".into(),
        }],
        lifts: vec![LiftedExport {
            async_lift: false,
            callback: None,
            key: "bump".into(),
            func_name: "bump".into(),
            sig: FuncSig {
                params: vec![("rep".into(), TypeRef::Primitive(Primitive::U32))],
                result: Some(TypeRef::Primitive(Primitive::U32)),
            },
            core_func: CoreFuncRef::Export {
                instance: 0,
                name: "bump".into(),
            },
            post_return: None,
            realloc: None,
            string_enc: StringEncoding::Utf8,
        }],
        ..Default::default()
    };

    let engine = engine();
    let component = Component::from_plan(&engine, plan).unwrap();

    let mut linker = Linker::new();
    linker.func(
        Some("test:res/res@1.0.0"),
        "[m[method]counter.tick",
        |ctx: &mut waclay::HostCtx<'_>, args: &[Val], out: &mut Vec<Val>| {
            // resolve the borrowed handle to its rep, double it
            let rep = match &args[0] {
                Val::Handle(h) => ctx.tables.lock().unwrap().resource_rep(0, h.index())?,
                other => return Err(waclay::Error::value(format!("bad handle {other:?}"))),
            };
            out.push(Val::U32((rep * 2) as u32));
            Ok(())
        },
    );

    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();
    let bump = instance.typed_func::<(u32,), u32>("bump").unwrap();

    assert_eq!(bump.call(&mut store, (77,)).unwrap(), 308); // 77*2 + 77*2
                                                            // resource tables exercised: two allocations + two drops
    assert_eq!(instance.tables().lock().unwrap().live(0), 0);
}

// ---------------------------------------------------------------------------
// 6. _initialize reactor start
// ---------------------------------------------------------------------------

#[test]
fn initialize_runs_once() {
    let wasm = wat::parse_str(format!(
        r#"(module {SUPPORT}
            (global $g (mut i32) (i32.const 0))
            (func (export "_initialize")
                (global.set $g (i32.add (global.get $g) (i32.const 1))))
            (func (export "initialized") (result i32)
                (global.get $g)))
        "#
    ))
    .unwrap();

    let plan = ComponentPlan {
        core_module: wasm,
        types: types(),
        has_initialize: true,
        lifts: vec![LiftedExport {
            async_lift: false,
            callback: None,
            key: "initialized".into(),
            func_name: "initialized".into(),
            sig: FuncSig {
                params: vec![],
                result: Some(TypeRef::Primitive(Primitive::U32)),
            },
            core_func: CoreFuncRef::Export {
                instance: 0,
                name: "initialized".into(),
            },
            post_return: None,
            realloc: None,
            string_enc: StringEncoding::Utf8,
        }],
        ..Default::default()
    };

    let engine = engine();
    let component = Component::from_plan(&engine, plan).unwrap();
    let mut linker = Linker::new();
    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let f = instance.typed_func::<(), u32>("initialized").unwrap();
    assert_eq!(
        f.call(&mut store, ()).unwrap(),
        1,
        "_initialize ran at instantiate"
    );
}

// ---------------------------------------------------------------------------
// 7. WASI on a core module through the component pipeline
// ---------------------------------------------------------------------------

#[test]
fn wasi_p1_core_module() {
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
            (import "wasi_snapshot_preview1" "random_get"
                (func $random_get (param i32 i32) (result i32)))
            (import "wasi_snapshot_preview1" "args_sizes_get"
                (func $args_sizes (param i32 i32) (result i32)))
            {SUPPORT}

            ;; write "hi\n" to stdout via fd_write, then draw one random byte
            (func (export "run") (result i32)
                (local $iovec i32) (local $nwritten i32) (local $rand i32)
                (local.set $iovec (i32.const 64))
                (i32.store (local.get $iovec) (i32.const 72))   ;; msg ptr
                (i32.store offset=4 (local.get $iovec) (i32.const 3)) ;; len
                (i32.store (i32.const 72) (i32.const 682344))  ;; "hi\n"
                (drop (call $random_get (i32.const 96) (i32.const 4)))
                (call $fd_write (i32.const 1) (local.get $iovec) (i32.const 1)
                                (i32.const 80)))
            (func (export "args-count") (result i32)
                (drop (call $args_sizes (i32.const 100) (i32.const 104)))
                (i32.load (i32.const 100))))
        "#
    ))
    .unwrap();

    let engine = engine();
    let component = Component::from_core_module(&engine, &wasm).unwrap();

    let mut linker = Linker::new();
    waclay::wasi::add_p1_to_linker(&mut linker);

    let sink = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink2 = sink.clone();
    struct SharedSink(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for SharedSink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let ctx = WasiCtx::builder()
        .arg("prog")
        .arg("flag")
        .stdout(Box::new(SharedSink(sink2)))
        .build();

    let mut store = Store::new(&engine, ctx).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let mut out = [CoreVal::I32(0)];
    instance
        .call_core(&mut store, "run", &[], &mut out)
        .unwrap();
    assert_eq!(out[0].i32().unwrap(), 0, "fd_write errno SUCCESS");
    assert_eq!(&*sink.lock().unwrap(), b"hi\n");

    let mut out = [CoreVal::I32(0)];
    instance
        .call_core(&mut store, "args-count", &[], &mut out)
        .unwrap();
    assert_eq!(out[0].i32().unwrap(), 2, "two args registered");
}
