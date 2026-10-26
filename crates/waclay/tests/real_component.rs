//! Real-component integration test: components produced by `wit-component`
//! (the exact output of `cargo component build`) must decode, instantiate
//! and run.
//!
//! The fixture embeds WIT world metadata into a tiny core module via
//! `wit_component::embed_component_metadata`, then wraps it with
//! `ComponentEncoder` — yielding a genuine component binary (type sections,
//! canonical lift/lower, aliases), not a hand-written plan.

use waclay::component::decode::decode;
use waclay::component::types::{Primitive, TypeRef, Val};
use waclay::{Component, Engine, Linker, Store};
use waclay_wasmi::WasmiEngine;

/// Core module importing `test:calc/ops.add` and exporting `do-add` + memory.
fn core_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (import "test:calc/ops" "add" (func $add (param i32 i32) (result i32)))
            (memory (export "memory") 1)
            (func (export "do-add") (param i32 i32) (result i32)
                (call $add (local.get 0) (local.get 1)))
        )
        "#,
    )
    .unwrap()
}

/// A real component binary built the same way cargo-component does it.
fn real_component() -> Vec<u8> {
    let mut resolve = wit_parser::Resolve::new();
    let pkg = resolve
        .push_str(
            "test:calc",
            r#"
package test:calc;

interface ops {
    add: func(a: u32, b: u32) -> u32;
}

world calc-world {
    import ops;
    export do-add: func(a: u32, b: u32) -> u32;
}
"#,
        )
        .unwrap();
    let world = resolve
        .select_world(&[pkg], Some("test:calc/calc-world"))
        .unwrap();

    let mut module = core_wasm();
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
        true,
    )
    .unwrap();

    wit_component::ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .validate(true)
        .encode()
        .unwrap()
}

#[test]
fn real_wit_component_decodes_and_runs() {
    let component_bytes = real_component();

    // sanity: the encoder output really is a component
    assert_eq!(&component_bytes[0..4], b"\0asm");
    assert_eq!(&component_bytes[4..6], b"\x0d\0", "component version layer");

    // 1) the decoder understands a REAL component binary
    let plan = decode(&component_bytes).expect("decode real component");
    assert!(
        !plan.lifts.is_empty(),
        "real component must produce lifted exports, got {} exports",
        plan.exports.len()
    );

    // 2) it runs on the engine
    let engine = Engine::new(WasmiEngine::default());
    let component = Component::from_binary(&engine, &component_bytes).expect("from_binary");

    let mut linker = Linker::new();
    linker.func(Some("test:calc/ops"), "add", |_ctx, args, out| {
        let (a, b) = match (&args[0], &args[1]) {
            (Val::U32(a), Val::U32(b)) => (a, b),
            other => panic!("expected u32 pair, got {other:?}"),
        };
        out.push(Val::U32(a.wrapping_add(*b)));
        Ok(())
    });

    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component).unwrap();

    let out = instance
        .call_func(&mut store, "do-add", &[Val::U32(40), Val::U32(2)])
        .expect("call do-add");
    assert_eq!(out, Some(Val::U32(42)));
}

#[test]
fn core_module_decodes_as_core_only_plan() {
    let core = wat::parse_str("(module (func))").unwrap();
    let plan = decode(&core).expect("core module decodes with empty plan");
    assert!(plan.lifts.is_empty() && plan.lowers.is_empty());
}

#[test]
fn component_type_parsing_sees_primitives() {
    // u32 flat class used by layouts
    assert_eq!(
        waclay::abi::layout::flat_types(&TypeRef::Primitive(Primitive::U32), &[]),
        vec![waclay::abi::layout::CoreTy::I32]
    );
}
