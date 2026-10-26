//! `waclay-cli` — inspect and run wasm components with any waclay engine.
//!
//! ```text
//! waclay-cli inspect <component.wasm>
//! waclay-cli run <component.wasm> [--engine wasmi|wasmtime|wasmer]
//!                [--invoke <export>] [--args '<json>'] [--wasi]
//! ```

use std::process::ExitCode;

use waclay::component::decode::decode;
use waclay::component::types::Val;
use waclay::{Component, Engine, Linker, Store};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("inspect") if args.len() >= 2 => cmd_inspect(&args[1]),
        Some("run") => cmd_run(&args[1..]),
        Some("engines") | Some("list-engines") => cmd_engines(),
        Some("--version") | Some("-V") | Some("version") => {
            println!("waclay-cli {}", waclay::VERSION);
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "waclay-cli {}\n\nUSAGE:\n  waclay-cli inspect <component.wasm>\n  waclay-cli run <component.wasm> [--engine wasmi|wasmtime|wasmer] [--invoke <export>] [--args '<json>'] [--wasi]\n  waclay-cli engines\n  waclay-cli version",
                waclay::VERSION
            );
            ExitCode::from(2)
        }
    }
}

fn make_engine(name: &str) -> Engine {
    match name {
        "wasmi" => Engine::new(waclay_wasmi::WasmiEngine::default()),
        "wasmtime" => Engine::new(waclay_wasmtime::WasmtimeEngine::default()),
        #[cfg(not(windows))]
        "wasmer" => Engine::new(waclay_wasmer::WasmerEngine::default()),
        other => {
            eprintln!("unknown engine `{other}` (expected wasmi|wasmtime|wasmer)");
            std::process::exit(2);
        }
    }
}

// ---------------------------------------------------------------------------
// engines
// ---------------------------------------------------------------------------

