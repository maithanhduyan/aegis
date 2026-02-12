#!/usr/bin/env bash
# AegisOS Pre-Push Check — run before git push
#
# Usage:
#   ./scripts/pre-push.sh          # Native (if you have qemu + rust)
#   ./scripts/pre-push.sh --docker # Via Docker (matches CI exactly)
#
# Can also be used as a git hook:
#   cp scripts/pre-push.sh .git/hooks/pre-push

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ─── Docker mode ───────────────────────────────────────────────────
if [[ "${1:-}" == "--docker" ]]; then
    echo -e "${CYAN}Running CI in Docker (mirrors GitHub Actions)...${NC}"
    echo ""

    # Build image if not exists or Dockerfile changed
    if ! docker image inspect aegis-ci >/dev/null 2>&1; then
        echo -e "${YELLOW}Building Docker image (first time, ~2-3 min)...${NC}"
        docker build -t aegis-ci ci/
    fi

    # Run CI — mount workspace read-only, use separate target dir
    docker run --rm \
        -v "$(pwd):/workspace:ro" \
        -v aegis-ci-target:/workspace/target \
        aegis-ci

    exit $?
fi

# ─── Native mode ───────────────────────────────────────────────────
echo -e "${CYAN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║       AegisOS Pre-Push Check (native)        ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════╝${NC}"
echo ""

FAILED=0

# Step 1: Host unit tests
echo -e "${YELLOW}[1/2] Host Unit Tests...${NC}"
if cargo test --target x86_64-unknown-linux-gnu --lib --test host_tests -- --test-threads=1 2>&1; then
    echo -e "${GREEN}✓ Host tests passed${NC}"
else
    echo -e "${RED}✗ Host tests FAILED${NC}"
    FAILED=1
fi

# Step 2: QEMU boot (only if qemu available)
echo ""
if command -v qemu-system-aarch64 >/dev/null 2>&1; then
    echo -e "${YELLOW}[2/2] QEMU Boot Test...${NC}"
    if bash tests/qemu_boot_test.sh; then
        echo -e "${GREEN}✓ QEMU boot passed${NC}"
    else
        echo -e "${RED}✗ QEMU boot FAILED${NC}"
        FAILED=1
    fi
else
    echo -e "${YELLOW}[2/2] Skipping QEMU (not installed). Use --docker for full CI.${NC}"
fi

echo ""
if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}✅ Pre-push checks passed!${NC}"
    exit 0
else
    echo -e "${RED}❌ Pre-push checks failed!${NC}"
    exit 1
fi
