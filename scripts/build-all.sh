#!/bin/bash
# AegisOS — Build All (user crates + kernel)
# Phase O: Multi-ELF user ecosystem requires building user workspace first,
# then kernel (which embeds user binaries via include_bytes!).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Building user crates ==="
(cd "$ROOT_DIR/user" && cargo build --release -Zjson-target-spec)

echo "=== Building kernel ==="
(cd "$ROOT_DIR" && cargo build --release -Zjson-target-spec -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem)

echo "=== Build complete ==="
echo "Run: qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel target/aarch64-aegis/release/aegis_os"
