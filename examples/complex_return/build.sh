#!/bin/bash

# Build the guest component
cargo build --target wasm32-unknown-unknown --release

# Generate the component
wasm-tools component new target/wasm32-unknown-unknown/release/complex_return_component.wasm -o component.wasm