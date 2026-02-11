# Cấu trúc Dự án như sau:

```
./
├── .cargo
│   └── config.toml
├── Cargo.toml
├── aarch64-aegis.json
├── linker.ld
├── rust-toolchain.toml
└── src
    ├── boot.s
    ├── cap.rs
    ├── exception.rs
    ├── gic.rs
    ├── grant.rs
    ├── ipc.rs
    ├── irq.rs
    ├── lib.rs
    ├── main.rs
    ├── mmu.rs
    ├── sched.rs
    ├── timer.rs
    └── uart.rs
```

# Danh sách chi tiết các file:

## File ./aarch64-aegis.json:
```json
{
    "arch": "aarch64",
    "crt-objects-fallback": "false",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32",
    "disable-redzone": true,
    "features": "+v8a,+strict-align,+neon,+fp-armv8",
    "linker": "rust-lld",
    "linker-flavor": "gnu-lld",
    "llvm-target": "aarch64-unknown-none",
    "max-atomic-width": 128,
    "panic-strategy": "abort",
    "relocation-model": "static",
    "target-pointer-width": 64
}

```

## File ./Cargo.toml:
```
[package]
name = "aegis_os"
version = "0.1.0"
edition = "2021"
authors = ["AegisOS Team"]
description = "Safety-critical AArch64 microkernel"

[profile.release]
panic = "abort"
opt-level = "s"
lto = true

```

## File ./linker.ld:
```
ENTRY(_start)

SECTIONS
{
    . = 0x40080000;

    /* === Code (RX) === */
    __text_start = .;
    .text : {
        KEEP(*(.text._start))
        *(.text*)
    }
    __text_end = .;

    /* === Read-Only Data (RO-NX) === */
    . = ALIGN(4096);
    __rodata_start = .;
    .rodata : { *(.rodata*) }
    __rodata_end = .;

    /* === Mutable Data (RW-NX) === */
    . = ALIGN(4096);
    __data_start = .;
    .data : { *(.data*) }
    __data_end = .;

    /* === BSS (RW-NX, zeroed) === */
    .bss : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    }

    /* === Page Tables (16 × 4096 = 64 KiB, 4KB-aligned) === */
    /* Layout: [0..2]=L2_device per task, [3..5]=L1 per task, [6..8]=L2_ram per task,
       [9..11]=L3 per task, [12]=L2_device kernel, [13]=L1 kernel, [14]=L2_ram kernel, [15]=L3 kernel */
    . = ALIGN(4096);
    __page_tables_start = .;
    .page_tables (NOLOAD) : {
        . += 16 * 4096;
    }
    __page_tables_end = .;

    /* === Kernel Stacks per task (3 × 4KB = 12 KiB, 4KB-aligned) === */
    /* Used as SP_EL1 when handling exceptions from EL0 tasks */
    . = ALIGN(4096);
    __task_stacks_start = .;
    .task_stacks (NOLOAD) : {
        . += 3 * 4096;
    }
    __task_stacks_end = .;

    /* === User Stacks per task (3 × 4KB = 12 KiB, 4KB-aligned) === */
    /* Used as SP_EL0 when tasks run in user mode (EL0) */
    . = ALIGN(4096);
    __user_stacks_start = .;
    .user_stacks (NOLOAD) : {
        . += 3 * 4096;
    }
    __user_stacks_end = .;

    /* === Grant Pages (2 × 4KB = 8 KiB, 4KB-aligned) === */
    /* Shared memory regions for inter-task data sharing (Phase J) */
    . = ALIGN(4096);
    __grant_pages_start = .;
    .grant_pages (NOLOAD) : {
        . += 2 * 4096;
    }
    __grant_pages_end = .;

    /* === Stack Guard Page (4KB invalid — catches stack overflow) === */
    . = ALIGN(4096);
    __stack_guard = .;
    . += 4096;

    /* === Stack (16 KB) === */
    . = ALIGN(16);
    __stack_start = .;
    . += 0x4000;
    __stack_end = .;

    /* === End of kernel image === */
    . = ALIGN(4096);
    __kernel_end = .;

    /DISCARD/ : { *(.comment*) *(.eh_frame*) *(.gcc_except_table*) }
}

```

## File ./rust-toolchain.toml:
```
[toolchain]
channel = "nightly"
components = ["rust-src"]
targets = ["aarch64-unknown-none"]

```

## File ./src\boot.s:
```asm
.section .text._start
.global _start

_start:
    /* Chỉ core 0 chạy, các core khác park */
    mrs x0, mpidr_el1
    and x0, x0, #3
    cbz x0, 1f

0:
    wfe
    b 0b

1:
    /* Setup stack pointer */
    /* Cài đặt con trỏ ngăn xếp */
    ldr x0, =__stack_end
    mov sp, x0

    /* Check EL — QEMU virt may start at EL2 or EL1 */
    /* Kiểm tra EL — QEMU virt có thể khởi động ở EL2 hoặc EL1 */
    mrs x0, CurrentEL
    lsr x0, x0, #2
    cmp x0, #2
    b.ne at_el1

    /* === Drop from EL2 to EL1 === */
    /* Chuyển từ EL2 xuống EL1 */
    mrs x0, hcr_el2
    orr x0, x0, #(1 << 31)   /* HCR_EL2.RW = 1 (EL1 is AArch64) */
    msr hcr_el2, x0

    mov x0, #0x33FF
    msr cptr_el2, x0
    msr hstr_el2, xzr

    /* SCTLR_EL1 reset value */
    /* Giá trị khởi tạo SCTLR_EL1 */
    mov x0, #0x0800
    movk x0, #0x30D0, lsl #16
    msr sctlr_el1, x0

    /* Enable EL1 physical timer access from EL2 */
    /* Cho phép EL1 truy cập bộ đếm thời gian vật lý từ EL2 */
    mrs x0, CNTHCTL_EL2
    orr x0, x0, #3        /* EL1PCTEN + EL1PCEN */
    msr CNTHCTL_EL2, x0
    msr CNTVOFF_EL2, xzr  /* Zero virtual offset */

    /* Return to EL1h */
    /* Quay lại EL1h */
    mov x0, #0x3C5
    msr spsr_el2, x0
    adr x0, at_el1
    msr elr_el2, x0
    eret

at_el1:
    /* Re-setup SP (SP_EL1 after eret) */
    /* Cài đặt lại con trỏ ngăn xếp (SP_EL1 sau eret) */
    ldr x0, =__stack_end
    mov sp, x0

    /* Clear BSS + page tables */
    /* Xóa BSS + bảng trang */
    ldr x0, =__bss_start
    ldr x1, =__page_tables_end

2:
    cmp x0, x1
    b.eq 3f
    str xzr, [x0], #8
    b 2b

3:
    /* === MMU Setup === */

    /* Build page tables in Rust */
    /* Khởi tạo bảng trang trong Rust */
    bl  mmu_init

    /* Invalidate all TLB entries */
    /* Vô hiệu hóa tất cả các mục TLB */
    tlbi vmalle1
    dsb  ish
    isb

    /* MAIR_EL1: idx0=Device-nGnRnE(0x00), idx1=Normal-NC(0x44),
                  idx2=Normal-WB(0xFF), idx3=Device-nGnRE(0x04) */
    ldr x0, =0x04FF4400
    msr mair_el1, x0

    /* TCR_EL1: 39-bit VA, 4KB granule, TTBR0 only, 48-bit PA */
    /* TCR_EL1: 39-bit địa chỉ ảo, kích thước trang 4KB, chỉ dùng TTBR0, 48-bit địa chỉ vật lý */
    ldr x0, =0x5B5993519
    msr tcr_el1, x0

    /* TTBR0_EL1 = kernel boot L1 (page 13 in .page_tables) */
    /* TTBR0_EL1 = bảng trang cấp 1 khởi động kernel (trang 13 trong .page_tables) */
    ldr x0, =__page_tables_start
    add x0, x0, #(13 * 4096)
    msr ttbr0_el1, x0

    isb

    /* Enable MMU: set M + C + SA + I + WXN in SCTLR_EL1 */
    /* Bật MMU: đặt các bit M + C + SA + I + WXN trong SCTLR_EL1 */
    mrs x0, sctlr_el1
    ldr x1, =0x0008100D
    orr x0, x0, x1
    msr sctlr_el1, x0
    isb

    /* MMU is now active — jump to Rust */
    /* MMU đã được kích hoạt — nhảy vào Rust */
    bl  kernel_main

4:
    wfe
    b 4b

```

## File ./src\cap.rs:
```rust
/// AegisOS Capability Module — Flat bitmask access control
///
/// Each task holds a `CapBits` (u64) bitmask in its TCB.
/// Before dispatching a syscall, the kernel checks that the task's
/// capability mask includes the required bit(s). Unauthorized
/// syscalls → fault (software defect in safety-critical context).
///
/// Design: flat u64 bitmask (not seL4 CSpace) — appropriate for
/// a static 3-task microkernel with no heap and no dynamic cap transfer.

// ─── Types ─────────────────────────────────────────────────────────

/// Capability bitmask — each bit grants one permission.
pub type CapBits = u64;

// ─── Capability bit constants ──────────────────────────────────────

/// Permission to send IPC on endpoint 0
pub const CAP_IPC_SEND_EP0: CapBits = 1 << 0;
/// Permission to receive IPC on endpoint 0
pub const CAP_IPC_RECV_EP0: CapBits = 1 << 1;
/// Permission to send IPC on endpoint 1
pub const CAP_IPC_SEND_EP1: CapBits = 1 << 2;
/// Permission to receive IPC on endpoint 1
pub const CAP_IPC_RECV_EP1: CapBits = 1 << 3;
/// Permission to write to UART (SYS_WRITE)
pub const CAP_WRITE: CapBits        = 1 << 4;
/// Permission to yield CPU (SYS_YIELD)
pub const CAP_YIELD: CapBits        = 1 << 5;
/// Permission to send notifications (SYS_NOTIFY)
pub const CAP_NOTIFY: CapBits       = 1 << 6;
/// Permission to wait for notifications (SYS_WAIT_NOTIFY)
pub const CAP_WAIT_NOTIFY: CapBits  = 1 << 7;
/// Permission to send IPC on endpoint 2
pub const CAP_IPC_SEND_EP2: CapBits = 1 << 8;
/// Permission to receive IPC on endpoint 2
pub const CAP_IPC_RECV_EP2: CapBits = 1 << 9;
/// Permission to send IPC on endpoint 3
pub const CAP_IPC_SEND_EP3: CapBits = 1 << 10;
/// Permission to receive IPC on endpoint 3
pub const CAP_IPC_RECV_EP3: CapBits = 1 << 11;
/// Permission to create shared memory grants (SYS_GRANT_CREATE)
pub const CAP_GRANT_CREATE: CapBits = 1 << 12;
/// Permission to revoke shared memory grants (SYS_GRANT_REVOKE)
pub const CAP_GRANT_REVOKE: CapBits = 1 << 13;
/// Permission to bind an IRQ to a notification (SYS_IRQ_BIND)
pub const CAP_IRQ_BIND: CapBits = 1 << 14;
/// Permission to acknowledge an IRQ (SYS_IRQ_ACK)
pub const CAP_IRQ_ACK: CapBits = 1 << 15;
/// Permission to map a device's MMIO into user-space (SYS_DEVICE_MAP)
pub const CAP_DEVICE_MAP: CapBits = 1 << 16;

// ─── Convenience combos ────────────────────────────────────────────

/// All capabilities (for privileged tasks)
pub const CAP_ALL: CapBits = CAP_IPC_SEND_EP0
    | CAP_IPC_RECV_EP0
    | CAP_IPC_SEND_EP1
    | CAP_IPC_RECV_EP1
    | CAP_WRITE
    | CAP_YIELD
    | CAP_NOTIFY
    | CAP_WAIT_NOTIFY
    | CAP_IPC_SEND_EP2
    | CAP_IPC_RECV_EP2
    | CAP_IPC_SEND_EP3
    | CAP_IPC_RECV_EP3
    | CAP_GRANT_CREATE
    | CAP_GRANT_REVOKE
    | CAP_IRQ_BIND
    | CAP_IRQ_ACK
    | CAP_DEVICE_MAP;

/// No capabilities
pub const CAP_NONE: CapBits = 0;

// ─── Core functions ────────────────────────────────────────────────

/// Check whether `caps` includes all bits in `required`.
/// Returns `true` if the task has the required capability.
///
/// O(1), pure, no side effects — safe for use in hot path.
#[inline]
pub fn cap_check(caps: CapBits, required: CapBits) -> bool {
    (caps & required) == required
}

/// Map a syscall number + endpoint ID to the required capability bit(s).
///
/// Syscall ABI: x7 = syscall_nr, x6 = endpoint_id.
/// Returns 0 if the syscall/endpoint combo is unrecognized (caller
/// should treat as "no cap can grant this" → fault).
pub fn cap_for_syscall(syscall_nr: u64, ep_id: u64) -> CapBits {
    match syscall_nr {
        // SYS_YIELD = 0
        0 => CAP_YIELD,
        // SYS_SEND = 1
        1 => match ep_id {
            0 => CAP_IPC_SEND_EP0,
            1 => CAP_IPC_SEND_EP1,
            2 => CAP_IPC_SEND_EP2,
            3 => CAP_IPC_SEND_EP3,
            _ => 0, // invalid endpoint
        },
        // SYS_RECV = 2
        2 => match ep_id {
            0 => CAP_IPC_RECV_EP0,
            1 => CAP_IPC_RECV_EP1,
            2 => CAP_IPC_RECV_EP2,
            3 => CAP_IPC_RECV_EP3,
            _ => 0,
        },
        // SYS_CALL = 3: needs both send and recv on the endpoint
        3 => match ep_id {
            0 => CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0,
            1 => CAP_IPC_SEND_EP1 | CAP_IPC_RECV_EP1,
            2 => CAP_IPC_SEND_EP2 | CAP_IPC_RECV_EP2,
            3 => CAP_IPC_SEND_EP3 | CAP_IPC_RECV_EP3,
            _ => 0,
        },
        // SYS_WRITE = 4
        4 => CAP_WRITE,
        // SYS_NOTIFY = 5
        5 => CAP_NOTIFY,
        // SYS_WAIT_NOTIFY = 6
        6 => CAP_WAIT_NOTIFY,
        // SYS_GRANT_CREATE = 7
        7 => CAP_GRANT_CREATE,
        // SYS_GRANT_REVOKE = 8
        8 => CAP_GRANT_REVOKE,
        // SYS_IRQ_BIND = 9
        9 => CAP_IRQ_BIND,
        // SYS_IRQ_ACK = 10
        10 => CAP_IRQ_ACK,
        // SYS_DEVICE_MAP = 11
        11 => CAP_DEVICE_MAP,
        // Unknown syscall — no valid cap
        _ => 0,
    }
}

/// Return a human-readable name for a single capability bit.
/// Used for UART debug output when denying a syscall.
pub fn cap_name(cap: CapBits) -> &'static str {
    match cap {
        CAP_IPC_SEND_EP0 => "IPC_SEND_EP0",
        CAP_IPC_RECV_EP0 => "IPC_RECV_EP0",
        CAP_IPC_SEND_EP1 => "IPC_SEND_EP1",
        CAP_IPC_RECV_EP1 => "IPC_RECV_EP1",
        CAP_WRITE         => "WRITE",
        CAP_YIELD         => "YIELD",
        CAP_NOTIFY        => "NOTIFY",
        CAP_WAIT_NOTIFY   => "WAIT_NOTIFY",
        CAP_IPC_SEND_EP2  => "IPC_SEND_EP2",
        CAP_IPC_RECV_EP2  => "IPC_RECV_EP2",
        CAP_IPC_SEND_EP3  => "IPC_SEND_EP3",
        CAP_IPC_RECV_EP3  => "IPC_RECV_EP3",
        CAP_GRANT_CREATE  => "GRANT_CREATE",
        CAP_GRANT_REVOKE  => "GRANT_REVOKE",
        CAP_IRQ_BIND      => "IRQ_BIND",
        CAP_IRQ_ACK       => "IRQ_ACK",
        CAP_DEVICE_MAP    => "DEVICE_MAP",
        CAP_ALL           => "ALL",
        CAP_NONE          => "NONE",
        _                 => "UNKNOWN",
    }
}

```

