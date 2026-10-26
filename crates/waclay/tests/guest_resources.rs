//! Guest-exported resources: a component declares `(resource (rep i32))`,
//! exports it plus a constructor and a method, and the host drives handles
//! through the shared resource tables (`canon resource.new/drop/rep`).
//!
//! The guest's constructor allocates rep=123 via `[resource-new]kv`; the
//! guest's `get` recovers the rep via `[resource-rep]kv`. The host also
//! creates its own handles with `ComponentInstance::resource_new` and sees
//! them flow into guest methods — proving one shared table.

use waclay::component::types::{Handle, Val};
use waclay::{Component, Engine, Linker, Store};
use wasm_encoder::{
    ComponentExportKind, ComponentValType, ExportKind, PrimitiveValType as PV, ValType,
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

fn ftype(
    b: &mut wasm_encoder::ComponentBuilder,
    params: &[ComponentValType],
    result: Option<ComponentValType>,
) -> u32 {
    let (idx, mut enc) = b.type_function(None);
    let ps: Vec<(&str, ComponentValType)> = params.iter().map(|p| ("_", *p)).collect();
    enc.params(ps);
    enc.result(result);
    idx
}

fn kv_component() -> Vec<u8> {
    let guest = wat::parse_str(
        r#"
(module
  (import "demo:kv/api" "[resource-new]kv" (func $rnew (param i32) (result i32)))
  (import "demo:kv/api" "[resource-rep]kv" (func $rrep (param i32) (result i32)))
  (import "demo:kv/api" "[resource-drop]kv" (func $rdrop (param i32)))
  (memory (export "memory") 1)
  (func (export "kv_ctor") (result i32)
    (call $rnew (i32.const 123)))
  (func (export "kv_get") (param $h i32) (result i32)
    (call $rrep (local.get $h)))
  (func (export "kv_dtor") (param $h i32)
    (call $rdrop (local.get $h))))
"#,
    )
    .expect("guest wat");

    let mut b = wasm_encoder::ComponentBuilder::default();

    // the guest-defined resource type
    let r = b.type_resource(None, ValType::I32, None);
    let own_r = ComponentValType::Type(r);
    let borrow_r = ComponentValType::Type(r);

    let t_ctor = ftype(&mut b, &[], Some(own_r));
    let t_get = ftype(
        &mut b,
        &[borrow_r],
        Some(ComponentValType::Primitive(PV::U32)),
    );
    let t_dtor = ftype(&mut b, &[borrow_r], None);

    // canon resource intrinsics -> core funcs (the linker wires them into
    // the guest by import name via `parse_intrinsic`)
    let _c_new = b.resource_new(r);
    let _c_rep = b.resource_rep(r);
    let _c_drop = b.resource_drop(r);

    let m = b.core_module_raw(None, &guest);
    let inst = b.core_instantiate(Some("guest"), m, [] as [(&str, wasm_encoder::ModuleArg); 0]);

    // lift the guest functions
    let ctor = b.core_alias_export(None, inst, "kv_ctor", ExportKind::Func);
    let f_ctor = b.lift_func(None, ctor, t_ctor, []);
    let get = b.core_alias_export(None, inst, "kv_get", ExportKind::Func);
    let f_get = b.lift_func(None, get, t_get, []);
    let dtor = b.core_alias_export(None, inst, "kv_dtor", ExportKind::Func);
    let f_dtor = b.lift_func(None, dtor, t_dtor, []);

    // export the interface instance: resource type + methods
    let api = b.instantiate_exports(
        Some("kv-api"),
        vec![
            ("kv", ComponentExportKind::Type, r),
            ("[constructor]kv", ComponentExportKind::Func, f_ctor),
            ("[method]kv.get", ComponentExportKind::Func, f_get),
            ("[method]kv.drop", ComponentExportKind::Func, f_dtor),
        ],
    );
    b.export("demo:kv/api", ComponentExportKind::Instance, api, None);
    b.finish()
}

#[test]
fn guest_resource_lifecycle() {
    for (name, engine) in engines() {
        let component = Component::from_binary(&engine, &kv_component()).unwrap();
        let mut linker = Linker::new();
        let mut store = Store::new(&engine, ()).unwrap();
        let instance = linker.instantiate(&mut store, &component).unwrap();

        let rid = instance.resource_id(Some("demo:kv/api"), "kv").unwrap();

        // 1) guest constructor returns a handle into the shared table
        let out = instance
            .call_func(&mut store, "demo:kv/api|[constructor]kv", &[])
            .unwrap();
        let h = match out {
            Some(Val::Handle(h @ Handle::Own { .. })) => {
                assert_eq!(h.resource(), rid, "{name}: handle names the right type");
                h
            }
            other => panic!("{name}: expected a handle, got {other:?}"),
        };

        // 2) the guest method reads the rep back through resource-rep
        let rep = instance
            .call_func(&mut store, "demo:kv/api|[method]kv.get", &[Val::Handle(h)])
            .unwrap();
        assert_eq!(rep, Some(Val::U32(123)), "{name}: guest sees its own rep");
        assert_eq!(instance.live_resources(rid), 1, "{name}: one live handle");

        // 3) host-side resource_new: the host creates a handle the guest
        //    can use (proving the shared table)
        let h2_idx = instance.resource_new(rid, 999);
        let h2 = Handle::Own {
            resource: rid,
            index: h2_idx,
        };
        let rep2 = instance
            .call_func(&mut store, "demo:kv/api|[method]kv.get", &[Val::Handle(h2)])
            .unwrap();
        assert_eq!(
            rep2,
            Some(Val::U32(999)),
            "{name}: host handle reaches guest"
        );
        assert_eq!(instance.live_resources(rid), 2, "{name}: two live handles");

        // 4) host-side rep + drop
        assert_eq!(instance.resource_rep(&h).unwrap(), 123);
        assert_eq!(instance.resource_rep(&h2).unwrap(), 999);
        instance.resource_drop(h2).unwrap();
        assert_eq!(instance.live_resources(rid), 1, "{name}: after host drop");

        // 5) guest-side drop via the exported method
        instance
            .call_func(&mut store, "demo:kv/api|[method]kv.drop", &[Val::Handle(h)])
            .unwrap();
        assert_eq!(instance.live_resources(rid), 0, "{name}: after guest drop");

        // 6) a dropped handle is rejected
        let stale = instance
            .call_func(&mut store, "demo:kv/api|[method]kv.get", &[Val::Handle(h)])
            .unwrap_err();
        assert!(
            stale.to_string().contains("invalid resource handle")
                || stale.to_string().contains("no table"),
            "{name}: stale handle should trap clearly, got: {stale}"
        );
    }
}
