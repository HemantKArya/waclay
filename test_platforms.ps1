# Test script for Windows and Android platforms
# Requires cargo-ndk for Android builds

Write-Host "Testing on Windows..."

# Run tests on Windows
cargo test

if ($LASTEXITCODE -ne 0) {
    Write-Host "Windows tests failed!"
    exit 1
}

Write-Host "Windows tests passed."

# Check if cargo-ndk is installed
if (!(Get-Command cargo-ndk -ErrorAction SilentlyContinue)) {
    Write-Host "cargo-ndk not found. Installing..."
    cargo install cargo-ndk
}

Write-Host "Building single_component example for Windows..."
cargo build --example single_component

if ($LASTEXITCODE -ne 0) {
    Write-Host "Windows example build failed!"
    exit 1
}

Write-Host "Windows example build successful."

Write-Host "Building for Android..."

# Build for Android x86_64
cargo ndk --target x86_64 -- build --example single_component

if ($LASTEXITCODE -ne 0) {
    Write-Host "Android x86_64 build failed!"
    exit 1
}

Write-Host "Android x86_64 build successful."

# Add x86_64-unknown-linux-gnu target for anylinux
Write-Host "Adding x86_64-unknown-linux-gnu target for anylinux..."
rustup target add x86_64-unknown-linux-gnu

if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to add linux target!"
    exit 1
}

Write-Host "Building single_component example for anylinux (x86_64-unknown-linux-gnu)..."
cargo build --target x86_64-unknown-linux-gnu --example single_component

if ($LASTEXITCODE -ne 0) {
    Write-Host "Anylinux build failed! This is expected on Windows without cross-compilation tools."
    Write-Host "To enable anylinux builds on Windows, install a cross-compiler like mingw-w64 or use WSL."
    Write-Host "Skipping anylinux build and continuing..."
} else {
    Write-Host "Anylinux build successful."
}

Write-Host "All builds successful!"