## File ./src\exception.rs:
```rust
/// AegisOS Exception Handling — AArch64
///
/// Full context save/restore (288-byte TrapFrame), ESR_EL1 dispatch,
/// separate Sync/IRQ paths. TrapFrame layout is ABI-fixed for Phase C.

#[cfg(target_arch = "aarch64")]
use crate::uart_print;
#[cfg(target_arch = "aarch64")]
use crate::uart_print_hex;

// ─── TrapFrame: ABI-fixed layout, 288 bytes ────────────────────────

/// Saved CPU context on exception entry.
/// 36 × u64 = 288 bytes, 16-byte aligned.
/// This layout is shared between Rust and assembly — DO NOT reorder.
#[repr(C)]
pub struct TrapFrame {
    /// x0–x30 general-purpose registers (31 × 8 = 248 bytes)
    pub x: [u64; 31],      // offset   0..248
    /// Saved SP_EL0 (user stack pointer)
    pub sp_el0: u64,        // offset 248
    /// Saved ELR_EL1 (return address)
    pub elr_el1: u64,       // offset 256
    /// Saved SPSR_EL1 (saved processor state)
    pub spsr_el1: u64,      // offset 264
    /// Padding for 16-byte alignment
    pub _pad: [u64; 2],     // offset 272..288
}

/// TrapFrame size — must match assembly
#[allow(dead_code)]
pub const TRAPFRAME_SIZE: usize = 288;

// ─── Exception vector table + save/restore macros ──────────────────

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(r#"
.section .text

/* ═══════════════════════════════════════════════════════════════════
 * Save all registers into a TrapFrame on the current SP.
 * After this macro: x0 = pointer to TrapFrame (= sp).
 * ═══════════════════════════════════════════════════════════════════ */
.macro SAVE_CONTEXT
    /* Allocate TrapFrame on stack */
    sub sp, sp, #288

    /* Save x0–x30 in pairs */
    stp x0,  x1,  [sp, #0]
    stp x2,  x3,  [sp, #16]
    stp x4,  x5,  [sp, #32]
    stp x6,  x7,  [sp, #48]
    stp x8,  x9,  [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30,      [sp, #240]

    /* Save SP_EL0 */
    mrs x9, sp_el0
    str x9, [sp, #248]

    /* Save ELR_EL1 + SPSR_EL1 */
    mrs x9,  elr_el1
    mrs x10, spsr_el1
    stp x9,  x10, [sp, #256]

    /* x0 = pointer to TrapFrame for Rust handler */
    mov x0, sp
.endm

/* ═══════════════════════════════════════════════════════════════════
 * Restore all registers from a TrapFrame on the current SP, then eret.
 * Used for same-EL exceptions (SP stays the same).
 * ═══════════════════════════════════════════════════════════════════ */
.macro RESTORE_CONTEXT
    /* Restore ELR_EL1 + SPSR_EL1 */
    ldp x9,  x10, [sp, #256]
    msr elr_el1, x9
    msr spsr_el1, x10

    /* Restore SP_EL0 */
    ldr x9, [sp, #248]
    msr sp_el0, x9

    /* Restore x0–x30 */
    ldp x0,  x1,  [sp, #0]
    ldp x2,  x3,  [sp, #16]
    ldp x4,  x5,  [sp, #32]
    ldp x6,  x7,  [sp, #48]
    ldp x8,  x9,  [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30,      [sp, #240]

    /* Deallocate TrapFrame */
    add sp, sp, #288

    /* Return from exception — restores PC from ELR, PSTATE from SPSR */
    eret
.endm

/* ═══════════════════════════════════════════════════════════════════
 * Save context for lower-EL (EL0→EL1) exceptions.
 * Load SP from the kernel boot stack top before saving, so all
 * exception handling uses the shared 16KB kernel stack regardless
 * of which task was running. Single-core, no nesting.
 * ═══════════════════════════════════════════════════════════════════ */
.macro SAVE_CONTEXT_LOWER
    /* When exception fires from EL0, CPU sets SP = SP_EL1 (whatever
     * it was when we last eret'd). We must switch to the shared kernel
     * boot stack (__stack_end) for handler execution.
     *
     * Problem: we need to clobber a register to load __stack_end,
     * but we haven't saved anything yet. Solution: save x9 to
     * TPIDR_EL1 (kernel scratch sysreg), switch SP, then save
     * all regs including the real x9 from TPIDR_EL1. */
    msr tpidr_el1, x9          /* stash x9 in kernel scratch reg */
    ldr x9, =__stack_end
    mov sp, x9

    /* Allocate TrapFrame on the kernel stack */
    sub sp, sp, #288

    /* Save x0–x8 */
    stp x0,  x1,  [sp, #0]
    stp x2,  x3,  [sp, #16]
    stp x4,  x5,  [sp, #32]
    stp x6,  x7,  [sp, #48]
    str x8,        [sp, #64]

    /* Recover and save the REAL x9 from TPIDR_EL1 */
    mrs x9, tpidr_el1
    str x9, [sp, #72]

    /* Save x10–x30 */
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30,      [sp, #240]

    /* Save SP_EL0 (user stack pointer) */
    mrs x9, sp_el0
    str x9, [sp, #248]

    /* Save ELR_EL1 + SPSR_EL1 */
    mrs x9,  elr_el1
    mrs x10, spsr_el1
    stp x9,  x10, [sp, #256]

    /* x0 = pointer to TrapFrame for Rust handler */
    mov x0, sp
.endm
.macro RESTORE_CONTEXT_LOWER
    /* Restore ELR_EL1 + SPSR_EL1 */
    ldp x9,  x10, [sp, #256]
    msr elr_el1, x9
    msr spsr_el1, x10

    /* Restore SP_EL0 (user stack for the target task) */
    ldr x9, [sp, #248]
    msr sp_el0, x9

    /* Restore x0–x30 */
    ldp x0,  x1,  [sp, #0]
    ldp x2,  x3,  [sp, #16]
    ldp x4,  x5,  [sp, #32]
    ldp x6,  x7,  [sp, #48]
    ldp x8,  x9,  [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30,      [sp, #240]

    /* Deallocate TrapFrame — SP now at kernel stack top (of OLD task) */
    add sp, sp, #288

    /* Return from exception */
    eret
.endm

/* ═══════════════════════════════════════════════════════════════════
 * Exception Vector Table — 2048-byte aligned, 16 entries × 128 bytes
 * ═══════════════════════════════════════════════════════════════════ */
.balign 2048
.global __exception_vectors
__exception_vectors:

/* ─── Group 0: Current EL with SP_EL0 ─────────────────────────── */
.balign 0x80
    b   _exc_sync_cur_sp0       /* Synchronous */
.balign 0x80
    b   _exc_irq_cur_sp0        /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_serror_stub        /* SError */

/* ─── Group 1: Current EL with SP_ELx ─────────────────────────── */
.balign 0x80
    b   _exc_sync_cur_spx       /* Synchronous */
.balign 0x80
    b   _exc_irq_cur_spx        /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_serror_stub        /* SError */

/* ─── Group 2: Lower EL, AArch64 ──────────────────────────────── */
.balign 0x80
    b   _exc_sync_lower64       /* Synchronous */
.balign 0x80
    b   _exc_irq_lower64        /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_serror_stub        /* SError */

/* ─── Group 3: Lower EL, AArch32 ──────────────────────────────── */
.balign 0x80
    b   _exc_fiq_stub           /* Synchronous (AArch32 not used) */
.balign 0x80
    b   _exc_fiq_stub           /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_fiq_stub           /* SError */

/* ═══════════════════════════════════════════════════════════════════
 * Synchronous exception handlers — save context, call Rust, restore
 * ═══════════════════════════════════════════════════════════════════ */

_exc_sync_cur_sp0:
    SAVE_CONTEXT
    mov x1, #0          /* source = 0: current EL, SP_EL0 */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT

_exc_sync_cur_spx:
    SAVE_CONTEXT
    mov x1, #1          /* source = 1: current EL, SP_ELx */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT

_exc_sync_lower64:
    SAVE_CONTEXT_LOWER
    mov x1, #2          /* source = 2: lower EL, AArch64 */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT_LOWER

/* ═══════════════════════════════════════════════════════════════════
 * IRQ handlers — save context, call Rust, restore
 * ═══════════════════════════════════════════════════════════════════ */

_exc_irq_cur_sp0:
    SAVE_CONTEXT
    bl  exception_dispatch_irq
    RESTORE_CONTEXT

_exc_irq_cur_spx:
    SAVE_CONTEXT
    bl  exception_dispatch_irq
    RESTORE_CONTEXT

_exc_irq_lower64:
    SAVE_CONTEXT_LOWER
    bl  exception_dispatch_irq
    RESTORE_CONTEXT_LOWER

/* ═══════════════════════════════════════════════════════════════════
 * FIQ / SError stub — halt safely
 * ═══════════════════════════════════════════════════════════════════ */

_exc_fiq_stub:
    wfe
    b   _exc_fiq_stub

_exc_serror_stub:
    SAVE_CONTEXT
    bl  exception_dispatch_serror
    b   _exc_fiq_stub       /* halt after SError */
"#);

// ─── Rust exception dispatch ───────────────────────────────────────

/// Synchronous exception dispatch — called from assembly with TrapFrame pointer
/// x0 = &mut TrapFrame, x1 = source (0=cur/SP_EL0, 1=cur/SP_ELx, 2=lower64)
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn exception_dispatch_sync(frame: &mut TrapFrame, source: u64) {
    let esr: u64;
    unsafe { core::arch::asm!("mrs {}, esr_el1", out(reg) esr, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;

    match ec {
        0x15 => handle_svc(frame, esr),
        0x20 | 0x21 => handle_instruction_abort(frame, esr, source),
        0x24 | 0x25 => handle_data_abort(frame, esr, source),
        0x07 => handle_fp_trap(frame, esr, source),
        _ => handle_unknown(frame, esr, ec, source),
    }
}

/// IRQ dispatch — acknowledge GIC, dispatch by INTID, EOI
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn exception_dispatch_irq(frame: &mut TrapFrame) {
    let intid = crate::gic::acknowledge();

    if intid == crate::gic::INTID_SPURIOUS {
        return; // spurious, ignore
    }

    match intid {
        crate::timer::TIMER_INTID => crate::timer::tick_handler(frame),
        _ => crate::irq::irq_route(intid, frame),
    }

    crate::gic::end_interrupt(intid);
}

/// SError dispatch — always fatal
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn exception_dispatch_serror(_frame: &mut TrapFrame) {
    uart_print("\n!!! SERROR (fatal) !!!\n");
    // Will halt in assembly stub after return
}

// ─── Individual exception handlers ─────────────────────────────────

/// SVC handler — dispatch syscalls by x7
#[cfg(target_arch = "aarch64")]
fn handle_svc(frame: &mut TrapFrame, _esr: u64) {
    let syscall_nr = frame.x[7];
    let ep_id = frame.x[6];

    // ─── Phase G: Capability check ─────────────────────────────────
    let required = crate::cap::cap_for_syscall(syscall_nr, ep_id);
    let task_caps = unsafe { crate::sched::TCBS[crate::sched::CURRENT].caps };

    if !crate::cap::cap_check(task_caps, required) {
        uart_print("!!! CAP DENIED: task ");
        uart_print_hex(unsafe { crate::sched::CURRENT } as u64);
        uart_print(" syscall #");
        uart_print_hex(syscall_nr);
        uart_print(" needs ");
        uart_print(crate::cap::cap_name(required));
        uart_print(" — faulting\n");
        crate::sched::fault_current_task(frame);
        return;
    }

    match syscall_nr {
        // SYS_YIELD = 0: voluntarily yield CPU
        0 => crate::sched::schedule(frame),
        // SYS_SEND = 1: send IPC message (ep_id in x6)
        1 => crate::ipc::sys_send(frame, frame.x[6] as usize),
        // SYS_RECV = 2: receive IPC message (ep_id in x6)
        2 => crate::ipc::sys_recv(frame, frame.x[6] as usize),
        // SYS_CALL = 3: send + receive (ep_id in x6)
        3 => crate::ipc::sys_call(frame, frame.x[6] as usize),
        // SYS_WRITE = 4: write buffer to UART (x0=buf, x1=len)
        4 => handle_sys_write(frame),
        // SYS_NOTIFY = 5: send notification to target (x6=target_id, x0=bitmask)
        5 => handle_notify(frame),
        // SYS_WAIT_NOTIFY = 6: wait for notification (returns pending bits in x0)
        6 => handle_wait_notify(frame),
        // SYS_GRANT_CREATE = 7: create shared memory grant (x0=grant_id, x6=peer_task_id)
        7 => handle_grant_create(frame),
        // SYS_GRANT_REVOKE = 8: revoke shared memory grant (x0=grant_id)
        8 => handle_grant_revoke(frame),
        // SYS_IRQ_BIND = 9: bind IRQ to notification (x0=intid, x1=notify_bit)
        9 => handle_irq_bind(frame),
        // SYS_IRQ_ACK = 10: acknowledge IRQ handled (x0=intid)
        10 => handle_irq_ack(frame),
        // SYS_DEVICE_MAP = 11: map device MMIO into user-space (x0=device_id)
        11 => handle_device_map(frame),
        _ => {
            uart_print("!!! unknown syscall #");
            uart_print_hex(syscall_nr);
            uart_print(" — faulting task\n");
            crate::sched::fault_current_task(frame);
        }
    }
}

/// SYS_WRITE handler: write bytes to UART on behalf of EL0 task.
/// x0 = pointer to buffer, x1 = length in bytes.
/// Validates that the buffer pointer is in user-accessible memory.
#[cfg(target_arch = "aarch64")]
fn handle_sys_write(frame: &TrapFrame) {
    let buf_ptr = frame.x[0] as usize;
    let len = frame.x[1] as usize;

    let (valid, checked_len) = validate_write_args(buf_ptr, len);
    if !valid {
        if len > 0 {
            uart_print("!!! SYS_WRITE: bad pointer !!!\n");
        }
        return;
    }

    // Safe to read — copy bytes to UART
    for i in 0..checked_len {
        let byte = unsafe { core::ptr::read_volatile((buf_ptr + i) as *const u8) };
        crate::uart_write(byte);
    }
}

/// SYS_NOTIFY handler: send async notification bits to a target task.
/// x6 = target task ID, x0 = notification bitmask.
/// OR-merges bits into target's notify_pending. If target is blocked
/// in wait_notify, unblock it immediately.
#[cfg(target_arch = "aarch64")]
fn handle_notify(frame: &mut TrapFrame) {
    let target_id = frame.x[6] as usize;
    let bits = frame.x[0];

    if target_id >= crate::sched::NUM_TASKS {
        uart_print("!!! SYS_NOTIFY: invalid target\n");
        frame.x[0] = 0xFFFF_DEAD;
        return;
    }

    if bits == 0 {
        return; // no-op
    }

    unsafe {
        // OR-merge notification bits into target's pending mask
        crate::sched::TCBS[target_id].notify_pending |= bits;

        // If the target is blocked waiting for notifications, unblock it
        if crate::sched::TCBS[target_id].notify_waiting {
            crate::sched::TCBS[target_id].notify_waiting = false;

            // Deliver pending bits into the target's x0 and clear
            let pending = crate::sched::TCBS[target_id].notify_pending;
            crate::sched::TCBS[target_id].notify_pending = 0;
            crate::sched::set_task_reg(target_id, 0, pending);

            crate::sched::set_task_state(target_id, crate::sched::TaskState::Ready);
        }
    }
}

/// SYS_WAIT_NOTIFY handler: wait for notification bits.
/// If caller has pending bits: return immediately in x0 and clear.
/// Otherwise: block caller, set notify_waiting=true, schedule away.
#[cfg(target_arch = "aarch64")]
fn handle_wait_notify(frame: &mut TrapFrame) {
    unsafe {
        let current = crate::sched::CURRENT;

        let pending = crate::sched::TCBS[current].notify_pending;
        if pending != 0 {
            // Notifications already pending — deliver immediately
            frame.x[0] = pending;
            crate::sched::TCBS[current].notify_pending = 0;
        } else {
            // No pending notifications — block and wait
            crate::sched::save_frame(current, frame);
            crate::sched::TCBS[current].notify_waiting = true;
            crate::sched::set_task_state(current, crate::sched::TaskState::Blocked);
            crate::sched::schedule(frame);
        }
    }
}

/// SYS_GRANT_CREATE handler: create shared memory grant.
/// x0 = grant_id, x6 = peer_task_id.
/// Returns result in x0 (0 = success, else error code).
#[cfg(target_arch = "aarch64")]
fn handle_grant_create(frame: &mut TrapFrame) {
    let grant_id = frame.x[0] as usize;
    let peer_id = frame.x[6] as usize;
    let current = unsafe { crate::sched::CURRENT };

    let result = crate::grant::grant_create(grant_id, current, peer_id);
    frame.x[0] = result;
}

/// SYS_GRANT_REVOKE handler: revoke shared memory grant.
/// x0 = grant_id.
/// Returns result in x0 (0 = success, else error code).
#[cfg(target_arch = "aarch64")]
fn handle_grant_revoke(frame: &mut TrapFrame) {
    let grant_id = frame.x[0] as usize;
    let current = unsafe { crate::sched::CURRENT };

    let result = crate::grant::grant_revoke(grant_id, current);
    frame.x[0] = result;
}

/// SYS_IRQ_BIND handler: bind IRQ INTID to notification bit.
/// x0 = intid, x1 = notify_bit.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
fn handle_irq_bind(frame: &mut TrapFrame) {
    let intid = frame.x[0] as u32;
    let notify_bit = frame.x[1];
    let current = unsafe { crate::sched::CURRENT };
    let result = crate::irq::irq_bind(intid, current, notify_bit);
    frame.x[0] = result;
}

/// SYS_IRQ_ACK handler: acknowledge IRQ handled, unmask INTID.
/// x0 = intid.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
fn handle_irq_ack(frame: &mut TrapFrame) {
    let intid = frame.x[0] as u32;
    let current = unsafe { crate::sched::CURRENT };
    let result = crate::irq::irq_ack(intid, current);
    frame.x[0] = result;
}

/// SYS_DEVICE_MAP handler: map device MMIO into user-space.
/// x0 = device_id (0 = UART0).
/// Returns result in x0 (0 = success, else error code).
#[cfg(target_arch = "aarch64")]
fn handle_device_map(frame: &mut TrapFrame) {
    let device_id = frame.x[0];
    let current = unsafe { crate::sched::CURRENT };
    let result = unsafe { crate::mmu::map_device_for_task(device_id, current) };
    frame.x[0] = result;
}

/// Instruction Abort handler — fault task if from lower EL, halt if from same EL
#[cfg(target_arch = "aarch64")]
fn handle_instruction_abort(frame: &mut TrapFrame, esr: u64, source: u64) {
    let far: u64;
    unsafe { core::arch::asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;
    let ifsc = esr & 0x3F;

    uart_print("\n!!! INSTRUCTION ABORT !!!");
    if ec == 0x20 {
        // Lower EL (EL0 task) — recoverable fault
        uart_print(" [EL0 task]\n");
        uart_print("  IFSC: 0x");
        uart_print_hex(ifsc);
        print_fault_class(ifsc);
        uart_print("\n  FAR:  0x");
        uart_print_hex(far);
        uart_print("\n  ELR:  0x");
        uart_print_hex(frame.elr_el1);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal, halt
    uart_print(" [KERNEL]\n");
    uart_print("  Source: same EL\n");
    uart_print("  IFSC: 0x");
    uart_print_hex(ifsc);
    print_fault_class(ifsc);
    uart_print("\n  ESR:  0x");
    uart_print_hex(esr);
    uart_print("\n  FAR:  0x");
    uart_print_hex(far);
    uart_print("\n  ELR:  0x");
    uart_print_hex(frame.elr_el1);
    uart_print("\n  src:  ");
    uart_print_hex(source);
    uart_print("\n  HALTED.\n");
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Data Abort handler — fault task if from lower EL, halt if from same EL
#[cfg(target_arch = "aarch64")]
fn handle_data_abort(frame: &mut TrapFrame, esr: u64, source: u64) {
    let far: u64;
    unsafe { core::arch::asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;
    let dfsc = esr & 0x3F;

    uart_print("\n!!! DATA ABORT !!!");
    if ec == 0x24 {
        // Lower EL (EL0 task) — recoverable fault
        uart_print(" [EL0 task]\n");
        uart_print("  DFSC: 0x");
        uart_print_hex(dfsc);
        print_fault_class(dfsc);
        let wnr = (esr >> 6) & 1;
        if wnr == 1 {
            uart_print("\n  Access: WRITE");
        } else {
            uart_print("\n  Access: READ");
        }
        uart_print("\n  FAR:  0x");
        uart_print_hex(far);
        uart_print("\n  ELR:  0x");
        uart_print_hex(frame.elr_el1);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal, halt
    uart_print(" [KERNEL]\n");
    uart_print("  Source: same EL\n");
    uart_print("  DFSC: 0x");
    uart_print_hex(dfsc);
    print_fault_class(dfsc);
    let wnr = (esr >> 6) & 1;
    if wnr == 1 {
        uart_print("\n  Access: WRITE");
    } else {
        uart_print("\n  Access: READ");
    }
    uart_print("\n  ESR:  0x");
    uart_print_hex(esr);
    uart_print("\n  FAR:  0x");
    uart_print_hex(far);
    uart_print("\n  ELR:  0x");
    uart_print_hex(frame.elr_el1);
    uart_print("\n  src:  ");
    uart_print_hex(source);
    uart_print("\n  HALTED.\n");
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// FP/SIMD trap — fault task if from lower EL, halt if from same EL
#[cfg(target_arch = "aarch64")]
fn handle_fp_trap(frame: &mut TrapFrame, esr: u64, source: u64) {
    uart_print("\n!!! FP/SIMD TRAP !!!");
    if source == 2 {
        // Lower EL (EL0 task) — recoverable
        uart_print(" [EL0 task]\n");
        uart_print("  Task attempted FP/SIMD instruction.\n");
        uart_print("  ESR: 0x");
        uart_print_hex(esr);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal
    uart_print(" [KERNEL]\n");
    uart_print("  Kernel code attempted FP instruction.\n");
    uart_print("  ESR: 0x");
    uart_print_hex(esr);
    uart_print("\n  HALTED.\n");
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Unknown/unhandled exception class — fault task if from lower EL, halt if same EL
#[cfg(target_arch = "aarch64")]
fn handle_unknown(frame: &mut TrapFrame, esr: u64, ec: u64, source: u64) {
    uart_print("\n!!! UNHANDLED EXCEPTION !!!");
    if source == 2 {
        // Lower EL (EL0 task) — recoverable
        uart_print(" [EL0 task]\n");
        uart_print("  EC:   0x");
        uart_print_hex(ec);
        uart_print("\n  ESR:  0x");
        uart_print_hex(esr);
        uart_print("\n  ELR:  0x");
        uart_print_hex(frame.elr_el1);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal
    uart_print(" [KERNEL]\n");
    uart_print("  EC:   0x");
    uart_print_hex(ec);
    uart_print("\n  ESR:  0x");
    uart_print_hex(esr);
    uart_print("\n  ELR:  0x");
    uart_print_hex(frame.elr_el1);
    uart_print("\n  src:  ");
    uart_print_hex(source);
    uart_print("\n  HALTED.\n");
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Decode fault status code (DFSC/IFSC) bits [5:0] into human-readable class
#[cfg(target_arch = "aarch64")]
fn print_fault_class(fsc: u64) {
    let level = fsc & 0x3;
    match (fsc >> 2) & 0xF {
        0b0001 => {
            uart_print(" (Translation fault L");
            uart_print_hex(level);
            uart_print(")");
        }
        0b0010 => {
            uart_print(" (Access flag fault L");
            uart_print_hex(level);
            uart_print(")");
        }
        0b0011 => {
            uart_print(" (Permission fault L");
            uart_print_hex(level);
            uart_print(")");
        }
        _ => uart_print(" (other)"),
    }
}

// ─── Init ──────────────────────────────────────────────────────────

/// Install exception vector table — write VBAR_EL1
#[cfg(target_arch = "aarch64")]
pub fn init() {
    extern "C" {
        static __exception_vectors: u8;
    }
    unsafe {
        let vbar = &__exception_vectors as *const u8 as u64;
        core::arch::asm!(
            "msr vbar_el1, {v}",
            "isb",
            v = in(reg) vbar,
            options(nomem, nostack)
        );
    }
}

// ─── Pure validation logic (testable on host) ──────────────────────

/// Validate a SYS_WRITE pointer+length from EL0.
/// Returns (valid, clamped_len). Pure function — no side effects.
pub fn validate_write_args(buf_ptr: usize, len: usize) -> (bool, usize) {
    if len == 0 || len > 256 {
        return (false, 0);
    }
    let buf_end = buf_ptr.wrapping_add(len);
    if buf_ptr < 0x4000_0000 || buf_end > 0x4800_0000 || buf_end < buf_ptr {
        return (false, 0);
    }
    (true, len)
}

```

