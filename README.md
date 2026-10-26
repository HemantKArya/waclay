<div align="center">

# Waclay

**WebAssembly Component Layer: one Component Model runtime, many engines.**

Write your host once, then pick wasmtime, wasmer or wasmi at runtime.

[![CI](https://github.com/HemantKArya/waclay/actions/workflows/ci.yml/badge.svg)](https://github.com/HemantKArya/waclay/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/waclay.svg)](https://crates.io/crates/waclay)
[![docs.rs](https://img.shields.io/docsrs/waclay)](https://docs.rs/waclay)
[![License](https://img.shields.io/crates/l/waclay.svg)](#license)

[Quick Start](#quick-start) •
[Features](#features) •
[Architecture](#architecture) •
[WASI](#wasi) •
[bindgen!](#bindgen) •
[Platforms](#engines-and-platforms) •
[Contributing](#contributing)

</div>

---

## About

Waclay is a runtime-agnostic implementation of the
[WebAssembly Component Model](https://github.com/WebAssembly/component-model) for Rust. It loads
real component binaries (the output of `cargo component`, `wit-component` and `wit-bindgen`),
links their imports, and runs them on any core-wasm engine through its own Canonical ABI
implementation.

The component layer is written once. Engines plug in underneath as thin backend crates, so
switching engines is a one-line change, you only compile the engines you use, and adding a new
engine means implementing a handful of small traits. Waclay does not fork or patch the engines:
upstream releases drop in by rebuilding the backend crate.

## Quick start

```toml
[dependencies]
waclay = "2.0"
waclay-wasmtime = "2.0"   # or waclay-wasmer / waclay-wasmi
```

```rust
use waclay::{Component, Engine, Linker, Store, Val};
use waclay_wasmtime::WasmtimeEngine;

fn main() -> waclay::Result<()> {
    // The only engine-specific line: swap in WasmerEngine or WasmiEngine.
    let engine = Engine::new(WasmtimeEngine::default());

    let component = Component::from_file(&engine, "adder.component.wasm")?;

    let mut linker = Linker::new();
    linker.func(Some("test:adder/calc"), "log", |_ctx, args, _out| {
        println!("guest says: {:?}", args.first());
        Ok(())
    });

    let mut store = Store::new(&engine, ()).unwrap();
    let instance = linker.instantiate(&mut store, &component)?;

    let sum = instance.call_func(&mut store, "add", &[Val::U32(40), Val::U32(2)])?;
    println!("40 + 2 = {sum:?}"); // Some(U32(42))
    Ok(())
}
```

Every engine also reports what it is: `engine.name()`, `engine.version()` and
`engine.describe()` (for example `"wasmtime 50.0.0"`).

## Features

- **Real components.** Decodes genuine component binaries with `wasmparser` and `wit-component`
  0.262; no `wasmtime-environ`, no engine-specific code above the facade.
- **Full type surface.** Numerics, `char`, `string` (UTF-8, UTF-16 and latin-1+UTF-16), `list`,
  `record`, `tuple`, `variant`, `enum`, `flags` (including more than 64 flags), `option`, `result`.
- **Resources.** Host-side tables with `resource.new/drop/rep`, `[constructor]` and `[method]`
  entry points, and guest-side resource export.
- **Async.** `canon async`, subtasks, streams and futures (WASI P3).
- **Nested components** and component start.
- **Built-in WASI.** P1, P2 and P3, with no external wasi crate.
- **`bindgen!`.** Typed Rust bindings generated from a WIT world at compile time.
- **Fast.** Canonical ABI layouts are computed once when a component loads, so a call adds roughly
  130-200 ns over the raw engine (see [docs/BENCHMARKS.md](docs/BENCHMARKS.md)).
- **Hardened.** Guest-supplied lengths are bounds-checked before anything is allocated, and the
  core crate plus the wasmtime and wasmi backends use `#![forbid(unsafe_code)]`.

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│                        your host app                       │
├────────────────────────────────────────────────────────────┤
│  waclay (engine-agnostic core)                             │
│   • component decoding (wasmparser 0.262)                  │
│   • Canonical ABI: flattening, lift/lower, realloc         │
│   • linker + resource tables + WASI primitives             │
│   • bindgen! macro (typed WIT bindings)                    │
├────────────────────────────────────────────────────────────┤
│  engine facade: Engine / Store / CompiledModule / Imports  │
├──────────────┬──────────────────┬──────────────────────────┤
│ waclay-      │ waclay-          │ waclay-                  │
│ wasmtime     │ wasmer           │ wasmi                    │
│ (Cranelift   │ (Cranelift       │ (pure-Rust               │
│  JIT)        │  JIT)            │  interpreter)            │
└──────────────┴──────────────────┴──────────────────────────┘
```

```
waclay/
├── crates/
│   ├── waclay/              core: decoding, Canonical ABI, linker, WASI, engine facade
│   ├── waclay-macros/       the bindgen! procedural macro
│   ├── waclay-cli/          inspect and run components from the terminal
│   └── backends/
│       ├── waclay-wasmtime/
│       ├── waclay-wasmer/
│       └── waclay-wasmi/
└── docs/                    benchmarks
```

## Crates

Waclay is published as several crates so you only compile the engines you use.

| Crate | Purpose | Add it when |
|---|---|---|
| [`waclay`](https://crates.io/crates/waclay) | Core library: component decoding, Canonical ABI, linker, WASI, `bindgen!` | always |
| [`waclay-wasmtime`](https://crates.io/crates/waclay-wasmtime) | wasmtime backend | you want the wasmtime JIT |
| [`waclay-wasmer`](https://crates.io/crates/waclay-wasmer) | wasmer backend | you want the wasmer JIT (not on Windows) |
| [`waclay-wasmi`](https://crates.io/crates/waclay-wasmi) | wasmi backend | you want a small, pure-Rust interpreter |
| [`waclay-cli`](https://crates.io/crates/waclay-cli) | `waclay-cli` command-line tool | you want to inspect or run components from a terminal |
| [`waclay-macros`](https://crates.io/crates/waclay-macros) | Procedural macro behind `bindgen!` | never directly; `waclay` re-exports it |

## Engines and platforms

| Backend crate | Engine | Linux | macOS | Android | Windows |
|---|---|:---:|:---:|:---:|:---:|
| `waclay-wasmtime` | wasmtime (Cranelift JIT) | yes | yes | yes | yes |
| `waclay-wasmer` | wasmer (Cranelift JIT) | yes | yes | yes | no |
| `waclay-wasmi` | wasmi (interpreter) | yes | yes | yes | yes |

wasmer 7.3 removed its native compilers on Windows, so `waclay-wasmer` is not built there. The
rest of the workspace, including the tests and the CLI, skips it automatically. On Android,
`waclay-wasmer` needs a 64-bit or ARM target (aarch64, armv7, x86_64); wasmer does not support
32-bit x86 (`i686-linux-android`), which CI does not build.

Every engine runs the same test suite and must give identical results:

| Covered by tests | wasmi | wasmtime | wasmer |
|---|:---:|:---:|:---:|
| Core wasm: arithmetic, host calls, memory | yes | yes | yes |
| Canonical ABI: strings, lists, records, variants | yes | yes | yes |
| WASI P1 programs (`_start`) | yes | yes | yes |
| WASI P2/P3 components | yes | yes | yes |
| Resources and stdio streams | yes | yes | yes |

## WASI

```rust
use waclay::wasi::{self, WasiCtx};

let ctx = WasiCtx::builder().arg("demo").env("GREETING", "hello").inherit_stdio().build();

// Run a whole program (core module or component, detected automatically):
let exit_code = wasi::run(&engine, &wasm_bytes, ctx)?;

// Or add WASI to your own linker:
let mut linker = Linker::new();
wasi::add_to_linker(&mut linker, "0.3.0");   // P2/P3 components ("0.2.0" also works)
wasi::add_p1_to_linker(&mut linker);         // core modules
```

| Layer | Interfaces |
|---|---|
| P1 (`wasi_snapshot_preview1`) | args, environ, clocks, random, `fd_write`, `fd_read`, `fd_fdstat`, `proc_exit` |
| P2/P3 components | `wasi:random`, `wasi:clocks`, `wasi:cli` (environment, arguments, exit, stdio, terminal), `wasi:io` (error, poll, streams) |

Interfaces that are not implemented fail soft: the component still instantiates, and calling one
traps with a clear message.

## bindgen!

```rust
// greeter.wit: world greeter { import host; export calc; }
waclay::bindgen!("tests/wit/greeter.wit");

struct Logger { lines: Vec<String> }
impl HostImports for Logger {
    fn log(&mut self, msg: String) { self.lines.push(msg); }
}

let mut linker = Linker::new();
add_host_to_linker(&mut linker, |data| data.downcast_mut::<Logger>().unwrap());
let instance = linker.instantiate(&mut store, &component)?;
assert_eq!(exports(&instance).add(&mut store, 40, 2)?, 42);
```

The macro generates Rust types for records, variants, enums and flags (with `ComponentValue`
conversions), a trait and an `add_*_to_linker` function for each imported interface, and a typed
wrapper for the world's exports. Versioned packages (`package foo:bar@1.0.0;`) and top-level world
functions work too.

## CLI

```sh
cargo install waclay-cli

waclay-cli engines                                   # available engines and versions
waclay-cli inspect component.wasm                    # imports, exports, canonical ops
waclay-cli run component.wasm --invoke add --args '[40, 2]'
waclay-cli run program.wasm --wasi --engine wasmi
```

## Contributing

Bug reports, new backends, more test coverage (especially fuzzing the Canonical ABI lift and
lower paths) and documentation are all welcome. Before opening a pull request, run:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
```

On Windows, `.\test-all.ps1` runs the same checks plus the docs build. The minimum supported Rust
version is 1.97, which wasmtime 50 requires. Benchmarks: `cargo bench -p waclay`.

## Roadmap

- [x] Component decoding and the full Canonical ABI
- [x] wasmtime, wasmer and wasmi backends
- [x] WASI P1, P2 and P3 primitives
- [x] `bindgen!`
- [ ] Publish 2.0 to crates.io
- [ ] More WASI interfaces (filesystem, sockets, http)
- [ ] Additional engine backends
- [ ] Fuzzing the lift and lower paths

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at
your option.
