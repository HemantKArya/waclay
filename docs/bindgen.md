# Host-side Bindgen Macro

The `wasm_component_layer` library now includes a `bindgen!` macro for generating host-side bindings from WIT files, similar to wasmtime's bindgen functionality but in a runtime-agnostic way.

## Features

- **Runtime-Agnostic**: Works with any WebAssembly runtime supported by `wasm_runtime_layer`
- **Type-Safe**: Generates strongly-typed Rust interfaces from WIT definitions
- **Simple to Use**: Just point the macro to your WIT file

## Usage

Add the `macro` feature to your `Cargo.toml`:

```toml
[dependencies]
wasm_component_layer = { version = "0.1", features = ["macro"] }
```

Then use the `bindgen!` macro in your code:

```rust
use wasm_component_layer::bindgen;

bindgen!({
    path: "path/to/your/world.wit",
    world: "my-world",
});
```

### Options

- `path`: Path to a WIT file or directory containing WIT files (relative to Cargo.toml)
- `inline`: Inline WIT definition as a string (alternative to `path`)
- `world`: Name of the world to generate bindings for (required if multiple worlds exist)
- `imports_only`: Generate only import bindings (optional, default: false)
- `exports_only`: Generate only export bindings (optional, default: false)

## Example

Given a WIT file:

```wit
package example:greeter;

world greeter {
    import log: func(message: string);
    export greet: func(name: string) -> string;
}
```

The macro generates:

1. **HostImports trait** - Trait for implementing imports
2. **World struct** - Struct representing the component
3. **Export methods** - Typed methods for calling exported functions

## Current Status

This is an initial implementation with basic functionality:

- ✅ Macro infrastructure
- ✅ WIT file parsing
- ✅ Basic type generation (primitives, lists, options, results, tuples)
- ✅ Import trait generation
- ✅ Export method skeletons
- 🚧 Full type support (records, variants, enums, flags, resources)
- 🚧 Actual export function implementation
- 🚧 add_to_linker helper
- 🚧 Comprehensive documentation

## Roadmap

Future enhancements will include:

- Complete type system support
- Resource handling
- Interface imports/exports
- Helper functions for common patterns
- Better error messages
- More examples

## Comparison to Wasmtime

This implementation is designed to mirror wasmtime's bindgen functionality while remaining runtime-agnostic. The API may differ slightly but aims to provide the same developer experience.
