# AegisOS Pre-Push Check (Windows PowerShell)
#
# Usage:
#   .\scripts\pre-push.ps1            # Native (host tests + QEMU if available)
#   .\scripts\pre-push.ps1 -Docker    # Via Docker (mirrors GitHub Actions CI exactly)
#
# This script validates your code before pushing to GitHub,
# preventing CI failures by catching issues locally.

param(
    [switch]$Docker
)

$ErrorActionPreference = "Continue"

# ─── Docker mode ───────────────────────────────────────────────────
if ($Docker) {
    Write-Host "Running CI in Docker (mirrors GitHub Actions)..." -ForegroundColor Cyan
    Write-Host ""

    # Check Docker is available
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Write-Host "Docker not found! Install Docker Desktop first." -ForegroundColor Red
        Write-Host "  https://docs.docker.com/desktop/install/windows-install/" -ForegroundColor Yellow
        exit 2
    }

    # Build image if needed
    $imageExists = docker image inspect aegis-ci 2>$null
    if (-not $imageExists) {
        Write-Host "Building Docker image (first time, ~2-3 min)..." -ForegroundColor Yellow
        docker build -t aegis-ci ci/
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Docker build failed!" -ForegroundColor Red
            exit 2
        }
    }

    # Run CI in container
    $currentDir = (Get-Location).Path -replace '\\', '/'
    docker run --rm `
        -v "${currentDir}:/workspace:ro" `
        -v "aegis-ci-target:/workspace/target" `
        aegis-ci

    exit $LASTEXITCODE
}

# ─── Native mode ───────────────────────────────────────────────────
Write-Host "╔══════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║       AegisOS Pre-Push Check (native)        ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

$failed = 0

# Step 1: Host unit tests
Write-Host "[1/2] Host Unit Tests..." -ForegroundColor Yellow
$testOutput = & cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1 2>&1
$testText = ($testOutput | Out-String)

if ($testText -match 'test result: ok\. (\d+) passed') {
    $count = $Matches[1]
    Write-Host "  ✓ Host tests passed ($count tests)" -ForegroundColor Green
} else {
    Write-Host "  ✗ Host tests FAILED" -ForegroundColor Red
    Write-Host $testText
    $failed = 1
}

# Step 2: QEMU boot test (only if qemu available)
Write-Host ""
$qemuExists = Get-Command qemu-system-aarch64 -ErrorAction SilentlyContinue
if ($qemuExists) {
    Write-Host "[2/2] QEMU Boot Test..." -ForegroundColor Yellow
    & powershell -ExecutionPolicy Bypass -File tests\qemu_boot_test.ps1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ QEMU boot passed" -ForegroundColor Green
    } else {
        Write-Host "  ✗ QEMU boot FAILED" -ForegroundColor Red
        $failed = 1
    }
} else {
    Write-Host "[2/2] Skipping QEMU (not installed). Use -Docker for full CI." -ForegroundColor Yellow
}

# Summary
Write-Host ""
if ($failed -eq 0) {
    Write-Host "✅ Pre-push checks passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "❌ Pre-push checks failed!" -ForegroundColor Red
    exit 1
}
