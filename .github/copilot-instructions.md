# AegisOS — Copilot Instructions

## What is this?

A bare-metal AArch64 microkernel targeting safety-critical systems (rockets, medical, autonomous vehicles). Runs on QEMU `virt` machine with Cortex-A53. Written in `#![no_std]` Rust + AArch64 assembly, zero heap, zero external dependencies.

## Architecture

Boot flow: `arch/aarch64/boot.s` (_start) → EL2→EL1 drop → BSS clear → `mmu::init()` → `kernel_main()` → exception/GIC/timer/scheduler init → multi-ELF load (hello/sensor/logger → tasks 2–4) → `sched::bootstrap()` ereting into uart_driver at **EL0**.

### Module Structure (Phase O)

```
src/
├── arch/                    # Architecture-specific code
│   ├── mod.rs               # cfg(aarch64) pub use aarch64 as current
│   └── aarch64/
│       ├── boot.s           # Entry, EL2→EL1, SP, BSS clear, CPACR_EL1.FPEN=0b01
│       ├── exception.rs     # Vector table, TrapFrame (288B), SVC dispatch (14 syscalls), fault handlers
│       ├── mmu.rs           # Page tables, identity map, W^X, TLB, map/unmap
│       └── gic.rs           # GICv2 driver (GICD + GICC)
│
├── kernel/                  # Portable kernel logic
│   ├── sched.rs             # Priority scheduler, 8 TCBs, budget, epoch, watchdog, TaskState::Exited, sys_exit()
│   ├── ipc.rs               # Synchronous endpoint IPC, blocking send/recv, pure functions for Kani
│   ├── cap.rs               # Capability access control (u64 bitmask, 19 bits: 0–18)
│   ├── timer.rs             # Tick counter + tick handler logic
│   ├── grant.rs             # Shared memory grants (owner/peer)
│   ├── irq.rs               # IRQ binding + routing → notification
│   └── elf.rs               # ELF64 parser + loader + load_elf_to_task() (no heap, no FP)
│
├── platform/
│   └── qemu_virt.rs         # MMIO addresses, memory map, ELF load constants
│
├── main.rs                  # kernel_main(), 14 syscall wrappers, multi-ELF loading, task entries
├── lib.rs                   # Crate root — module tree + re-exports
├── exception.rs             # Host-only stub (x86_64 tests)
├── mmu.rs                   # Host-only stub (x86_64 tests)
└── uart.rs                  # PL011 UART (dual cfg: real HW + host stub)

user/                        # Separate Cargo workspace (aarch64-user.json target)
├── Cargo.toml               # workspace = ["libsyscall", "hello", "sensor", "logger"]
├── aarch64-user.json        # Shared custom target spec for all user crates
├── libsyscall/              # Shared syscall library (14 wrappers, single source of truth)
├── hello/                   # EL0 task → slot 0 (task 2), WRITE + YIELD
├── sensor/                  # EL0 task → slot 1 (task 3), SEND + YIELD + HEARTBEAT
└── logger/                  # EL0 task → slot 2 (task 4), RECV + WRITE + YIELD
```

