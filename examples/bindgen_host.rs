//! Example showing the bindgen macro with a simple world
//!
//! Run with: `cargo run --example bindgen_host --features macro`
//!
//! This demonstrates basic bindgen functionality for generating host-side
//! bindings from WIT definitions.

use wasm_component_layer::bindgen;

bindgen!({
    path: "examples/bindgen_host/wit/world.wit",
    world: "host-example",
});

fn main() {
    println!("Bindgen test compiled successfully!");
}
