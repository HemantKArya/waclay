#!/usr/bin/env pwsh

Write-Host "Building WebAssembly Component..." -ForegroundColor Cyan

# Build the component
cargo build --target wasm32-unknown-unknown --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# Convert to component
Write-Host "Converting to WebAssembly Component..." -ForegroundColor Cyan
wasm-tools component new ./target/wasm32-unknown-unknown/release/web_scraper_component.wasm -o component.wasm

if ($LASTEXITCODE -ne 0) {
    Write-Host "Component conversion failed!" -ForegroundColor Red
    exit 1
}

Write-Host "✅ Component built successfully: component.wasm" -ForegroundColor Green
