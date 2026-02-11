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

check() {
    local label="$1"
    local pattern="$2"
    if echo "$OUTPUT" | grep -qF "$pattern"; then
        echo -e "  ${GREEN}✓${NC} $label"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}✗${NC} $label (expected: '$pattern')"
        FAIL=$((FAIL + 1))
    fi
}

# ─── Build kernel ───────────────────────────────────────────────────
echo -e "${YELLOW}[1/3] Building kernel...${NC}"
if ! cargo build --release -Zjson-target-spec -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem 2>&1; then
    echo -e "${RED}Build failed!${NC}"
    exit 2
fi

if [ ! -f "$KERNEL" ]; then
    echo -e "${RED}Kernel not found at $KERNEL${NC}"
    exit 2
fi

# ─── Run QEMU ──────────────────────────────────────────────────────
echo -e "${YELLOW}[2/3] Running QEMU (timeout ${TIMEOUT_SEC}s)...${NC}"
OUTPUT=$(timeout "$TIMEOUT_SEC" "$QEMU" \
    -machine virt \
    -cpu cortex-a53 \
    -nographic \
    -semihosting \
    -kernel "$KERNEL" 2>&1 || true)

# ─── Check boot checkpoints ────────────────────────────────────────
echo -e "${YELLOW}[3/3] Checking boot checkpoints...${NC}"

check "Kernel boot message"         "[AegisOS] boot"
check "MMU enabled"                 "[AegisOS] MMU enabled"
check "W^X enforced"                "[AegisOS] W^X enforced"
check "Exceptions ready"            "[AegisOS] exceptions ready"
check "Scheduler ready"             "[AegisOS] scheduler ready"
check "Capabilities assigned"       "[AegisOS] capabilities assigned"
check "Notification ready"          "[AegisOS] notification system ready"
check "Address spaces assigned"     "[AegisOS] per-task address spaces assigned"
check "Timer started"               "[AegisOS] timer started"
check "Bootstrap into EL0"          "[AegisOS] bootstrapping into task_a"
check "Task A sends PING"           "A:PING"
check "Task B sends PONG"           "B:PONG"

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
