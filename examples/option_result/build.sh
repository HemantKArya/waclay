#!/bin/bash

# Build the guest component
cargo build --target wasm32-unknown-unknown

# Generate the component
wasm-tools component new target/wasm32-unknown-unknown/debug/option_result.wasm -o component.wasm