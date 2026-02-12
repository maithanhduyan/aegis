# AegisOS — Copilot Instructions

## What is this?

A bare-metal AArch64 microkernel targeting safety-critical systems (rockets, medical, autonomous vehicles). Runs on QEMU `virt` machine with Cortex-A53. Written in `#![no_std]` Rust + AArch64 assembly, zero heap, zero external dependencies.

## Architecture

Boot flow: `arch/aarch64/boot.s` (_start) → EL2→EL1 drop → BSS clear → `mmu::init()` → `kernel_main()` → exception/GIC/timer/scheduler init → ELF load (user/hello binary) → `sched::bootstrap()` ereting into uart_driver at **EL0**.

### Module Structure (Phase L)

```
src/
├── arch/                    # Architecture-specific code
│   ├── mod.rs               # cfg(aarch64) pub use aarch64 as current
│   └── aarch64/
│       ├── boot.s           # Entry, EL2→EL1, SP, BSS clear
│       ├── exception.rs     # Vector table, TrapFrame (288B), SVC dispatch, fault handlers
│       ├── mmu.rs           # Page tables, identity map, W^X, TLB, map/unmap
│       └── gic.rs           # GICv2 driver (GICD + GICC)
│
├── kernel/                  # Portable kernel logic
│   ├── sched.rs             # Priority scheduler, 3 TCBs, budget, epoch, watchdog
│   ├── ipc.rs               # Synchronous endpoint IPC, blocking send/recv
│   ├── cap.rs               # Capability access control (u64 bitmask, 18 bits)
│   ├── timer.rs             # Tick counter + tick handler logic
│   ├── grant.rs             # Shared memory grants (owner/peer)
│   ├── irq.rs               # IRQ binding + routing → notification
│   └── elf.rs               # ELF64 parser + loader (no heap, no FP)
│
├── platform/
│   └── qemu_virt.rs         # MMIO addresses, memory map constants
│
├── main.rs                  # kernel_main(), 13 syscall wrappers, task entries
├── lib.rs                   # Crate root — module tree + re-exports
├── exception.rs             # Host-only stub (x86_64 tests)
├── mmu.rs                   # Host-only stub (x86_64 tests)
└── uart.rs                  # PL011 UART (dual cfg: real HW + host stub)

user/hello/                  # Standalone EL0 user task (ELF binary)
```

