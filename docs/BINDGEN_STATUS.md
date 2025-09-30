# Bindgen Integration Status

## Overview

The bindgen macro has been successfully integrated from wasmtime, but there's an important compatibility gap that needs to be addressed.

## Current State

### ✅ What Works

1. **Bindgen Macro Available**: The `bindgen!` macro from wasmtime has been adapted and is available with the `macro` feature flag.

2. **WIT Parsing**: The macro can parse WIT files and generate Rust type definitions for:
   - Records (structs)
   - Variants (enums with payloads)
   - Enums
   - Flags
   - Lists, Options, Results
   - Interfaces

3. **Code Generation**: The macro generates Rust code that compiles.

### ⚠️ The Compatibility Gap

The generated code assumes **wasmtime's component model API**, which is different from **wasm_component_layer's API**:

#### Wasmtime's API Style (what bindgen generates):
```rust
// Assumes traits like ComponentType, Lift, Lower
let mut linker = component::Linker::new(&engine);
linker.root().func_wrap(|ctx: StoreContextMut<T>, (arg1, arg2): (i32, String)| {
    // Automatic type conversion via ComponentType trait
    Ok((result,))
})?;

let instance = linker.instantiate(&mut store, &component)?;
let func = instance.get_typed_func::<(i32, String), (String,)>(&mut store, "my-func")?;
let result = func.call(&mut store, (42, "hello".to_string()))?;
```

#### wasm_component_layer's API Style (what currently works):
```rust
// Uses Value enum, FuncType, manual conversion
let mut linker = Linker::default();
let interface = linker.define_instance("my:package/interface".try_into()?)?;
interface.define_func(
    "my-func",
    Func::new(
        &mut store,
        FuncType::new([ValueType::S32, ValueType::String], [ValueType::String]),
        |_, params, results| {
            // Manual pattern matching on Value enum
            let Value::S32(arg1) = params[0] else { ... };
            let Value::String(arg2) = &params[1] else { ... };
            results[0] = Value::String(compute_result(arg1, arg2));
            Ok(())
        },
    ),
)?;

let instance = linker.instantiate(&mut store, &component)?;
let interface = instance.exports().instance(&"my:package/interface".try_into()?)?;
let func = interface.func("my-func")?;
let mut results = vec![Value::Bool(false)]; // placeholder
func.call(&mut store, &[Value::S32(42), Value::String("hello".into())], &mut results)?;
```

## Why the Gap Exists

1. **wasmtime** has built-in component model support with traits (`ComponentType`, `Lift`, `Lower`) for automatic type conversion.

2. **wasm_component_layer** is runtime-agnostic and uses a more explicit `Value` enum approach that works across different WASM runtimes (wasmi, wasmtime, etc.).

3. The bindgen code was copied from wasmtime and generates code assuming wasmtime's traits exist.

## Solutions

There are two main approaches to bridge this gap:

### Option 1: Implement Component Model Traits (Recommended)

Add wasmtime-style traits to `wasm_component_layer`:

```rust
// In wasm_component_layer
pub trait ComponentType {
    fn ty() -> ValueType;
    fn from_value(value: &Value) -> Result<Self>;
    fn into_value(self) -> Result<Value>;
}

// Implement for all supported types
impl ComponentType for i32 {
    fn ty() -> ValueType { ValueType::S32 }
    fn from_value(value: &Value) -> Result<Self> {
        match value {
            Value::S32(v) => Ok(*v),
            _ => bail!("expected S32"),
        }
    }
    fn into_value(self) -> Result<Value> {
        Ok(Value::S32(self))
    }
}

// Similar for String, Vec<T>, Option<T>, Result<T, E>, records, etc.
```

Then bindgen-generated code would work as-is.

### Option 2: Modify Bindgen Codegen

Modify the bindgen code generator to output `wasm_component_layer`-style code:

```rust
// Generated linker wrapper
pub struct CalculatorLinker {
    linker: wasm_component_layer::Linker,
}

impl CalculatorLinker {
    pub fn new() -> Self {
        Self { linker: Linker::default() }
    }
    
    pub fn logger(&mut self) -> LoggerImports {
        LoggerImports { linker: &mut self.linker }
    }
    
    pub fn instantiate(
        self,
        store: &mut Store<()>,
        component: &Component,
    ) -> Result<CalculatorInstance> {
        let instance = self.linker.instantiate(store, component)?;
        Ok(CalculatorInstance { instance })
    }
}

pub struct LoggerImports<'a> {
    linker: &'a mut Linker,
}

impl<'a> LoggerImports<'a> {
    pub fn log<F>(&mut self, func: F) -> Result<()>
    where
        F: Fn(&str) -> Result<()> + 'static,
    {
        let interface = self.linker
            .define_instance("example:calculator/logger".try_into()?)?;
        interface.define_func(
            "log",
            Func::new(
                store,
                FuncType::new([ValueType::String], []),
                move |_, params, _| {
                    let Value::String(msg) = &params[0] else {
                        bail!("expected string");
                    };
                    func(msg)?;
                    Ok(())
                },
            ),
        )?;
        Ok(())
    }
}
```

## Recommendation

**Option 1 (Implement Traits) is recommended** because:

1. ✅ Maintains compatibility with wasmtime's bindgen
2. ✅ Easier to track upstream wasmtime changes
3. ✅ Provides a better developer experience with automatic type conversion
4. ✅ Less code to maintain (traits vs custom codegen)

## Examples

- **`examples/bindgen_example_host.rs`** - Demonstrates the target API and current gap
- **`examples/option_result.rs`** - Shows current manual integration style
- **`examples/single_component.rs`** - Shows typed func access pattern

## Next Steps

1. Implement `ComponentType` trait and basic implementations
2. Add `Lift` and `Lower` traits for value marshalling
3. Test with existing examples
4. Update bindgen tests to verify compatibility
5. Create end-to-end example with actual component binary

## References

- [Wasmtime Component Model Documentation](https://docs.wasmtime.dev/api/wasmtime/component/)
- [WebAssembly Component Model Specification](https://github.com/WebAssembly/component-model)
- Current Implementation: `crates/bindgen/`, `crates/component-macro/`
