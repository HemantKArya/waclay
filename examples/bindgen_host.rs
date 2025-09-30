use wasm_component_layer::bindgen;

bindgen!({
    path: "examples/bindgen_host/wit/world.wit",
    world: "host-example",
});

fn main() {
    println!("Bindgen test compiled successfully!");
}
