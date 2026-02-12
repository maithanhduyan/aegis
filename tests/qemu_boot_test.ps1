# AegisOS QEMU Boot Integration Test (Windows PowerShell)
# Verifies the kernel boots, initializes subsystems, and tasks run.
#
# Usage: .\tests\qemu_boot_test.ps1 [-KernelPath <path>] [-TimeoutSec <n>]
#
# Exit codes:
#   0 = all boot checkpoints passed
#   1 = one or more checkpoints failed
#   2 = build or QEMU error

param(
    [string]$KernelPath = "target\aarch64-aegis\release\aegis_os",
    [int]$TimeoutSec = 15
)

$ErrorActionPreference = "Stop"

$pass = 0
$fail = 0

function Check-Output {
    param(
        [string]$Label,
        [string]$Pattern
    )
    if ($script:output -match [regex]::Escape($Pattern)) {
        Write-Host "  ✓ $Label" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "  ✗ $Label (expected: '$Pattern')" -ForegroundColor Red
        $script:fail++
    }
}

# ─── Build kernel ───────────────────────────────────────────────────
Write-Host "[1/3] Building kernel..." -ForegroundColor Yellow
$ErrorActionPreference = "Continue"
$buildOutput = & cargo build --release -Zjson-target-spec -Zbuild-std=core -Zbuild-std-features=compiler-builtins-mem 2>&1
$buildText = ($buildOutput | Out-String)
if ($buildText -match '(?m)^error') {
    Write-Host "Build failed!" -ForegroundColor Red
    Write-Host $buildText
    exit 2
}
$ErrorActionPreference = "Stop"

if (-not (Test-Path $KernelPath)) {
    Write-Host "Kernel not found at $KernelPath" -ForegroundColor Red
    exit 2
}

# ─── Run QEMU ──────────────────────────────────────────────────────
Write-Host "[2/3] Running QEMU (timeout ${TimeoutSec}s)..." -ForegroundColor Yellow

$qemu = "qemu-system-aarch64"
$qemuArgs = @(
    "-machine", "virt",
    "-cpu", "cortex-a53",
    "-nographic",
    "-semihosting",
    "-kernel", $KernelPath
)

try {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $qemu
    $psi.Arguments = $qemuArgs -join " "
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true

    $process = [System.Diagnostics.Process]::Start($psi)

    if (-not $process.WaitForExit($TimeoutSec * 1000)) {
        # Timeout — kill QEMU (expected, since kernel loops forever)
        $process.Kill()
        $process.WaitForExit(3000) | Out-Null
    }

    $script:output = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $script:output += $stderr
} catch {
    Write-Host "QEMU failed to start: $_" -ForegroundColor Red
    Write-Host "Make sure qemu-system-aarch64 is in PATH" -ForegroundColor Yellow
    exit 2
}

# ─── Check boot checkpoints ────────────────────────────────────────
Write-Host "[3/3] Checking boot checkpoints..." -ForegroundColor Yellow

Check-Output "Kernel boot message"    "[AegisOS] boot"
Check-Output "MMU enabled"            "[AegisOS] MMU enabled"
Check-Output "W^X enforced"           "[AegisOS] W^X enforced"
Check-Output "Exceptions ready"       "[AegisOS] exceptions ready"
Check-Output "Scheduler ready"        "[AegisOS] scheduler ready"
Check-Output "Capabilities assigned"  "[AegisOS] capabilities assigned"
Check-Output "Priority scheduler"     "[AegisOS] priority scheduler configured"
Check-Output "Time budget enforcement" "[AegisOS] time budget enforcement enabled"
Check-Output "Watchdog heartbeat"     "[AegisOS] watchdog heartbeat enabled"
Check-Output "Notification ready"     "[AegisOS] notification system ready"
Check-Output "Grant system ready"     "[AegisOS] grant system ready"
Check-Output "IRQ routing ready"      "[AegisOS] IRQ routing ready"
Check-Output "Device MMIO ready"      "[AegisOS] device MMIO mapping ready"
Check-Output "Address spaces assigned" "[AegisOS] per-task address spaces assigned"
Check-Output "Arch separation L1"     "[AegisOS] arch separation: module tree ready"
Check-Output "Timer started"          "[AegisOS] timer started"
Check-Output "Bootstrap into EL0"     "[AegisOS] bootstrapping into uart_driver"
Check-Output "UART Driver ready"      "DRV:ready"
Check-Output "Client uses driver"     "J4:UserDrv"

# ─── Summary ───────────────────────────────────────────────────────
Write-Host ""
Write-Host "Results: $pass passed, $fail failed"

if ($fail -gt 0) {
    Write-Host "`nQEMU output:" -ForegroundColor Red
    Write-Host ($script:output | Select-Object -First 50)
    exit 1
}

Write-Host "All boot checkpoints passed!" -ForegroundColor Green
exit 0
