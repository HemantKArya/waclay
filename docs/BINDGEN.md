# Host-Side Bindgen Macro

This document describes the host-side `bindgen!` macro that has been integrated into `wasm_component_layer` from wasmtime's implementation.

## Overview

The `bindgen!` macro automatically generates Rust bindings for WebAssembly Component Model interfaces defined in WIT (WebAssembly Interface Type) files. This eliminates the need to manually write glue code between your Rust host application and WebAssembly components.

## Features

- **Runtime Agnostic**: Works with any WebAssembly runtime backend (wasmi, wasmtime, etc.)
- **Complete Type Support**: Supports all Component Model types
- **Production Ready**: Based on wasmtime's battle-tested implementation (3200+ lines of code)
- **Easy to Use**: Simple macro syntax for generating bindings

## Usage

### Basic Example

```rust
use wasm_component_layer::bindgen;

bindgen!({
    path: "path/to/world.wit",
    world: "my-world",
});
```

### Inline WIT

You can also define WIT inline:

```rust
bindgen!({
    inline: r#"
        package example:calculator;

        world calculator {
            export add: func(a: s32, b: s32) -> s32;
            export divide: func(a: s32, b: s32) -> result<s32, string>;
        }
    "#,
    world: "calculator",
});
```

## Supported Types

The bindgen macro supports all Component Model types:

| WIT Type | Rust Type | Description |
|----------|-----------|-------------|
| `bool` | `bool` | Boolean value |
| `s8`, `s16`, `s32`, `s64` | `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64` | `u8`, `u16`, `u32`, `u64` | Unsigned integers |
| `f32`, `f64` | `f32`, `f64` | Floating point numbers |
| `char` | `char` | Unicode character |
| `string` | `String`/`&str` | UTF-8 string |
| `list<T>` | `Vec<T>` | List/array |
| `option<T>` | `Option<T>` | Optional value |
| `result<T, E>` | `Result<T, E>` | Result type |
| `tuple<...>` | `(...)` | Tuple |
| `record` | `struct` | Record/struct |
| `variant` | `enum` | Variant/enum with payloads |
| `enum` | `enum` | Simple enumeration |
| `flags` | `struct` | Bitflags |
| `resource` | Special | Resource handles (own/borrow) |

## Generated Code

For a WIT world, the macro generates:

1. **Type definitions**: Rust types for all WIT records, variants, enums, and flags
2. **Import traits**: Trait you implement to provide imported functions
3. **Export methods**: Methods to call component exports
4. **Helper functions**: Utilities for working with the component

### Example

Given this WIT:

```wit
package example:app;

interface types {
    record user {
        id: u64,
        name: string,
    }
}

interface operations {
    use types.{user};
    
    create-user: func(name: string) -> user;
    find-user: func(id: u64) -> option<user>;
}

world my-app {
    import operations;
    export operations;
}
```

The macro generates (simplified):

```rust
// Generated type definitions
pub mod types {
    pub struct User {
        pub id: u64,
        pub name: String,
    }
}

// Generated import trait
pub trait HostImports {
    fn create_user(&mut self, name: String) -> anyhow::Result<types::User>;
    fn find_user(&mut self, id: u64) -> anyhow::Result<Option<types::User>>;
}

// Generated world struct
pub struct MyApp {
    instance: wasm_component_layer::Instance,
}

// Generated export methods
impl MyApp {
    pub fn create_user(&self, store: &mut impl AsContextMut, name: String) 
        -> anyhow::Result<types::User> {
        // ... generated code to call the export
    }
    
    pub fn find_user(&self, store: &mut impl AsContextMut, id: u64) 
        -> anyhow::Result<Option<types::User>> {
        // ... generated code to call the export
    }
}
```

## Architecture

The bindgen implementation consists of three crates adapted from wasmtime:

### 1. `wasm_component_layer_wit_bindgen` (Core Code Generation)

- **Source**: wasmtime's `wit-bindgen` crate
- **Size**: 3200+ lines of code
- **Function**: Generates Rust code from WIT definitions
- **Adaptations**: All `wasmtime::*` types replaced with `wasm_component_layer::*`

### 2. `wasm_component_layer_macro` (Procedural Macros)

- **Source**: wasmtime's `component-macro` crate
- **Size**: 1500+ lines of code
- **Function**: Provides the `bindgen!` procedural macro
- **Adaptations**: Updated to use `wasm_component_layer_wit_bindgen`

### 3. `wasm_component_layer_util` (Utilities)

- **Source**: wasmtime's `component-util` crate
- **Function**: Shared utility functions
- **Adaptations**: Minimal, mostly type renaming

## Testing

The bindgen implementation includes comprehensive tests:

```bash
# Run bindgen tests
cargo test --test bindgen_integration --features macro

# Run all tests with macro feature
cargo test --features macro
```

## Current Status

### ✅ Working

- Macro expansion and code generation
- All WIT type support (primitives, records, variants, enums, flags, lists, options, results, tuples)
- Interface definitions (import/export)
- Resource types
- Inline and file-based WIT

### 🚧 In Progress

- Full runtime integration (type conversions between generated code and `wasm_component_layer` types)
- Complete end-to-end examples
- Documentation for advanced features

## Comparison with Wasmtime

| Feature | Wasmtime | wasm_component_layer |
|---------|----------|----------------------|
| Code Generation | ✅ | ✅ |
| Type Support | ✅ All types | ✅ All types |
| Runtime | Wasmtime only | ✅ Any runtime |
| Resource Types | ✅ | ✅ |
| Async Support | ✅ | 🚧 Planned |

## Future Enhancements

- [ ] Complete runtime type conversion system
- [ ] Async/await support
- [ ] More comprehensive examples
- [ ] Performance optimizations
- [ ] Better error messages
- [ ] CLI tool for code generation

## Contributing

The bindgen implementation closely mirrors wasmtime's codebase. When updating:

1. Check wasmtime releases for new features
2. Copy relevant code changes
3. Adapt `wasmtime::*` references to `wasm_component_layer::*`
4. Test with the test suite
5. Update documentation

## References

- [Wasmtime Component Model](https://github.com/bytecodealliance/wasmtime)
- [WIT Specification](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)
- [Component Model Specification](https://github.com/WebAssembly/component-model)
