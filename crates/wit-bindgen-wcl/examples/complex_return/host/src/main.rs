mod bindings;

use anyhow::Result;
use bindings::*;
use wasm_component_layer::*;

fn main() -> Result<()> {
    println!("📦 Complex Return Example");
    println!("==========================\n");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, ());
    let component_bytes = std::fs::read("examples/complex_return/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;

    let linker = Linker::default();
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component loaded\n");

    let get_complex_data = exports_exports::get_get_complex_data(&instance, &mut store)?;
    let complex_data = get_complex_data.call(&mut store, ())?;

    println!("🔍 Testing Complex Data Structure:\n");
    println!("📊 Complex Data:");
    println!("  ID: {}", complex_data.id);
    println!("  Name: {}", complex_data.name);
    println!("  Values: {:?}", complex_data.values);
    println!("  Metadata: {:?}", complex_data.metadata);
    println!("  Status: {:?}\n", complex_data.status);

    // Verify the data
    assert_eq!(complex_data.id, 42);
    assert_eq!(complex_data.name, "complex-object");
    assert_eq!(complex_data.values, vec![1.1, 2.2, 3.3, 4.4, 5.5]);
    assert_eq!(complex_data.metadata, Some("metadata-value".to_string()));
    assert_eq!(complex_data.status, Ok("success".to_string()));

    println!("✅ All tests passed!\n");
    println!("💡 Key Features:");
    println!("   • Complex record with multiple field types");
    println!("   • List<f64> for numeric arrays");
    println!("   • Option<string> for optional fields");
    println!("   • Result<string, string> in record");

    Ok(())
}
