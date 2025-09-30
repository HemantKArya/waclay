// Example demonstrating bindgen usage with wasm_component_layer
//
// CURRENT STATUS: This demonstrates the gap between wasmtime's bindgen output
// and wasm_component_layer's API style.
//
// The bindgen macro from wasmtime generates code that expects traits like:
// - `ComponentType` for type conversion
// - `Lift` and `Lower` for value marshalling  
// - `Store<T>` with typed context
//
// But wasm_component_layer uses a different API style:
// - Manual `Linker` and `Func::new` for function definitions
// - `Value` enum for all component values
// - Untyped `Store<()>` context
//
// This example shows how bindgen SHOULD work once adapted.

use anyhow::Result;
use wasm_component_layer::*;

fn main() -> Result<()> {
    println!("Bindgen Example for wasm_component_layer");
    println!("=========================================\n");
    
    // This is the TARGET API - what we want bindgen to generate
    
    // The engine and store setup
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, ());
    
    // In the future, bindgen should generate this:
    // use example::calculator::*;
    // let mut linker = CalculatorLinker::new();
    // linker.logger().log(|ctx, message| {
    //     println!("[Logger] {}", message);
    //     Ok(())
    // });
    // let instance = linker.instantiate(&mut store, &component)?;
    // let math = instance.math();
    // let result = math.add(&mut store, 5, 3)?;
    
    // For now, we show the manual approach that bindings should automate:
    println!("✅ Manual integration example");
    println!("   (bindgen would generate this automatically)\n");
    
    println!("1. Create linker and define imports:");
    let mut linker = Linker::default();
    
    let logger_interface = linker
        .define_instance("example:calculator/logger".try_into().unwrap())
        .unwrap();
    
    logger_interface
        .define_func(
            "log",
            Func::new(
                &mut store,
                FuncType::new([ValueType::String], []),
                |_, params, _| {
                    if let Value::String(msg) = &params[0] {
                        println!("[Logger] {}", msg);
                    }
                    Ok(())
                },
            ),
        )
        .unwrap();
    
    println!("   ✓ Defined logger interface");
    
    println!("\n2. Load component (would have pre-compiled .wasm):");
    println!("   ⚠️  No component binary yet - example shows structure");
    
    println!("\n3. Instantiate and call exports:");
    println!("   (bindgen would create typed wrappers)");
    
    // Example of what the generated typed API would look like:
    println!("\n📝 Target API (what bindgen should generate):");
    println!("   ```rust");
    println!("   let mut linker = CalculatorLinker::new();");
    println!("   linker.logger().log(|_ctx, msg: &str| {{");
    println!("       println!(\"[Logger] {{}}\", msg);");
    println!("       Ok(())");
    println!("   }})?;");
    println!("   ");
    println!("   let instance = linker.instantiate(&mut store, &component)?;");
    println!("   let math = instance.math();");
    println!("   let result = math.add(&mut store, 5, 3)?;");
    println!("   assert_eq!(result, 8);");
    println!("   ```");
    
    println!("\n✅ Example complete");
    println!("\nNext Steps:");
    println!("  1. Modify bindgen codegen to output wasm_component_layer-style code");
    println!("  2. Generate Linker wrapper with typed import methods");
    println!("  3. Generate Instance wrapper with typed export methods");
    println!("  4. Implement Value conversion helpers");
    
    Ok(())
}
