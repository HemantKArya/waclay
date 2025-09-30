//! Example showing the bindgen macro with a calculator world
//!
//! Run with: `cargo run --example bindgen_calculator --features macro`
//!
//! This demonstrates:
//! - Generating bindings from WIT
//! - Import interfaces
//! - Export functions with various types (primitives, results)

use wasm_component_layer::bindgen;

bindgen!({
    path: "examples/bindgen_calculator/wit/world.wit",
    world: "calculator",
});

fn main() {
    println!("Calculator bindgen example");
    println!("============================");
    println!();
    println!("Successfully generated bindings for:");
    println!("- HostImports trait for logger interface");
    println!("- Calculator world struct");
    println!("- Export methods: add, multiply, divide");
    println!();
    println!("The generated code includes:");
    println!("- Type-safe function signatures");
    println!("- Support for Result types");
    println!("- Interface imports");
}
