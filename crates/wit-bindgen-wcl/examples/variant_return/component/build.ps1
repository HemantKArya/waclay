cargo build --target wasm32-unknown-unknown --release
wasm-tools component new target/wasm32-unknown-unknown/release/variant_return.wasm -o component.wasm
Write-Host "✅ Component built"
