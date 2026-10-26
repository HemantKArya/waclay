//! Canonical ABI layout conformance tests (spec: CanonicalABI.md).

use waclay::abi::layout::{alignment, flat_types, size, Context, CoreTy, FuncLayout};
use waclay::component::types::{Primitive, TypeDef, TypeRef};

fn table() -> Vec<TypeDef> {
    vec![
        // 0: record { a: u32, b: u64 }
        TypeDef::Record(vec![
            ("a".into(), TypeRef::Primitive(Primitive::U32)),
            ("b".into(), TypeRef::Primitive(Primitive::U64)),
        ]),
        // 1: variant { none, some(u64) }
        TypeDef::Variant(vec![
            ("none".into(), None),
            ("some".into(), Some(TypeRef::Primitive(Primitive::U64))),
        ]),
        // 2: list<u8>
        TypeDef::List(TypeRef::Primitive(Primitive::U8)),
        // 3: string
        TypeDef::Primitive(Primitive::String),
        // 4: flags with 3 names
        TypeDef::Flags(vec!["a".into(), "b".into(), "c".into()]),
        // 5: enum with 4 cases
        TypeDef::Enum(vec!["w".into(), "x".into(), "y".into(), "z".into()]),
        // 6: tuple<u32, string>
        TypeDef::Tuple(vec![
            TypeRef::Primitive(Primitive::U32),
            TypeRef::Primitive(Primitive::String),
        ]),
        // 7: option<u32>
        TypeDef::Option(TypeRef::Primitive(Primitive::U32)),
        // 8: result<_, string>
        TypeDef::Result {
            ok: None,
            err: Some(TypeRef::Primitive(Primitive::String)),
        },
        // 9: big record with 20 u32 fields (forces indirect params)
        TypeDef::Record(
            (0..20)
                .map(|i| (format!("f{i}"), TypeRef::Primitive(Primitive::U32)))
                .collect(),
        ),
    ]
}

#[test]
fn primitive_layouts() {
    let t = table();
    assert_eq!(alignment(&TypeRef::Primitive(Primitive::Bool), &t), 1);
    assert_eq!(alignment(&TypeRef::Primitive(Primitive::U16), &t), 2);
    assert_eq!(alignment(&TypeRef::Primitive(Primitive::Char), &t), 4);
    assert_eq!(size(&TypeRef::Primitive(Primitive::Char), &t), 4);
    assert_eq!(size(&TypeRef::Primitive(Primitive::String), &t), 8);
    assert_eq!(
        flat_types(&TypeRef::Primitive(Primitive::String), &t),
        vec![CoreTy::I32, CoreTy::I32]
    );
}

#[test]
fn record_layout_is_packed_with_alignment() {
    let t = table();
    let r = TypeRef::Def(0);
    // a at 0, padding to 8, b at 8 => size 16, align 8
    assert_eq!(alignment(&r, &t), 8);
    assert_eq!(size(&r, &t), 16);
}

#[test]
fn variant_flat_uses_join() {
    let t = table();
    // variant { none, some(u64) } => [i32 discriminant] + [i64 payload]
    let v = TypeRef::Def(1);
    assert_eq!(flat_types(&v, &t), vec![CoreTy::I32, CoreTy::I64]);
    // memory repr: 4-byte discriminant aligned to 8, payload at 8 => 16
    assert_eq!(size(&v, &t), 16);
}

#[test]
fn flags_and_enums() {
    let t = table();
    assert_eq!(flat_types(&TypeRef::Def(4), &t), vec![CoreTy::I32]);
    assert_eq!(flat_types(&TypeRef::Def(5), &t), vec![CoreTy::I32]);
    // 40 flags => one i64 (spec: 33..=64 flags pack into i64)
    let big = TypeDef::Flags((0..40).map(|i| format!("f{i}")).collect());
    let t2 = vec![big];
    assert_eq!(flat_types(&TypeRef::Def(0), &t2), vec![CoreTy::I64]);
    assert_eq!(size(&TypeRef::Def(0), &t2), 8);
    // 96 flags => record of three i32 chunks
    let huge = TypeDef::Flags((0..96).map(|i| format!("f{i}")).collect());
    let t3 = vec![huge];
    assert_eq!(
        flat_types(&TypeRef::Def(0), &t3),
        vec![CoreTy::I32, CoreTy::I32, CoreTy::I32]
    );
    assert_eq!(size(&TypeRef::Def(0), &t3), 12);
}

#[test]
fn direct_signature_for_small_funcs() {
    let t = table();
    // func(a: u32, b: u32) -> u32
    let l = FuncLayout::new(
        &[("a".into(), TypeRef::Primitive(Primitive::U32))],
        Some(TypeRef::Primitive(Primitive::U32)),
        &t,
    );
    assert!(!l.params_indirect);
    assert!(!l.results_indirect);
    assert_eq!(l.core_params(Context::Lift), vec![CoreTy::I32]);
    assert_eq!(l.core_results(Context::Lift), vec![CoreTy::I32]);
    assert_eq!(l.core_params(Context::Lower), vec![CoreTy::I32]);
    assert_eq!(l.core_results(Context::Lower), vec![CoreTy::I32]);
}

