# AegisOS
[![Deploy Static Blog](https://github.com/maithanhduyan/aegis/actions/workflows/static-page.yml/badge.svg)](https://github.com/maithanhduyan/aegis/actions/workflows/static-page.yml)
[![AegisOS CI](https://github.com/maithanhduyan/aegis/actions/workflows/ci.yml/badge.svg)](https://github.com/maithanhduyan/aegis/actions/workflows/ci.yml)

**Bare-metal AArch64 microkernel for safety-critical systems.**

AegisOS is a `#![no_std]` Rust microkernel targeting QEMU `virt` machine (Cortex-A53). Zero heap, zero external dependencies — designed from scratch for environments where failure is not an option: rockets, medical devices, autonomous vehicles.

---

## ✨ Features

| Feature | Status | Phase | Description |
|---|---|---|---|
| AArch64 boot | ✅ | A | EL2 → EL1 drop, BSS clear, stack setup |
| MMU + W^X | ✅ | B | Identity-mapped page tables (L1→L2→L3, 4KB pages), WXN enforced |
| GICv2 | ✅ | C | Interrupt controller driver (GICD + GICC) |
| Generic Timer | ✅ | C | ARM CNTP_EL0, 10ms tick, INTID 30 |
| Preemptive Scheduler | ✅ | C | 3 static tasks, round-robin + priority, context switch through TrapFrame |
| User/Kernel Separation | ✅ | D | Tasks run at EL0, kernel at EL1, AP-bit isolation |
| Fault Isolation | ✅ | E | EL0 faults → task killed + auto-restart (1s delay), kernel keeps running |
| Synchronous IPC | ✅ | C | Blocking send/recv on 4 endpoints, 4-word messages |
| Capability Access Control | ✅ | G | Per-task u64 bitmask (18 bits), least-privilege enforcement on every syscall |
| Per-Task Address Space | ✅ | H | Per-task L3 page tables, ASID-tagged TTBR0 |
| Async Notifications | ✅ | I | Bitmask notify/wait, non-blocking |
| Shared Memory Grants | ✅ | J | Owner/peer grant pages, revocable |
| IRQ Routing | ✅ | J | Bind GIC INTID → task notification bit |
| User-Mode Driver | ✅ | J | UART driver runs at EL0 via MMIO map + IRQ |
| Priority Scheduler | ✅ | K | 8-level priority, time budget, epoch reset |
| Watchdog | ✅ | K | Heartbeat monitoring, fault on timeout |
| Arch Separation | ✅ | L | `arch/aarch64/` + `kernel/` + `platform/` modular structure |
| ELF64 Loader | ✅ | L | Parse + load ELF binaries, W^X enforced, `include_bytes!` embed |
| Test Infrastructure | ✅ | F–L | 189 host unit tests + 25 QEMU boot checkpoints |
| CI/CD | ✅ | F | GitHub Actions — host tests + QEMU integration on every push |

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
        ├── ELF load (user/hello binary → task slot)
        ├── Timer start (10ms tick)
        └── bootstrap() ── ERET ──► uart_driver @ EL0
                                      │
                              ┌───────┴───────────────┐
                              │           │           │
                         task 0       task 1       task 2
                       (UART drv)  (ELF hello)    (idle)
                       prio=10     prio=5         prio=0
                          SVC #0      SVC #0       SVC #0
                              │           │           │
                              └─── IPC + Notify ──────┘
```

### Source Layout

```
src/
├── arch/
│   ├── mod.rs              # cfg(aarch64) → pub use aarch64 as current
│   └── aarch64/
│       ├── mod.rs           # Re-exports all arch modules
│       ├── boot.s           # Entry point, EL2→EL1, SP + BSS setup
│       ├── exception.rs     # Vector table, TrapFrame (288B), SVC dispatch
│       ├── mmu.rs           # Page tables, identity map, W^X (WXN + AP bits)
│       └── gic.rs           # GICv2 driver (GICD + GICC)
│
├── kernel/
│   ├── mod.rs               # Re-exports all kernel modules
│   ├── sched.rs             # Priority scheduler, 3 TCBs, budget, watchdog
│   ├── ipc.rs               # Synchronous endpoint IPC, blocking send/recv
│   ├── cap.rs               # Capability access control (u64 bitmask, 18 bits)
│   ├── timer.rs             # Tick counter + tick handler logic
│   ├── grant.rs             # Shared memory grants (owner/peer)
│   ├── irq.rs               # IRQ binding + routing → notification
│   └── elf.rs               # ELF64 parser + loader (no heap)
│
├── platform/
│   ├── mod.rs               # Platform module gate
│   └── qemu_virt.rs         # MMIO addresses, memory map constants
│
├── main.rs                  # kernel_main(), 13 syscall wrappers, task entries
├── lib.rs                   # Crate root — module tree + re-exports
├── exception.rs             # Host-only stub (x86_64 tests)
├── mmu.rs                   # Host-only stub (x86_64 tests)
└── uart.rs                  # PL011 UART (dual cfg: real HW + host stub)

user/
└── hello/                   # Standalone EL0 user task (ELF binary)
    ├── Cargo.toml           # no_std, no_main, panic=abort
    ├── src/main.rs          # Entry + syscall wrappers
    └── link.ld              # User-space linker script

tests/
├── host_tests.rs            # 189 unit tests (x86_64, pure logic)
├── qemu_boot_test.sh        # QEMU integration (Linux/CI) — 25 checkpoints
└── qemu_boot_test.ps1       # QEMU integration (Windows) — 25 checkpoints

docs/
├── blog/                    # 12 articles explaining OS concepts (Vietnamese, for kids)
├── plan/                    # Phase plans (A through L)
├── standard/                # DO-178C, IEC 62304, ISO 26262 references
└── test/report/             # Test reports
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

### Host Unit Tests (189 tests)

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
| Scheduler | 30 | Priority, round-robin, budget, epoch, watchdog, fault/restart |
| IPC | 14 | Endpoint cleanup, message copy, sender queue FIFO, blocking |
| Capabilities | 18 | Bit checks, syscall mapping (0–12), least-privilege |
| Notifications | 7 | Pending bits, merge, wait flag, restart clear |
| Grants | 14 | Create, revoke, cleanup, page addr, re-create |
| IRQ Routing | 12 | Bind, ack, route, cleanup, rebind, accumulate |
| Per-Task Address Space | 10 | ASID, TTBR0, page table base, schedule preserve |
| Device Map | 4 | Valid/invalid task/device, UART L2 index |
| ELF Parser | 14 | Magic, class, arch, segments, bounds, entry point |
| ELF Loader | 5 | Segment copy, BSS zero, validate, W^X permissions |
| Page Table Constants | 1 | Phase J table constants |
| L6 Integration | 6 | Arch module, kernel exports, platform, cfg separation |
| **Total** | **189** | |

### QEMU Boot Integration (25 checkpoints)

```bash
# Linux
bash tests/qemu_boot_test.sh

# Windows (PowerShell)
.\tests\qemu_boot_test.ps1
```

| # | Checkpoint | Phase |
|---|---|---|
| 1–6 | Kernel boot, MMU, W^X, exceptions, scheduler, capabilities | A–G |
| 7–9 | Priority scheduler, time budget, watchdog | K |
| 10–14 | Notification, grant, IRQ routing, device MMIO, address spaces | H–J |
| 15–16 | Arch separation L1, L2 | L |
| 17–19 | ELF parser, loader, task loaded | L |
| 20 | L5 ELF binary loaded | L |
| 21–25 | Timer, bootstrap EL0, UART driver, ELF task output, client | A–L |

### CI

GitHub Actions runs both test suites on every push to `main`/`develop`:
- **Host Unit Tests** — `x86_64-unknown-linux-gnu` (189 tests)
- **QEMU Boot Test** — Build AArch64 kernel + verify 25 boot checkpoints

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

| # | Syscall | Description | Phase |
|---|---|---|---|
| 0 | `SYS_YIELD` | Voluntarily yield CPU | C |
| 1 | `SYS_SEND` | Send message on endpoint | C |
| 2 | `SYS_RECV` | Receive (blocking) from endpoint | C |
| 3 | `SYS_CALL` | Send + wait for reply (SEND + RECV) | C |
| 4 | `SYS_WRITE` | Write string to UART | D |
| 5 | `SYS_NOTIFY` | Send notification bitmask to task | I |
| 6 | `SYS_WAIT_NOTIFY` | Block until notification arrives | I |
| 7 | `SYS_GRANT_CREATE` | Create shared memory grant | J |
| 8 | `SYS_GRANT_REVOKE` | Revoke shared memory grant | J |
| 9 | `SYS_IRQ_BIND` | Bind IRQ INTID → notification bit | J |
| 10 | `SYS_IRQ_ACK` | Acknowledge IRQ, re-enable INTID | J |
| 11 | `SYS_DEVICE_MAP` | Map device MMIO into user-space | J |
| 12 | `SYS_HEARTBEAT` | Register/refresh watchdog heartbeat | K |

## 🛡️ Design Constraints

- **No heap.** All allocation is static (`static mut` arrays, linker sections). No `alloc` crate.
- **No FP/SIMD.** `CPACR_EL1.FPEN = 0` — any float instruction traps.
- **TrapFrame is ABI-locked.** 288 bytes, shared between Rust struct and assembly macros.
- **W^X everywhere.** No page is both writable and executable.
- **Capability-enforced.** Every syscall is checked against the task's capability bitmask before dispatch.

## 📚 Blog Series (Vietnamese, 14 articles)

Explanations of OS concepts written for 5th-graders — making kernel development accessible:

1. [Tại sao chúng ta cần một Hệ Điều Hành?](docs/blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh.md)
2. [Bộ nhớ là gì và tại sao phải bảo vệ nó?](docs/blog/02-bo-nho-la-gi-va-tai-sao-phai-bao-ve-no.md)
3. [Dạy máy tính làm nhiều việc cùng lúc](docs/blog/03-day-may-tinh-lam-nhieu-viec-cung-luc.md)
4. [Chìa khóa và cánh cửa — Bảo vệ Kernel](docs/blog/04-chia-khoa-va-canh-cua-bao-ve-kernel.md)
5. [Khi một task ngã, cả hệ thống không được ngã theo](docs/blog/05-khi-mot-task-nga-ca-he-thong-khong-duoc-nga-theo.md)
6. [Làm sao biết hệ thống an toàn thật?](docs/blog/06-lam-sao-biet-he-thong-an-toan-that.md)
7. [Giấy phép cho phần mềm — Ai được làm gì?](docs/blog/07-giay-phep-cho-phan-mem-ai-duoc-lam-gi.md)
8. [Mỗi chương trình một bản đồ riêng](docs/blog/08-moi-chuong-trinh-mot-ban-do-rieng.md)
9. [Chuông cửa và hàng đợi — Nói chuyện không cần chờ](docs/blog/09-chuong-cua-va-hang-doi-noi-chuyen-khong-can-cho.md)
10. [Khi chương trình tự nói chuyện với phần cứng](docs/blog/10-khi-chuong-trinh-tu-noi-chuyen-voi-phan-cung.md)
11. [Ai được chạy trước? Và ai canh gác?](docs/blog/11-ai-duoc-chay-truoc-va-ai-canh-gac.md)
12. [Dọn Nhà Và Đọc Sách Mục Lục — Arch Separation & ELF Loading](docs/blog/12-don-nha-va-doc-sach-muc-luc.md)
13. [Làm Sao Chứng Minh Phần Mềm Không Có Lỗi? — Safety Assurance](docs/blog/13-lam-sao-chung-minh-phan-mem-khong-co-loi.md)
14. [Từ 3 Lên 8 — Và Chứng Minh Bằng Toán Học](docs/blog/14-tu-3-len-8-va-chung-minh-bang-toan-hoc.md)

## 📜 Safety Standards Reference

AegisOS is developed with awareness of industry safety standards:

- **DO-178C** — Software for airborne systems
- **IEC 62304** — Medical device software lifecycle
- **ISO 26262** — Automotive functional safety

See [docs/standard/](docs/standard/) for Vietnamese summaries.

## 📄 License

This project is for educational and research purposes.
