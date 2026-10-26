//! Nested component instantiation: a parent component embeds child
//! components, passes its own imports down (`ParentFunc` and `ParentImport`
//! argument sources), canon-lowers a child's export into its own core
//! module (the linker trampoline through the child's own Canonical ABI),
//! and re-exports child functions directly.

use waclay::component::types::Val;
use waclay::{Component, Engine, Linker, Store};
use wasm_encoder::{
    CanonicalOption, ComponentExportKind, ComponentTypeRef, ComponentValType, ExportKind,
    InstanceType, ModuleArg, PrimitiveValType as PV,
};

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

/// Child A: imports a ROOT-level function `add` (u32,u32)->u32 and exports
/// `doubled(a, b) = add(add(a, b), b)`.
fn child_a() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (import "" "add" (func $add (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "doubled") (param $a i32) (param $b i32) (result i32)
    (call $add (call $add (local.get $a) (local.get $b)) (local.get $b))))
"#,
    )
    .expect("child wat");

    let mut b = wasm_encoder::ComponentBuilder::default();
    let t_add = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    b.import("add", ComponentTypeRef::Func(t_add));

    let m = b.core_module_raw(None, &guest);
    // canon lower of the imported component func -> core func ("" "add")
    let lowered = b.lower_func(None, 0, [] as [CanonicalOption; 0]);
    let shim = b.core_instantiate_exports(Some("shim"), vec![("add", ExportKind::Func, lowered)]);
    let inst = b.core_instantiate(Some("guest"), m, [("", ModuleArg::Instance(shim))]);

    let cf = b.core_alias_export(None, inst, "doubled", ExportKind::Func);
    let t_doubled = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    let f = b.lift_func(None, cf, t_doubled, []);
    b.export("doubled", ComponentExportKind::Func, f, None);
    b.finish()
}

/// Child B: imports the interface instance `test:calc/ops` (func `add`) and
/// exports `triple(a, b) = add(add(a, b), add(a, b))`.
fn child_b() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (import "test:calc/ops" "add" (func $add (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "triple") (param $a i32) (param $b i32) (result i32)
    (call $add (call $add (local.get $a) (local.get $b))
                (call $add (local.get $a) (local.get $b)))))
"#,
    )
    .expect("child wat");

    let mut b = wasm_encoder::ComponentBuilder::default();

    // instance type: { add: func(u32, u32) -> u32 }
    let mut it = InstanceType::new();
    it.ty()
        .function()
        .params([
            ("a", ComponentValType::Primitive(PV::U32)),
            ("b", ComponentValType::Primitive(PV::U32)),
        ])
        .result(Some(ComponentValType::Primitive(PV::U32)));
    it.export("add", ComponentTypeRef::Func(0));
    let t_inst = b.type_instance(None, &it);
    // the import occupies component instance index 0
    b.import("test:calc/ops", ComponentTypeRef::Instance(t_inst));

    let m = b.core_module_raw(None, &guest);
    // alias the imported instance's `add` (component func index 0), then
    // canon lower it into the core func the shim imports
    let f_add = b.alias_export(0, "add", ComponentExportKind::Func);
    let lowered = b.lower_func(None, f_add, [] as [CanonicalOption; 0]);
    let shim = b.core_instantiate_exports(Some("shim"), vec![("add", ExportKind::Func, lowered)]);
    let inst = b.core_instantiate(
        Some("guest"),
        m,
        [("test:calc/ops", ModuleArg::Instance(shim))],
    );

    let cf = b.core_alias_export(None, inst, "triple", ExportKind::Func);
    let t_triple = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    let f = b.lift_func(None, cf, t_triple, []);
    b.export("triple", ComponentExportKind::Func, f, None);
    b.finish()
}

/// Parent: imports root `add`, embeds both children, wires:
///  - child A gets the root import `add`   (ParentFunc)
///  - child B gets an instance import      (ParentImport)
///
/// It lowers child A's `doubled` and child B's `triple` into its own core
/// module, and re-exports child A's `doubled` directly.
fn parent() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (import "child-a" "wrapped" (func $wrapped (param i32 i32) (result i32)))
  (import "child-b" "wrapped2" (func $wrapped2 (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "run") (result i32)
    (i32.add (call $wrapped (i32.const 10) (i32.const 5))
             (call $wrapped2 (i32.const 3) (i32.const 4)))))
