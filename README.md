# waclay Workspace

[![Crates.io](https://img.shields.io/crates/v/waclay.svg)](https://crates.io/crates/waclay)
[![Docs.rs](https://docs.rs/waclay/badge.svg)](https://docs.rs/waclay)
[![Unsafe Forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

This workspace contains multiple crates for WebAssembly Component Model development:

## Workspace Structure

```
waclay/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── waclay/             # Main library crate
│   │   ├── src/            # Core component layer implementation
│   │   ├── examples/       # Library usage examples
│   │   └── docs/           # Documentation
│   └── wit-bindgen-wcl/    # WIT binding generator binary
│       └── src/            # Code generator implementation
└── README.md               # This file
```

## Crates

### `waclay`
Runtime agnostic implementation of the [WebAssembly component model](https://github.com/WebAssembly/component-model).
It supports loading and linking WASM components, inspecting and generating component interface types at runtime, and more atop any WebAssembly backend. The implementation is based upon the [`wasmtime`](https://github.com/bytecodealliance/wasmtime), [`js-component-bindgen`](https://github.com/bytecodealliance/jco), and [`wit-parser`](https://github.com/bytecodealliance/wasm-tools/tree/main) crates.

- **Version:** 0.1.3
- **License:** Apache-2.0
- **Repository:** https://github.com/HemantKArya/waclay

### `wit-bindgen-wcl`
A command-line tool for generating Rust bindings from WIT (WebAssembly Interface Type) files for use with `waclay`.

## Quick Start

### Building the workspace
```bash
cargo build
```

### Building a specific crate
```bash
cargo build -p waclay
cargo build -p wit-bindgen-wcl
```

### Running the binding generator
```bash
cargo run --bin wit-bindgen-wcl -- <wit-file-or-dir> <output-file>
```

Example:
```bash
cargo run --bin wit-bindgen-wcl -- ./my-component/wit ./bindings.rs
```

### Installing the binding generator
```bash
cargo install --path crates/wit-bindgen-wcl
wit-bindgen-wcl <wit-dir> <output-file>
```

## Usage

To use `waclay`, a runtime is required. The [`wasm_runtime_layer`](https://github.com/DouglasDwyer/wasm_runtime_layer) crate provides the common interface used for WebAssembly runtimes, so when using this crate it must also be added to the `Cargo.toml` file with the appropriate runtime selected. For instance, the examples in this repository use the [`wasmi_runtime_layer`](https://crates.io/crates/wasmi_runtime_layer) runtime:

```toml
waclay = "0.1.16"
wasmi_runtime_layer = "0.31.0"
# wasmtime_runtime_layer = "21.0.0"
# js_wasm_runtime_layer = "0.4.0"
```

The following is a small overview of `waclay`'s API. The complete example may be found in the [examples folder](/examples). Consider a WASM component with the following WIT:

```wit
package test:guest

interface foo {
    // Selects the item in position n within list x
    select-nth: func(x: list<string>, n: u32) -> string
}

world guest {
    export foo
}
```

The component can be loaded into `waclay` and invoked as follows:

```rust
use waclay::*;

// The bytes of the component.
const WASM: &[u8] = include_bytes!("single_component/component.wasm");

pub fn main() {
    // Create a new engine for instantiating a component.
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());

    // Create a store for managing WASM data and any custom user-defined state.
    let mut store = Store::new(&engine, ());

    // Parse the component bytes and load its imports and exports.
    let component = Component::new(&engine, WASM).unwrap();
    // Create a linker that will be used to resolve the component's imports, if any.
    let linker = Linker::default();
    // Create an instance of the component using the linker.
    let instance = linker.instantiate(&mut store, &component).unwrap();

    // Get the interface that the interface exports.
    let interface = instance.exports().instance(&"test:guest/foo".try_into().unwrap()).unwrap();
    // Get the function for selecting a list element.
    let select_nth = interface.func("select-nth").unwrap().typed::<(Vec<String>, u32), String>().unwrap();

    // Create an example list to test upon.
    let example = ["a", "b", "c"].iter().map(ToString::to_string).collect::<Vec<_>>();

    println!("Calling select-nth({example:?}, 1) == {}", select_nth.call(&mut store, (example.clone(), 1)).unwrap());
    // Prints 'Calling select-nth(["a", "b", "c"], 1) == b'
}
```

## Supported capabilities

`waclay` supports the following major capabilities:

- Parsing and instantiating WASM component binaries
- Runtime generation of component interface types
- Specialized list types for faster lifting/lowering
- Structural equality of component interface types, as mandated by the spec
- Support for guest resources
- Support for strongly-typed host resources with destructors

The following things have yet to be implemented:

- String transcoders
- A macro for generating host bindings
- More comprehensive tests
- Subtyping

## Optional features

**serde** - Allows for the serialization of identifiers, types, and values. Note that serializing resources is not allowed, because resources may be tied to specific instances.

## Examples

### waclay Examples

Basic component model examples demonstrating core features:

```shell
# Run from workspace root
cargo run --example single_component     # Simple component instantiation
cargo run --example string_host_guest    # String passing between host/guest
cargo run --example func_param           # Function parameters
cargo run --example record_response      # Record types
cargo run --example option_result        # Option and Result types
cargo run --example variant_return       # Variant types
cargo run --example complex_return       # Complex return types
cargo run --example resource             # Resource handling
cargo run --example guest_resource       # Guest-defined resources
cargo run --example multilevel_resource  # Multi-level resources
```

### wit-bindgen-wcl Examples

Advanced examples using generated bindings (9 examples, prefixed with `bindgen-`):

```shell
# Run from workspace root
cargo run --example bindgen-calculator         # Calculator with logging & error handling
cargo run --example bindgen-web-scraper        # Web scraping component
cargo run --example bindgen-single-component   # Basic binding generation
cargo run --example bindgen-string-host-guest  # String passing with generated bindings
cargo run --example bindgen-func-param         # Function parameters with generated bindings
cargo run --example bindgen-record-response    # Record types with generated bindings
cargo run --example bindgen-option-result      # Option and Result with generated bindings
cargo run --example bindgen-variant-return     # Variant types with generated bindings
cargo run --example bindgen-complex-return     # Complex return types with generated bindings
```

**Note:** wit-bindgen-wcl examples are prefixed with `bindgen-` to distinguish them from 
waclay examples. The key difference is that wit-bindgen-wcl examples use 
**generated bindings** (more ergonomic, type-safe), while waclay examples 
use the **raw API** (more flexible, runtime introspection).

### Building Example Components

To rebuild the WASM components for examples:

```shell
# Example: rebuilding calculator component
cd crates/wit-bindgen-wcl/examples/calculator/component
rustup toolchain install nightly
rustup override set nightly
cargo build --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/debug/calculator.wasm -o component.wasm
```

## Testing

The workspace includes comprehensive test scripts for cross-platform testing:

```shell
# Test both crates (full suite including Android/Linux builds)
.\test-all.ps1

# Quick tests (skip Android/Linux builds)
.\test-all.ps1 -Fast

# Test individual crates
.\test-wcomp-layer.ps1      # Test waclay only
.\test-wit-bindgen.ps1      # Test wit-bindgen-wcl only

# Skip specific platforms
.\test-wcomp-layer.ps1 -SkipAndroid -SkipLinux
.\test-wit-bindgen.ps1 -SkipExamples
```

The test scripts validate:
- Unit tests and compilation
- Example builds and execution
- Cross-platform compatibility (Windows, Android, Linux)
- Binary functionality
- Workspace-level integration