## File ./src\gic.rs:
```rust
/// Generic Interrupt Controller (Bộ điều khiển ngắt chung).
/// AegisOS GICv2 Driver — Minimal interrupt controller
///
/// QEMU virt machine GICv2 addresses:
///   GICD (Distributor):   0x0800_0000
///   GICC (CPU Interface): 0x0801_0000
use core::ptr;

// ─── Base addresses ────────────────────────────────────────────────

const GICD_BASE: usize = 0x0800_0000;
const GICC_BASE: usize = 0x0801_0000;

// ─── GICD register offsets ─────────────────────────────────────────

const GICD_CTLR: usize = 0x000;
const GICD_ISENABLER: usize = 0x100; // Set-enable (1 bit per INTID, registers of 32 bits)
const GICD_ICENABLER: usize = 0x180; // Clear-enable (write-1-to-disable, 1 bit per INTID)
const GICD_IPRIORITYR: usize = 0x400; // Priority (1 byte per INTID)

// ─── GICC register offsets ─────────────────────────────────────────

const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;

// ─── Helpers ───────────────────────────────────────────────────────

#[inline(always)]
fn gicd_write(offset: usize, val: u32) {
    unsafe { ptr::write_volatile((GICD_BASE + offset) as *mut u32, val) }
}

#[inline(always)]
fn gicd_read(offset: usize) -> u32 {
    unsafe { ptr::read_volatile((GICD_BASE + offset) as *const u32) }
}

#[inline(always)]
fn gicc_write(offset: usize, val: u32) {
    unsafe { ptr::write_volatile((GICC_BASE + offset) as *mut u32, val) }
}

#[inline(always)]
fn gicc_read(offset: usize) -> u32 {
    unsafe { ptr::read_volatile((GICC_BASE + offset) as *const u32) }
}

#[inline(always)]
fn gicd_write_byte(offset: usize, val: u8) {
    unsafe { ptr::write_volatile((GICD_BASE + offset) as *mut u8, val) }
}

// ─── Public API ────────────────────────────────────────────────────

/// Initialize GICv2: enable distributor + CPU interface, accept all priorities
pub fn init() {
    // 1. Disable distributor while configuring
    gicd_write(GICD_CTLR, 0);

    // 2. Enable distributor
    gicd_write(GICD_CTLR, 1);

    // 3. Set CPU interface: accept all priorities
    gicc_write(GICC_PMR, 0xFF);

    // 4. Enable CPU interface
    gicc_write(GICC_CTLR, 1);
}

/// Enable a specific interrupt ID
pub fn enable_intid(intid: u32) {
    // GICD_ISENABLER[n]: each register covers 32 INTIDs
    let reg_index = (intid / 32) as usize;
    let bit = 1u32 << (intid % 32);
    let offset = GICD_ISENABLER + reg_index * 4;

    let val = gicd_read(offset);
    gicd_write(offset, val | bit);
}

/// Disable (mask) a specific interrupt ID.
/// GICD_ICENABLER uses write-1-to-clear semantics — no read-modify-write needed.
pub fn disable_intid(intid: u32) {
    let reg_index = (intid / 32) as usize;
    let bit = 1u32 << (intid % 32);
    let offset = GICD_ICENABLER + reg_index * 4;
    gicd_write(offset, bit);
}

/// Set priority for a specific INTID (0 = highest, 0xFF = lowest)
pub fn set_priority(intid: u32, priority: u8) {
    // GICD_IPRIORITYR: 1 byte per INTID
    let offset = GICD_IPRIORITYR + intid as usize;
    gicd_write_byte(offset, priority);
}

/// Acknowledge IRQ — read GICC_IAR, returns INTID (1023 = spurious)
pub fn acknowledge() -> u32 {
    gicc_read(GICC_IAR) & 0x3FF // INTID is bits [9:0]
}

/// Signal End-Of-Interrupt for given INTID
pub fn end_interrupt(intid: u32) {
    gicc_write(GICC_EOIR, intid);
}

/// Spurious INTID constant
pub const INTID_SPURIOUS: u32 = 1023;

```

