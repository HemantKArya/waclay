cargo build --target wasm32-unknown-unknown
wasm-tools.exe component new target/wasm32-unknown-unknown/debug/string_host_guest_component.wasm -o component.wasm