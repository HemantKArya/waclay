#!/usr/bin/env pwsh

# Build the guest component
cargo build --target wasm32-unknown-unknown --release

# Generate the component
wasm-tools.exe component new target/wasm32-unknown-unknown/release/variant_return_guest.wasm -o component.wasm