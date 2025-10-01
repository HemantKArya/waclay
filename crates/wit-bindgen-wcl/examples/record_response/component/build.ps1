# Build and componentize the WASM module
cargo build --target wasm32-unknown-unknown --release
wasm-tools component new target/wasm32-unknown-unknown/release/record_response.wasm -o component.wasm
Write-Host "✅ Component built: component.wasm"
