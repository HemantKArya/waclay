cargo build --target wasm32-unknown-unknown
wasm-tools.exe component new target/wasm32-unknown-unknown/debug/component_example.wasm -o component.wasm