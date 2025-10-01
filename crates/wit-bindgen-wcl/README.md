# WIT Binding Generator for wasm-component-layer

A tool that generates clean, type-safe Rust bindings from WIT files for use with `wasm-component-layer`.

## Features

- ✅ **Auto-generates type definitions** - Records, variants, enums with full `ComponentType` implementations
- ✅ **Host function traits** - Easy-to-implement traits that abstract away `Value` types
- ✅ **Export helpers** - Type-safe getter functions for guest exports
- ✅ **Clean separation** - Generated code separate from user implementation
- ✅ **Full WIT support** - Records, variants, options, results, lists, primitives

## Installation

```bash
cd wit-bindgen-wcl
cargo build --release
```

## Usage

```bash
wit-bindgen-wcl <wit-file-or-directory> <output-file>
```

### Example

```bash
# Generate bindings from a WIT file
./target/release/wit-bindgen-wcl my-component.wit bindings.rs
```

## Generated Code Structure

### For Exports (Guest functions)

```rust
// Generated: bindings.rs
pub mod exports_interface_name {
    pub fn get_function_name<T>(
        instance: &Instance,
        store: &mut Store<T>,
    ) -> Result<TypedFunc<InputType, OutputType>> {
        // ... implementation
    }
}

// Your code: main.rs
mod bindings;
use bindings::exports_interface_name;

let func = exports_interface_name::get_function_name(&instance, &mut store)?;
let result = func.call(&mut store, input)?;
```

### For Imports (Host functions)

```rust
// Generated: bindings.rs
pub trait InterfaceNameHost {
    fn function_name(&mut self, param: ParamType) -> ReturnType;
}

pub mod imports {
    pub fn register_interface_nameHost<T: InterfaceNameHost + 'static>(
        linker: &mut Linker<T>,
        store: &mut Store<T>,
    ) -> Result<()> {
        // ... implementation
    }
}

// Your code: main.rs
mod bindings;
use bindings::{InterfaceNameHost, imports};

struct MyHost;
impl InterfaceNameHost for MyHost {
    fn function_name(&mut self, param: ParamType) -> ReturnType {
        // Your implementation here
    }
}

let mut store = Store::new(&engine, MyHost);
imports::register_interface_nameHost(&mut linker, &mut store)?;
```

### For Complex Types

```rust
// Generated: bindings.rs
#[derive(Debug, Clone)]
pub struct MyRecord {
    pub field1: u32,
    pub field2: String,
}

impl ComponentType for MyRecord {
    // ... full implementation
}

// Your code: Works seamlessly!
let result: MyRecord = function.call(&mut store, input)?;
println!("Field1: {}, Field2: {}", result.field1, result.field2);
```

## Complete Example

### 1. Create WIT file

```wit
// my-component.wit
package example:demo;

interface logger {
    log: func(message: string);
}

interface processor {
    record result {
        status: string,
        value: u32,
    }
    
    process: func(input: string) -> result;
}

world guest {
    import logger;
    export processor;
}
```

### 2. Generate bindings

```bash
wit-bindgen-wcl my-component.wit bindings.rs
```

### 3. Use in your project

```rust
mod bindings;
use bindings::*;
use wasm_component_layer::*;

const WASM: &[u8] = include_bytes!("component.wasm");

// Implement host functions
struct MyHost;
impl LoggerHost for MyHost {
    fn log(&mut self, message: String) {
        println!("[Guest Log] {}", message);
    }
}

fn main() {
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, MyHost);
    let component = Component::new(&engine, WASM).unwrap();
    let mut linker = Linker::default();
    
    // Register host functions - one line!
    imports::register_loggerHost(&mut linker, &mut store).unwrap();
    
    let instance = linker.instantiate(&mut store, &component).unwrap();
    
    // Call guest functions - type-safe!
    let process = exports_processor::get_process(&instance, &mut store).unwrap();
    let result = process.call(&mut store, ("hello".to_string(),)).unwrap();
    
    println!("Status: {}, Value: {}", result.status, result.value);
}
```

## Benefits

### Before (Manual)

```rust
// 100+ lines of manual Value conversion code
let host_interface = linker.define_instance("example:demo/logger".try_into().unwrap()).unwrap();
host_interface.define_func(
    "log",
    Func::new(
        &mut store,
        FuncType::new([ValueType::String], []),
        move |_, params, _results| {
            let message = match &params[0] {
                Value::String(s) => s,
                _ => panic!("Unexpected parameter type"),
            };
            println!("[Host] {}", message);
            Ok(())
        },
    ),
).unwrap();
// ... many more lines
```

### After (Generated)

```rust
// 3 lines!
impl LoggerHost for MyHost {
    fn log(&mut self, message: String) {
        println!("[Host] {}", message);
    }
}
imports::register_loggerHost(&mut linker, &mut store).unwrap();
```

## Supported WIT Features

- ✅ Primitive types (bool, u8-u64, s8-s64, f32, f64, char, string)
- ✅ Records (structs)
- ✅ Variants (enums with payloads)
- ✅ Enums (simple enums)
- ✅ Options (Option<T>)
- ✅ Results (Result<T, E>)
- ✅ Lists (Vec<T>)
- ✅ Tuples
- ✅ Nested types
- ⏳ Resources (coming soon)
- ⏳ Flags (coming soon)

## Architecture

The generator separates concerns:

1. **Auto-generated** (`bindings.rs`):
   - Type definitions
   - ComponentType implementations
   - Host function traits
   - Helper functions

2. **User-written** (`main.rs`):
   - Engine/Store/Linker setup
   - Trait implementations
   - Business logic
   - Component instantiation

This separation means:
- ✅ Regenerate bindings anytime without losing user code
- ✅ Type-safe APIs with minimal boilerplate
- ✅ Easy to test and maintain
- ✅ IDE auto-completion works perfectly

## Examples

The `examples/` directory contains complete working examples demonstrating various WIT features:

### Running Examples

From the workspace root:
```bash
# Build and run a specific example
cd wcomp_layer/crates/wit-bindgen-wcl/examples/calculator/host
cargo run

# Or test all examples automatically
cd wcomp_layer/crates/wit-bindgen-wcl
python test_examples.py

# Test a specific example
python test_examples.py calculator
```

### Available Examples

- **`calculator`** - Enums, records, variants, results, and stateful components
- **`complex_return`** - Complex nested records with options and results
- **`file-manager`** - Flags with bitwise operations and variant error handling
- **`func_param`** - Complex function parameters (lists, records, variants, options, results)
- **`option_result`** - All variants of Option and Result types
- **`record_response`** - Simple record types with multiple fields
- **`single_component`** - Basic list operations and string handling
- **`string_host_guest`** - Host function imports and bidirectional communication
- **`variant_return`** - Variants with multiple cases and nested payloads
- **`web_scraper`** - Deeply nested types (5+ levels), multiple interfaces, and complex data structures

Each example includes:
- `component/wit/` - WIT interface definitions
- `component/src/` - Guest (component) implementation
- `host/src/` - Host application using generated bindings
- `host/bindings.rs` - Generated type-safe bindings

## Testing

Run all example tests:
```bash
cd wcomp_layer/crates/wit-bindgen-wcl
python test_examples.py
```

This will:
1. Regenerate bindings for each example
2. Build the host application
3. Run the example and verify success

## Contributing

Issues and PRs welcome! The generator is designed to be extended with new WIT features.

## License

MIT OR Apache-2.0
