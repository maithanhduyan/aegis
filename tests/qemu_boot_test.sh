#!/usr/bin/env bash
# AegisOS QEMU Boot Integration Test
# Verifies the kernel boots, initializes subsystems, and tasks run.
#
# Usage: ./tests/qemu_boot_test.sh [kernel_path]
# Default kernel: target/aarch64-aegis/release/aegis_os
#
# Exit codes:
#   0 = all boot checkpoints passed
#   1 = one or more checkpoints failed
#   2 = build or QEMU error

set -euo pipefail

KERNEL="${1:-target/aarch64-aegis/release/aegis_os}"
TIMEOUT_SEC=15
QEMU="qemu-system-aarch64"
PASS=0
FAIL=0

# ─── Colors ─────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# NOTE: Uses bash built-in pattern matching instead of echo|grep pipe.
# The old `echo "$OUTPUT" | grep -qF` approach caused SIGPIPE (broken pipe)
# when OUTPUT was large and grep -q exited early, combined with `set -o pipefail`
# this made the pipeline return non-zero even on successful matches.
check() {
    local label="$1"
    local pattern="$2"
    if [[ "$OUTPUT" == *"$pattern"* ]]; then
        echo -e "  ${GREEN}✓${NC} $label"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} $label (expected: '$pattern')"
        FAIL=$((FAIL + 1))
    fi
}

# ─── Build user/hello ELF binary (needed by include_bytes!) ─────────
echo -e "${YELLOW}[1/4] Building user ELF binaries...${NC}"
if ! (cd user && cargo build --release -Zjson-target-spec -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem) 2>&1; then
    echo -e "${RED}user build failed!${NC}"
    exit 2
fi

# ─── Build kernel ───────────────────────────────────────────────────
echo -e "${YELLOW}[2/4] Building kernel...${NC}"
if ! cargo build --release -Zjson-target-spec -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem 2>&1; then
    echo -e "${RED}Build failed!${NC}"
    exit 2
fi

if [ ! -f "$KERNEL" ]; then
    echo -e "${RED}Kernel not found at $KERNEL${NC}"
    exit 2
fi

# ─── Run QEMU ──────────────────────────────────────────────────────
echo -e "${YELLOW}[3/4] Running QEMU (timeout ${TIMEOUT_SEC}s)...${NC}"
OUTPUT=$(timeout "$TIMEOUT_SEC" "$QEMU" \
    -machine virt \
    -cpu cortex-a53 \
    -nographic \
    -semihosting \
    -kernel "$KERNEL" 2>&1 || true)

# ─── Check boot checkpoints ────────────────────────────────────────
echo -e "${YELLOW}[4/4] Checking boot checkpoints...${NC}"

check "Kernel boot message"         "[AegisOS] boot"
check "MMU enabled"                 "[AegisOS] MMU enabled"
check "W^X enforced"                "[AegisOS] W^X enforced"
check "Exceptions ready"            "[AegisOS] exceptions ready"
check "Scheduler ready"             "[AegisOS] scheduler ready"
check "Capabilities assigned"       "[AegisOS] capabilities assigned"
check "Priority scheduler"          "[AegisOS] priority scheduler configured"
check "Time budget enforcement"     "[AegisOS] time budget enforcement enabled"
check "Watchdog heartbeat"          "[AegisOS] watchdog heartbeat enabled"
check "Notification ready"          "[AegisOS] notification system ready"
check "Grant system ready"          "[AegisOS] grant system ready"
check "IRQ routing ready"           "[AegisOS] IRQ routing ready"
check "Device MMIO mapping ready"   "[AegisOS] device MMIO mapping ready"
check "Address spaces assigned"     "[AegisOS] per-task address spaces assigned"
check "Arch separation L1"          "[AegisOS] arch separation: module tree ready"
check "Arch separation L2"          "[AegisOS] arch separation: complete"
check "ELF64 parser ready"          "[AegisOS] ELF64 parser ready"
check "ELF loader ready"            "[AegisOS] ELF loader ready"
check "ELF task 2 loaded"           "[AegisOS] task 2 (hello) loaded from ELF"
check "ELF task 3 loaded"           "[AegisOS] task 3 (sensor) loaded from ELF"
check "ELF task 4 loaded"           "[AegisOS] task 4 (logger) loaded from ELF"
check "Multi-ELF complete"          "[AegisOS] multi-ELF loading complete"
check "Timer started"               "[AegisOS] timer started"
check "Enhanced panic handler"      "[AegisOS] enhanced panic handler ready"
check "klog ready"                   "[AegisOS] klog ready"
check "Safety audit complete"        "[AegisOS] safety audit complete"
check "Bootstrap into EL0"          "[AegisOS] bootstrapping into uart_driver"
check "UART driver ready"           "DRV:ready"
check "L5 ELF task output"          "L5:ELF"
check "Task 2 exited"               "[AegisOS] task 2 exited (code=0)"
check "Sensor initialized"          "SENSOR:init"
check "Client uses driver"          "J4:UserDrv"

# ─── Summary ───────────────────────────────────────────────────────
echo ""
echo -e "Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}"

if [ "$FAIL" -gt 0 ]; then
    echo -e "\n${RED}QEMU output:${NC}"
    echo "$OUTPUT" | head -50
    exit 1
fi

echo -e "${GREEN}All boot checkpoints passed!${NC}"
exit 0
