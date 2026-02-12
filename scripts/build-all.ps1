# AegisOS — Build All (user crates + kernel)
# Phase O: Multi-ELF user ecosystem requires building user workspace first,
# then kernel (which embeds user binaries via include_bytes!).
$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent (Split-Path -Parent $PSCommandPath)

Write-Host "=== Building user crates ==="
Push-Location "$RootDir\user"
try {
    cargo build --release -Zjson-target-spec
} finally {
    Pop-Location
}

Write-Host "=== Building kernel ==="
Push-Location $RootDir
try {
    cargo build --release -Zjson-target-spec -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem
} finally {
    Pop-Location
}

Write-Host "=== Build complete ==="
Write-Host "Run: qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel target/aarch64-aegis/release/aegis_os"
