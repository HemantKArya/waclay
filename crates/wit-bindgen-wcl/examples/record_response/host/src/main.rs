mod bindings;

use anyhow::Result;
use bindings::*;
use waclay::*;

fn main() -> Result<()> {
    println!("📝 Record Response Example");
    println!("===========================\n");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, ());
    let component_bytes = std::fs::read("examples/record_response/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;

    let linker = Linker::default();
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component loaded\n");

    let process_message = exports_message::get_process_message(&instance, &mut store)?;

    println!("📊 Testing Record Returns:\n");

    // Test 1
    println!("Test 1: Send 'Hello'");
    let response = process_message.call(&mut store, "Hello".to_string())?;
    println!("  Response ID: {}", response.id);
    println!("  Response: \"{}\"\n", response.reply);

    // Test 2
    println!("Test 2: Send 'How are you?'");
    let response = process_message.call(&mut store, "How are you?".to_string())?;
    println!("  Response ID: {}", response.id);
    println!("  Response: \"{}\"\n", response.reply);

    // Test 3
    println!("Test 3: Send empty string");
    let response = process_message.call(&mut store, String::new())?;
    println!("  Response ID: {}", response.id);
    println!("  Response: \"{}\"\n", response.reply);

    println!("✅ All tests completed!\n");
    println!("💡 Key Features Demonstrated:");
    println!("   • Record types with multiple fields");
    println!("   • String parameters and returns");
    println!("   • u32 fields in records");

    Ok(())
}
