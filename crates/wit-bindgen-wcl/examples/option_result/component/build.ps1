# Build option_result component
cargo component build --release --target wasm32-unknown-unknown
Copy-Item "target\wasm32-unknown-unknown\release\option_result_component.wasm" "component.wasm"