## File ./src\grant.rs:
```rust
/// AegisOS Shared Memory Grant Module
///
/// Allows two tasks to share a specific physical memory page under
/// kernel-controlled access. The owner creates a grant, mapping the
/// page into both tasks' L3 page tables as AP_RW_EL0. Revoking
/// unmaps the peer's access (sets entry back to AP_RW_EL1).
///
/// Grant pages are statically allocated in the `.grant_pages` linker
/// section — no heap, no dynamic allocation.
///
/// Syscalls:
///   SYS_GRANT_CREATE = 7: owner grants a page to a peer task
///   SYS_GRANT_REVOKE = 8: owner revokes peer's access

use crate::sched;
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

/// Maximum number of grant pages (statically allocated in linker.ld)
pub const MAX_GRANTS: usize = 2;

/// Grant page size (must match linker.ld allocation)
pub const GRANT_PAGE_SIZE: usize = 4096;

// ─── Grant struct ──────────────────────────────────────────────────

/// A shared memory grant — tracks who owns and shares a page.
#[derive(Clone, Copy)]
pub struct Grant {
    /// Task that created the grant (None = slot unused)
    pub owner: Option<usize>,
    /// Task that was granted access (None = not shared)
    pub peer: Option<usize>,
    /// Physical address of the grant page
    pub phys_addr: u64,
    /// Whether this grant is currently active
    pub active: bool,
}

pub const EMPTY_GRANT: Grant = Grant {
    owner: None,
    peer: None,
    phys_addr: 0,
    active: false,
};

// ─── Static grant table ────────────────────────────────────────────

pub static mut GRANTS: [Grant; MAX_GRANTS] = [EMPTY_GRANT; MAX_GRANTS];

// ─── Grant page addresses (from linker) ────────────────────────────

/// Get the physical address of grant page `grant_id`.
/// Returns None if grant_id is out of range.
#[cfg(target_arch = "aarch64")]
pub fn grant_page_addr(grant_id: usize) -> Option<u64> {
    if grant_id >= MAX_GRANTS {
        return None;
    }
    extern "C" {
        static __grant_pages_start: u8;
    }
    let base = unsafe { &__grant_pages_start as *const u8 as u64 };
    Some(base + (grant_id as u64) * GRANT_PAGE_SIZE as u64)
}

/// Host-test stub: return a fake but distinct address per grant.
#[cfg(not(target_arch = "aarch64"))]
pub fn grant_page_addr(grant_id: usize) -> Option<u64> {
    if grant_id >= MAX_GRANTS {
        return None;
    }
    // Fake addresses within the first 2MiB (L3 range) for test purposes
    Some(0x4010_0000_u64 + (grant_id as u64) * GRANT_PAGE_SIZE as u64)
}

// ─── Core operations ───────────────────────────────────────────────

/// Create a shared memory grant.
/// `grant_id`: which grant page (0..MAX_GRANTS)
/// `owner`: task creating the grant (current task)
/// `peer`: task receiving shared access
///
/// Returns 0 on success, error code on failure:
///   0xFFFF_0001 = invalid grant_id
///   0xFFFF_0002 = grant already active
///   0xFFFF_0003 = invalid peer
///   0xFFFF_0004 = owner == peer
pub fn grant_create(grant_id: usize, owner: usize, peer: usize) -> u64 {
    if grant_id >= MAX_GRANTS {
        uart_print("!!! GRANT: invalid grant_id\n");
        return 0xFFFF_0001;
    }

    unsafe {
        if GRANTS[grant_id].active {
            uart_print("!!! GRANT: already active\n");
            return 0xFFFF_0002;
        }

        if peer >= sched::NUM_TASKS {
            uart_print("!!! GRANT: invalid peer\n");
            return 0xFFFF_0003;
        }

        if owner == peer {
            uart_print("!!! GRANT: owner == peer\n");
            return 0xFFFF_0004;
        }

        let phys = match grant_page_addr(grant_id) {
            Some(addr) => addr,
            None => return 0xFFFF_0001,
        };

        // Map grant page into both tasks' L3 page tables
        #[cfg(target_arch = "aarch64")]
        {
            crate::mmu::map_grant_for_task(phys, owner);
            crate::mmu::map_grant_for_task(phys, peer);
        }

        GRANTS[grant_id] = Grant {
            owner: Some(owner),
            peer: Some(peer),
            phys_addr: phys,
            active: true,
        };

        uart_print("[AegisOS] GRANT: task ");
        crate::uart_print_hex(owner as u64);
        uart_print(" -> task ");
        crate::uart_print_hex(peer as u64);
        uart_print(" (grant ");
        crate::uart_print_hex(grant_id as u64);
        uart_print(")\n");
    }

    0 // success
}

/// Revoke a shared memory grant.
/// `grant_id`: which grant to revoke
/// `caller`: task requesting revoke (must be owner)
///
/// Returns 0 on success, error code on failure.
pub fn grant_revoke(grant_id: usize, caller: usize) -> u64 {
    if grant_id >= MAX_GRANTS {
        uart_print("!!! GRANT: invalid grant_id\n");
        return 0xFFFF_0001;
    }

    unsafe {
        if !GRANTS[grant_id].active {
            return 0; // no-op: already inactive
        }

        if GRANTS[grant_id].owner != Some(caller) {
            uart_print("!!! GRANT: caller is not owner\n");
            return 0xFFFF_0005;
        }

        // Unmap from peer's page table
        if let Some(peer) = GRANTS[grant_id].peer {
            #[cfg(target_arch = "aarch64")]
            {
                crate::mmu::unmap_grant_for_task(GRANTS[grant_id].phys_addr, peer);
            }
        }

        GRANTS[grant_id].active = false;
        GRANTS[grant_id].peer = None;

        uart_print("[AegisOS] GRANT REVOKED: grant ");
        crate::uart_print_hex(grant_id as u64);
        uart_print("\n");
    }

    0 // success
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Clean up all grants involving a faulted task.
/// If the task is owner: revoke grant (unmap peer).
/// If the task is peer: unmap peer's access.
/// Called from sched::fault_current_task() and sched::restart_task().
pub fn cleanup_task(task_idx: usize) {
    unsafe {
        for i in 0..MAX_GRANTS {
            if !GRANTS[i].active {
                continue;
            }

            if GRANTS[i].owner == Some(task_idx) {
                // Task is owner — unmap peer and deactivate
                if let Some(peer) = GRANTS[i].peer {
                    #[cfg(target_arch = "aarch64")]
                    {
                        crate::mmu::unmap_grant_for_task(GRANTS[i].phys_addr, peer);
                    }
                }
                // Also unmap from owner (faulted task gets fresh state on restart)
                #[cfg(target_arch = "aarch64")]
                {
                    crate::mmu::unmap_grant_for_task(GRANTS[i].phys_addr, task_idx);
                }
                GRANTS[i] = EMPTY_GRANT;
            } else if GRANTS[i].peer == Some(task_idx) {
                // Task is peer — unmap peer's access, keep owner's grant active but no peer
                #[cfg(target_arch = "aarch64")]
                {
                    crate::mmu::unmap_grant_for_task(GRANTS[i].phys_addr, task_idx);
                }
                GRANTS[i].peer = None;
                GRANTS[i].active = false;
            }
        }
    }
}

```

## File ./src\ipc.rs:
```rust
/// AegisOS IPC — Synchronous Endpoint-based messaging
/// Inter-Process Communication (IPC) Giao tiếp đồng bộ giữa các tiến trình qua các endpoint.
/// Synchronous IPC: sender blocks until receiver is ready and vice versa.
/// Message payload: x[0]..x[3] in TrapFrame (4 × u64 = 32 bytes).
///
/// Syscalls:
///   SYS_SEND = 1: send message to endpoint (blocks if no receiver)
///   SYS_RECV = 2: receive message from endpoint (blocks if no sender)
///   SYS_CALL = 3: send + recv (client call pattern)

use crate::exception::TrapFrame;
use crate::sched::{self, TaskState};
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

#[allow(dead_code)]
pub const SYS_SEND: u64 = 1;
#[allow(dead_code)]
pub const SYS_RECV: u64 = 2;
#[allow(dead_code)]
pub const SYS_CALL: u64 = 3;

pub const MAX_ENDPOINTS: usize = 4;
pub const MSG_REGS: usize = 4; // x[0]..x[3]
pub const MAX_WAITERS: usize = 4; // max senders queued per endpoint

// ─── Endpoint ──────────────────────────────────────────────────────

/// Circular queue for sender waiters on an endpoint.
pub struct SenderQueue {
    pub tasks: [usize; MAX_WAITERS],
    pub head: usize, // index of next task to dequeue
    pub count: usize, // number of tasks in queue
}

impl SenderQueue {
    pub const fn new() -> Self {
        SenderQueue { tasks: [0; MAX_WAITERS], head: 0, count: 0 }
    }

    /// Push a task index. Returns false if queue is full.
    pub fn push(&mut self, task: usize) -> bool {
        if self.count >= MAX_WAITERS { return false; }
        let tail = (self.head + self.count) % MAX_WAITERS;
        self.tasks[tail] = task;
        self.count += 1;
        true
    }

    /// Pop the next sender. Returns None if empty.
    pub fn pop(&mut self) -> Option<usize> {
        if self.count == 0 { return None; }
        let task = self.tasks[self.head];
        self.head = (self.head + 1) % MAX_WAITERS;
        self.count -= 1;
        Some(task)
    }

    /// Remove a specific task from the queue (for cleanup).
    pub fn remove(&mut self, task: usize) {
        // Rebuild queue without the target task
        let old_count = self.count;
        let old_head = self.head;
        let mut new_tasks = [0usize; MAX_WAITERS];
        let mut new_count = 0usize;
        for i in 0..old_count {
            let idx = (old_head + i) % MAX_WAITERS;
            if self.tasks[idx] != task {
                new_tasks[new_count] = self.tasks[idx];
                new_count += 1;
            }
        }
        self.tasks = new_tasks;
        self.head = 0;
        self.count = new_count;
    }

    /// Check if a task is in the queue.
    pub fn contains(&self, task: usize) -> bool {
        for i in 0..self.count {
            let idx = (self.head + i) % MAX_WAITERS;
            if self.tasks[idx] == task { return true; }
        }
        false
    }
}

/// An IPC endpoint. Multiple senders can queue, one receiver waits.
pub struct Endpoint {
    /// Circular queue of tasks blocked waiting to send on this endpoint
    pub sender_queue: SenderQueue,
    /// Task blocked waiting to receive on this endpoint (None = no waiter)
    pub receiver: Option<usize>,
}

pub const EMPTY_EP: Endpoint = Endpoint {
    sender_queue: SenderQueue { tasks: [0; MAX_WAITERS], head: 0, count: 0 },
    receiver: None,
};

pub static mut ENDPOINTS: [Endpoint; MAX_ENDPOINTS] = [EMPTY_EP; MAX_ENDPOINTS];

// ─── IPC operations ────────────────────────────────────────────────

/// sys_send(frame, ep_id): send message on endpoint.
/// Message payload: frame.x[0..4] → receiver's x[0..4].
/// If a receiver is already waiting: deliver immediately, unblock receiver.
/// Otherwise: block sender, enqueue, schedule away.
pub fn sys_send(frame: &mut TrapFrame, ep_id: usize) {
    if ep_id >= MAX_ENDPOINTS {
        uart_print("!!! IPC: invalid endpoint\n");
        return;
    }

    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB so copy_message can read from it
        sched::save_frame(current, frame);

        if let Some(recv_task) = ENDPOINTS[ep_id].receiver.take() {
            // Receiver is waiting — deliver message directly
            copy_message(current, recv_task);

            // Unblock receiver
            sched::set_task_state(recv_task, TaskState::Ready);

            // Sender continues (not blocked)
        } else {
            // No receiver — enqueue sender and block
            if !ENDPOINTS[ep_id].sender_queue.push(current) {
                uart_print("!!! IPC: sender queue full\n");
                return;
            }
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

/// sys_recv(frame, ep_id): receive message from endpoint.
/// If a sender is already waiting: receive immediately, unblock sender.
/// Otherwise: block receiver, enqueue, schedule away.
pub fn sys_recv(frame: &mut TrapFrame, ep_id: usize) {
    if ep_id >= MAX_ENDPOINTS {
        uart_print("!!! IPC: invalid endpoint\n");
        return;
    }

    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB
        sched::save_frame(current, frame);

        if let Some(send_task) = ENDPOINTS[ep_id].sender_queue.pop() {
            // Sender is waiting — receive message directly
            copy_message(send_task, current);

            // Unblock sender
            sched::set_task_state(send_task, TaskState::Ready);

            // Load received message back into our frame so caller sees it
            sched::load_frame(current, frame);
        } else {
            // No sender — block receiver and wait
            ENDPOINTS[ep_id].receiver = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

/// sys_call(frame, ep_id): send message, then block to receive reply.
/// Equivalent to send + recv atomically.
pub fn sys_call(frame: &mut TrapFrame, ep_id: usize) {
    if ep_id >= MAX_ENDPOINTS {
        uart_print("!!! IPC: invalid endpoint\n");
        return;
    }

    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB so message can be copied
        sched::save_frame(current, frame);

        if let Some(recv_task) = ENDPOINTS[ep_id].receiver.take() {
            // Receiver is waiting — deliver message
            copy_message(current, recv_task);
            sched::set_task_state(recv_task, TaskState::Ready);

            // Now block ourselves waiting for reply
            ENDPOINTS[ep_id].receiver = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        } else {
            // No receiver — enqueue as sender, will also need reply
            if !ENDPOINTS[ep_id].sender_queue.push(current) {
                uart_print("!!! IPC: sender queue full\n");
                return;
            }
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────

/// Copy message registers x[0]..x[3] from sender's TCB to receiver's TCB.
pub unsafe fn copy_message(from_task: usize, to_task: usize) {
    for i in 0..MSG_REGS {
        let val = sched::get_task_reg(from_task, i);
        sched::set_task_reg(to_task, i, val);
    }
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Remove a faulted task from all IPC endpoint slots.
/// If a partner was blocked waiting for this task, unblock the partner
/// so it can be rescheduled (partner will retry IPC or find no match).
pub fn cleanup_task(task_idx: usize) {
    unsafe {
        for i in 0..MAX_ENDPOINTS {
            // If the faulted task was a pending sender, remove from queue
            ENDPOINTS[i].sender_queue.remove(task_idx);

            // If the faulted task was a pending receiver, clear the slot
            if ENDPOINTS[i].receiver == Some(task_idx) {
                ENDPOINTS[i].receiver = None;
            }
        }
    }
}

```

