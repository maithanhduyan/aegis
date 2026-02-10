# AegisOS — Copilot Instructions

## What is this?

A bare-metal AArch64 microkernel targeting safety-critical systems (rockets, medical, autonomous vehicles). Runs on QEMU `virt` machine with Cortex-A53. Written in `#![no_std]` Rust + AArch64 assembly, zero heap, zero external dependencies.

## Architecture

Boot flow: `boot.s` (_start) → EL2→EL1 drop → BSS clear → `mmu::init()` → `kernel_main()` → exception/GIC/timer/scheduler init → `sched::bootstrap()` ereting into task_a at **EL0**.

| Module | Role | Key details |
|---|---|---|
| `boot.s` | Entry, EL2→EL1, SP, BSS clear | Included via `global_asm!` in main.rs |
| `mmu.rs` | Identity-mapped page tables, W^X | L1→L2→L3, 4KB pages for kernel, 2MB blocks for RAM, device at indices 64–72. User stacks: `AP_RW_EL0`, code: `SHARED_CODE_PAGE`, kernel data: `AP_RW_EL1` (EL0 no access) |
| `exception.rs` | Vector table, TrapFrame, ESR dispatch | **TrapFrame is ABI-locked at 288 bytes**. Lower-EL vectors use `SAVE_CONTEXT_LOWER` (loads SP from `__stack_end`, stashes x9 in `TPIDR_EL1`). **Fault isolation:** lower-EL faults → `fault_current_task()` + schedule away; same-EL faults → halt (kernel bug). |
| `gic.rs` | GICv2 driver | GICD `0x0800_0000`, GICC `0x0801_0000` |
| `timer.rs` | ARM Generic Timer (CNTP_EL0) | INTID 30, 10ms tick, 62.5 MHz on QEMU |
| `sched.rs` | Round-robin scheduler, 3 static TCBs | SPSR = `0x000` (EL0t). Context switch = copy TrapFrame in/out of `TCBS[]`. Shared kernel boot stack for all handlers. **Fault isolation:** `TaskState::Faulted` + auto-restart after 100 ticks (1s). TCB stores `entry_point`/`user_stack_top` for restart. |
| `ipc.rs` | Synchronous endpoint IPC | Blocking send/recv, message in x[0..3]. `cleanup_task()` removes faulted task from endpoint slots. |
| `main.rs` | UART, syscall wrappers, task entries | EL0 tasks use `user_print()` → `SYS_WRITE` syscall (#4) for UART output. `uart_print` is kernel-only. |

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
- **TrapFrame is ABI-fixed.** 288 bytes, offsets shared between `exception.rs` Rust struct and `SAVE_CONTEXT`/`RESTORE_CONTEXT` asm macros. Never reorder fields.
- **Linker script matters.** Sections are 4KB-aligned for W^X page permissions. Adding a section (e.g., `.task_stacks`) requires updating both `linker.ld` and `mmu.rs`.
- **UART at `0x0900_0000`** maps to L2 index 72 (`0x0900_0000 / 0x20_0000`), not 4. Device memory indices in `mmu.rs` are 64..=72.
- **Syscall ABI:** `x7` = syscall number, `x6` = endpoint ID, `x0–x3` = message payload. Dispatched via SVC in `exception.rs` `handle_svc`. Syscalls: 0=YIELD, 1=SEND, 2=RECV, 3=CALL, 4=WRITE.

## Conventions

- **Incremental phases.** Work is done in sub-phases (A→B→C1→C2→…) with a UART checkpoint per phase. Each phase must boot on QEMU before moving on.
- **Plans go in `docs/plan/`** as `plan-{name}_{yyyy-MM-dd_hh-mm}.md`, written in Vietnamese.
- **Blog posts go in `docs/blog/`**, written in Vietnamese for 5th-graders (see `StoryTeller.agent.md`).
- **Section separators** use `// ─── Section Name ───…` style comments.
- **MMIO access** always via `ptr::write_volatile` / `ptr::read_volatile`.
- **Inline asm** uses named operands and `options(nomem, nostack)` where applicable.
- **Kernel code runs at EL1. Tasks run at EL0** (user mode). Tasks cannot access kernel data, device MMIO, or system registers. All task I/O goes through syscalls. Shared identity-mapped address space with AP-bit isolation.

## Memory Map (QEMU virt)

| Address | What |
|---|---|
| `0x0800_0000` | GIC Distributor (GICD) |
| `0x0801_0000` | GIC CPU Interface (GICC) |
| `0x0900_0000` | UART0 PL011 |
| `0x4008_0000` | Kernel load address (`_start`) |
| Linker-placed | `.page_tables` (16KB) → `.task_stacks` (3×4KB, kernel SP) → `.user_stacks` (3×4KB, user SP) → guard page (4KB) → stack (16KB) |
