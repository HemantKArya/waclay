#!/bin/bash

# Build the guest component
cargo build --target wasm32-unknown-unknown

# Generate the component
wasm-tools component new target/wasm32-unknown-unknown/debug/guest_resource.wasm -o component.wasm