## File ./src\irq.rs:
```rust
/// AegisOS IRQ Routing Module
///
/// Routes hardware interrupts (SPIs, INTID ≥ 32) to user tasks via
/// the notification system. A task registers interest in an INTID
/// by calling SYS_IRQ_BIND; the kernel masks/unmasks the interrupt
/// and delivers a notification bit when it fires.
///
/// Flow:
///   1. Task → SYS_IRQ_BIND(intid, notify_bit) → kernel enables INTID
///   2. HW IRQ fires → irq_route() → notify task, mask INTID
///   3. Task handles device → SYS_IRQ_ACK(intid) → kernel unmasks INTID
///
/// Syscalls:
///   SYS_IRQ_BIND = 9:  register to receive IRQ as notification
///   SYS_IRQ_ACK  = 10: acknowledge IRQ handled, re-enable INTID

use crate::sched;
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

/// Maximum number of IRQ bindings (SPI slots)
pub const MAX_IRQ_BINDINGS: usize = 8;

/// Minimum INTID for user-bindable interrupts (SPIs start at 32)
pub const MIN_SPI_INTID: u32 = 32;

// ─── Error codes ───────────────────────────────────────────────────

pub const ERR_INVALID_INTID: u64 = 0xFFFF_1001;
pub const ERR_ALREADY_BOUND: u64 = 0xFFFF_1002;
pub const ERR_TABLE_FULL: u64 = 0xFFFF_1003;
pub const ERR_NOT_BOUND: u64 = 0xFFFF_1004;
pub const ERR_NOT_OWNER: u64 = 0xFFFF_1005;

// ─── IrqBinding struct ────────────────────────────────────────────

/// An IRQ-to-task binding — routes a hardware INTID to a notification.
#[derive(Clone, Copy)]
pub struct IrqBinding {
    /// Hardware interrupt ID (SPI, ≥ 32)
    pub intid: u32,
    /// Task receiving notifications when this IRQ fires
    pub task_id: usize,
    /// Which bit to OR into notify_pending
    pub notify_bit: u64,
    /// Whether this binding slot is in use
    pub active: bool,
    /// IRQ fired but task hasn't ACK'd yet (INTID masked)
    pub pending_ack: bool,
}

pub const EMPTY_BINDING: IrqBinding = IrqBinding {
    intid: 0,
    task_id: 0,
    notify_bit: 0,
    active: false,
    pending_ack: false,
};

// ─── Static binding table ──────────────────────────────────────────

pub static mut IRQ_BINDINGS: [IrqBinding; MAX_IRQ_BINDINGS] =
    [EMPTY_BINDING; MAX_IRQ_BINDINGS];

// ─── Core operations ───────────────────────────────────────────────

/// Bind an IRQ (INTID) to a task's notification system.
///
/// Validates:
///   - INTID ≥ 32 (SPIs only; PPIs/SGIs are kernel-reserved)
///   - Not already bound by another task
///   - Table not full
///
/// On success: enables INTID in GIC, returns 0.
pub fn irq_bind(intid: u32, task_id: usize, notify_bit: u64) -> u64 {
    // Reject PPIs/SGIs (INTID < 32), including timer (INTID 30)
    if intid < MIN_SPI_INTID {
        uart_print("!!! IRQ: invalid INTID (< 32)\n");
        return ERR_INVALID_INTID;
    }

    // notify_bit must have exactly one bit set (or at least be non-zero)
    if notify_bit == 0 {
        uart_print("!!! IRQ: notify_bit is zero\n");
        return ERR_INVALID_INTID;
    }

    unsafe {
        // Check for duplicate: same INTID already bound
        for i in 0..MAX_IRQ_BINDINGS {
            if IRQ_BINDINGS[i].active && IRQ_BINDINGS[i].intid == intid {
                uart_print("!!! IRQ: INTID already bound\n");
                return ERR_ALREADY_BOUND;
            }
        }

        // Find empty slot
        let mut slot: Option<usize> = None;
        for i in 0..MAX_IRQ_BINDINGS {
            if !IRQ_BINDINGS[i].active {
                slot = Some(i);
                break;
            }
        }

        let idx = match slot {
            Some(i) => i,
            None => {
                uart_print("!!! IRQ: binding table full\n");
                return ERR_TABLE_FULL;
            }
        };

        IRQ_BINDINGS[idx] = IrqBinding {
            intid,
            task_id,
            notify_bit,
            active: true,
            pending_ack: false,
        };

        // Enable this INTID in the GIC
        #[cfg(target_arch = "aarch64")]
        {
            crate::gic::enable_intid(intid);
        }

        uart_print("[AegisOS] IRQ BIND: INTID ");
        crate::uart_print_hex(intid as u64);
        uart_print(" -> task ");
        crate::uart_print_hex(task_id as u64);
        uart_print(", bit ");
        crate::uart_print_hex(notify_bit);
        uart_print("\n");
    }

    0 // success
}

/// Acknowledge an IRQ, allowing the kernel to unmask it.
///
/// The task must be the one that received the notification.
/// Clears pending_ack and re-enables the INTID in the GIC.
pub fn irq_ack(intid: u32, task_id: usize) -> u64 {
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if IRQ_BINDINGS[i].active
                && IRQ_BINDINGS[i].intid == intid
            {
                if IRQ_BINDINGS[i].task_id != task_id {
                    uart_print("!!! IRQ ACK: not the bound task\n");
                    return ERR_NOT_OWNER;
                }

                if !IRQ_BINDINGS[i].pending_ack {
                    // Already ACK'd or never fired — no-op
                    return 0;
                }

                IRQ_BINDINGS[i].pending_ack = false;

                // Re-enable (unmask) the INTID in GIC
                #[cfg(target_arch = "aarch64")]
                {
                    crate::gic::enable_intid(intid);
                }

                return 0;
            }
        }
    }

    // No binding found for this INTID
    ERR_NOT_BOUND
}

/// Route a hardware IRQ to the bound task (called from exception handler).
///
/// Looks up the INTID in the binding table. If bound:
///   - OR notify_bit into task's notify_pending
///   - If task is waiting on notifications → unblock it
///   - Set pending_ack = true
///   - Mask the INTID until task calls SYS_IRQ_ACK
///
/// If not bound, prints a warning and ignores.
#[cfg(target_arch = "aarch64")]
pub fn irq_route(intid: u32, _frame: &mut crate::exception::TrapFrame) {
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if IRQ_BINDINGS[i].active && IRQ_BINDINGS[i].intid == intid {
                let tid = IRQ_BINDINGS[i].task_id;
                let bit = IRQ_BINDINGS[i].notify_bit;

                // OR notification bit into task's pending mask
                sched::TCBS[tid].notify_pending |= bit;

                // If task is waiting for notifications, unblock it
                if sched::TCBS[tid].notify_waiting {
                    sched::TCBS[tid].notify_waiting = false;
                    sched::TCBS[tid].state = sched::TaskState::Ready;
                    // Deliver pending bits into x0
                    let pending = sched::TCBS[tid].notify_pending;
                    sched::TCBS[tid].context.x[0] = pending;
                    sched::TCBS[tid].notify_pending = 0;
                }

                // Mark pending ACK — INTID stays masked until task ACKs
                IRQ_BINDINGS[i].pending_ack = true;

                // Mask this INTID until ACK
                crate::gic::disable_intid(intid);

                return;
            }
        }

        // No binding found — log and ignore
        uart_print("!!! IRQ INTID=");
        crate::uart_print_hex(intid as u64);
        uart_print(" (unbound, ignored)\n");
    }
}

/// Stub for host tests — irq_route requires TrapFrame which is AArch64-only.
#[cfg(not(target_arch = "aarch64"))]
pub fn irq_route_test(intid: u32, task_id: usize) {
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if IRQ_BINDINGS[i].active && IRQ_BINDINGS[i].intid == intid {
                let tid = IRQ_BINDINGS[i].task_id;
                let bit = IRQ_BINDINGS[i].notify_bit;

                sched::TCBS[tid].notify_pending |= bit;

                if sched::TCBS[tid].notify_waiting {
                    sched::TCBS[tid].notify_waiting = false;
                    sched::TCBS[tid].state = sched::TaskState::Ready;
                    let pending = sched::TCBS[tid].notify_pending;
                    sched::TCBS[tid].context.x[0] = pending;
                    sched::TCBS[tid].notify_pending = 0;
                }

                IRQ_BINDINGS[i].pending_ack = true;
                // No GIC on host
                return;
            }
        }
    }
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Clean up all IRQ bindings for a faulted/restarted task.
/// If binding has pending_ack, re-enable the INTID (unmask orphaned IRQ).
pub fn irq_cleanup_task(task_id: usize) {
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if IRQ_BINDINGS[i].active && IRQ_BINDINGS[i].task_id == task_id {
                // If IRQ was masked waiting for ACK, unmask it
                if IRQ_BINDINGS[i].pending_ack {
                    #[cfg(target_arch = "aarch64")]
                    {
                        crate::gic::enable_intid(IRQ_BINDINGS[i].intid);
                    }
                }

                uart_print("[AegisOS] IRQ cleanup: unbind INTID ");
                crate::uart_print_hex(IRQ_BINDINGS[i].intid as u64);
                uart_print(" from task ");
                crate::uart_print_hex(task_id as u64);
                uart_print("\n");

                // Disable the INTID since no one is listening
                #[cfg(target_arch = "aarch64")]
                {
                    crate::gic::disable_intid(IRQ_BINDINGS[i].intid);
                }

                IRQ_BINDINGS[i] = EMPTY_BINDING;
            }
        }
    }
}

```

## File ./src\lib.rs:
```rust
//! AegisOS — Kernel library crate
//!
//! Re-exports kernel modules so they can be used by both the kernel
//! binary (main.rs) and host-side unit tests (tests/host_tests.rs).
//!
//! On AArch64: full kernel with asm, MMIO, linker symbols.
//! On host (x86_64): only pure logic available (types, constants, validation).

#![no_std]

pub mod cap;
pub mod uart;
pub mod mmu;
pub mod exception;
pub mod sched;
pub mod ipc;
pub mod timer;
pub mod grant;
pub mod irq;

#[cfg(target_arch = "aarch64")]
pub mod gic;

// Re-export common UART functions at crate root for convenience
pub use uart::{uart_write, uart_print, uart_print_hex};

```