#[test]
fn heap_results_use_retptr() {
    let t = table();
    // func(x: u32) -> string  (heap result)
    let l = FuncLayout::new(
        &[("x".into(), TypeRef::Primitive(Primitive::U32))],
        Some(TypeRef::Def(3)),
        &t,
    );
    assert!(l.results_indirect);
    // lift: callee returns the retptr
    assert_eq!(l.core_params(Context::Lift), vec![CoreTy::I32]);
    assert_eq!(l.core_results(Context::Lift), vec![CoreTy::I32]);
    // lower: caller passes retptr as trailing param
    assert_eq!(
        l.core_params(Context::Lower),
        vec![CoreTy::I32, CoreTy::I32]
    );
    assert_eq!(l.core_results(Context::Lower), vec![]);
}

#[test]
fn oversized_params_go_indirect() {
    let t = table();
    // func(big: record-of-20-u32) -> ()  => 20 flats > 16 => indirect
    let l = FuncLayout::new(&[("big".into(), TypeRef::Def(9))], None, &t);
    assert!(l.params_indirect);
    assert_eq!(l.core_params(Context::Lift), vec![CoreTy::I32]);
    // param area: 20 * 4 bytes
    assert_eq!(l.param_area_size, 80);
}

#[test]
fn string_param_direct_flats() {
    let t = table();
    // func(s: string, n: u32) -> u32
    let l = FuncLayout::new(
        &[
            ("s".into(), TypeRef::Primitive(Primitive::String)),
            ("n".into(), TypeRef::Primitive(Primitive::U32)),
        ],
        Some(TypeRef::Primitive(Primitive::U32)),
        &t,
    );
    assert!(!l.params_indirect);
    assert_eq!(
        l.core_params(Context::Lift),
        vec![CoreTy::I32, CoreTy::I32, CoreTy::I32]
    );
}

// ---------------------------------------------------------------------------
// regression: option/result despecialization must resolve nested payloads
// against the REAL type table (wasip2 stream-error chains hit this)
// ---------------------------------------------------------------------------

#[test]
fn option_and_result_flatten_nested_defs() {
    let t = table(); // 10 entries (0..9); indices must resolve, never panic

    // existing option<u32> at 7: [i32, i32]
    assert_eq!(
        flat_types(&TypeRef::Def(7), &t),
        vec![CoreTy::I32, CoreTy::I32]
    );
    // pushed option<u64> at index 10: [i32, i64]
    assert_eq!(
        flat_types(&TypeRef::Def(10), &option_table(&t)),
        vec![CoreTy::I32, CoreTy::I64]
    );

    // stream-error chain built exactly like wasi:io/streams:
    //   0' own<1'>  1' variant{last-operation-failed(own<0'>), closed}
    //   2' result<_, 1'>
    let types = vec![
        TypeDef::Own(0), // 0: resource #0 (output-stream)
        TypeDef::Variant(vec![
            ("last-operation-failed".into(), Some(TypeRef::Def(0))),
            ("closed".into(), None),
        ]), // 1: stream-error
        TypeDef::Result {
            ok: None,
            err: Some(TypeRef::Def(1)),
        }, // 2: result<_, stream-error>
    ];
    // result<_, stream-error> = [discrim] ++ flatten(stream-error)
    //   = [i32] ++ [i32 (variant discrim), i32 (own payload)] — 3 flats,
    //   which is exactly why real P2 write results are indirect (retptr).
    assert_eq!(
        flat_types(&TypeRef::Def(2), &types),
        vec![CoreTy::I32, CoreTy::I32, CoreTy::I32]
    );
    // result<_, u64> joins to [i32, i64]
    let types2 = vec![TypeDef::Result {
        ok: None,
        err: Some(TypeRef::Primitive(Primitive::U64)),
    }];
    assert_eq!(
        flat_types(&TypeRef::Def(0), &types2),
        vec![CoreTy::I32, CoreTy::I64]
    );
    // option<string> -> [i32, i32, i32] (discrim + ptr + len)
    let types3 = vec![TypeDef::Option(TypeRef::Primitive(Primitive::String))];
    assert_eq!(
        flat_types(&TypeRef::Def(0), &types3),
        vec![CoreTy::I32, CoreTy::I32, CoreTy::I32]
    );
}

fn option_table(base: &[TypeDef]) -> Vec<TypeDef> {
    let mut v = base.to_vec();
    v.push(TypeDef::Option(TypeRef::Primitive(Primitive::U64)));
    v
}
