# AegisOS

[![AegisOS CI](https://github.com/maithanhduyan/aegis/actions/workflows/ci.yml/badge.svg)](https://github.com/maithanhduyan/aegis/actions/workflows/ci.yml)

**Bare-metal AArch64 microkernel for safety-critical systems.**

AegisOS is a `#![no_std]` Rust microkernel targeting QEMU `virt` machine (Cortex-A53). Zero heap, zero external dependencies — designed from scratch for environments where failure is not an option: rockets, medical devices, autonomous vehicles.

---

## ✨ Features

| Feature | Status | Description |
|---|---|---|
| AArch64 boot | ✅ | EL2 → EL1 drop, BSS clear, stack setup |
| MMU + W^X | ✅ | Identity-mapped page tables (L1→L2→L3, 4KB pages), WXN enforced |
| GICv2 | ✅ | Interrupt controller driver (GICD + GICC) |
| Generic Timer | ✅ | ARM CNTP_EL0, 10ms tick, INTID 30 |
| Round-robin Scheduler | ✅ | 3 static tasks, preemptive via timer, context switch through TrapFrame |
| User/Kernel Separation | ✅ | Tasks run at EL0, kernel at EL1, AP-bit isolation |
| Synchronous IPC | ✅ | Blocking send/recv on endpoints, 4-word messages |
| Fault Isolation | ✅ | EL0 faults → task killed + auto-restart (1s delay), kernel keeps running |
| Capability Access Control | ✅ | Per-task bitmask, least-privilege enforcement on every syscall |
| Test Infrastructure | ✅ | 69 host unit tests + 10 QEMU boot checkpoints |
| CI/CD | ✅ | GitHub Actions — host tests + QEMU integration on every push |

## 📐 Architecture

```
boot.s (_start)
  │
  ├── EL2 → EL1 transition
  ├── BSS clear
  └── kernel_main()
        ├── MMU init (identity map, W^X)
        ├── Exception vectors install
        ├── GICv2 init
        ├── Scheduler init (3 tasks)
        ├── Capability assignment
        ├── Timer start (10ms tick)
        └── bootstrap() ── ERET ──► task_a @ EL0
                                      │
                              ┌───────┴───────┐
                              │               │
                          task_a          task_b
                         (PING)          (PONG)
                          SVC #0          SVC #0
                              │               │
                              └───── IPC ──────┘
```

### Source Layout

```
src/
├── boot.s          # Entry point, EL2→EL1, SP + BSS setup (inline via global_asm!)
├── main.rs         # kernel_main(), syscall wrappers, EL0 task entries
├── lib.rs          # Library crate — re-exports all modules for tests
├── mmu.rs          # Page tables, identity map, W^X (WXN + AP bits)
├── exception.rs    # Vector table, TrapFrame (288B ABI-locked), SVC dispatch
├── gic.rs          # GICv2 driver (GICD 0x0800_0000, GICC 0x0801_0000)
├── timer.rs        # ARM Generic Timer, 10ms tick, INTID 30
├── sched.rs        # Round-robin scheduler, 3 static TCBs, fault/restart
├── ipc.rs          # Synchronous endpoint IPC, blocking send/recv
├── cap.rs          # Capability-based access control (u64 bitmask per task)
└── uart.rs         # PL011 UART driver (0x0900_0000)

tests/
├── host_tests.rs       # 69 unit tests (x86_64, pure logic)
├── qemu_boot_test.sh   # QEMU integration test (Linux/CI)
└── qemu_boot_test.ps1  # QEMU integration test (Windows)

docs/
├── blog/           # 7 articles explaining OS concepts (Vietnamese, for kids)
├── plan/           # Phase plans (A through G)
├── standard/       # DO-178C, IEC 62304, ISO 26262 references
└── test/report/    # Test reports
```

## 🔧 Build & Run

### Prerequisites

- **Rust nightly** with `rust-src` component
- **QEMU** with `qemu-system-aarch64`

```bash
# Rust toolchain is pinned in rust-toolchain.toml (nightly + rust-src)
rustup show   # verifies nightly is active
```

### Build

```bash
cargo build --release \
  -Zjson-target-spec \
  -Zbuild-std=core \
  -Zbuild-std-features=compiler-builtins-mem
```

Output: `target/aarch64-aegis/release/aegis_os`

### Run on QEMU

```bash
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a53 \
  -nographic \
  -kernel target/aarch64-aegis/release/aegis_os
```

Expected output:
```
[AegisOS] boot
[AegisOS] MMU enabled (identity map)
[AegisOS] W^X enforced (WXN + 4KB pages)
[AegisOS] exceptions ready
[AegisOS] scheduler ready (3 tasks)
[AegisOS] capabilities assigned
[AegisOS] timer started (10ms tick)
[AegisOS] bootstrapping into task_a (EL0)...
A:PING B:PONG A:PING B:PONG A:PING B:PONG ...
```

Press `Ctrl+A`, then `X` to exit QEMU.

## 🧪 Testing

### Host Unit Tests (69 tests)

Pure-logic tests running on x86_64 — no QEMU needed:

```bash
# Linux
cargo test --target x86_64-unknown-linux-gnu --lib --test host_tests -- --test-threads=1

# Windows
cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1
```

| Test Group | Count | What it covers |
|---|---|---|
| TrapFrame Layout | 4 | Size (288B), alignment, field offsets matching assembly |
| MMU Descriptors | 18 | Bit composition, W^X invariants, AP permissions, XN, AF |
| SYS_WRITE Validation | 12 | Pointer range checks, boundary, overflow, null |
| Scheduler | 11 | Round-robin, skip Faulted/Blocked, auto-restart timing |
| IPC | 10 | Endpoint cleanup, message copy, blocking states |
| Capabilities | 14 | Bit checks, syscall mapping, least-privilege enforcement |

### QEMU Boot Integration (10 checkpoints)

```bash
# Linux
bash tests/qemu_boot_test.sh

# Windows (PowerShell)
.\tests\qemu_boot_test.ps1
```

### CI

GitHub Actions runs both test suites on every push to `main`/`develop`:
- **Host Unit Tests** — `x86_64-unknown-linux-gnu`
- **QEMU Boot Test** — Build AArch64 kernel + verify 10 boot checkpoints

## 🗺️ Memory Map (QEMU virt)

| Address | Region |
|---|---|
| `0x0800_0000` | GIC Distributor (GICD) |
| `0x0801_0000` | GIC CPU Interface (GICC) |
| `0x0900_0000` | UART0 (PL011) |
| `0x4008_0000` | Kernel load address (`_start`) |
| Linker-placed | `.text` → `.rodata` → `.data` → `.bss` → `.page_tables` (16KB) → `.task_stacks` (3×4KB) → `.user_stacks` (3×4KB) → guard page (4KB) → boot stack (16KB) |

## 🔐 Syscall ABI

| Register | Purpose |
|---|---|
| `x7` | Syscall number |
| `x6` | Endpoint ID (for IPC) |
| `x0`–`x3` | Message payload |

| # | Syscall | Description |
|---|---|---|
| 0 | `SYS_YIELD` | Voluntarily yield CPU |
| 1 | `SYS_SEND` | Send message on endpoint |
| 2 | `SYS_RECV` | Receive (blocking) from endpoint |
| 3 | `SYS_CALL` | Send + wait for reply (SEND + RECV) |
| 4 | `SYS_WRITE` | Write byte to UART |

## 🛡️ Design Constraints

- **No heap.** All allocation is static (`static mut` arrays, linker sections). No `alloc` crate.
- **No FP/SIMD.** `CPACR_EL1.FPEN = 0` — any float instruction traps.
- **TrapFrame is ABI-locked.** 288 bytes, shared between Rust struct and assembly macros.
- **W^X everywhere.** No page is both writable and executable.
- **Capability-enforced.** Every syscall is checked against the task's capability bitmask before dispatch.

## 📚 Blog Series (Vietnamese)

Explanations of OS concepts written for 5th-graders — making kernel development accessible:

1. [Tại sao chúng ta cần một Hệ Điều Hành?](docs/blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh.md)
2. [Bộ nhớ là gì và tại sao phải bảo vệ nó?](docs/blog/02-bo-nho-la-gi-va-tai-sao-phai-bao-ve-no.md)
3. [Dạy máy tính làm nhiều việc cùng lúc](docs/blog/03-day-may-tinh-lam-nhieu-viec-cung-luc.md)
4. [Chìa khóa và cánh cửa — Bảo vệ Kernel](docs/blog/04-chia-khoa-va-canh-cua-bao-ve-kernel.md)
5. [Khi một task ngã, cả hệ thống không được ngã theo](docs/blog/05-khi-mot-task-nga-ca-he-thong-khong-duoc-nga-theo.md)
6. [Làm sao biết hệ thống an toàn thật?](docs/blog/06-lam-sao-biet-he-thong-an-toan-that.md)
7. [Giấy phép cho phần mềm — Ai được làm gì?](docs/blog/07-giay-phep-cho-phan-mem-ai-duoc-lam-gi.md)

## 📜 Safety Standards Reference

AegisOS is developed with awareness of industry safety standards:

- **DO-178C** — Software for airborne systems
- **IEC 62304** — Medical device software lifecycle
- **ISO 26262** — Automotive functional safety

See [docs/standard/](docs/standard/) for Vietnamese summaries.

## 📄 License

This project is for educational and research purposes.