## File ./src\main.rs:
```rust
// AegisOS — Kernel binary entry point
// This entire file is AArch64-only. When building for host tests (x86_64),
// the content is gated off and only the lib crate is tested.

// On AArch64: full kernel binary with boot asm, syscall wrappers, tasks
#![cfg_attr(target_arch = "aarch64", no_std)]
#![cfg_attr(target_arch = "aarch64", no_main)]

// On host (x86_64): empty bin that does nothing (tests use --lib --test)
#![cfg_attr(not(target_arch = "aarch64"), allow(unused))]

#[cfg(target_arch = "aarch64")]
use core::panic::PanicInfo;

#[cfg(target_arch = "aarch64")]
use aegis_os::uart_print;
#[cfg(target_arch = "aarch64")]
use aegis_os::exception;
#[cfg(target_arch = "aarch64")]
use aegis_os::sched;
#[cfg(target_arch = "aarch64")]
use aegis_os::timer;
#[cfg(target_arch = "aarch64")]
use aegis_os::gic;

// Boot assembly — inline vào binary thông qua global_asm!
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(include_str!("boot.s"));

// ─── Syscall wrappers ──────────────────────────────────────────────

/// SYS_YIELD (syscall #0): voluntarily yield the CPU to the next task.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_yield() {
    unsafe {
        core::arch::asm!(
            "mov x7, #0",
            "svc #0",
            out("x7") _,
            options(nomem, nostack)
        );
    }
}

/// SYS_SEND (syscall #1): send message on endpoint.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_send(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") 1u64, // SYS_SEND
            options(nomem, nostack)
        );
    }
}

/// SYS_RECV (syscall #2): receive message from endpoint.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_recv(ep_id: u64) -> u64 {
    let msg0: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") 2u64, // SYS_RECV
            lateout("x0") msg0,
            options(nomem, nostack)
        );
    }
    msg0
}

/// SYS_RECV variant returning first two message registers (x0, x1).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_recv2(ep_id: u64) -> (u64, u64) {
    let msg0: u64;
    let msg1: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") 2u64, // SYS_RECV
            lateout("x0") msg0,
            lateout("x1") msg1,
            options(nomem, nostack)
        );
    }
    (msg0, msg1)
}

/// SYS_CALL (syscall #3): send message then wait for reply.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_call(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) -> u64 {
    let reply0: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") 3u64, // SYS_CALL
            lateout("x0") reply0,
            options(nomem, nostack)
        );
    }
    reply0
}

/// SYS_WRITE (syscall #4): write string to UART via kernel.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_write(buf: *const u8, len: usize) {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") buf as u64,
            in("x1") len as u64,
            in("x7") 4u64, // SYS_WRITE
            options(nomem, nostack)
        );
    }
}

/// Print a string from EL0 via SYS_WRITE syscall
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn user_print(s: &str) {
    syscall_write(s.as_ptr(), s.len());
}

/// SYS_NOTIFY (syscall #5): send notification bitmask to target task.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_notify(target_id: u64, bits: u64) {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") bits,
            in("x6") target_id,
            in("x7") 5u64, // SYS_NOTIFY
            options(nomem, nostack)
        );
    }
}

/// SYS_WAIT_NOTIFY (syscall #6): block until notification arrives.
/// Returns the pending bitmask in x0.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_wait_notify() -> u64 {
    let bits: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x7") 6u64, // SYS_WAIT_NOTIFY
            lateout("x0") bits,
            options(nomem, nostack)
        );
    }
    bits
}
/// SYS_GRANT_CREATE (syscall #7): create shared memory grant.
/// x0 = grant_id, x6 = peer_task_id.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_grant_create(grant_id: u64, peer_task_id: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") grant_id,
            in("x6") peer_task_id,
            in("x7") 7u64, // SYS_GRANT_CREATE
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_GRANT_REVOKE (syscall #8): revoke shared memory grant.
/// x0 = grant_id.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_grant_revoke(grant_id: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") grant_id,
            in("x7") 8u64, // SYS_GRANT_REVOKE
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}
/// SYS_IRQ_BIND (syscall #9): bind an IRQ INTID to a notification bit.
/// x0 = intid (must be ≥ 32, SPIs only), x1 = notify_bit.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_irq_bind(intid: u64, notify_bit: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") intid,
            in("x1") notify_bit,
            in("x7") 9u64, // SYS_IRQ_BIND
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_IRQ_ACK (syscall #10): acknowledge an IRQ handled, re-enable INTID.
/// x0 = intid.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_irq_ack(intid: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") intid,
            in("x7") 10u64, // SYS_IRQ_ACK
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_DEVICE_MAP (syscall #11): map device MMIO into user-space.
/// x0 = device_id (0 = UART0).
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_device_map(device_id: u64) -> u64 {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") device_id,
            in("x7") 11u64, // SYS_DEVICE_MAP
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

// ─── Task entry points (Phase J4: User-Mode UART Driver PoC) ───────

/// UART0 PL011 Data Register address (identity-mapped after SYS_DEVICE_MAP)
#[cfg(target_arch = "aarch64")]
const UART0_DR: *mut u8 = 0x0900_0000 as *mut u8;

/// Task 0 — UART User-Mode Driver
///
/// Requests UART MMIO access from kernel, then loops serving IPC requests
/// from client tasks. Reads data from shared grant page and writes each
/// byte directly to UART DR — a genuine EL0 device driver.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn uart_driver_entry() -> ! {
    // 1. Map UART0 MMIO into our address space (EL0 accessible)
    syscall_device_map(0); // device_id=0 = UART0

    // 2. Announce we're ready (still using SYS_WRITE for initial status)
    user_print("DRV:ready ");

    // 3. Serve client requests forever
    loop {
        // Block waiting for an IPC request on EP 0
        let (buf_addr_raw, len_raw) = syscall_recv2(0);

        // msg x0 = buffer address in grant page
        // msg x1 = byte count to write
        let buf_addr = buf_addr_raw as *const u8;
        let len = len_raw as usize;

        // Write each byte directly to UART DR (EL0 MMIO write!)
        for i in 0..len {
            unsafe {
                let byte = core::ptr::read_volatile(buf_addr.add(i));
                core::ptr::write_volatile(UART0_DR, byte);
            }
        }

        // Reply "OK" to unblock the client
        syscall_send(0, 0x4F4B, 0, 0, 0); // "OK"
    }
}

/// Task 1 — Client using UART driver via IPC + shared memory
///
/// Creates a shared memory grant, writes a message into the grant page,
/// then calls the UART driver via IPC to output it. This demonstrates
/// the full user-mode driver stack: grant + IPC + MMIO.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn client_entry() -> ! {
    // 1. Create a shared memory grant: grant 0, owner=us(task 1), peer=driver(task 0)
    syscall_grant_create(0, 0); // grant_id=0, peer_task_id=0

    // 2. Get the grant page address (identity-mapped, known at compile time)
    let grant_addr = aegis_os::grant::grant_page_addr(0).unwrap_or(0) as *mut u8;

    loop {
        // 3. Write the message into the grant page
        let msg = b"J4:UserDrv ";
        unsafe {
            for (i, &byte) in msg.iter().enumerate() {
                core::ptr::write_volatile(grant_addr.add(i), byte);
            }
        }

        // 4. Call the UART driver: send buffer address + length via IPC
        syscall_call(0, grant_addr as u64, msg.len() as u64, 0, 0);
    }
}

/// Idle task: just wfi in a loop
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn idle_entry() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

// ─── Kernel main ───────────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart_print("\n[AegisOS] boot\n");
    uart_print("[AegisOS] MMU enabled (identity map)\n");
    uart_print("[AegisOS] W^X enforced (WXN + 4KB pages)\n");

    exception::init();
    uart_print("[AegisOS] exceptions ready\n");

    gic::init();
    gic::set_priority(timer::TIMER_INTID, 0);
    gic::enable_intid(timer::TIMER_INTID);

    sched::init(
        uart_driver_entry as *const () as u64,
        client_entry as *const () as u64,
        idle_entry as *const () as u64,
    );

    // ─── Phase G: Assign capabilities ──────────────────────────────
    unsafe {
        use aegis_os::cap::*;
        // Task 0 (UART driver): needs RECV/SEND on EP0 + WRITE + YIELD + notifications + grants + IRQ + device map
        sched::TCBS[0].caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
            | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
            | CAP_IRQ_BIND | CAP_IRQ_ACK | CAP_DEVICE_MAP;
        // Task 1 (client): needs CALL on EP0 + WRITE + YIELD + notifications + grants
        sched::TCBS[1].caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
            | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE;
        // Task 2 (idle): only needs YIELD (WFI loop)
        sched::TCBS[2].caps = CAP_YIELD;
    }
    uart_print("[AegisOS] capabilities assigned\n");
    uart_print("[AegisOS] notification system ready\n");
    uart_print("[AegisOS] grant system ready\n");
    uart_print("[AegisOS] IRQ routing ready\n");
    uart_print("[AegisOS] device MMIO mapping ready\n");

    // ─── Phase H: Assign per-task address spaces ───────────────────
    unsafe {
        use aegis_os::mmu;
        // ASID = task_id + 1 (ASID 0 is reserved for kernel boot)
        sched::TCBS[0].ttbr0 = mmu::ttbr0_for_task(0, 1);
        sched::TCBS[1].ttbr0 = mmu::ttbr0_for_task(1, 2);
        sched::TCBS[2].ttbr0 = mmu::ttbr0_for_task(2, 3);
    }
    uart_print("[AegisOS] per-task address spaces assigned\n");

    timer::init(10);

    uart_print("[AegisOS] bootstrapping into uart_driver (EL0)...\n");
    sched::bootstrap();
}

#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}

// On host target: provide a main() so the bin target compiles
#[cfg(not(target_arch = "aarch64"))]
fn main() {}

```

