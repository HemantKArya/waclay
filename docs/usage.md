# Usage Guide

`wasm_component_layer` is a runtime-agnostic implementation of the WebAssembly component model. This guide provides comprehensive instructions for using the library to load, instantiate, and interact with WebAssembly components.

## Prerequisites

Before using `wasm_component_layer`, you need to add it to your `Cargo.toml` along with a WebAssembly runtime backend:

```toml
[dependencies]
wasm_component_layer = "0.1.16"
wasmi_runtime_layer = "0.31.0"
# OR
# wasmtime_runtime_layer = "21.0.0"
# js_wasm_runtime_layer = "0.4.0"
```

## Basic Usage

### Loading and Instantiating a Component

The fundamental workflow involves:
1. Creating an engine with your chosen WebAssembly backend
2. Loading component bytes into a `Component`
3. Creating a `Store` to manage WebAssembly state
4. Using a `Linker` to resolve imports (if any)
5. Instantiating the component to get an `Instance`
6. Accessing exported functions and calling them

```rust
use wasm_component_layer::*;

// The bytes of the component (usually from include_bytes!)
const WASM: &[u8] = include_bytes!("path/to/component.wasm");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create an engine with your chosen backend
    let engine = Engine::new(wasmi::Engine::default());

    // 2. Create a store for managing WebAssembly data and state
    let mut store = Store::new(&engine, ());

    // 3. Parse the component bytes
    let component = Component::new(&engine, WASM)?;

    // 4. Create a linker (empty if no imports needed)
    let linker = Linker::default();

    // 5. Instantiate the component
    let instance = linker.instantiate(&mut store, &component)?;

    // 6. Use the instance...
    Ok(())
}
```

### Calling Functions

Components export functions that you can call. The library provides both untyped and strongly-typed APIs:

#### Untyped API

```rust
// Get an interface from the instance
let interface = instance.exports().instance(&"test:guest/foo".try_into()?)?;

// Get a function from the interface
let func = interface.func("select_nth")?;

// Call with untyped values
let args = vec![
    Value::List(List::from(&["a", "b", "c"].map(|s| s.to_string()))),
    Value::U32(1)
];
let mut results = vec![Value::String("".into())];
func.call(&mut store, &args, &mut results)?;
println!("Result: {:?}", results[0]);
```

#### Strongly-Typed API

```rust
// Get and type the function
let select_nth = interface.func("select_nth")?
    .typed::<(Vec<String>, u32), String>()?;

// Call with strongly-typed parameters
let result = select_nth.call(&mut store, (vec!["a".to_string(), "b".to_string(), "c".to_string()], 1))?;
println!("Result: {}", result);
```

## Working with Types

The component model supports rich type system. Here's how to work with different types:

### Primitive Types

```rust
use wasm_component_layer::Value;

// Boolean
let bool_val = Value::Bool(true);

// Integers (signed and unsigned, 8-64 bits)
let s32_val = Value::S32(-42);
let u64_val = Value::U64(18446744073709551615);

// Floats
let f32_val = Value::F32(3.14159);
let f64_val = Value::F64(2.718281828459045);

// Characters and strings
let char_val = Value::Char('🚀');
let string_val = Value::String("Hello, WebAssembly!".into());
```

### Complex Types

#### Lists

```rust
use wasm_component_layer::{List, ListType, Value};

// Create a list type for strings
let list_ty = ListType::new(ValueType::String);

// Create a list value
let string_list = List::new(list_ty, vec![
    Value::String("first".into()),
    Value::String("second".into()),
    Value::String("third".into()),
])?;

// Access elements
for item in &string_list {
    println!("Item: {:?}", item);
}

// For primitive types, use specialized access
let numbers: List = vec![1i32, 2, 3].as_slice().into();
let typed_numbers = numbers.typed::<i32>()?;
assert_eq!(typed_numbers, &[1, 2, 3]);
```

#### Records

```rust
use wasm_component_layer::{Record, RecordType, Value};

// Define a record type
let person_ty = RecordType::new(None, vec![
    ("name", ValueType::String),
    ("age", ValueType::U32),
    ("active", ValueType::Bool),
])?;

// Create a record value
let person = Record::new(person_ty, vec![
    ("name", Value::String("Alice".into())),
    ("age", Value::U32(30)),
    ("active", Value::Bool(true)),
])?;

// Access fields
let name = person.field("name")?;
let age = person.field("age")?;
```

#### Tuples

```rust
use wasm_component_layer::{Tuple, TupleType, Value};

// Define a tuple type
let point_ty = TupleType::new(None, vec![
    ValueType::F32,  // x
    ValueType::F32,  // y
    ValueType::F32,  // z
]);

// Create a tuple value
let point = Tuple::new(point_ty, vec![
    Value::F32(1.0),
    Value::F32(2.5),
    Value::F64(3.0),
])?;

// Access by index
let x = &point[0];
let y = &point[1];
```

#### Variants and Enums

```rust
use wasm_component_layer::{Variant, VariantType, Enum, EnumType, Value};

// Define an enum type
let status_ty = EnumType::new(None, vec!["pending", "active", "inactive"])?;

// Create an enum value
let status = Enum::new(status_ty, 1)?; // "active"

// Define a variant type
let result_ty = VariantType::new(None, vec![
    VariantCase::new("ok", Some(ValueType::String)),
    VariantCase::new("error", Some(ValueType::String)),
    VariantCase::new("none", None),
])?;

// Create variant values
let ok_result = Variant::new(result_ty.clone(), 0, Some(Value::String("Success!".into())))?;
let none_result = Variant::new(result_ty, 2, None)?;
```