"#,
    )
    .expect("parent wat");

    let child_a_bytes = child_a();
    let child_b_bytes = child_b();

    let mut b = wasm_encoder::ComponentBuilder::default();

    // parent's own imports
    let t_add = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    // root func import: component func index 0
    b.import("add", ComponentTypeRef::Func(t_add));
    // interface instance import: component instance index 0
    let mut it = InstanceType::new();
    it.ty()
        .function()
        .params([
            ("a", ComponentValType::Primitive(PV::U32)),
            ("b", ComponentValType::Primitive(PV::U32)),
        ])
        .result(Some(ComponentValType::Primitive(PV::U32)));
    it.export("add", ComponentTypeRef::Func(0));
    let t_inst = b.type_instance(None, &it);
    b.import("test:calc/ops", ComponentTypeRef::Instance(t_inst));

    // embed the children (component index space: A=0, B=1)
    let comp_a = b.component_raw(Some("child-a"), &child_a_bytes);
    let comp_b = b.component_raw(Some("child-b"), &child_b_bytes);

    // instantiate children with parent items
    // child A args: ("add", Func, parent root import index 0)
    let inst_a = b.instantiate(
        Some("a"),
        comp_a,
        vec![("add", ComponentExportKind::Func, 0u32)],
    );
    // child B args: ("test:calc/ops", Instance, parent import instance 0)
    let inst_b = b.instantiate(
        Some("b"),
        comp_b,
        vec![("test:calc/ops", ComponentExportKind::Instance, 0u32)],
    );

    // alias the children's exports (component funcs)
    let f_doubled = b.alias_export(inst_a, "doubled", ComponentExportKind::Func);
    let f_triple = b.alias_export(inst_b, "triple", ComponentExportKind::Func);

    // lower both into the parent's core func space
    let l1 = b.lower_func(None, f_doubled, [] as [CanonicalOption; 0]);
    let l2 = b.lower_func(None, f_triple, [] as [CanonicalOption; 0]);

    let m = b.core_module_raw(None, &guest);
    let shim_a =
        b.core_instantiate_exports(Some("shim-a"), vec![("wrapped", ExportKind::Func, l1)]);
    let shim_b =
        b.core_instantiate_exports(Some("shim-b"), vec![("wrapped2", ExportKind::Func, l2)]);
    let _pinst = b.core_instantiate(
        Some("parent-core"),
        m,
        [
            ("child-a", ModuleArg::Instance(shim_a)),
            ("child-b", ModuleArg::Instance(shim_b)),
        ],
    );

    // parent export: run() lifts the core export
    let run = b.core_alias_export(None, _pinst, "run", ExportKind::Func);
    let t_run = ftype(&mut b, &[], Some(PV::U32));
    let f_run = b.lift_func(None, run, t_run, []);
    b.export("run", ComponentExportKind::Func, f_run, None);

    // direct re-export of child A's doubled (goes through ChildExport)
    b.export("doubled_direct", ComponentExportKind::Func, f_doubled, None);
    b.finish()
}

fn host_add(
    _ctx: &mut waclay::HostCtx<'_>,
    args: &[Val],
    out: &mut Vec<Val>,
) -> waclay::Result<()> {
    let a: u32 = match args.first() {
        Some(Val::U32(x)) => *x,
        _ => 0,
    };
    let b: u32 = match args.get(1) {
        Some(Val::U32(x)) => *x,
        _ => 0,
    };
    out.push(Val::U32(a.wrapping_add(b)));
    Ok(())
}

#[test]
fn nested_composition_end_to_end() {
    for (name, engine) in engines() {
        let component = Component::from_binary(&engine, &parent()).unwrap();
        let mut linker = Linker::new();
        // root-level (for child A's root import) and per-interface
        // (for child B's instance import passed from the parent)
        linker.func(None, "add", host_add);
        linker.func(Some("test:calc/ops"), "add", host_add);

        let mut store = Store::new(&engine, ()).unwrap();
        let instance = linker.instantiate(&mut store, &component).unwrap();

        // parent core -> trampoline -> child A core -> host add
        // run() = doubled(10,5) + triple(3,4) = (15+5) + ((7)+(7)) = 20 + 14
        let out = instance.call_func(&mut store, "run", &[]).unwrap();
        assert_eq!(out, Some(Val::U32(34)), "{name}: full composition chain");

        // direct re-export of the child's function (ChildExport binding)
        let out = instance
            .call_func(&mut store, "doubled_direct", &[Val::U32(3), Val::U32(4)])
            .unwrap();
        assert_eq!(out, Some(Val::U32(11)), "{name}: direct child re-export");
    }
}

#[test]
fn nested_missing_parent_import_is_reported() {
    let engine = Engine::new(waclay_wasmi::WasmiEngine::default());
    let component = Component::from_binary(&engine, &parent()).unwrap();
    let mut linker = Linker::new(); // no `add` registered
    let mut store = Store::new(&engine, ()).unwrap();
    let msg = match linker.instantiate(&mut store, &component) {
        Ok(_) => panic!("instantiation should fail without the parent import"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("add"),
        "error should reference the missing parent import: {msg}"
    );
}