## File ./src\mmu.rs:
```rust
/// Memory Management Unit (Bộ phận Quản lý Bộ nhớ).
/// AegisOS MMU — AArch64 Page Table Setup
///
/// Sub-phase 1: Identity map with 2 MiB blocks
/// Sub-phase 2: Refine to 4KB pages for kernel region, W^X enforcement

#[cfg(target_arch = "aarch64")]
use core::ptr;

// ─── Descriptor bits ───────────────────────────────────────────────

/// L1/L2 table descriptor — points to next-level table
pub const TABLE: u64 = 0b11;

/// L1/L2 block descriptor — maps a large region directly
pub const BLOCK: u64 = 0b01;

/// L3 page descriptor
pub const PAGE: u64 = 0b11;

// AttrIndx — index into MAIR_EL1 (bits [4:2])
/// MAIR index 0 = Device-nGnRnE (UART, GIC)
pub const ATTR_DEVICE: u64 = 0 << 2;
/// MAIR index 1 = Normal Non-Cacheable
#[allow(dead_code)]
pub const ATTR_NORMAL_NC: u64 = 1 << 2;
/// MAIR index 2 = Normal Write-Back (kernel code/data/stack)
pub const ATTR_NORMAL_WB: u64 = 2 << 2;

// Access Permission (bits [7:6])
/// EL1 Read-Write, EL0 No Access
pub const AP_RW_EL1: u64 = 0b00 << 6;
/// EL1 Read-Only, EL0 No Access
#[allow(dead_code)]
pub const AP_RO_EL1: u64 = 0b10 << 6;
/// EL1 Read-Write, EL0 Read-Write
pub const AP_RW_EL0: u64 = 0b01 << 6;
/// EL1 Read-Only, EL0 Read-Only
#[allow(dead_code)]
pub const AP_RO_EL0: u64 = 0b11 << 6;

// Shareability (bits [9:8])
#[allow(dead_code)]
pub const SH_NON: u64 = 0b00 << 8;
pub const SH_INNER: u64 = 0b11 << 8;

/// Access Flag — MUST be 1 (Cortex-A53 has no HW AF management)
pub const AF: u64 = 1 << 10;

/// Privileged Execute Never
pub const PXN: u64 = 1 << 53;
/// Unprivileged Execute Never
pub const UXN: u64 = 1 << 54;
/// Combined: no execution at any privilege level
pub const XN: u64 = PXN | UXN;

// ─── Composed descriptor templates ────────────────────────────────

/// Device MMIO: Device-nGnRnE, RW, non-executable, AF=1
pub const DEVICE_BLOCK: u64 = BLOCK | ATTR_DEVICE | AP_RW_EL1 | AF | XN;

/// Device MMIO for EL0: Device-nGnRnE, RW for EL0+EL1, non-executable, AF=1
/// Used by map_device_for_task() to grant user-mode access to a device.
pub const DEVICE_BLOCK_EL0: u64 = BLOCK | ATTR_DEVICE | AP_RW_EL0 | AF | XN;

/// Normal RAM: Write-Back, RW, Inner Shareable, AF=1 (executable for sub-phase 1)
pub const RAM_BLOCK: u64 = BLOCK | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF;

/// Kernel code page: Normal WB, RO, executable, Inner Shareable, AF=1
#[allow(dead_code)]
pub const KERNEL_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL1 | SH_INNER | AF;

/// Kernel rodata page: Normal WB, RO (EL0+EL1), non-executable, Inner Shareable, AF=1
/// Must be EL0-accessible because EL0 tasks reference string literals in .rodata
pub const KERNEL_RODATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | XN;

/// Kernel data/bss/stack page: Normal WB, RW, non-executable, Inner Shareable, AF=1
pub const KERNEL_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF | XN;

/// User data/stack page: Normal WB, RW (EL0+EL1), non-executable, Inner Shareable, AF=1
pub const USER_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL0 | SH_INNER | AF | XN;

/// User code page: Normal WB, RO (EL0+EL1), EL0-executable (UXN=0), PXN=1, Inner Shareable, AF=1
/// PXN prevents kernel from executing user code; UXN=0 allows EL0 execution
#[allow(dead_code)]
pub const USER_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | PXN;

/// Shared code page: Normal WB, RO (EL0+EL1), executable by both EL1 and EL0
/// Used for .text section where kernel and task code coexist
pub const SHARED_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF;

// ─── MAIR / TCR constants ─────────────────────────────────────────

/// MAIR_EL1: idx0=Device-nGnRnE(0x00), idx1=Normal-NC(0x44), idx2=Normal-WB(0xFF), idx3=Device-nGnRE(0x04)
#[cfg(target_arch = "aarch64")]
const MAIR_VALUE: u64 = 0x00000000_04FF4400;

/// TCR_EL1 for 39-bit VA, 4KB granule, TTBR0 only
///   T0SZ=25 (bits[5:0])        → 39-bit VA
///   IRGN0=0b01 (bits[9:8])     → Inner WB-WA
///   ORGN0=0b01 (bits[11:10])   → Outer WB-WA
///   SH0=0b11 (bits[13:12])     → Inner Shareable
///   TG0=0b00 (bits[15:14])     → 4KB granule
///   T1SZ=25 (bits[21:16])      → (unused, EPD1=1)
///   EPD1=1 (bit[23])           → Disable TTBR1 walks
///   IPS=0b101 (bits[34:32])    → 48-bit PA
#[cfg(target_arch = "aarch64")]
const TCR_VALUE: u64 =
      25                      // T0SZ
    | (0b01 << 8)             // IRGN0
    | (0b01 << 10)            // ORGN0
    | (0b11 << 12)            // SH0
    | (0b00 << 14)            // TG0 = 4KB
    | (25 << 16)              // T1SZ
    | (1 << 23)               // EPD1 = disable TTBR1
    | (0b01 << 24)            // IRGN1
    | (0b01 << 26)            // ORGN1
    | (0b11 << 28)            // SH1
    | (0b10_u64 << 30)        // TG1 = 4KB
    | (0b101_u64 << 32);      // IPS = 48-bit

/// SCTLR_EL1 bits to SET for MMU enable
#[cfg(target_arch = "aarch64")]
const SCTLR_MMU_ON: u64 =
      (1 << 0)   // M   — MMU enable
    | (1 << 2)   // C   — Data cache enable
    | (1 << 3)   // SA  — SP alignment check
    | (1 << 12); // I   — Instruction cache enable

/// SCTLR_EL1.WXN (bit 19) — Write XOR Execute, for sub-phase 2
#[cfg(target_arch = "aarch64")]
const SCTLR_WXN: u64 = 1 << 19;

// ─── Page table storage (AArch64 only) ─────────────────────────────

/// Number of page table pages (Phase J3: per-task L2_device for MMIO isolation)
/// [0]  = L2_device task 0   [1]  = L2_device task 1   [2]  = L2_device task 2
/// [3]  = L1 for task 0       [4]  = L1 for task 1       [5]  = L1 for task 2
/// [6]  = L2_ram for task 0   [7]  = L2_ram for task 1   [8]  = L2_ram for task 2
/// [9]  = L3 for task 0       [10] = L3 for task 1       [11] = L3 for task 2
/// [12] = L2_device kernel    [13] = L1 kernel boot      [14] = L2_ram kernel boot
/// [15] = L3 kernel boot
pub const NUM_PAGE_TABLE_PAGES: usize = 16;

// Page indices — per-task L2_device (J3)
pub const PT_L2_DEVICE_0: usize = 0;
pub const PT_L2_DEVICE_1: usize = 1;
pub const PT_L2_DEVICE_2: usize = 2;
pub const PT_L1_TASK0: usize = 3;
pub const PT_L1_TASK1: usize = 4;
pub const PT_L1_TASK2: usize = 5;
pub const PT_L2_RAM_TASK0: usize = 6;
pub const PT_L2_RAM_TASK1: usize = 7;
pub const PT_L2_RAM_TASK2: usize = 8;
pub const PT_L3_TASK0: usize = 9;
pub const PT_L3_TASK1: usize = 10;
pub const PT_L3_TASK2: usize = 11;
pub const PT_L2_DEVICE_KERNEL: usize = 12;
pub const PT_L1_KERNEL: usize = 13;
pub const PT_L2_RAM_KERNEL: usize = 14;
pub const PT_L3_KERNEL: usize = 15;

// Linker-provided symbols for page table memory (in .page_tables section)
#[cfg(target_arch = "aarch64")]
extern "C" {
    static __page_tables_start: u8;
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __bss_end: u8;
    static __stack_guard: u8;
    static __stack_start: u8;
    static __stack_end: u8;
    static __kernel_end: u8;
    static __page_tables_end: u8;
    static __user_stacks_start: u8;
    static __user_stacks_end: u8;
    static __task_stacks_start: u8;
    static __task_stacks_end: u8;
    static __grant_pages_start: u8;
    static __grant_pages_end: u8;
}

/// Get address of a linker symbol as usize
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn sym_addr(sym: &u8) -> usize {
    sym as *const u8 as usize
}

/// Pointer to one of the 13 page tables (each 512 × u64 = 4096 bytes)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn table_ptr(index: usize) -> *mut u64 {
    unsafe {
        let base = sym_addr(&__page_tables_start);
        (base + index * 4096) as *mut u64
    }
}

/// Write a page table entry
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn write_entry(table: *mut u64, index: usize, value: u64) {
    ptr::write_volatile(table.add(index), value);
}

// ─── Phase H: Per-task page tables ─────────────────────────────────

/// Build an L2_device table at page index `l2dev_index`.
/// Maps device MMIO at indices 64..=72 (0x0800_0000–0x09FF_FFFF).
/// All entries start as DEVICE_BLOCK (AP_RW_EL1, EL0 no access).
/// map_device_for_task() later upgrades specific entries to DEVICE_BLOCK_EL0.
#[cfg(target_arch = "aarch64")]
unsafe fn build_l2_device(l2dev_index: usize) {
    let l2_device = table_ptr(l2dev_index);
    for i in 64..=72 {
        let pa = (i as u64) * 0x20_0000;
        write_entry(l2_device, i, pa | DEVICE_BLOCK);
    }
}

/// Build an L3 table for a given task.
/// `l3_index` = page index in .page_tables for this L3.
/// `owner_task` = which task (0,1,2) owns this table. 0xFF = kernel boot (all stacks EL1-only).
#[cfg(target_arch = "aarch64")]
unsafe fn build_l3(l3_index: usize, owner_task: u8) {
    let l3 = table_ptr(l3_index);

    let text_start = sym_addr(&__text_start);
    let text_end = sym_addr(&__text_end);
    let rodata_start = sym_addr(&__rodata_start);
    let rodata_end = sym_addr(&__rodata_end);
    let data_start = sym_addr(&__data_start);
    let kernel_end = sym_addr(&__kernel_end);
    let user_stacks_start = sym_addr(&__user_stacks_start);
    let user_stacks_end = sym_addr(&__user_stacks_end);
    let grant_pages_start = sym_addr(&__grant_pages_start);
    let grant_pages_end = sym_addr(&__grant_pages_end);
    let guard_addr = sym_addr(&__stack_guard);

    let base: usize = 0x4000_0000;
    for i in 0..512 {
        let pa = base + i * 4096;

        let desc = if pa == guard_addr {
            // Stack guard page — always invalid
            0
        } else if pa >= user_stacks_start && pa < user_stacks_end {
            // User stack page — per-task isolation
            let stack_idx = (pa - user_stacks_start) / 4096;
            if owner_task == 0xFF {
                // Kernel boot table: all user stacks EL1-only
                (pa as u64) | KERNEL_DATA_PAGE
            } else if stack_idx == owner_task as usize {
                // This task's own stack: EL0 RW
                (pa as u64) | USER_DATA_PAGE
            } else {
                // Other task's stack: EL1-only (EL0 → Permission Fault)
                (pa as u64) | KERNEL_DATA_PAGE
            }
        } else if pa >= grant_pages_start && pa < grant_pages_end {
            // Grant pages — default EL1-only; map_grant_for_task() upgrades to EL0
            (pa as u64) | KERNEL_DATA_PAGE
        } else if pa >= text_start && pa < text_end {
            (pa as u64) | SHARED_CODE_PAGE
        } else if pa >= rodata_start && pa < rodata_end {
            (pa as u64) | KERNEL_RODATA_PAGE
        } else if pa >= data_start && pa < kernel_end {
            (pa as u64) | KERNEL_DATA_PAGE
        } else if pa < text_start {
            (pa as u64) | KERNEL_DATA_PAGE
        } else {
            0
        };

        write_entry(l3, i, desc);
    }
}

/// Build an L2_ram table that points to a specific L3 table.
/// `l2_index` = page index for this L2_ram, `l3_index` = page index for its L3.
#[cfg(target_arch = "aarch64")]
unsafe fn build_l2_ram(l2_index: usize, l3_index: usize) {
    let l2_ram = table_ptr(l2_index);
    let l3 = table_ptr(l3_index);

    // Entry [0] → L3 table (first 2MiB, fine-grained)
    write_entry(l2_ram, 0, (l3 as u64) | TABLE);

    // Entries [1..63] → 2MiB RAM blocks (EL1-only, same as before)
    for i in 1..64 {
        let pa = 0x4000_0000_u64 + (i as u64) * 0x20_0000;
        write_entry(l2_ram, i, pa | RAM_BLOCK);
    }
}

/// Build an L1 table for a specific task (or kernel boot).
/// `l1_index` = page index for this L1, `l2_ram_index` = page index for its L2_ram.
/// `l2_device_index` = page index for this task's L2_device table.
#[cfg(target_arch = "aarch64")]
unsafe fn build_l1(l1_index: usize, l2_ram_index: usize, l2_device_index: usize) {
    let l1 = table_ptr(l1_index);
    let l2_device = table_ptr(l2_device_index);
    let l2_ram = table_ptr(l2_ram_index);

    write_entry(l1, 0, (l2_device as u64) | TABLE);
    write_entry(l1, 1, (l2_ram as u64) | TABLE);
}

/// Get physical address of L1 page table for a task.
/// Returns the base address suitable for TTBR0_EL1 (bits [47:12]).
pub fn page_table_base(task_id: usize) -> u64 {
    // task 0 → page 1, task 1 → page 2, task 2 → page 3
    #[cfg(target_arch = "aarch64")]
    {
        let ptr = table_ptr(PT_L1_TASK0 + task_id);
        ptr as u64
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        // Host tests: return a fake but distinct address per task
        0x8000_0000_u64 + (task_id as u64) * 0x1_0000
    }
}

/// Get the TTBR0_EL1 value for a task: (ASID << 48) | page_table_base.
pub fn ttbr0_for_task(task_id: usize, asid: u16) -> u64 {
    let base = page_table_base(task_id);
    ((asid as u64) << 48) | base
}

// ─── MMU enable sequence (called from assembly) ───────────────────

/// Full MMU initialization — called from boot.s after BSS clear.
/// Phase J3: builds 16 page tables (4 L2_device + 3 per task + 4 kernel boot).
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub unsafe extern "C" fn mmu_init() {
    // Per-task L2_device tables (pages 0, 1, 2) — all devices EL1-only initially
    for task in 0..3_usize {
        build_l2_device(PT_L2_DEVICE_0 + task);
    }
    // Kernel boot L2_device (page 12) — all devices EL1-accessible
    build_l2_device(PT_L2_DEVICE_KERNEL);

    // Per-task tables (task 0, 1, 2)
    for task in 0..3_u8 {
        let t = task as usize;
        build_l3(PT_L3_TASK0 + t, task);
        build_l2_ram(PT_L2_RAM_TASK0 + t, PT_L3_TASK0 + t);
        build_l1(PT_L1_TASK0 + t, PT_L2_RAM_TASK0 + t, PT_L2_DEVICE_0 + t);
    }

    // Kernel boot tables (owner_task = 0xFF → all user stacks EL1-only)
    build_l3(PT_L3_KERNEL, 0xFF);
    build_l2_ram(PT_L2_RAM_KERNEL, PT_L3_KERNEL);
    build_l1(PT_L1_KERNEL, PT_L2_RAM_KERNEL, PT_L2_DEVICE_KERNEL);

    // Flush page table writes to memory
    core::arch::asm!(
        "dsb ish",
        "isb",
        options(nomem, nostack)
    );
}

// ─── Phase J3: Device MMIO mapping ─────────────────────────────────

/// Device registry — whitelisted devices that can be mapped for EL0 tasks.
/// GIC MMIO (L2 indices 64–66) is NEVER exposed — only safe peripherals.
pub struct DeviceInfo {
    /// L2 entry index (e.g., 72 for UART at 0x0900_0000)
    pub l2_index: usize,
    /// Hardware INTID for IRQ routing (e.g., 33 for UART0)
    pub intid: u32,
    /// Human-readable name
    pub name: &'static str,
}

/// Device table — device_id indexes into this array.
pub const DEVICES: &[DeviceInfo] = &[
    DeviceInfo { l2_index: 72, intid: 33, name: "UART0" }, // device_id=0
];

/// Maximum device_id (for host tests)
pub const MAX_DEVICE_ID: usize = 0; // only UART0 for now

// Error codes for map_device_for_task
pub const DEVICE_MAP_ERR_INVALID_ID: u64 = 0xFFFF_2001;
pub const DEVICE_MAP_ERR_INVALID_TASK: u64 = 0xFFFF_2002;

/// Map a device's MMIO region into a task's L2_device table as DEVICE_BLOCK_EL0.
/// This allows the EL0 task to directly read/write the device's MMIO registers.
///
/// Safety: device_id must index into DEVICES. GIC is never exposed.
#[cfg(target_arch = "aarch64")]
pub unsafe fn map_device_for_task(device_id: u64, task_id: usize) -> u64 {
    let did = device_id as usize;
    if did >= DEVICES.len() {
        crate::uart_print("!!! DEVICE MAP: invalid device_id\n");
        return DEVICE_MAP_ERR_INVALID_ID;
    }
    if task_id >= 3 {
        crate::uart_print("!!! DEVICE MAP: invalid task_id\n");
        return DEVICE_MAP_ERR_INVALID_TASK;
    }

    let dev = &DEVICES[did];
    let l2_device = table_ptr(PT_L2_DEVICE_0 + task_id);
    let pa = (dev.l2_index as u64) * 0x20_0000;

    // Upgrade entry from DEVICE_BLOCK (EL1-only) to DEVICE_BLOCK_EL0 (EL0 accessible)
    write_entry(l2_device, dev.l2_index, pa | DEVICE_BLOCK_EL0);

    // TLB invalidate for this task's ASID
    let asid = (task_id as u64 + 1) << 48;
    core::arch::asm!(
        "tlbi aside1is, {asid}",
        "dsb ish",
        "isb",
        asid = in(reg) asid,
        options(nomem, nostack)
    );

    crate::uart_print("[AegisOS] DEVICE MAP: ");
    crate::uart_print(dev.name);
    crate::uart_print(" -> task ");
    crate::uart_print_hex(task_id as u64);
    crate::uart_print("\n");

    0 // success
}

/// Host-test stub for map_device_for_task
#[cfg(not(target_arch = "aarch64"))]
pub fn map_device_for_task(device_id: u64, task_id: usize) -> u64 {
    let did = device_id as usize;
    if did >= DEVICES.len() {
        return DEVICE_MAP_ERR_INVALID_ID;
    }
    if task_id >= 3 {
        return DEVICE_MAP_ERR_INVALID_TASK;
    }
    0 // success
}

// ─── Phase J1: Grant page mapping ──────────────────────────────────

/// Map a grant page into a task's L3 table as AP_RW_EL0 (user accessible).
/// Must be followed by TLB invalidation for the task's ASID.
#[cfg(target_arch = "aarch64")]
pub unsafe fn map_grant_for_task(grant_phys: u64, task_id: usize) {
    let l3 = table_ptr(PT_L3_TASK0 + task_id);
    let base: u64 = 0x4000_0000;
    let index = ((grant_phys - base) / 4096) as usize;
    if index < 512 {
        write_entry(l3, index, grant_phys | USER_DATA_PAGE);
        // TLB invalidate for this task's ASID
        let asid = (task_id as u64 + 1) << 48;
        core::arch::asm!(
            "tlbi aside1is, {asid}",
            "dsb ish",
            "isb",
            asid = in(reg) asid,
            options(nomem, nostack)
        );
    }
}

/// Unmap a grant page from a task's L3 table (revert to AP_RW_EL1, EL0 no access).
/// Must be followed by TLB invalidation for the task's ASID.
#[cfg(target_arch = "aarch64")]
pub unsafe fn unmap_grant_for_task(grant_phys: u64, task_id: usize) {
    let l3 = table_ptr(PT_L3_TASK0 + task_id);
    let base: u64 = 0x4000_0000;
    let index = ((grant_phys - base) / 4096) as usize;
    if index < 512 {
        write_entry(l3, index, grant_phys | KERNEL_DATA_PAGE);
        // TLB invalidate for this task's ASID
        let asid = (task_id as u64 + 1) << 48;
        core::arch::asm!(
            "tlbi aside1is, {asid}",
            "dsb ish",
            "isb",
            asid = in(reg) asid,
            options(nomem, nostack)
        );
    }
}

/// Enable MMU — called from assembly after mmu_init()
/// This is kept in Rust for the register constant values, but the actual
/// MSR sequence is in boot.s for precise control over instruction ordering.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub unsafe extern "C" fn mmu_get_config(out: *mut [u64; 4]) {
    // Kernel boot L1 (page 13) — no EL0 user stack access
    let l1_kernel = table_ptr(PT_L1_KERNEL);
    (*out)[0] = MAIR_VALUE;
    (*out)[1] = TCR_VALUE;
    (*out)[2] = l1_kernel as u64; // TTBR0 = kernel boot table
    (*out)[3] = SCTLR_MMU_ON | SCTLR_WXN;
}

```

