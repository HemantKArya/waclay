// Calculator Host - Demonstrates using generated runtime-agnostic bindings
mod bindings;

use anyhow::Result;
use bindings::*;
use wasm_component_layer::*;

// Logger host implementation
struct MyLoggerHost;

impl bindings::LoggerHost for MyLoggerHost {
    fn log(&mut self, level: LogLevel, message: String) {
        let level_str = match level {
            LogLevel::Info => "ℹ️  INFO",
            LogLevel::Warning => "⚠️  WARN",
            LogLevel::Error => "❌ ERROR",
        };
        println!("[{}] {}", level_str, message);
    }
}

fn main() -> Result<()> {
    println!("🧮 Calculator Component Host");
    println!("================================\n");

    // Create engine with wasmi runtime
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, MyLoggerHost);

    // Load the component
    let component_bytes = include_bytes!("../../component/component.wasm");
    let component = Component::new(&engine, component_bytes)?;
    println!("✅ Loaded calculator component\n");

    // Create linker and register host functions
    let mut linker = Linker::default();
    imports::register_logger_host(&mut linker, &mut store)?;

    // Instantiate the component
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Instantiated calculator component\n");

    // Get the exported functions using our generated helpers
    let calculate = exports_operations::get_calculate(&instance, &mut store)?;
    let get_history = exports_operations::get_get_history(&instance, &mut store)?;

    println!("📊 Running Calculations:");
    println!("------------------------\n");

    // Test addition
    test_calculation(
        &calculate,
        &mut store,
        Operation::Add,
        10.0,
        5.0,
        "Addition: 10 + 5",
    )?;

    // Test subtraction
    test_calculation(
        &calculate,
        &mut store,
        Operation::Subtract,
        10.0,
        5.0,
        "Subtraction: 10 - 5",
    )?;

    // Test multiplication
    test_calculation(
        &calculate,
        &mut store,
        Operation::Multiply,
        10.0,
        5.0,
        "Multiplication: 10 * 5",
    )?;

    // Test division
    test_calculation(
        &calculate,
        &mut store,
        Operation::Divide,
        10.0,
        5.0,
        "Division: 10 / 5",
    )?;

    // Test division by zero (error case)
    println!("\n🧪 Testing Error Cases:");
    println!("----------------------\n");

    match calculate.call(&mut store, (Operation::Divide, 10.0, 0.0)) {
        Ok(Ok(result)) => {
            println!("❌ Expected error but got result: {:?}", result);
        }
        Ok(Err(CalcError::DivisionByZero)) => {
            println!("✅ Division by zero correctly handled!");
        }
        Ok(Err(err)) => {
            println!("❌ Unexpected error: {:?}", err);
        }
        Err(e) => {
            println!("❌ Call failed: {}", e);
        }
    }

    // Get calculation history
    println!("\n📜 Calculation History:");
    println!("----------------------\n");

    let history = get_history.call(&mut store, ())?;
    for (i, result) in history.iter().enumerate() {
        println!(
            "  {}. {:?}: {} (success: {})",
            i + 1,
            result.operation,
            result.value,
            result.success
        );
    }

    println!("\n✅ Calculator demo completed successfully!");
    println!("\n💡 Key Features Demonstrated:");
    println!("   • Runtime-agnostic bindings (using wasmi)");
    println!("   • Host function implementation (logger)");
    println!("   • Complex types (enums, records, variants, results)");
    println!("   • Error handling (division by zero)");
    println!("   • Component state (history tracking)");

    Ok(())
}

fn test_calculation<T, E: wasm_runtime_layer::backend::WasmEngine>(
    calculate: &TypedFunc<(Operation, f64, f64), Result<CalcResult, CalcError>>,
    store: &mut Store<T, E>,
    op: Operation,
    a: f64,
    b: f64,
    description: &str,
) -> Result<()> {
    println!("🔢 {}", description);
    match calculate.call(store, (op, a, b))? {
        Ok(result) => {
            println!("   Result: {}\n", result.value);
        }
        Err(err) => {
            println!("   Error: {:?}\n", err);
        }
    }
    Ok(())
}
