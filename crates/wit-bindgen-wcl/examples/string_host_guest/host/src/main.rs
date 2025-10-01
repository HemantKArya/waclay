mod bindings;

use anyhow::Result;
use bindings::*;
use waclay::*;

struct MyHostLogger;

impl HostLoggerHost for MyHostLogger {
    fn host_log(&mut self, message: String) {
        println!("  [HOST LOG] {}", message);
    }
}

fn main() -> Result<()> {
    println!("🔗 String Host-Guest Example");
    println!("==============================\n");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, MyHostLogger);
    let component_bytes = std::fs::read("examples/string_host_guest/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;

    let mut linker = Linker::default();
    // Register host functions
    imports::register_host_logger_host(&mut linker, &mut store)?;

    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component loaded\n");

    let process_message = exports_message::get_process_message(&instance, &mut store)?;

    println!("📊 Testing Host-Guest Communication:\n");

    println!("Test 1: 'Hello World'");
    let result = process_message.call(&mut store, "Hello World".to_string())?;
    println!("  Result: \"{}\"\n", result);

    println!("Test 2: 'Testing callbacks'");
    let result = process_message.call(&mut store, "Testing callbacks".to_string())?;
    println!("  Result: \"{}\"\n", result);

    println!("✅ All tests completed!\n");
    println!("💡 Key Features:");
    println!("   • Host function imports");
    println!("   • Bidirectional communication");
    println!("   • String passing both ways");

    Ok(())
}