## File ./src\sched.rs:
```rust
/// Thời Khóa Biểu / Bộ lập lịch (Scheduler).
/// AegisOS Scheduler — Round-Robin, 3 static tasks
///
/// Tasks run at EL0 (user mode). Each task has:
///   - A TrapFrame (saved/restored on context switch)
///   - Its own 4KB kernel stack (SP_EL1, in .task_stacks section)
///   - Its own 4KB user stack (SP_EL0, in .user_stacks section)
///   - A state (Ready, Running, Inactive)
///
/// Context switch: timer IRQ → save frame → pick next Ready → switch SP_EL1 → load frame → eret to EL0

use crate::cap::CapBits;
use crate::exception::TrapFrame;
use crate::uart_print;

// ─── Task state ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum TaskState {
    Inactive = 0,
    Ready    = 1,
    Running  = 2,
    Blocked  = 3,
    Faulted  = 4,
}

// ─── Task Control Block ────────────────────────────────────────────

/// TCB — one per task. Context is saved/loaded during context switch.
#[repr(C)]
pub struct Tcb {
    pub context: TrapFrame,
    pub state: TaskState,
    pub id: u16,
    pub stack_top: u64,       // top of this task's kernel stack (SP_EL1)
    pub entry_point: u64,     // original entry address (for restart)
    pub user_stack_top: u64,  // original SP_EL0 top (for restart)
    pub fault_tick: u64,      // tick when task was marked Faulted
    pub caps: CapBits,        // capability bitmask (survives restart)
    pub ttbr0: u64,           // TTBR0_EL1 value (ASID << 48 | L1 base)
    pub notify_pending: u64,  // bitmask of pending notification bits
    pub notify_waiting: bool, // true if task is blocked in wait_notify
}

// ─── Static task table ─────────────────────────────────────────────

/// 3 tasks: 0 = task_a, 1 = task_b, 2 = idle
pub const NUM_TASKS: usize = 3;

pub static mut TCBS: [Tcb; NUM_TASKS] = [EMPTY_TCB; NUM_TASKS];
pub static mut CURRENT: usize = 0;

/// Delay before auto-restarting a faulted task (100 ticks × 10ms = 1 second)
pub const RESTART_DELAY_TICKS: u64 = 100;

pub const EMPTY_TCB: Tcb = Tcb {
    context: TrapFrame {
        x: [0; 31],
        sp_el0: 0,
        elr_el1: 0,
        spsr_el1: 0,
        _pad: [0; 2],
    },
    state: TaskState::Inactive,
    id: 0,
    stack_top: 0,
    entry_point: 0,
    user_stack_top: 0,
    fault_tick: 0,
    caps: 0,
    ttbr0: 0,
    notify_pending: 0,
    notify_waiting: false,
};

// ─── Public API ────────────────────────────────────────────────────

/// Initialize scheduler: set up TCBs for task_a, task_b, idle.
/// Must be called before enabling timer interrupts.
#[cfg(target_arch = "aarch64")]
pub fn init(
    task_a_entry: u64,
    task_b_entry: u64,
    idle_entry: u64,
) {
    extern "C" {
        static __task_stacks_start: u8;  // kernel stacks (SP_EL1)
        static __user_stacks_start: u8;  // user stacks (SP_EL0)
    }

    let kstacks_base = unsafe { &__task_stacks_start as *const u8 as u64 };
    let ustacks_base = unsafe { &__user_stacks_start as *const u8 as u64 };

    // Each stack is 4KB. Stack grows downward, so top = base + (i+1)*4096
    // SPSR = 0x000 = EL0t: eret drops to EL0, uses SP_EL0
    // When exception from EL0 → EL1, CPU automatically uses SP_EL1
    unsafe {
        // Task 0: task_a
        TCBS[0].id = 0;
        TCBS[0].state = TaskState::Ready;
        TCBS[0].stack_top = kstacks_base + 1 * 4096;
        TCBS[0].entry_point = task_a_entry;
        TCBS[0].user_stack_top = ustacks_base + 1 * 4096;
        TCBS[0].context.elr_el1 = task_a_entry;
        TCBS[0].context.spsr_el1 = 0x000; // EL0t
        TCBS[0].context.sp_el0 = ustacks_base + 1 * 4096;

        // Task 1: task_b
        TCBS[1].id = 1;
        TCBS[1].state = TaskState::Ready;
        TCBS[1].stack_top = kstacks_base + 2 * 4096;
        TCBS[1].entry_point = task_b_entry;
        TCBS[1].user_stack_top = ustacks_base + 2 * 4096;
        TCBS[1].context.elr_el1 = task_b_entry;
        TCBS[1].context.spsr_el1 = 0x000; // EL0t
        TCBS[1].context.sp_el0 = ustacks_base + 2 * 4096;

        // Task 2: idle
        TCBS[2].id = 2;
        TCBS[2].state = TaskState::Ready;
        TCBS[2].stack_top = kstacks_base + 3 * 4096;
        TCBS[2].entry_point = idle_entry;
        TCBS[2].user_stack_top = ustacks_base + 3 * 4096;
        TCBS[2].context.elr_el1 = idle_entry;
        TCBS[2].context.spsr_el1 = 0x000; // EL0t
        TCBS[2].context.sp_el0 = ustacks_base + 3 * 4096;
    }

    uart_print("[AegisOS] scheduler ready (3 tasks, EL0)\n");
}

/// Schedule: save current context, pick next Ready task, load its context.
/// Called from timer IRQ handler with the current TrapFrame.
///
/// The trick: we modify `*frame` in-place. When the IRQ handler does
/// RESTORE_CONTEXT, it restores the NEW task's registers, and `eret`
/// jumps to the new task's ELR — completing the context switch.
///
/// SP_EL1 switching: We update CURRENT_KSTACK with the new task's kernel
/// stack top. The RESTORE_CONTEXT_EL0 macro reads this and sets SP
/// before eret, so the next exception from EL0 uses the correct stack.
pub fn schedule(frame: &mut TrapFrame) {
    unsafe {
        let old = CURRENT;

        // Save current task's context from the TrapFrame
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut TCBS[old].context as *mut TrapFrame,
            1,
        );

        // Mark old task as Ready (unless it's Blocked or Faulted)
        if TCBS[old].state == TaskState::Running {
            TCBS[old].state = TaskState::Ready;
        }

        // Auto-restart: check if any Faulted task has waited long enough
        let now = crate::timer::tick_count();
        for i in 0..NUM_TASKS {
            if TCBS[i].state == TaskState::Faulted
                && now.wrapping_sub(TCBS[i].fault_tick) >= RESTART_DELAY_TICKS
            {
                restart_task(i);
            }
        }

        // Round-robin: find next Ready task
        let mut next = (old + 1) % NUM_TASKS;
        let mut found = false;
        for _ in 0..NUM_TASKS {
            if TCBS[next].state == TaskState::Ready {
                found = true;
                break;
            }
            next = (next + 1) % NUM_TASKS;
        }

        if !found {
            // No ready task — force-restart idle (index 2) if faulted
            next = 2;
            if TCBS[2].state == TaskState::Faulted {
                restart_task(2);
            }
            TCBS[2].state = TaskState::Ready;
        }

        // Switch to new task
        TCBS[next].state = TaskState::Running;
        CURRENT = next;

        // Load new task's context into the frame.
        core::ptr::copy_nonoverlapping(
            &TCBS[next].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );

        // Phase H: Switch TTBR0 to the new task's page table
        #[cfg(target_arch = "aarch64")]
        {
            let ttbr0 = TCBS[next].ttbr0;
            core::arch::asm!(
                "msr ttbr0_el1, {val}",
                "isb",
                val = in(reg) ttbr0,
                options(nomem, nostack)
            );
        }
    }
}

/// Get current task ID
pub fn current_task_id() -> u16 {
    unsafe { TCBS[CURRENT].id }
}

/// Set task state (used by IPC to block/unblock tasks)
pub fn set_task_state(task_idx: usize, state: TaskState) {
    if task_idx < NUM_TASKS {
        unsafe { TCBS[task_idx].state = state; }
    }
}

/// Get a register value from a task's saved context
pub fn get_task_reg(task_idx: usize, reg: usize) -> u64 {
    unsafe { TCBS[task_idx].context.x[reg] }
}

/// Set a register value in a task's saved context
pub fn set_task_reg(task_idx: usize, reg: usize, val: u64) {
    unsafe { TCBS[task_idx].context.x[reg] = val; }
}

/// Save the current TrapFrame into a task's TCB context
pub fn save_frame(task_idx: usize, frame: &TrapFrame) {
    unsafe {
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut TCBS[task_idx].context as *mut TrapFrame,
            1,
        );
    }
}

/// Load a task's TCB context into the TrapFrame
pub fn load_frame(task_idx: usize, frame: &mut TrapFrame) {
    unsafe {
        core::ptr::copy_nonoverlapping(
            &TCBS[task_idx].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );
    }
}

/// Mark the currently running task as Faulted, cleanup IPC, and schedule away.
/// Called from exception handlers when a lower-EL fault is recoverable.
pub fn fault_current_task(frame: &mut TrapFrame) {
    unsafe {
        let current = CURRENT;
        let id = TCBS[current].id;

        uart_print("[AegisOS] TASK ");
        crate::uart_print_hex(id as u64);
        uart_print(" FAULTED\n");

        TCBS[current].state = TaskState::Faulted;
        TCBS[current].fault_tick = crate::timer::tick_count();

        // Clean up IPC endpoints — unblock any partner waiting for this task
        crate::ipc::cleanup_task(current);

        // Clean up shared memory grants — revoke all grants involving this task
        crate::grant::cleanup_task(current);

        // Clean up IRQ bindings — unbind all IRQs owned by this task
        crate::irq::irq_cleanup_task(current);

        // Schedule away to the next ready task
        schedule(frame);
    }
}

/// Restart a faulted task: zero context, reload entry point + stack, mark Ready.
/// Called from schedule() when restart delay has elapsed.
pub fn restart_task(task_idx: usize) {
    unsafe {
        if TCBS[task_idx].state != TaskState::Faulted {
            return;
        }

        let id = TCBS[task_idx].id;

        // Zero user stack (4KB) to prevent state leakage
        // Only on AArch64 — on host tests, user_stack_top is a fake address
        #[cfg(target_arch = "aarch64")]
        {
            let ustack_top = TCBS[task_idx].user_stack_top;
            let ustack_base = (ustack_top - 4096) as *mut u8;
            core::ptr::write_bytes(ustack_base, 0, 4096);
        }

        // Zero entire TrapFrame
        core::ptr::write_bytes(
            &mut TCBS[task_idx].context as *mut TrapFrame as *mut u8,
            0,
            core::mem::size_of::<TrapFrame>(),
        );

        // Reload entry point, stack, SPSR
        TCBS[task_idx].context.elr_el1 = TCBS[task_idx].entry_point;
        TCBS[task_idx].context.spsr_el1 = 0x000; // EL0t
        TCBS[task_idx].context.sp_el0 = TCBS[task_idx].user_stack_top;

        TCBS[task_idx].state = TaskState::Ready;
        TCBS[task_idx].notify_pending = 0;
        TCBS[task_idx].notify_waiting = false;

        uart_print("[AegisOS] TASK ");
        crate::uart_print_hex(id as u64);
        uart_print(" RESTARTED\n");
    }
}

/// Bootstrap: load first task's context and eret into EL0.
/// This never returns — it erets into task_a at EL0.
///
/// SPSR = 0x000 (EL0t) means eret drops to EL0 using SP_EL0.
/// SP_EL1 stays at __stack_end (the shared kernel boot stack).
/// SAVE_CONTEXT_LOWER reloads SP to __stack_end on every exception
/// entry, so the bootstrap SP value doesn't matter after this point.
#[cfg(target_arch = "aarch64")]
pub fn bootstrap() -> ! {
    unsafe {
        TCBS[0].state = TaskState::Running;
        CURRENT = 0;

        let frame = &TCBS[0].context;
        let ttbr0 = TCBS[0].ttbr0;

        // Load the task's context into registers and eret into EL0
        core::arch::asm!(
            // Phase H: Switch TTBR0 to task 0's per-task page table
            "msr ttbr0_el1, {ttbr0}",
            "isb",
            // Set ELR_EL1 = task entry, SPSR_EL1 = 0x000 (EL0t)
            "msr elr_el1, {elr}",
            "msr spsr_el1, {spsr}",
            // Set SP_EL0 = user stack (task will use this at EL0)
            "msr sp_el0, {sp0}",
            // eret: CPU restores PSTATE from SPSR (EL0t), PC from ELR.
            // Task runs at EL0 with SP = SP_EL0 (user stack).
            "eret",
            ttbr0 = in(reg) ttbr0,
            elr = in(reg) frame.elr_el1,
            spsr = in(reg) frame.spsr_el1,
            sp0 = in(reg) frame.sp_el0,
            options(noreturn)
        );
    }
}

```

## File ./src\timer.rs:
```rust
/// Đồng hồ hệ thống (Timer)
/// AegisOS Timer — ARM Generic Timer (CNTP_EL0)
///
/// Uses the EL1 Physical Timer (CNTP) with PPI INTID 30.
/// QEMU virt timer frequency: 62,500,000 Hz (62.5 MHz).

#[cfg(target_arch = "aarch64")]
use crate::uart_print;

/// GIC INTID for EL1 Physical Timer (PPI 14)
pub const TIMER_INTID: u32 = 30;

/// Tick interval in ticks (computed at init)
#[cfg(target_arch = "aarch64")]
static mut TICK_INTERVAL: u64 = 0;

/// Monotonic tick counter
pub static mut TICK_COUNT: u64 = 0;

/// Initialize timer for periodic ticks
/// `tick_ms` = interval in milliseconds (e.g., 10 for 10ms)
#[cfg(target_arch = "aarch64")]
pub fn init(tick_ms: u32) {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, CNTFRQ_EL0", out(reg) freq, options(nomem, nostack));
    }

    let ticks = freq * (tick_ms as u64) / 1000;
    unsafe { TICK_INTERVAL = ticks; }

    // Set countdown value
    unsafe {
        core::arch::asm!(
            "msr CNTP_TVAL_EL0, {t}",
            t = in(reg) ticks,
            options(nomem, nostack)
        );
    }

    // Enable timer, unmask interrupt (ENABLE=1, IMASK=0)
    unsafe {
        core::arch::asm!(
            "mov x0, #1",
            "msr CNTP_CTL_EL0, x0",
            out("x0") _,
            options(nomem, nostack)
        );
    }

    uart_print("[AegisOS] timer started (");
    // Print tick_ms as simple decimal
    print_decimal(tick_ms);
    uart_print("ms, freq=");
    print_decimal(freq as u32 / 1_000_000);
    uart_print("MHz)\n");
}

/// Re-arm timer — call from IRQ handler
#[cfg(target_arch = "aarch64")]
pub fn rearm() {
    let ticks = unsafe { TICK_INTERVAL };
    unsafe {
        core::arch::asm!(
            "msr CNTP_TVAL_EL0, {t}",
            t = in(reg) ticks,
            options(nomem, nostack)
        );
    }
}

/// Timer tick handler — called from IRQ dispatch with TrapFrame
#[cfg(target_arch = "aarch64")]
pub fn tick_handler(frame: &mut crate::exception::TrapFrame) {
    unsafe { TICK_COUNT += 1; }

    // Re-arm for next tick
    rearm();

    // Context switch via scheduler
    crate::sched::schedule(frame);
}

/// Get current tick count
#[allow(dead_code)]
pub fn tick_count() -> u64 {
    unsafe { TICK_COUNT }
}

/// Simple decimal printer for small numbers
#[cfg(target_arch = "aarch64")]
fn print_decimal(mut val: u32) {
    if val == 0 {
        crate::uart_write(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        crate::uart_write(buf[i]);
    }
}

```

## File ./src\uart.rs:
```rust
/// AegisOS UART driver — PL011 on QEMU virt machine
///
/// UART0 data register at 0x0900_0000. Write-only for simplicity.
/// On host (non-AArch64), UART functions are no-ops for testing.

#[cfg(target_arch = "aarch64")]
use core::ptr;

/// UART0 PL011 data register on QEMU virt machine
#[cfg(target_arch = "aarch64")]
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

/// Write a single byte to UART
#[cfg(target_arch = "aarch64")]
pub fn uart_write(byte: u8) {
    unsafe { ptr::write_volatile(UART0, byte) }
}

/// No-op on host (tests don't have UART)
#[cfg(not(target_arch = "aarch64"))]
pub fn uart_write(_byte: u8) {}

/// Print a string to UART
pub fn uart_print(s: &str) {
    for b in s.bytes() {
        uart_write(b);
    }
}

/// Print a u64 value as hexadecimal to UART
pub fn uart_print_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart_write(hex[nibble]);
    }
}

```

