#!/usr/bin/env bash
# AegisOS Local CI Entrypoint — mirrors GitHub Actions workflow
#
# Runs inside Docker container with ubuntu + rust nightly + qemu.
# Exit code 0 = all tests pass, non-zero = failure.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║     AegisOS Local CI (mirrors GitHub CI)     ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  rustc:  $(rustc --version)"
echo -e "  cargo:  $(cargo --version)"
echo -e "  qemu:   $(qemu-system-aarch64 --version | head -1)"
echo ""

FAILED=0

# ─── Job 1: Host Unit Tests ────────────────────────────────────────
echo -e "${YELLOW}━━━ Job 1/2: Host Unit Tests (x86_64) ━━━${NC}"
if cargo test \
    --target x86_64-unknown-linux-gnu \
    --lib --test host_tests \
    -- --test-threads=1; then
    echo -e "${GREEN}✓ Host unit tests passed${NC}"
else
    echo -e "${RED}✗ Host unit tests FAILED${NC}"
    FAILED=1
fi

echo ""

# ─── Job 2: AArch64 Build + QEMU Boot ─────────────────────────────
echo -e "${YELLOW}━━━ Job 2/2: QEMU Boot Test (AArch64) ━━━${NC}"

echo -e "${YELLOW}Building user/hello ELF binary...${NC}"
if (cd user/hello && cargo build --release \
    -Zjson-target-spec \
    -Zbuild-std=core \
    -Zbuild-std-features=compiler-builtins-mem); then
    echo -e "${GREEN}✓ user/hello built${NC}"
else
    echo -e "${RED}✗ user/hello build FAILED${NC}"
    exit 2
fi

echo -e "${YELLOW}Building AArch64 kernel...${NC}"
if cargo build --release \
    -Zjson-target-spec \
    -Zbuild-std=core \
    -Zbuild-std-features=compiler-builtins-mem; then
    echo -e "${GREEN}✓ Build succeeded${NC}"
else
    echo -e "${RED}✗ Build FAILED${NC}"
    exit 2
fi

echo -e "${YELLOW}Running QEMU boot integration test...${NC}"
if bash tests/qemu_boot_test.sh; then
    echo -e "${GREEN}✓ QEMU boot tests passed${NC}"
else
    echo -e "${RED}✗ QEMU boot tests FAILED${NC}"
    FAILED=1
fi

# ─── Summary ───────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}✅ All CI checks passed — safe to push!${NC}"
    exit 0
else
    echo -e "${RED}❌ CI checks failed — do NOT push.${NC}"
    exit 1
fi
