$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = Join-Path $root "target\native"
$env:RUSTFLAGS = "-C target-cpu=native"

Push-Location $root
try {
    cargo build --release
    Write-Output "Native engine: $env:CARGO_TARGET_DIR\release\paguro.exe"
} finally {
    Pop-Location
}
