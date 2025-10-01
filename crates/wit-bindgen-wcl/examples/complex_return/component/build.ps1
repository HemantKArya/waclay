# Build complex_return component
cargo component build --release --target wasm32-unknown-unknown
Copy-Item "target\wasm32-unknown-unknown\release\complex_return_component.wasm" "component.wasm"
