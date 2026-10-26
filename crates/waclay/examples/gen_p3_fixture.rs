use wasm_encoder::{
    CanonicalOption, ComponentExportKind, ComponentValType, ExportKind, ModuleArg,
    PrimitiveValType as PV, ValType,
};

const GUEST: &str = r#"
(module
  (import "p3" "[stream-new]" (func $snew (result i64)))
  (import "p3" "[stream-read]" (func $sread (param i32 i32 i32) (result i32)))
  (import "p3" "[stream-read-async]" (func $sreada (param i32 i32 i32) (result i32)))
  (import "p3" "[stream-write]" (func $swrite (param i32 i32 i32) (result i32)))
  (import "p3" "[stream-cancel-read]" (func $scread (param i32) (result i32)))
  (import "p3" "[stream-cancel-write]" (func $scwrite (param i32) (result i32)))
  (import "p3" "[stream-drop-readable]" (func $sdropr (param i32)))
  (import "p3" "[stream-drop-writable]" (func $sdropw (param i32)))
  (import "p3" "[stream-forward]" (func $sfwd (param i32 i32)))
  (import "p3" "[future-new]" (func $fnew (result i64)))
  (import "p3" "[future-read]" (func $fread (param i32 i32) (result i32)))
  (import "p3" "[future-read-async]" (func $freada (param i32 i32) (result i32)))
  (import "p3" "[future-write]" (func $fwrite (param i32 i32) (result i32)))
  (import "p3" "[future-drop-readable]" (func $fdropr (param i32)))
  (import "p3" "[future-drop-writable]" (func $fdropw (param i32)))
  (import "p3" "[future-forward]" (func $ffwd (param i32 i32)))
  (import "p3" "[waitable-set-new]" (func $wsnew (result i32)))
  (import "p3" "[waitable-set-wait]" (func $wswait (param i32 i32) (result i32)))
  (import "p3" "[waitable-set-poll]" (func $wspoll (param i32 i32) (result i32)))
  (import "p3" "[waitable-set-drop]" (func $wsdrop (param i32)))
  (import "p3" "[waitable-join]" (func $wjoin (param i32 i32)))
  (import "p3" "[error-context-new]" (func $ecnew (param i32 i32) (result i32)))
  (import "p3" "[error-context-debug-message]" (func $ecmsg (param i32 i32)))
  (import "p3" "[error-context-drop]" (func $ecdrop (param i32)))
  (import "p3" "[backpressure-inc]" (func $binc))
  (import "p3" "[backpressure-dec]" (func $bdec))
  (import "p3" "[thread-yield]" (func $yld (result i32)))
  (import "p3" "[context-get]" (func $cget (result i32)))
  (import "p3" "[context-set]" (func $cset (param i32)))
  (memory (export "memory") 1)

  (global $heap (mut i32) (i32.const 8192))
  (func (export "cabi_realloc") (param $old i32) (param $oldsz i32)
        (param $align i32) (param $newsz i32) (result i32)
    (local $p i32)
    (global.set $heap
      (i32.and
        (i32.add (global.get $heap) (i32.sub (local.get $align) (i32.const 1)))
        (i32.xor (i32.sub (local.get $align) (i32.const 1)) (i32.const -1))))
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $newsz)))
    (local.get $p))

  (func (export "snew") (result i64) call $snew)
  (func (export "sread") (param i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 call $sread)
  (func (export "sreada") (param i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 call $sreada)
  (func (export "swrite") (param i32 i32 i32) (result i32)
    local.get 0 local.get 1 local.get 2 call $swrite)
  (func (export "scread") (param i32) (result i32) local.get 0 call $scread)
  (func (export "scwrite") (param i32) (result i32) local.get 0 call $scwrite)
  (func (export "sdropr") (param i32) local.get 0 call $sdropr)
  (func (export "sdropw") (param i32) local.get 0 call $sdropw)
  (func (export "sfwd") (param i32 i32) local.get 0 local.get 1 call $sfwd)

  (func (export "fnew") (result i64) call $fnew)
  (func (export "fread") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $fread)
  (func (export "freada") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $freada)
  (func (export "fwrite") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $fwrite)
  (func (export "fdropr") (param i32) local.get 0 call $fdropr)
  (func (export "fdropw") (param i32) local.get 0 call $fdropw)
  (func (export "ffwd") (param i32 i32) local.get 0 local.get 1 call $ffwd)

  (func (export "wsnew") (result i32) call $wsnew)
  (func (export "wswait") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $wswait)
  (func (export "wspoll") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $wspoll)
  (func (export "wsdrop") (param i32) local.get 0 call $wsdrop)
  (func (export "wjoin") (param i32 i32) local.get 0 local.get 1 call $wjoin)

  (func (export "ecnew") (param i32 i32) (result i32)
    local.get 0 local.get 1 call $ecnew)
  (func (export "ecmsg") (param i32 i32) local.get 0 local.get 1 call $ecmsg)
  (func (export "ecdrop") (param i32) local.get 0 call $ecdrop)

  (func (export "binc") call $binc)
  (func (export "bdec") call $bdec)
  (func (export "yld") (result i32) call $yld)
  (func (export "cget") (result i32) call $cget)
  (func (export "cset") (param i32) local.get 0 call $cset)

  ;; pump: one sync read from $r, then write whatever was read into $w,
  ;; returning the packed read payload
  (func (export "pump") (param $r i32) (param $w i32) (param $buf i32)
        (param $cap i32) (result i32)
    (local $n i32)
    (local.set $n (call $sread (local.get $r) (local.get $buf) (local.get $cap)))
    (if (i32.eq (i32.and (local.get $n) (i32.const 15)) (i32.const 1))
      (then (call $sdropw (local.get $w)) (return (local.get $n))))
    (drop (call $swrite (local.get $w) (local.get $buf)
                        (i32.shr_u (local.get $n) (i32.const 4))))
    (local.get $n))

  ;; async read, then join+poll a waitable set; returns the poll event code
  ;; and stores (p1, p2) at 1024
  (func (export "pread_async") (param $r i32) (param $ptr i32) (param $n i32)
        (param $wset i32) (result i32)
    (drop (call $sreada (local.get $r) (local.get $ptr) (local.get $n)))
    (call $wjoin (local.get $r) (local.get $wset))
    (call $wspoll (local.get $wset) (i32.const 1024)))

  ;; same for futures
  (func (export "fread_async") (param $r i32) (param $ptr i32)
        (param $wset i32) (result i32)
    (drop (call $freada (local.get $r) (local.get $ptr)))
    (call $wjoin (local.get $r) (local.get $wset))
    (call $wspoll (local.get $wset) (i32.const 1024)))
)
"#;

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

fn p3_component() -> Vec<u8> {
    let guest = wat::parse_str(GUEST).expect("guest wat");
    let mut b = wasm_encoder::ComponentBuilder::default();

    // -- types ---------------------------------------------------------------
    let (t_stream, e) = b.type_defined(None);
    e.stream(Some(ComponentValType::Primitive(PV::U8)));
    let (t_future, e) = b.type_defined(None);
    e.future(Some(ComponentValType::Primitive(PV::U32)));

    let t_snew = ftype(&mut b, &[], Some(PV::U64));
    let t_rw3 = ftype(&mut b, &[PV::U32, PV::U32, PV::U32], Some(PV::U32));
    let t_r2 = ftype(&mut b, &[PV::U32, PV::U32], Some(PV::U32));
    let t_u32_u32 = ftype(&mut b, &[PV::U32], Some(PV::U32));
    let t_u32_void = ftype(&mut b, &[PV::U32], None);
    let t_2void = ftype(&mut b, &[PV::U32, PV::U32], None);
    let t_void_u32 = ftype(&mut b, &[], Some(PV::U32));
    let t_3u32 = ftype(&mut b, &[PV::U32, PV::U32, PV::U32], Some(PV::U32));
    let t_4u32 = ftype(&mut b, &[PV::U32, PV::U32, PV::U32, PV::U32], Some(PV::U32));
    let t_void = ftype(&mut b, &[], None);

    // -- core module ----------------------------------------------------------
    let m = b.core_module_raw(None, &guest);

    // -- canon builtins ---------------------------------------------------------
    // (memory options are recorded but resolved lazily at call time; the
    // debug-message realloc is resolved by name after instantiation)
    let utf8 = [CanonicalOption::Memory(0), CanonicalOption::UTF8];
    let utf8_async = [
        CanonicalOption::Memory(0),
        CanonicalOption::UTF8,
        CanonicalOption::Async,
    ];

    let canon: Vec<(&'static str, u32)> = vec![
        ("[stream-new]", b.stream_new(t_stream)),
        ("[stream-read]", b.stream_read(t_stream, utf8)),
        ("[stream-read-async]", b.stream_read(t_stream, utf8_async)),
        ("[stream-write]", b.stream_write(t_stream, utf8)),
        (
            "[stream-cancel-read]",
            b.stream_cancel_read(t_stream, false),
        ),
        (
            "[stream-cancel-write]",
            b.stream_cancel_write(t_stream, false),
        ),
        ("[stream-drop-readable]", b.stream_drop_readable(t_stream)),
        ("[stream-drop-writable]", b.stream_drop_writable(t_stream)),
        ("[stream-forward]", b.stream_forward(t_stream)),
        ("[future-new]", b.future_new(t_future)),
        ("[future-read]", b.future_read(t_future, utf8)),
        ("[future-read-async]", b.future_read(t_future, utf8_async)),
        ("[future-write]", b.future_write(t_future, utf8)),
        ("[future-drop-readable]", b.future_drop_readable(t_future)),
        ("[future-drop-writable]", b.future_drop_writable(t_future)),
        ("[future-forward]", b.future_forward(t_future)),
        ("[waitable-set-new]", b.waitable_set_new()),
        ("[waitable-set-wait]", b.waitable_set_wait(0)),
        ("[waitable-set-poll]", b.waitable_set_poll(0)),
        ("[waitable-set-drop]", b.waitable_set_drop()),
        ("[waitable-join]", b.waitable_join()),
        ("[error-context-new]", b.error_context_new(utf8)),
        (
            "[error-context-debug-message]",
            b.error_context_debug_message(utf8),
        ),
        ("[error-context-drop]", b.error_context_drop()),
        ("[backpressure-inc]", b.backpressure_inc()),
        ("[backpressure-dec]", b.backpressure_dec()),
        ("[thread-yield]", b.thread_yield()),
        ("[context-get]", b.context_get(ValType::I32, 0)),
        ("[context-set]", b.context_set(ValType::I32, 0)),
    ];

    // -- wrapper instance + guest instantiation ---------------------------------
    let shim = b.core_instantiate_exports(
        Some("p3-shim"),
        canon
            .iter()
            .map(|(n, f)| (*n, ExportKind::Func, *f))
            .collect::<Vec<_>>(),
    );
    let inst = b.core_instantiate(Some("guest"), m, [("p3", ModuleArg::Instance(shim))]);

    // -- lifts + component exports ------------------------------------------------
    let lifted: &[(&str, u32)] = &[
        ("snew", t_snew),
        ("sread", t_rw3),
        ("sreada", t_rw3),
        ("swrite", t_rw3),
        ("scread", t_u32_u32),
        ("scwrite", t_u32_u32),
        ("sdropr", t_u32_void),
        ("sdropw", t_u32_void),
        ("sfwd", t_2void),
        ("fnew", t_snew),
        ("fread", t_r2),
        ("freada", t_r2),
        ("fwrite", t_r2),
        ("fdropr", t_u32_void),
        ("fdropw", t_u32_void),
        ("ffwd", t_2void),
        ("wsnew", t_void_u32),
        ("wswait", t_r2),
        ("wspoll", t_r2),
        ("wsdrop", t_u32_void),
        ("wjoin", t_2void),
        ("ecnew", t_r2),
        ("ecmsg", t_2void),
        ("ecdrop", t_u32_void),
        ("binc", t_void),
        ("bdec", t_void),
        ("yld", t_void_u32),
        ("cget", t_void_u32),
        ("cset", t_u32_void),
        ("pump", t_4u32),
        ("pread_async", t_4u32),
        ("fread_async", t_3u32),
    ];
    for (name, ty) in lifted {
        let cf = b.core_alias_export(None, inst, name, ExportKind::Func);
        let f = b.lift_func(None, cf, *ty, []);
        b.export(*name, ComponentExportKind::Func, f, None);
    }

    b.finish()
}

fn main() {
    let bin = p3_component();
    std::fs::write("crates/waclay/tests/p3_component.bin", &bin).unwrap();
    println!("wrote {} bytes", bin.len());
}
