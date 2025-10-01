# Build calculator component
Write-Host "Building calculator component..." -ForegroundColor Cyan

# Build the WASM module
cargo build --target wasm32-unknown-unknown --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to build WASM module" -ForegroundColor Red
    exit 1
}

Write-Host "Componentizing WASM..." -ForegroundColor Cyan

# Convert to component
wasm-tools component new target/wasm32-unknown-unknown/release/calculator_component.wasm -o component.wasm

if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ Component built successfully: component.wasm" -ForegroundColor Green
    Write-Host "Size: $((Get-Item component.wasm).Length) bytes"
} else {
    Write-Host "Failed to componentize WASM" -ForegroundColor Red
    exit 1
}