| Module | Role | Key details |
|---|---|---|
| `arch/aarch64/boot.s` | Entry, EL2→EL1, SP, BSS clear | Included via `global_asm!` in main.rs |
| `arch/aarch64/mmu.rs` | Identity-mapped page tables, W^X | L1→L2→L3, 4KB pages for kernel, 2MB blocks for RAM, device at indices 64–72. User stacks: `AP_RW_EL0`, code: `SHARED_CODE_PAGE`, kernel data: `AP_RW_EL1` (EL0 no access) |
| `arch/aarch64/exception.rs` | Vector table, TrapFrame, ESR dispatch | **TrapFrame is ABI-locked at 288 bytes**. Lower-EL vectors use `SAVE_CONTEXT_LOWER` (loads SP from `__stack_end`, stashes x9 in `TPIDR_EL1`). **Fault isolation:** lower-EL faults → `fault_current_task()` + schedule away; same-EL faults → halt (kernel bug). |
| `arch/aarch64/gic.rs` | GICv2 driver | GICD `0x0800_0000`, GICC `0x0801_0000` |
| `platform/qemu_virt.rs` | Platform constants | GICD_BASE, GICC_BASE, UART0_BASE, RAM_BASE, KERNEL_BASE, TIMER_INTID, TICK_MS, TIMER_FREQ_HZ |
| `kernel/sched.rs` | Priority scheduler, 3 static TCBs | 8-level priority, time budget (ticks_used/budget), epoch reset, SPSR = `0x000` (EL0t). Context switch = copy TrapFrame in/out of `TCBS[]`. **Fault isolation:** `TaskState::Faulted` + auto-restart after 100 ticks (1s). **Watchdog:** heartbeat monitoring per task. |
| `kernel/ipc.rs` | Synchronous endpoint IPC | 4 endpoints, blocking send/recv, message in x[0..3]. `cleanup_task()` removes faulted task from endpoint slots. |
| `kernel/cap.rs` | Capability access control | Per-task u64 bitmask, 18 bits defined (0–17). `cap_for_syscall()` maps syscall # → required bit. |
| `kernel/timer.rs` | Tick counter + handler | `TICK_COUNT` global, `tick_handler()` decrements budgets, epoch check, watchdog scan. |
| `kernel/grant.rs` | Shared memory grants | 4 grant slots, owner/peer model, revocable. Calls `arch::current::mmu` for page mapping. |
| `kernel/irq.rs` | IRQ routing | Bind GIC INTID → task + notification bit. Route sets pending bits + unblocks. |
| `kernel/elf.rs` | ELF64 parser + loader | `parse_elf64(&[u8])` → `ElfInfo` (entry + ≤4 PT_LOAD segments). `load_elf_segments()` copies to memory. W^X enforced. No heap. |
| `main.rs` | UART, syscall wrappers, task entries | EL0 tasks use `user_print()` → `SYS_WRITE` syscall (#4) for UART output. `uart_print` is kernel-only. ELF binary embedded via `include_bytes!`. |
| `uart.rs` | PL011 UART (dual cfg) | On AArch64: write_volatile to 0x0900_0000. On host: no-op stub. |

## Build & Run

```
cargo build --release -Zjson-target-spec
```
QEMU (use `Start-Process` with redirect for automated test — raw invocation blocks):
```
qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel target/aarch64-aegis/release/aegis_os
```

## Critical Constraints

- **No heap.** All allocation is static (`static mut` arrays, linker sections). No `alloc` crate.
- **No FP/SIMD.** `CPACR_EL1.FPEN = 0`; any float instruction traps. Avoid `f32`/`f64` and libs that emit them.
- **TrapFrame is ABI-fixed.** 288 bytes, offsets shared between `arch/aarch64/exception.rs` Rust struct and `SAVE_CONTEXT`/`RESTORE_CONTEXT` asm macros. Never reorder fields.
- **Linker script matters.** Sections are 4KB-aligned for W^X page permissions. Adding a section requires updating both `linker.ld` and `arch/aarch64/mmu.rs`.
- **UART at `0x0900_0000`** maps to L2 index 72 (`0x0900_0000 / 0x20_0000`), not 4. Device memory indices in `mmu.rs` are 64..=72.
- **Syscall ABI:** `x7` = syscall number, `x6` = endpoint ID, `x0–x3` = message payload. Dispatched via SVC in `arch/aarch64/exception.rs` `handle_svc`. Syscalls: 0=YIELD, 1=SEND, 2=RECV, 3=CALL, 4=WRITE, 5=NOTIFY, 6=WAIT_NOTIFY, 7=GRANT_CREATE, 8=GRANT_REVOKE, 9=IRQ_BIND, 10=IRQ_ACK, 11=DEVICE_MAP, 12=HEARTBEAT.
- **Arch/kernel boundary.** `kernel/` modules call arch functions via `crate::arch::current::*` or use `#[cfg(target_arch = "aarch64")]` guards at call sites. On host (x86_64), arch modules are not compiled — only `kernel/`, `platform/`, stubs are available.

## Conventions

- **Incremental phases.** Work is done in phases (A→B→C→…→L) with a UART checkpoint per phase. Each phase must boot on QEMU before moving on.
- **Plans go in `docs/plan/`** as `plan-{name}_{yyyy-MM-dd_hh-mm}.md`, written in Vietnamese.
- **Blog posts go in `docs/blog/`**, written in Vietnamese for 5th-graders (see `StoryTeller.agent.md`).
- **Section separators** use `// ─── Section Name ───…` style comments.
- **MMIO access** always via `ptr::write_volatile` / `ptr::read_volatile`.
- **Inline asm** uses named operands and `options(nomem, nostack)` where applicable.
- **Kernel code runs at EL1. Tasks run at EL0** (user mode). Tasks cannot access kernel data, device MMIO, or system registers. All task I/O goes through syscalls. Shared identity-mapped address space with AP-bit isolation.
- **Module paths.** Backward-compatible re-exports exist at crate root (`crate::ipc`, `crate::cap`, `crate::sched`, etc.) for convenience. Canonical paths are `crate::kernel::ipc`, `crate::arch::current::gic`, etc.

## Memory Map (QEMU virt)

| Address | What |
|---|---|
| `0x0800_0000` | GIC Distributor (GICD) |
| `0x0801_0000` | GIC CPU Interface (GICC) |
| `0x0900_0000` | UART0 PL011 |
| `0x4008_0000` | Kernel load address (`_start`) |
| Linker-placed | `.page_tables` (16KB) → `.task_stacks` (3×4KB, kernel SP) → `.user_stacks` (3×4KB, user SP) → guard page (4KB) → stack (16KB) |

## Test Infrastructure

- **189 host unit tests** — `cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1`
- **25 QEMU boot checkpoints** — `powershell -ExecutionPolicy Bypass -File tests\qemu_boot_test.ps1`
- Tests cover: TrapFrame, MMU descriptors, scheduler (priority+budget+watchdog), IPC, capabilities, notifications, grants, IRQ routing, address spaces, ELF parser/loader, device map, arch/kernel separation
