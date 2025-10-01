# Master test script for wcomp_layer workspace
# Tests both wasm_component_layer and wit-bindgen-wcl crates

param(
    [switch]$SkipAndroid,
    [switch]$SkipLinux,
    [switch]$SkipExamples,
    [switch]$Fast  # Skip Android, Linux, and only run core tests
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "WCOMP_LAYER WORKSPACE TEST SUITE" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$StartTime = Get-Date

# Parse Fast mode
if ($Fast) {
    $SkipAndroid = $true
    $SkipLinux = $true
    Write-Host "Fast mode enabled - skipping Android and Linux builds" -ForegroundColor Yellow
    Write-Host ""
}

# Test wasm_component_layer
Write-Host "Testing wasm_component_layer crate..." -ForegroundColor Magenta
Write-Host ""

$wcompArgs = @()
if ($SkipAndroid) { $wcompArgs += "-SkipAndroid" }
if ($SkipLinux) { $wcompArgs += "-SkipLinux" }

& .\test-wcomp-layer.ps1 @wcompArgs

if ($LASTEXITCODE -ne 0) {
    Write-Host "wasm_component_layer tests failed!" -ForegroundColor Red
    exit 1
}

# Test wit-bindgen-wcl
Write-Host "Testing wit-bindgen-wcl crate..." -ForegroundColor Magenta
Write-Host ""

$bindgenArgs = @()
if ($SkipExamples) { $bindgenArgs += "-SkipExamples" }

& .\test-wit-bindgen.ps1 @bindgenArgs

if ($LASTEXITCODE -ne 0) {
    Write-Host "wit-bindgen-wcl tests failed!" -ForegroundColor Red
    exit 1
}

# Workspace-level tests
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Workspace-level checks" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Checking workspace build..." -ForegroundColor Yellow
cargo build --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "Workspace build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Workspace builds successfully." -ForegroundColor Green
Write-Host ""

Write-Host "Running workspace tests..." -ForegroundColor Yellow
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Host "Workspace tests failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Workspace tests passed." -ForegroundColor Green
Write-Host ""

# Summary
$EndTime = Get-Date
$Duration = $EndTime - $StartTime

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "ALL TESTS PASSED!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Test Summary:" -ForegroundColor Cyan
Write-Host "  - wasm_component_layer: PASSED" -ForegroundColor Green
Write-Host "  - wit-bindgen-wcl: PASSED" -ForegroundColor Green
Write-Host "  - Workspace: PASSED" -ForegroundColor Green
Write-Host ""
Write-Host "Total time: $($Duration.ToString('mm\:ss'))" -ForegroundColor Cyan
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

# Instructions
Write-Host "Quick Test Commands:" -ForegroundColor Yellow
Write-Host "  .\test-all.ps1              # Full test suite (all platforms)" -ForegroundColor White
Write-Host "  .\test-all.ps1 -Fast        # Quick tests (skip Android/Linux)" -ForegroundColor White
Write-Host "  .\test-wcomp-layer.ps1      # Test wasm_component_layer only" -ForegroundColor White
Write-Host "  .\test-wit-bindgen.ps1      # Test wit-bindgen-wcl only" -ForegroundColor White
Write-Host ""