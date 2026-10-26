//! End-to-end test of the `bindgen!` macro: a WIT world in `tests/wit/`
//! is compiled into typed Rust bindings, a hand-written guest implements
//! the world, and the host drives it through the generated API.

// The macro reads the WIT file relative to this crate's manifest dir.
waclay_macros::bindgen!("tests/wit/greeter.wit");

use waclay::abi::StringEncoding;
use waclay::component::decode::{ComponentPlan, CoreFuncRef, LiftedExport, LoweredImport};
use waclay::component::types::{FuncSig, Primitive, TypeDef, TypeRef, WorldImport};

use waclay::{Component, Engine, Linker, Store};
use waclay_wasmi::WasmiEngine;

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

fn greeter_plan() -> ComponentPlan {
    let wasm = wat::parse_str(format!(
        r#"(module
            (import "test:greeter/host@1.0.0" "log" (func $log (param i32 i32)))
            {SUPPORT}

            (func (export "add") (param $a i32) (param $b i32) (result i32)
                (i32.add (local.get $a) (local.get $b)))

            ;; swap(point) -> point: 2-flat result > MAX_FLAT_RESULTS -> retptr
            (func (export "swap") (param $x i32) (param $y i32) (result i32)
                (local $sp i32)
                (local.set $sp (call $realloc (i32.const 0) (i32.const 0)
                                              (i32.const 4) (i32.const 8)))
                (i32.store (local.get $sp) (local.get $y))
                (i32.store offset=4 (local.get $sp) (local.get $x))
                (local.get $sp))

            ;; area(shape) -> f64; shape flats: [i32 disc, i64 joined, i32]
            (func (export "area") (param $disc i32) (param $a i64) (param $b i32)
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

            ;; announce(msg) -> len: forwards the string to the host log
            (func (export "announce") (param $p i32) (param $l i32) (result i32)
                (call $log (local.get $p) (local.get $l))
                (local.get $l))
        )
        "#
    ))
    .unwrap();

    let types = vec![
        // 0: point
        TypeDef::Record(vec![
            ("x".into(), TypeRef::Primitive(Primitive::S32)),
            ("y".into(), TypeRef::Primitive(Primitive::S32)),
        ]),
        // 1: shape
        TypeDef::Variant(vec![
            ("circle".into(), Some(TypeRef::Primitive(Primitive::F64))),
            ("rect".into(), Some(TypeRef::Def(0))),
            ("none".into(), None),
        ]),
    ];

    let realloc = CoreFuncRef::Export {
        instance: 0,
        name: "cabi_realloc".into(),
    };

    ComponentPlan {
        core_module: wasm,
        types,
        imports: vec![WorldImport {
            name: "test:greeter/host@1.0.0".into(),
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
            interface: Some("test:greeter/host@1.0.0".into()),
            func_name: "log".into(),
            core_module_name: "test:greeter/host@1.0.0".into(),
            core_func_name: "log".into(),
            realloc: None,
            string_enc: StringEncoding::Utf8,
        }],
        lifts: vec![
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "calc|add".into(),
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
                key: "calc|swap".into(),
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
                key: "calc|area".into(),
                func_name: "area".into(),
                sig: FuncSig {
                    params: vec![("s".into(), TypeRef::Def(1))],
                    result: Some(TypeRef::Primitive(Primitive::F64)),
                },
                core_func: CoreFuncRef::Export {
                    instance: 0,
                    name: "area".into(),
                },
                post_return: None,
                realloc: None,
                string_enc: StringEncoding::Utf8,
            },
            LiftedExport {
                async_lift: false,
                callback: None,
                key: "calc|announce".into(),
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

// Host state implementing the generated `HostImports` trait.
#[derive(Default)]
struct Logger {
    lines: Vec<String>,
}

impl HostImports for Logger {
    fn log(&mut self, msg: String) {
        self.lines.push(msg);
    }
}

#[test]
fn bindgen_world_end_to_end() {
    let engine = Engine::new(WasmiEngine::default());
    let component = Component::from_plan(&engine, greeter_plan()).unwrap();

    let mut linker = Linker::new();
    add_host_to_linker(&mut linker, |data| {
        data.downcast_mut::<Logger>()
            .expect("store data must be Logger")
    });

    let mut store = Store::new(&engine, Logger::default()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let calc = exports(&instance);

    // primitives
    assert_eq!(calc.add(&mut store, 40, 2).unwrap(), 42);

    // generated record type round-trips through the Canonical ABI
    let swapped = calc.swap(&mut store, Point { x: 7, y: -3 }).unwrap();
    assert_eq!(swapped, Point { x: -3, y: 7 });

    // generated variant type (joined flat slots)
    let a = calc.area(&mut store, Shape::Circle(2.0)).unwrap();
    assert!((a - 12.5663706).abs() < 1e-4);
    let a = calc
        .area(&mut store, Shape::Rect(Point { x: 3, y: 4 }))
        .unwrap();
    assert_eq!(a, 12.0);
    assert_eq!(calc.area(&mut store, Shape::None).unwrap(), 0.0);

    // string param through realloc + host import through the trait
    let n = calc
        .announce(&mut store, "bindgen works".to_string())
        .unwrap();
    assert_eq!(n, "bindgen works".len() as u32);

    let logger: &Logger = store.data().downcast_ref::<Logger>().unwrap();
    assert_eq!(logger.lines, vec!["bindgen works".to_string()]);
}
