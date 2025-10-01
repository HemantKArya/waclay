# Build and componentize the WASM module
cargo build --target wasm32-unknown-unknown --release
wasm-tools component new target/wasm32-unknown-unknown/release/single_component.wasm -o component.wasm
Write-Host "✅ Component built: component.wasm"