| Module | Role | Key details |
|---|---|---|
| `arch/aarch64/boot.s` | Entry, EL2→EL1, SP, BSS clear | Included via `global_asm!` in main.rs. CPACR_EL1.FPEN=0b01 (FP at EL1, trap at EL0). |
| `arch/aarch64/mmu.rs` | Identity-mapped page tables, W^X | L1→L2→L3, 4KB pages for kernel, 2MB blocks for RAM, device at indices 64–72. User stacks: `AP_RW_EL0`, code: `SHARED_CODE_PAGE`, kernel data: `AP_RW_EL1` (EL0 no access). 6 ELF load regions mapped. |
| `arch/aarch64/exception.rs` | Vector table, TrapFrame, ESR dispatch | **TrapFrame is ABI-locked at 288 bytes**. Lower-EL vectors use `SAVE_CONTEXT_LOWER` (loads SP from `__stack_end`, stashes x9 in `TPIDR_EL1`). **Fault isolation:** lower-EL faults → `fault_current_task()` + schedule away; same-EL faults → halt (kernel bug). Dispatches 14 syscalls (0–13). |
| `arch/aarch64/gic.rs` | GICv2 driver | GICD `0x0800_0000`, GICC `0x0801_0000` |
| `platform/qemu_virt.rs` | Platform constants | GICD_BASE, GICC_BASE, UART0_BASE, RAM_BASE, KERNEL_BASE, TIMER_INTID, TICK_MS, TIMER_FREQ_HZ, ELF_LOAD_BASE, ELF_LOAD_SIZE_PER_TASK, MAX_ELF_TASKS, `elf_load_addr()` |
| `kernel/sched.rs` | Priority scheduler, 8 static TCBs | 8-level priority, time budget (ticks_used/budget), epoch reset, SPSR = `0x000` (EL0t). Context switch = copy TrapFrame in/out of `TCBS[]`. **6 states:** Inactive, Ready, Running, Blocked, Faulted, **Exited**. Faulted → auto-restart after 100 ticks. **Exited → NO auto-restart** (graceful exit). `cleanup_task_resources()` shared by fault + exit paths. **Watchdog:** heartbeat monitoring per task. |
| `kernel/ipc.rs` | Synchronous endpoint IPC | 4 endpoints, blocking send/recv, message in x[0..3]. `cleanup_task()` removes task from endpoint slots. Pure functions (`copy_message_pure`, `cleanup_pure`) extracted for Kani verification. |
| `kernel/cap.rs` | Capability access control | Per-task u64 bitmask, **19 bits defined (0–18)**. `CAP_EXIT = 1 << 18`. `cap_for_syscall()` maps syscall # → required bit. |
| `kernel/timer.rs` | Tick counter + handler | `TICK_COUNT` global, `tick_handler()` decrements budgets, epoch check, watchdog scan. Skips Exited tasks. |
| `kernel/grant.rs` | Shared memory grants | 4 grant slots, owner/peer model, revocable. Calls `arch::current::mmu` for page mapping. |
| `kernel/irq.rs` | IRQ routing | Bind GIC INTID → task + notification bit. Route sets pending bits + unblocks. |
| `kernel/elf.rs` | ELF64 parser + loader | `parse_elf64(&[u8])` → `ElfInfo` (entry + ≤4 PT_LOAD segments). `load_elf_segments()` copies to memory. `load_elf_to_task(task_id, elf_data)` — reusable loader for multi-binary. W^X enforced. No heap. |
| `main.rs` | UART, syscall wrappers, task entries | 3 ELF binaries embedded via `include_bytes!` (hello/sensor/logger → tasks 2/3/4). Task 7 = IDLE (wfi loop, no ELF). `const_assert!` checks binary size ≤ 16 KiB per slot. |
| `uart.rs` | PL011 UART (dual cfg) | On AArch64: write_volatile to 0x0900_0000. On host: no-op stub. |
| `user/libsyscall` | Shared syscall library | 14 syscall wrappers (SYS_YIELD..SYS_EXIT). Single source of truth — user crates depend on this. |

## Build & Run

```bash
# 1. Build user crates (separate workspace)
cd user && cargo build --release -Zjson-target-spec --target aarch64-user.json

# 2. Build kernel (embeds user binaries via include_bytes!)
cargo build --release -Zjson-target-spec

# 3. Run on QEMU
qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel target/aarch64-aegis/release/aegis_os
```

Convenience scripts: `scripts/build-all.sh` (Linux) / `scripts/build-all.ps1` (Windows).

QEMU (use `Start-Process` with redirect for automated test — raw invocation blocks on Windows).

## Critical Constraints

- **No heap.** All allocation is static (`static mut` arrays, linker sections). No `alloc` crate.
- **No FP/SIMD at EL0.** `CPACR_EL1.FPEN = 0b01`; FP allowed at EL1 (compiler may emit NEON for memcpy), trapped at EL0. User tasks must not use `f32`/`f64`.
- **TrapFrame is ABI-fixed.** 288 bytes, offsets shared between `arch/aarch64/exception.rs` Rust struct and `SAVE_CONTEXT`/`RESTORE_CONTEXT` asm macros. Never reorder fields.
- **Linker script matters.** Sections are 4KB-aligned for W^X page permissions. Adding a section requires updating both `linker.ld` and `arch/aarch64/mmu.rs`.
- **UART at `0x0900_0000`** maps to L2 index 72 (`0x0900_0000 / 0x20_0000`), not 4. Device memory indices in `mmu.rs` are 64..=72.
- **Syscall ABI:** `x7` = syscall number, `x6` = endpoint ID, `x0–x3` = message payload. Dispatched via SVC in `arch/aarch64/exception.rs` `handle_svc`. Syscalls: 0=YIELD, 1=SEND, 2=RECV, 3=CALL, 4=WRITE, 5=NOTIFY, 6=WAIT_NOTIFY, 7=GRANT_CREATE, 8=GRANT_REVOKE, 9=IRQ_BIND, 10=IRQ_ACK, 11=DEVICE_MAP, 12=HEARTBEAT, **13=EXIT**.
- **Arch/kernel boundary.** `kernel/` modules call arch functions via `crate::arch::current::*` or use `#[cfg(target_arch = "aarch64")]` guards at call sites. On host (x86_64), arch modules are not compiled — only `kernel/`, `platform/`, stubs are available.
- **User binary ≤ 16 KiB.** Each ELF load slot = 4 pages. Enforced by `const_assert!` at compile time. Use `opt-level="s"` + LTO.
- **Two workspaces.** Kernel workspace (root `Cargo.toml`, target `aarch64-aegis.json`) and user workspace (`user/Cargo.toml`, target `aarch64-user.json`). Build user first, then kernel.

