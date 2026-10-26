# Local check for the waclay workspace: the same steps CI runs.
#
#   .\test-all.ps1          fmt, clippy, tests, docs
#   .\test-all.ps1 -Fast    skip the documentation build

param(
    [switch]$Fast
)

$ErrorActionPreference = "Stop"

function Invoke-Step {
    param([string]$Title, [scriptblock]$Command)
    Write-Host ""
    Write-Host "== $Title" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $Title" -ForegroundColor Red
        exit 1
    }
    Write-Host "ok: $Title" -ForegroundColor Green
}

$start = Get-Date

Invoke-Step "Formatting"  { cargo fmt --all -- --check }
Invoke-Step "Clippy"      { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Step "Tests"       { cargo test --workspace --release }
if (-not $Fast) {
    Invoke-Step "Documentation" { cargo doc --workspace --no-deps }
}

$elapsed = (Get-Date) - $start
Write-Host ""
Write-Host ("All checks passed in {0:N0}s" -f $elapsed.TotalSeconds) -ForegroundColor Green
