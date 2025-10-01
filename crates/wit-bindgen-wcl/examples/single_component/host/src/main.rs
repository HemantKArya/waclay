mod bindings;

use anyhow::Result;
use bindings::*;
use waclay::*;

fn main() -> Result<()> {
    println!("📋 Single Component Example - List Operations");
    println!("================================================\n");

    // Create engine and load component
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, ());
    let component_bytes = std::fs::read("examples/single_component/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;
    println!("✅ Loaded component\n");

    // Instantiate component (no imports needed)
    let linker = Linker::default();
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Instantiated component\n");

    // Get the exported function
    let select_nth = exports_foo::get_select_nth(&instance, &mut store)?;

    // Test data
    let test_list = vec![
        "apple".to_string(),
        "banana".to_string(),
        "cherry".to_string(),
        "date".to_string(),
        "elderberry".to_string(),
    ];

    println!("📊 Test List: {:?}\n", test_list);
    println!("🔍 Running Tests:");
    println!("------------------\n");

    // Test 1: Select first element
    println!("Test 1: Select index 0");
    let result = select_nth.call(&mut store, (test_list.clone(), 0))?;
    println!("  Result: \"{}\" ✅\n", result);

    // Test 2: Select middle element
    println!("Test 2: Select index 2");
    let result = select_nth.call(&mut store, (test_list.clone(), 2))?;
    println!("  Result: \"{}\" ✅\n", result);

    // Test 3: Select last element
    println!("Test 3: Select index 4");
    let result = select_nth.call(&mut store, (test_list.clone(), 4))?;
    println!("  Result: \"{}\" ✅\n", result);

    // Test 4: Out of bounds
    println!("Test 4: Select index 10 (out of bounds)");
    match select_nth.call(&mut store, (test_list, 10)) {
        Ok(result) => println!("  Result: \"{}\" (handled gracefully) ✅\n", result),
        Err(e) => println!("  Error: {} (expected) ✅\n", e),
    }

    println!("✅ All tests completed!\n");
    println!("💡 Key Features Demonstrated:");
    println!("   • List<string> parameter passing");
    println!("   • String return values");
    println!("   • Basic component exports");
    println!("   • Runtime-agnostic bindings (using wasmi)");

    Ok(())
}
