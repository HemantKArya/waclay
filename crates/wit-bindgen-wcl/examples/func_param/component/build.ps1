# Build func_param component
cargo component build --release --target wasm32-unknown-unknown
Copy-Item "target\wasm32-unknown-unknown\release\func_param_component.wasm" "component.wasm"
