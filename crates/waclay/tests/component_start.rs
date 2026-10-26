//! Component start section conformance: value imports, the start function
//! (invoked at the end of instantiation with the host-provided values), and
//! the start result value. Encoded with `wasm-encoder`, run on all engines.

use waclay::component::types::Val;
use waclay::{Component, Engine, Error, Extern, Linker, Store};
use wasm_encoder::{ComponentTypeRef, ComponentValType, PrimitiveValType as PV};

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

/// A component that imports two values (`cfg-x: u32`, `cfg-y: u64`), starts
/// with `run_start(x, y) -> u32` (the guest stores both and returns x+y),
/// and produces one result value.
fn start_component() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (memory (export "memory") 1)
  (func (export "run_start") (param $x i32) (param $y i64) (result i32)
    (i32.store (i32.const 64) (local.get $x))
    (i64.store (i32.const 72) (local.get $y))
    (i32.wrap_i64 (i64.add (i64.extend_i32_s (local.get $x)) (local.get $y)))))
"#,
    )
    .expect("guest wat");

    let mut b = wasm_encoder::ComponentBuilder::default();
    let t_run = ftype(&mut b, &[PV::U32, PV::U64], Some(PV::U32));

    // value imports occupy the value index space in declaration order
    b.import(
        "cfg-x",
        ComponentTypeRef::Value(ComponentValType::Primitive(PV::U32)),
    );
    b.import(
        "cfg-y",
        ComponentTypeRef::Value(ComponentValType::Primitive(PV::U64)),
    );

    let m = b.core_module_raw(None, &guest);
    let inst = b.core_instantiate(Some("guest"), m, [] as [(&str, wasm_encoder::ModuleArg); 0]);
    let cf = b.core_alias_export(None, inst, "run_start", wasm_encoder::ExportKind::Func);
    let f = b.lift_func(None, cf, t_run, []);

    // start: call `f` with value indices 0 and 1; one result value
    b.start(f, vec![0, 1], 1);
    b.finish()
}

#[test]
fn start_receives_host_values_and_returns_result() {
    for (name, engine) in engines() {
        let component = Component::from_binary(&engine, &start_component()).unwrap();
        let mut linker = Linker::new();
        linker.value("cfg-x", Val::U32(40));
        linker.value("cfg-y", Val::U64(2));

        let mut store = Store::new(&engine, ()).unwrap();
        let instance = linker.instantiate(&mut store, &component).unwrap();

        // the start function ran during instantiation and returned x+y
        assert_eq!(
            instance.start_result(),
            Some(&Val::U32(42)),
            "{name}: start result"
        );

        // the guest recorded both values in its memory
        let mem = match store.get_export(instance.core_instance(), "memory") {
            Some(Extern::Memory(m)) => m,
            other => panic!("{name}: memory export missing: {other:?}"),
        };
        let mut buf = [0u8; 16];
        store.memory_read(&mem, 64, &mut buf).unwrap();
        assert_eq!(u32::from_le_bytes(buf[0..4].try_into().unwrap()), 40);
        // i64 lives at address 72 => buf[8..16]
        assert_eq!(u64::from_le_bytes(buf[8..16].try_into().unwrap()), 2);
    }
}

#[test]
fn start_without_values_fails_with_clear_error() {
    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    let component = Component::from_binary(&engine, &start_component()).unwrap();
    let mut linker = Linker::new();
    linker.value("cfg-x", Val::U32(1));
    // `cfg-y` deliberately missing

    let mut store = Store::new(&engine, ()).unwrap();
    let err = match linker.instantiate(&mut store, &component) {
        Ok(_) => panic!("instantiation should fail when a value import is missing"),
        Err(e) => e,
    };
    match &err {
        Error::MissingImport { module, name } => {
            assert_eq!(module, "value");
            assert!(
                name.contains("cfg-y"),
                "error should name the missing value: {name}"
            );
        }
        other => panic!("expected MissingImport, got: {other:?}"),
    }
}

/// Same shape, but the start function returns nothing.
fn start_void_component() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (memory (export "memory") 1)
  (global $seen (mut i32) (i32.const 0))
  (func (export "run_start") (param $x i32)
    (global.set $seen (local.get $x)))
  (func (export "peek") (result i32) (global.get $seen)))
"#,
    )
    .expect("guest wat");

    let mut b = wasm_encoder::ComponentBuilder::default();
    let t_run = ftype(&mut b, &[PV::U32], None);
    b.import(
        "cfg",
        ComponentTypeRef::Value(ComponentValType::Primitive(PV::U32)),
    );
    let m = b.core_module_raw(None, &guest);
    let inst = b.core_instantiate(Some("guest"), m, [] as [(&str, wasm_encoder::ModuleArg); 0]);
    let peek = b.core_alias_export(None, inst, "peek", wasm_encoder::ExportKind::Func);
    let t_peek = ftype(&mut b, &[], Some(PV::U32));
    let f_peek = b.lift_func(None, peek, t_peek, []);
    b.export(
        "peek",
        wasm_encoder::ComponentExportKind::Func,
        f_peek,
        None,
    );

    let cf = b.core_alias_export(None, inst, "run_start", wasm_encoder::ExportKind::Func);
    let f = b.lift_func(None, cf, t_run, []);
    b.start(f, vec![0], 0);
    b.finish()
}

#[test]
fn start_without_result_value_runs() {
    for (name, engine) in engines() {
        let component = Component::from_binary(&engine, &start_void_component()).unwrap();
        let mut linker = Linker::new();
        linker.value("cfg", Val::U32(77));
        let mut store = Store::new(&engine, ()).unwrap();
        let instance = linker.instantiate(&mut store, &component).unwrap();

        assert!(instance.start_result().is_none(), "{name}: no result");
        let out = instance.call_func(&mut store, "peek", &[]).unwrap();
        assert_eq!(out, Some(Val::U32(77)), "{name}: start side effect visible");
    }
}
