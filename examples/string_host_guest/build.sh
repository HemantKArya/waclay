#!/bin/bash

# Build the guest component
cargo build --target wasm32-unknown-unknown

# Generate the component
wasm-tools component new target/wasm32-unknown-unknown/debug/string_host_guest_component.wasm -o component.wasm