## Conventions

- **Incremental phases.** Work is done in phases (A→B→C→…→O) with a UART checkpoint per phase. Each phase must boot on QEMU before moving on.
- **Plans go in `docs/plan/`** as `plan-{name}_{yyyy-MM-dd_hh-mm}.md`, written in Vietnamese.
- **Blog posts go in `docs/blog/`**, written in Vietnamese for 5th-graders (see `StoryTeller.agent.md`).
- **Section separators** use `// ─── Section Name ───…` style comments.
- **MMIO access** always via `ptr::write_volatile` / `ptr::read_volatile`.
- **Inline asm** uses named operands and `options(nomem, nostack)` where applicable.
- **Kernel code runs at EL1. Tasks run at EL0** (user mode). Tasks cannot access kernel data, device MMIO, or system registers. All task I/O goes through syscalls. Shared identity-mapped address space with AP-bit isolation.
- **Module paths.** Backward-compatible re-exports exist at crate root (`crate::ipc`, `crate::cap`, `crate::sched`, etc.) for convenience. Canonical paths are `crate::kernel::ipc`, `crate::arch::current::gic`, etc.
- **Task lifecycle.** Tasks 0–1 are kernel-defined (uart_driver, client). Tasks 2–4 are ELF-loaded (hello, sensor, logger). Tasks 5–6 reserved. Task 7 = IDLE. Exited tasks are NOT auto-restarted; Faulted tasks auto-restart after 100 ticks.

## Memory Map (QEMU virt)

| Address | What |
|---|---|
| `0x0800_0000` | GIC Distributor (GICD) |
| `0x0801_0000` | GIC CPU Interface (GICC) |
| `0x0900_0000` | UART0 PL011 |
| `0x4008_0000` | Kernel load address (`_start`) |
| `0x4010_0000` | ELF load region (6 slots × 16 KiB = 96 KiB) |
| `0x4010_0000` | Slot 0 → task 2 (hello) |
| `0x4010_4000` | Slot 1 → task 3 (sensor) |
| `0x4010_8000` | Slot 2 → task 4 (logger) |
| `0x4010_C000`–`0x4011_7FFF` | Slots 3–5 → reserved |
| Linker-placed | `.page_tables` (16KB) → `.task_stacks` (8×4KB) → `.user_stacks` (8×4KB) → guard page (4KB) → stack (16KB) |

## Test Infrastructure

- **241 host unit tests** — `cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1`
- **32 QEMU boot checkpoints** — `powershell -ExecutionPolicy Bypass -File tests\qemu_boot_test.ps1`
- **10 Kani formal proofs** — `docker exec -w /workspaces/aegis aegis-dev cargo kani --tests` (requires `aegis-dev` container)
- Tests cover: TrapFrame, MMU descriptors, scheduler (priority+budget+watchdog+Exited), IPC (+ Kani proofs for queue overflow, message integrity, cleanup completeness), capabilities (19 bits), notifications, grants, IRQ routing, address spaces, ELF parser/loader, multi-ELF loading, device map, SYS_EXIT lifecycle, arch/kernel separation

### Kani Proofs (10 harnesses)

| Harness | Module | Property |
|---|---|---|
| `cap_check_bitwise_correctness` | `kernel/cap.rs` | Capability bitmask logic correct |
| `cap_for_syscall_no_panic_and_bounded` | `kernel/cap.rs` | No panic for syscall 0–13, result bounded |
| `schedule_idle_guarantee` | `kernel/sched.rs` | IDLE task always selected when no Ready tasks |
| `restart_task_state_machine` | `kernel/sched.rs` | Faulted→Ready, Exited stays Exited |
| `ipc_queue_no_overflow` | `kernel/ipc.rs` | push full→false, pop empty→None, count∈[0,4] |
| `ipc_message_integrity` | `kernel/ipc.rs` | Payload preserved across copy_message_pure |
| `ipc_cleanup_completeness` | `kernel/ipc.rs` | cleanup removes task from ALL endpoints |
| `pt_index_in_bounds` | `mmu.rs` | Page table index within valid range |
| `pt_index_no_task_aliasing` | `mmu.rs` | No two tasks share page table indices |
| `elf_load_addr_no_overlap` | `platform/qemu_virt.rs` | No slot overlap, all within bounds |
