cargo build --target wasm32-unknown-unknown
wasm-tools.exe component new target/wasm32-unknown-unknown/debug/complex_return_component.wasm -o component.wasm