fn cmd_engines() -> ExitCode {
    println!("waclay {} — available engines:", waclay::VERSION);
    #[allow(unused_mut)]
    let mut engines: Vec<(&str, Engine)> = vec![
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
    for (name, engine) in engines {
        println!(
            "  {name:<9} version {:<10} component-model: canonical ABI runtime (built-in)",
            engine.version()
        );
    }
    println!(
        "\nWASI support: P1 (wasi_snapshot_preview1) + P2/P3 component interfaces\n              (cli, io/streams, io/poll, io/error, clocks, random) at 0.2.0 and 0.3.0"
    );
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// inspect
// ---------------------------------------------------------------------------

fn cmd_inspect(path: &str) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let is_component = bytes.len() >= 8 && bytes[4] == 0x0d;
    println!("file:      {path}");
    println!(
        "kind:      {}",
        if is_component {
            "component"
        } else {
            "core module"
        }
    );

    let plan = match decode(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: decode failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    if plan.imports.is_empty() && plan.exports.is_empty() {
        println!("imports:   (none — plain core module)");
        println!("exports:   (see `waclay-cli run` or a wasm inspector)");
        return ExitCode::SUCCESS;
    }

    println!("imports:");
    for imp in &plan.imports {
        if imp.funcs.is_empty() {
            println!("  import {imp}", imp = imp.name);
            continue;
        }
        println!("  import {name} {{", name = imp.name);
        for (fname, sig) in &imp.funcs {
            println!("    {fname}: {}", sig.describe(&plan.types));
        }
        println!("  }}");
    }

    println!("exports:");
    for exp in &plan.exports {
        if exp.funcs.is_empty() {
            println!("  export {name}", name = exp.name);
            continue;
        }
        println!("  export {name} {{", name = exp.name);
        for (fname, sig) in &exp.funcs {
            println!("    {fname}: {}", sig.describe(&plan.types));
        }
        println!("  }}");
    }

    if !plan.resources.is_empty() {
        println!("resources:");
        for r in &plan.resources {
            println!(
                "  {iface}/{name} (type {ty})",
                iface = r.interface.clone().unwrap_or_default(),
                name = r.name,
                ty = r.type_idx
            );
        }
    }

    println!(
        "canon:     {} lifted exports, {} lowered imports",
        plan.lifts.len(),
        plan.lowers.len()
    );
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

fn cmd_run(rest: &[String]) -> ExitCode {
    let mut path = None;
    let mut engine_name = "wasmi";
    let mut invoke: Option<String> = None;
    let mut args_json = String::from("[]");
    let mut wasi = false;

    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--engine" => match it.next() {
                Some(v) => engine_name = v,
                None => {
                    eprintln!("error: --engine needs a value");
                    return ExitCode::from(2);
                }
            },
            "--invoke" => match it.next() {
                Some(v) => invoke = Some(v.clone()),
                None => {
                    eprintln!("error: --invoke needs a value");
                    return ExitCode::from(2);
                }
            },
            "--args" => match it.next() {
                Some(v) => args_json = v.clone(),
                None => {
                    eprintln!("error: --args needs a JSON value");
                    return ExitCode::from(2);
                }
            },
            "--wasi" => wasi = true,
            other if other.starts_with('-') => {
                eprintln!("error: unknown flag {other}");
                return ExitCode::from(2);
            }
            _ => path = Some(a.clone()),
        }
    }
    let path = match path {
        Some(p) => p,
        None => {
            eprintln!("error: no component file given");
            return ExitCode::from(2);
        }
    };

    let engine = make_engine(engine_name);
    let component = match Component::from_file(&engine, &path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut linker = Linker::new();
    if wasi {
        waclay::wasi::add_all_to_linker(&mut linker, "0.3.0");
        waclay::wasi::add_to_linker(&mut linker, "0.2.0");
    }

    let ctx = if wasi {
        waclay::wasi::WasiCtx::builder().inherit_stdio().build()
    } else {
        waclay::wasi::WasiCtx::builder().build()
    };
    let mut store = match Store::new(&engine, ctx) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: instantiation failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    // no --invoke: print available exports
    let invoke = match invoke {
        Some(i) => i,
        None => {
            println!("exports:");
            for name in instance.export_names() {
                println!("  {name}");
            }
            if instance.is_core_only() {
                println!("  (core module — use --invoke <export> to call it)");
            }
            return ExitCode::SUCCESS;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&args_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: --args is not valid JSON: {e}");
            return ExitCode::from(2);
        }
    };
    let args: Vec<serde_json::Value> = parsed.as_array().cloned().unwrap_or_default();

    let params: Vec<Val> = args.iter().map(json_to_val).collect();
    match instance.call_func(&mut store, &invoke, &params) {
        Ok(Some(v)) => {
            println!("{}", val_to_json(&v));
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: call failed: {e}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// JSON <-> Val (small, no serde derive)
// ---------------------------------------------------------------------------

fn json_to_val(v: &serde_json::Value) -> Val {
    match v {
        serde_json::Value::Bool(b) => Val::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= u32::MAX as i64 && (i < 0 || i > i32::MAX as i64) {
                    Val::S64(i)
                } else if i >= 0 && i <= u32::MAX as i64 {
                    Val::U32(i as u32)
                } else {
                    Val::S32(i as i32)
                }
            } else if let Some(u) = n.as_u64() {
                Val::U64(u)
            } else {
                Val::F64(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Val::String(s.clone()),
        serde_json::Value::Array(items) => Val::List(items.iter().map(json_to_val).collect()),
        serde_json::Value::Object(map) => Val::Record(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_val(v)))
                .collect(),
        ),
        serde_json::Value::Null => Val::Option(None),
    }
}

fn val_to_json(v: &Val) -> serde_json::Value {
    match v {
        Val::Bool(b) => serde_json::Value::Bool(*b),
        Val::S8(x) => (*x as i64).into(),
        Val::U8(x) => (*x as u64).into(),
        Val::S16(x) => (*x as i64).into(),
        Val::U16(x) => (*x as u64).into(),
        Val::S32(x) => (*x as i64).into(),
        Val::U32(x) => (*x as u64).into(),
        Val::S64(x) => (*x).into(),
        Val::U64(x) => (*x).into(),
        Val::F32(x) => (*x as f64).into(),
        Val::F64(x) => (*x).into(),
        Val::Char(c) => serde_json::Value::String(c.to_string()),
        Val::String(s) => serde_json::Value::String(s.clone()),
        Val::List(items) | Val::Tuple(items) => {
            serde_json::Value::Array(items.iter().map(val_to_json).collect())
        }
        Val::Record(fields) => {
            let mut map = serde_json::Map::new();
            for (k, v) in fields {
                map.insert(k.clone(), val_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        Val::Variant { case, payload } => {
            let mut map = serde_json::Map::new();
            map.insert("case".into(), serde_json::Value::String(case.clone()));
            if let Some(p) = payload {
                map.insert("value".into(), val_to_json(p));
            }
            serde_json::Value::Object(map)
        }
        Val::Enum(name) => serde_json::Value::String(name.clone()),
        Val::Handle(h) => serde_json::Value::String(format!("handle#{:?}", h)),
        Val::Stream { index, .. } => serde_json::Value::String(format!("stream#{index}")),
        Val::Future { index, .. } => serde_json::Value::String(format!("future#{index}")),
        Val::Flags(set) => {
            serde_json::Value::Array(set.iter().cloned().map(serde_json::Value::String).collect())
        }
        Val::Option(Some(v)) => val_to_json(v),
        Val::Option(None) => serde_json::Value::Null,
        Val::Result(Ok(v)) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "ok".into(),
                v.as_ref()
                    .map(|b| val_to_json(b))
                    .unwrap_or(serde_json::Value::Null),
            );
            serde_json::Value::Object(map)
        }
        Val::Result(Err(v)) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "err".into(),
                v.as_ref()
                    .map(|b| val_to_json(b))
                    .unwrap_or(serde_json::Value::Null),
            );
            serde_json::Value::Object(map)
        }
    }
}
