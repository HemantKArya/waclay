//! Builds a real WIT component binary (the cargo-component way) and writes
//! it to `adder.component.wasm`.
//!
//! ```sh
//! cargo run -p waclay --example build_adder
//! waclay-cli inspect adder.component.wasm
//! waclay-cli run adder.component.wasm --invoke add --args '[40, 2]' --wasi
//! ```

use wit_component::ComponentEncoder;

fn main() {
    let core = wat::parse_str(
        r#"(module
            (memory (export "memory") 1)
            (func (export "add") (param i32 i32) (result i32)
                (i32.add (local.get 0) (local.get 1)))
        )
        "#,
    )
    .unwrap();

    let mut resolve = wit_parser::Resolve::new();
    let pkg = resolve
        .push_str(
            "test:calc",
            r#"
package test:calc;

world calc-world {
    export add: func(a: u32, b: u32) -> u32;
}
"#,
        )
        .unwrap();
    let world = resolve
        .select_world(&[pkg], Some("test:calc/calc-world"))
        .unwrap();

    let mut module = core;
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
        true,
    )
    .unwrap();

    let component = ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .validate(true)
        .encode()
        .unwrap();

    std::fs::write("adder.component.wasm", &component).unwrap();
    println!(
        "wrote adder.component.wasm ({} bytes) — try:
  waclay-cli inspect adder.component.wasm
  waclay-cli run adder.component.wasm --invoke add --args '[40, 2]'",
        component.len()
    );
}
