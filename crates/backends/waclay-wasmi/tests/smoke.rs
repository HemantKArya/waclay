//! wasmi 2.0 backend smoke test: the trait layer works end to end.

use waclay::engine::{Val, ValType};
use waclay::{Engine, Extern, FuncType, Imports};

const ADD_WAT: &str = r#"
    (module
        (func (export "add") (param i32 i32) (result i32)
            local.get 0
            local.get 1
            i32.add)
        (func (export "mul64") (param i64 i64) (result i64)
            local.get 0
            local.get 1
            i64.mul)
        (memory (export "memory") 1)
        (func (export "fill") (param i32 i32)
            (memory.fill (local.get 0) (i32.const 0xAB) (local.get 1)))
    )
"#;

#[test]
fn wasmi_core_module_roundtrip() {
    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    assert_eq!(engine.name(), "wasmi");

    let wasm = wat::parse_str(ADD_WAT).unwrap();
    let module = engine.compile(&wasm).unwrap();

    let names: Vec<&str> = module.exports().iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"memory"));

    let mut store = engine.new_store().unwrap();
    let imports = Imports::new();
    let instance = module.instantiate(&mut store, &imports).unwrap();

    // get export + call
    let add = match store.get_export(&instance, "add") {
        Some(Extern::Func(f)) => f,
        other => panic!("expected add func, got {other:?}"),
    };
    let ty = store.func_type(&add).unwrap();
    assert_eq!(
        ty,
        FuncType::new([ValType::I32, ValType::I32], [ValType::I32])
    );

    let mut results = [Val::I32(0)];
    store
        .call(&add, &[Val::I32(20), Val::I32(22)], &mut results)
        .unwrap();
    assert_eq!(results[0], Val::I32(42));

    // i64 path
    let mul64 = match store.get_export(&instance, "mul64") {
        Some(Extern::Func(f)) => f,
        _ => panic!("missing mul64"),
    };
    let mut results = [Val::I64(0)];
    store
        .call(&mul64, &[Val::I64(6), Val::I64(7)], &mut results)
        .unwrap();
    assert_eq!(results[0], Val::I64(42));

    // memory path
    let memory = match store.get_export(&instance, "memory") {
        Some(Extern::Memory(m)) => m,
        _ => panic!("missing memory"),
    };
    let fill = match store.get_export(&instance, "fill") {
        Some(Extern::Func(f)) => f,
        _ => panic!("missing fill"),
    };
    store
        .call(&fill, &[Val::I32(16), Val::I32(8)], &mut [])
        .unwrap();
    let mut buf = [0u8; 8];
    store.memory_read(&memory, 16, &mut buf).unwrap();
    assert!(buf.iter().all(|b| *b == 0xAB));

    // grow
    let old = store.memory_grow(&memory, 1).unwrap();
    assert_eq!(old, 1);
    assert_eq!(store.memory_size(&memory).unwrap(), 2 * 64 * 1024);
}

#[test]
fn wasmi_host_func_roundtrip() {
    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    let wasm = wat::parse_str(
        r#"
        (module
            (import "host" "double" (func $double (param i32) (result i32)))
            (import "host" "note" (func $note (param i32)))
            (memory (export "memory") 1)
            (func (export "call_double") (param i32) (result i32)
                local.get 0
                call $double)
            (func (export "call_note") (param i32)
                local.get 0
                call $note)
        )
        "#,
    )
    .unwrap();

    let module = engine.compile(&wasm).unwrap();

    let mut store = engine.new_store().unwrap();
    store.set_data(Box::new(Vec::<String>::new()));

    let mut imports = Imports::new();
    let double = store
        .new_func(
            FuncType::new([ValType::I32], [ValType::I32]),
            Box::new(|_caller, args, results| {
                let x = args[0].i32()?;
                results.push(Val::I32(x * 2));
                Ok(())
            }),
        )
        .unwrap();

    // host func writing to guest memory mid-call (CallerView path)
    let memory = store
        .new_memory(waclay::MemoryType { min: 1, max: None })
        .unwrap();
    let note_mem = memory.clone();
    let note = store
        .new_func(
            FuncType::new([ValType::I32], []),
            Box::new(move |caller, args, _results| {
                let msg = b"hi from host";
                caller.memory_write(&note_mem, args[0].i32()? as usize, msg)?;
                Ok(())
            }),
        )
        .unwrap();

    imports.add_func("host", "double", double);
    imports.add_func("host", "note", note);
    imports.add_memory("host", "memory", memory.clone());

    let instance = module.instantiate(&mut store, &imports).unwrap();
    let f = match store.get_export(&instance, "call_double") {
        Some(Extern::Func(f)) => f,
        _ => panic!(),
    };
    let mut out = [Val::I32(0)];
    store.call(&f, &[Val::I32(21)], &mut out).unwrap();
    assert_eq!(out[0], Val::I32(42));

    // guest -> host memory write observed in the shared memory
    let note_call = match store.get_export(&instance, "call_note") {
        Some(Extern::Func(f)) => f,
        _ => panic!(),
    };
    store.call(&note_call, &[Val::I32(64)], &mut []).unwrap();
    let mut buf = [0u8; 12];
    store.memory_read(&memory, 64, &mut buf).unwrap();
    assert_eq!(&buf, b"hi from host");
}

#[test]
fn wasmi_trap_propagates() {
    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    let wasm = wat::parse_str(
        r#"(module (import "h" "f" (func $h (result i32))) (func (export "boom") unreachable) (func (export "hosterr") (result i32) (call $h)))"#,
    )
    .unwrap();
    let module = engine.compile(&wasm).unwrap();
    let mut store = engine.new_store().unwrap();
    let mut imports = Imports::new();
    let hf = store
        .new_func(
            FuncType::new([], [ValType::I32]),
            Box::new(|_c, _a, _r| Err(waclay::Error::Host("guest misbehaved".into()))),
        )
        .unwrap();
    imports.add_func("h", "f", hf);
    let inst = module.instantiate(&mut store, &imports).unwrap();

    let boom = match store.get_export(&inst, "boom") {
        Some(Extern::Func(f)) => f,
        _ => panic!(),
    };
    let err = store.call(&boom, &[], &mut []).unwrap_err();
    assert!(err.to_string().contains("trap") || err.to_string().contains("unreachable"));

    let hosterr = match store.get_export(&inst, "hosterr") {
        Some(Extern::Func(f)) => f,
        _ => panic!(),
    };
    let err = store.call(&hosterr, &[], &mut []).unwrap_err();
    assert!(err.to_string().contains("misbehaved"));
}
