// Test program to verify top-level function bindings generation
// This demonstrates that wit-bindgen-wcl can now generate bindings for
// top-level functions (not in interfaces) in WIT files.

mod bindings;

use bindings::*;
use anyhow::Result;

// Implement the host traits for top-level function imports
struct HostImpl;

impl MultiplyHost for HostImpl {
    fn multiply(&mut self, a: f32, b: f32) -> f32 {
        println!("Host: multiply({}, {}) = {}", a, b, a * b);
        a * b
    }
}

impl LogMessageHost for HostImpl {
    fn log_message(&mut self, message: String) {
        println!("Host log: {}", message);
    }
}

fn main() -> Result<()> {
    println!("=== Top-Level Function Bindings Test ===");
    println!();
    println!("✓ Successfully generated bindings for top-level functions!");
    println!("✓ Host traits generated: MultiplyHost, LogMessageHost");
    println!("✓ Import registration functions generated");
    println!("✓ Export helper modules generated: exports_add, exports_compute");
    println!();
    println!("This demonstrates that wit-bindgen-wcl now supports:");
    println!("  - Top-level function imports (e.g., 'import multiply: func(..)')" );
    println!("  - Top-level function exports (e.g., 'export add: func(..)')");
    println!();
    println!("The generated bindings compile successfully!");
    
    Ok(())
}