#### Options and Results

```rust
use wasm_component_layer::{OptionValue, OptionType, ResultValue, ResultType, Value};

// Option type
let opt_string_ty = OptionType::new(ValueType::String);
let some_value = OptionValue::new(opt_string_ty.clone(), Some(Value::String("Hello".into())))?;
let none_value = OptionValue::new(opt_string_ty, None)?;

// Result type
let result_ty = ResultType::new(Some(ValueType::String), Some(ValueType::String));
let ok_result = ResultValue::new(result_ty.clone(), Ok(Some(Value::String("Success".into()))))?;
let err_result = ResultValue::new(result_ty, Err(Some(Value::String("Error occurred".into()))))?;
```

### Flags

```rust
use wasm_component_layer::{Flags, FlagsType};

// Define flags type
let permissions_ty = FlagsType::new(None, vec!["read", "write", "execute"])?;

// Create flags value
let mut permissions = Flags::new(permissions_ty);

// Set flags
permissions.set("read", true);
permissions.set("write", true);
permissions.set("execute", false);

// Check flags
assert!(permissions.get("read"));
assert!(!permissions.get("execute"));
```

## Working with Resources

Resources represent opaque handles to data that can be passed between host and guest:

### Host Resources

```rust
use wasm_component_layer::{ResourceType, ResourceOwn};

// Create a host resource type for a custom struct
#[derive(Debug)]
struct MyData {
    value: i32,
}

let resource_ty = ResourceType::new::<MyData>(None);

// Create a resource instance
let resource = ResourceOwn::new(&mut store, MyData { value: 42 }, resource_ty)?;

// Borrow the resource
let borrow = resource.borrow(&mut store)?;

// Access the underlying data
{
    let data = resource.rep::<MyData, _, _>(&store)?;
    println!("Value: {}", data.value);
}

// Mutably access the data
{
    let data = resource.rep_mut::<MyData, _, _>(&mut store)?;
    data.value = 100;
}
```

### Guest Resources

Guest resources are created by WebAssembly components and can be received by the host:

```rust
// Assume a component exports a function that creates resources
let create_resource = instance.exports().root().func("create_resource")?
    .typed::<(), ResourceType>()?;

// Call the function to get a guest resource
let guest_resource: ResourceOwn = create_resource.call(&mut store, ())?;
```

## Advanced Usage

### Defining Host Functions

You can define functions in Rust that can be called from WebAssembly:

```rust
use wasm_component_layer::{Func, FuncType, Value};

// Define a function type
let func_ty = FuncType::new(
    vec![ValueType::String, ValueType::U32],
    vec![ValueType::String]
);

// Create a host function
let host_func = Func::new(&mut store, func_ty, |ctx, args, results| {
    let input_string = &args[0];
    let count = &args[1];

    if let (Value::String(s), Value::U32(n)) = (input_string, count) {
        let repeated = s.repeat(*n as usize);
        results[0] = Value::String(repeated.into());
    }
    Ok(())
});

// Add to linker
let mut linker = Linker::default();
linker.root_mut().define_func("repeat_string", host_func)?;
```

### Linking Multiple Interfaces

Components can import multiple interfaces:

```rust
// Define interfaces
let mut linker = Linker::default();

// Root interface
linker.root_mut().define_func("log", logging_func)?;

// Named interface
let math_interface = linker.define_instance("math:1.0".try_into()?)?;
math_interface.define_func("add", add_func)?;
math_interface.define_func("multiply", multiply_func)?;
```

### Error Handling

The library uses `anyhow::Result` for error handling:

```rust
match component_operation() {
    Ok(result) => println!("Success: {:?}", result),
    Err(e) => {
        eprintln!("Error: {}", e);
        // Check for specific error types
        if let Some(func_error) = e.downcast_ref::<FuncError>() {
            eprintln!("Function error in {}: {}", func_error.name(), func_error.error());
        }
    }
}
```

## Examples

The repository includes several examples demonstrating different features:

- `single_component`: Basic function calling
- `resource`: Working with resources
- `multilevel_resource`: Complex resource hierarchies
- `option_result`: Using option and result types
- `record_response`: Working with records
- `string_host_guest`: String passing between host and guest
- `variant_return`: Using variant types
- `func_param`: Function parameters

Each example includes:
- A WebAssembly component (`*.wasm`)
- Rust source code showing usage
- Build scripts for different platforms

To run an example:

```bash
cd examples/single_component
cargo run --example single_component
```

## Performance Considerations

- Use strongly-typed APIs (`TypedFunc`) when possible for better performance
- For large lists of primitive types, the library automatically uses optimized storage
- Resource operations involve reference counting - be mindful of borrow lifetimes
- Memory allocation in WebAssembly can be expensive; reuse buffers when possible

## Troubleshooting

### Common Issues

1. **"Incorrect parameter types"**: Ensure function signatures match exactly
2. **"Resource was already destroyed"**: Check resource lifetimes and borrowing rules
3. **"Could not find import"**: Verify all required imports are defined in the linker
4. **"Store mismatch"**: Use the same store instance for all operations

### Debugging

Enable debug logging to see detailed information about component operations:

```rust
env_logger::init();
```

Check component exports and imports:

```rust
println!("Exports: {:?}", component.exports());
println!("Imports: {:?}", component.imports());
```

---

> **⚠️ Warning: Documentation**
>
> This documentation was completely refined by AI and may contain inaccuracies, errors, or incomplete information. Please use it as a reference but verify critical details against the actual codebase. If you find any useful corrections, improvements, or additional content that would benefit other users, please submit a pull request to help improve this documentation for the community.
