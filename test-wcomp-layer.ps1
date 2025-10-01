param(
    [switch]$SkipAndroid,
    [switch]$SkipLinux
)

$ErrorActionPreference = "Stop"
$CratePath = "crates\wcomp_layer"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Testing wasm_component_layer" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Test on Windows
Write-Host "Testing on Windows..." -ForegroundColor Yellow
Push-Location $CratePath
try {
    cargo test
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Windows tests failed!" -ForegroundColor Red
        exit 1
    }
    Write-Host "Windows tests passed." -ForegroundColor Green
    Write-Host ""
} finally {
    Pop-Location
}

# Build examples on Windows
Write-Host "Building examples for Windows..." -ForegroundColor Yellow
Push-Location $CratePath
try {
    cargo build --examples
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Windows example builds failed!" -ForegroundColor Red
        exit 1
    }
    Write-Host "Windows examples built successfully." -ForegroundColor Green
    Write-Host ""
} finally {
    Pop-Location
}

# Test a sample example
Write-Host "Testing single_component example..." -ForegroundColor Yellow
Push-Location $CratePath
try {
    & cargo run --example single_component
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Example run failed!" -ForegroundColor Red
        exit 1
    }
    Write-Host "Example ran successfully." -ForegroundColor Green
    Write-Host ""
} finally {
    Pop-Location
}

# Android builds
if (-not $SkipAndroid) {
    Write-Host "Building for Android..." -ForegroundColor Yellow

    # Check if cargo-ndk is installed
    if (!(Get-Command cargo-ndk -ErrorAction SilentlyContinue)) {
        Write-Host "cargo-ndk not found. Installing..." -ForegroundColor Yellow
        cargo install cargo-ndk
    }

    Push-Location $CratePath
    try {
        # Build for Android x86_64
        cargo ndk --target x86_64 -- build --example single_component

        if ($LASTEXITCODE -ne 0) {
            Write-Host "Android x86_64 build failed!" -ForegroundColor Red
            exit 1
        }

        Write-Host "Android x86_64 build successful." -ForegroundColor Green
        Write-Host ""
    } finally {
        Pop-Location
    }
} else {
    Write-Host "Skipping Android builds." -ForegroundColor Gray
    Write-Host ""
}

# Linux builds
if (-not $SkipLinux) {
    Write-Host "Building for Linux (x86_64-unknown-linux-gnu)..." -ForegroundColor Yellow

    # Add target if not present
    cmd /c "rustup target add x86_64-unknown-linux-gnu >nul 2>&1"

    Push-Location $CratePath
    try {
        cargo build --target x86_64-unknown-linux-gnu --example single_component

        if ($LASTEXITCODE -ne 0) {
            Write-Host "Linux build failed! This is expected on Windows without cross-compilation tools." -ForegroundColor Yellow
            Write-Host "To enable Linux builds on Windows, install a cross-compiler like mingw-w64 or use WSL." -ForegroundColor Gray
            Write-Host "Skipping Linux build and continuing..." -ForegroundColor Gray
            Write-Host ""
        } else {
            Write-Host "Linux build successful." -ForegroundColor Green
            Write-Host ""
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "Skipping Linux builds." -ForegroundColor Gray
    Write-Host ""
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "wasm_component_layer tests complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""