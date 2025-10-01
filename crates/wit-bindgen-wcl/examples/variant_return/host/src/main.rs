mod bindings;

use anyhow::Result;
use bindings::*;
use wasm_component_layer::*;

fn main() -> Result<()> {
    println!("🔀 Variant Return Example");
    println!("==========================\n");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, ());
    let component_bytes = std::fs::read("examples/variant_return/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;
    
    let linker = Linker::default();
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component loaded\n");

    let get_status = exports_exports::get_get_status(&instance, &mut store)?;

    println!("📊 Testing Variant Returns:\n");

    // The component cycles through different status variants
    for i in 1..=5 {
        println!("Call {}: ", i);
        let status = get_status.call(&mut store, ())?;
        
        match status {
            Status::Pending => {
                println!("  Status: Pending ⏳\n");
            }
            Status::Running(msg) => {
                println!("  Status: Running");
                println!("  Message: \"{}\"\n", msg);
            }
            Status::Completed(result) => {
                println!("  Status: Completed ✅");
                match result {
                    Ok(value) => println!("  Value: \"{}\"\n", value),
                    Err(error) => println!("  Error: \"{}\"\n", error),
                }
            }
            Status::Failed(error) => {
                println!("  Status: Failed ❌");
                println!("  Error: \"{}\"\n", error);
            }
        }
    }

    println!("✅ All tests completed!\n");
    println!("💡 Key Features:");
    println!("   • Variant types with multiple cases");
    println!("   • Variants with and without payloads");
    println!("   • Nested Result inside variant");
    println!("   • Pattern matching on variants");

    Ok(())
}
