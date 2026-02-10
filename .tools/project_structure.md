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
    ├── exception.rs
    ├── gic.rs
    ├── ipc.rs
    ├── main.rs
    ├── mmu.rs
    ├── sched.rs
    └── timer.rs
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

    /* === Page Tables (4 × 4096 = 16 KiB, 4KB-aligned) === */
    . = ALIGN(4096);
    __page_tables_start = .;
    .page_tables (NOLOAD) : {
        . += 4 * 4096;
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
    ldr x0, =__stack_end
    mov sp, x0

    /* Check EL — QEMU virt may start at EL2 or EL1 */
    mrs x0, CurrentEL
    lsr x0, x0, #2
    cmp x0, #2
    b.ne at_el1

    /* === Drop from EL2 to EL1 === */
    mrs x0, hcr_el2
    orr x0, x0, #(1 << 31)   /* HCR_EL2.RW = 1 (EL1 is AArch64) */
    msr hcr_el2, x0

    mov x0, #0x33FF
    msr cptr_el2, x0
    msr hstr_el2, xzr

    /* SCTLR_EL1 reset value */
    mov x0, #0x0800
    movk x0, #0x30D0, lsl #16
    msr sctlr_el1, x0

    /* Enable EL1 physical timer access from EL2 */
    mrs x0, CNTHCTL_EL2
    orr x0, x0, #3        /* EL1PCTEN + EL1PCEN */
    msr CNTHCTL_EL2, x0
    msr CNTVOFF_EL2, xzr  /* Zero virtual offset */

    /* Return to EL1h */
    mov x0, #0x3C5
    msr spsr_el2, x0
    adr x0, at_el1
    msr elr_el2, x0
    eret

at_el1:
    /* Re-setup SP (SP_EL1 after eret) */
    ldr x0, =__stack_end
    mov sp, x0

    /* Clear BSS + page tables */
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
    bl  mmu_init

    /* Invalidate all TLB entries */
    tlbi vmalle1
    dsb  ish
    isb

    /* MAIR_EL1: idx0=Device-nGnRnE(0x00), idx1=Normal-NC(0x44),
                  idx2=Normal-WB(0xFF), idx3=Device-nGnRE(0x04) */
    ldr x0, =0x04FF4400
    msr mair_el1, x0

    /* TCR_EL1: 39-bit VA, 4KB granule, TTBR0 only, 48-bit PA */
    ldr x0, =0x5B5993519
    msr tcr_el1, x0

    /* TTBR0_EL1 = L1 page table base */
    ldr x0, =__page_tables_start
    msr ttbr0_el1, x0

    isb

    /* Enable MMU: set M + C + SA + I + WXN in SCTLR_EL1 */
    mrs x0, sctlr_el1
    ldr x1, =0x0008100D
    orr x0, x0, x1
    msr sctlr_el1, x0
    isb

    /* MMU is now active — jump to Rust */
    bl  kernel_main

4:
    wfe
    b 4b

```

## File ./src\exception.rs:
```rust
/// AegisOS Exception Handling — AArch64
///
/// Full context save/restore (288-byte TrapFrame), ESR_EL1 dispatch,
/// separate Sync/IRQ paths. TrapFrame layout is ABI-fixed for Phase C.

use crate::uart_print;
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
#[no_mangle]
pub extern "C" fn exception_dispatch_irq(frame: &mut TrapFrame) {
    let intid = crate::gic::acknowledge();

    if intid == crate::gic::INTID_SPURIOUS {
        return; // spurious, ignore
    }

    match intid {
        crate::timer::TIMER_INTID => crate::timer::tick_handler(frame),
        _ => {
            uart_print("!!! IRQ INTID=");
            uart_print_hex(intid as u64);
            uart_print(" (unhandled) !!!\n");
        }
    }

    crate::gic::end_interrupt(intid);
}

/// SError dispatch — always fatal
#[no_mangle]
pub extern "C" fn exception_dispatch_serror(_frame: &mut TrapFrame) {
    uart_print("\n!!! SERROR (fatal) !!!\n");
    // Will halt in assembly stub after return
}

// ─── Individual exception handlers ─────────────────────────────────

/// SVC handler — dispatch syscalls by x7
fn handle_svc(frame: &mut TrapFrame, _esr: u64) {
    let syscall_nr = frame.x[7];

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
fn handle_sys_write(frame: &TrapFrame) {
    let buf_ptr = frame.x[0] as usize;
    let len = frame.x[1] as usize;

    // Validate: max 256 bytes per write, non-zero length
    if len == 0 || len > 256 {
        return;
    }

    // Validate: buffer must be in the kernel image range (shared code/rodata)
    // or in user stack range. For now, allow any address in the identity-mapped
    // RAM region (0x4000_0000 – 0x4800_0000) since all tasks share address space.
    let buf_end = buf_ptr.wrapping_add(len);
    if buf_ptr < 0x4000_0000 || buf_end > 0x4800_0000 || buf_end < buf_ptr {
        uart_print("!!! SYS_WRITE: bad pointer !!!\n");
        return;
    }

    // Safe to read — copy bytes to UART
    for i in 0..len {
        let byte = unsafe { core::ptr::read_volatile((buf_ptr + i) as *const u8) };
        crate::uart_write(byte);
    }
}

/// Instruction Abort handler — fault task if from lower EL, halt if from same EL
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

## File ./src\ipc.rs:
```rust
/// AegisOS IPC — Synchronous Endpoint-based messaging
///
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

const MAX_ENDPOINTS: usize = 2;
const MSG_REGS: usize = 4; // x[0]..x[3]

// ─── Endpoint ──────────────────────────────────────────────────────

/// An IPC endpoint. One task can be waiting to send, one to receive.
/// For simplicity: single-slot (one waiter per direction).
struct Endpoint {
    /// Task blocked waiting to send on this endpoint (None = no waiter)
    sender: Option<usize>,
    /// Task blocked waiting to receive on this endpoint (None = no waiter)
    receiver: Option<usize>,
}

const EMPTY_EP: Endpoint = Endpoint {
    sender: None,
    receiver: None,
};

static mut ENDPOINTS: [Endpoint; MAX_ENDPOINTS] = [EMPTY_EP; MAX_ENDPOINTS];

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
            // No receiver — block sender and wait
            ENDPOINTS[ep_id].sender = Some(current);
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

        if let Some(send_task) = ENDPOINTS[ep_id].sender.take() {
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
            // No receiver — block as sender, will also need reply
            ENDPOINTS[ep_id].sender = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────

/// Copy message registers x[0]..x[3] from sender's TCB to receiver's TCB.
unsafe fn copy_message(from_task: usize, to_task: usize) {
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
            // If the faulted task was a pending sender, clear the slot
            if ENDPOINTS[i].sender == Some(task_idx) {
                ENDPOINTS[i].sender = None;
            }

            // If the faulted task was a pending receiver, clear the slot
            if ENDPOINTS[i].receiver == Some(task_idx) {
                ENDPOINTS[i].receiver = None;
            }
        }

        // Also check: is any other task Blocked because it was waiting
        // for a rendezvous with the faulted task? In synchronous IPC,
        // a task blocks *on an endpoint*, not on a specific partner.
        // So if partner is blocked on ep.sender/receiver, it stays blocked
        // until another task does the complementary operation.
        // This is correct for synchronous IPC — partner will be unblocked
        // when the faulted task restarts and re-enters IPC.
    }
}

```

## File ./src\main.rs:
```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

mod mmu;
mod exception;
mod gic;
mod timer;
mod sched;
mod ipc;

// Boot assembly — inline vào binary thông qua global_asm!
core::arch::global_asm!(include_str!("boot.s"));

/// UART0 PL011 data register trên QEMU virt machine
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

pub fn uart_write(byte: u8) {
    unsafe { ptr::write_volatile(UART0, byte) }
}

pub fn uart_print(s: &str) {
    for b in s.bytes() {
        uart_write(b);
    }
}

/// Print a u64 value as hexadecimal
pub fn uart_print_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart_write(hex[nibble]);
    }
}

// ─── Syscall wrappers ──────────────────────────────────────────────

/// SYS_YIELD (syscall #0): voluntarily yield the CPU to the next task.
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
/// msg[0..4] in x0..x3, ep_id in x6, syscall# in x7.
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
/// Returns msg[0] (first message word).
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

/// SYS_CALL (syscall #3): send message then wait for reply.
/// Returns msg[0] from reply.
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
/// buf = pointer to string data, len = byte count.
/// Used by EL0 tasks that cannot access UART directly.
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
#[inline(always)]
pub fn user_print(s: &str) {
    syscall_write(s.as_ptr(), s.len());
}

// ─── Task entry points ─────────────────────────────────────────────

/// Task A (client): send "PING" on EP 0, receive reply
#[no_mangle]
pub extern "C" fn task_a_entry() -> ! {
    loop {
        // Send PING (msg[0] = 0x50494E47 = "PING" in ASCII hex)
        user_print("A:PING ");
        syscall_call(0, 0x50494E47, 0, 0, 0);
    }
}

/// Task B (server): receive on EP 0, send PONG reply
#[no_mangle]
pub extern "C" fn task_b_entry() -> ! {
    loop {
        // Receive message
        let _msg = syscall_recv(0);
        user_print("B:PONG ");
        // Reply by sending back on same endpoint
        syscall_send(0, 0x504F4E47, 0, 0, 0); // "PONG"
    }
}

/// Idle task: just wfi in a loop
#[no_mangle]
pub extern "C" fn idle_entry() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

// ─── Kernel main ───────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart_print("\n[AegisOS] boot\n");
    uart_print("[AegisOS] MMU enabled (identity map)\n");
    uart_print("[AegisOS] W^X enforced (WXN + 4KB pages)\n");

    // Install exception vector table
    exception::init();
    uart_print("[AegisOS] exceptions ready\n");

    // Initialize GIC + enable timer interrupt
    gic::init();
    gic::set_priority(timer::TIMER_INTID, 0);
    gic::enable_intid(timer::TIMER_INTID);

    // Initialize scheduler with task entry points
    sched::init(
        task_a_entry as *const () as u64,
        task_b_entry as *const () as u64,
        idle_entry as *const () as u64,
    );

    // Start timer: 10ms periodic tick (IRQ still masked — won't fire yet)
    timer::init(10);

    uart_print("[AegisOS] bootstrapping into task_a (EL0)...\n");

    // Bootstrap: load task_a context and eret into it — never returns.
    // The eret restores SPSR = 0x000 (EL0t), dropping to user mode.
    // SAVE_CONTEXT_LOWER reloads SP to __stack_end on every exception entry.
    sched::bootstrap();
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}

```

## File ./src\mmu.rs:
```rust
/// Memory Management Unit (Bộ phận Quản lý Bộ nhớ).
/// AegisOS MMU — AArch64 Page Table Setup
///
/// Sub-phase 1: Identity map with 2 MiB blocks
/// Sub-phase 2: Refine to 4KB pages for kernel region, W^X enforcement

use core::ptr;

// ─── Descriptor bits ───────────────────────────────────────────────

/// L1/L2 table descriptor — points to next-level table
const TABLE: u64 = 0b11;

/// L1/L2 block descriptor — maps a large region directly
const BLOCK: u64 = 0b01;

/// L3 page descriptor
const PAGE: u64 = 0b11;

// AttrIndx — index into MAIR_EL1 (bits [4:2])
/// MAIR index 0 = Device-nGnRnE (UART, GIC)
const ATTR_DEVICE: u64 = 0 << 2;
/// MAIR index 1 = Normal Non-Cacheable
#[allow(dead_code)]
const ATTR_NORMAL_NC: u64 = 1 << 2;
/// MAIR index 2 = Normal Write-Back (kernel code/data/stack)
const ATTR_NORMAL_WB: u64 = 2 << 2;

// Access Permission (bits [7:6])
/// EL1 Read-Write, EL0 No Access
const AP_RW_EL1: u64 = 0b00 << 6;
/// EL1 Read-Only, EL0 No Access
#[allow(dead_code)]
const AP_RO_EL1: u64 = 0b10 << 6;
/// EL1 Read-Write, EL0 Read-Write
const AP_RW_EL0: u64 = 0b01 << 6;
/// EL1 Read-Only, EL0 Read-Only
#[allow(dead_code)]
const AP_RO_EL0: u64 = 0b11 << 6;

// Shareability (bits [9:8])
#[allow(dead_code)]
const SH_NON: u64 = 0b00 << 8;
const SH_INNER: u64 = 0b11 << 8;

/// Access Flag — MUST be 1 (Cortex-A53 has no HW AF management)
const AF: u64 = 1 << 10;

/// Privileged Execute Never
const PXN: u64 = 1 << 53;
/// Unprivileged Execute Never
const UXN: u64 = 1 << 54;
/// Combined: no execution at any privilege level
const XN: u64 = PXN | UXN;

// ─── Composed descriptor templates ────────────────────────────────

/// Device MMIO: Device-nGnRnE, RW, non-executable, AF=1
const DEVICE_BLOCK: u64 = BLOCK | ATTR_DEVICE | AP_RW_EL1 | AF | XN;

/// Normal RAM: Write-Back, RW, Inner Shareable, AF=1 (executable for sub-phase 1)
const RAM_BLOCK: u64 = BLOCK | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF;

/// Kernel code page: Normal WB, RO, executable, Inner Shareable, AF=1
#[allow(dead_code)]
const KERNEL_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL1 | SH_INNER | AF;

/// Kernel rodata page: Normal WB, RO (EL0+EL1), non-executable, Inner Shareable, AF=1
/// Must be EL0-accessible because EL0 tasks reference string literals in .rodata
const KERNEL_RODATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | XN;

/// Kernel data/bss/stack page: Normal WB, RW, non-executable, Inner Shareable, AF=1
const KERNEL_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF | XN;

/// User data/stack page: Normal WB, RW (EL0+EL1), non-executable, Inner Shareable, AF=1
const USER_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL0 | SH_INNER | AF | XN;

/// User code page: Normal WB, RO (EL0+EL1), EL0-executable (UXN=0), PXN=1, Inner Shareable, AF=1
/// PXN prevents kernel from executing user code; UXN=0 allows EL0 execution
#[allow(dead_code)]
const USER_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | PXN;

/// Shared code page: Normal WB, RO (EL0+EL1), executable by both EL1 and EL0
/// Used for .text section where kernel and task code coexist
const SHARED_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF;

// ─── MAIR / TCR constants ─────────────────────────────────────────

/// MAIR_EL1: idx0=Device-nGnRnE(0x00), idx1=Normal-NC(0x44), idx2=Normal-WB(0xFF), idx3=Device-nGnRE(0x04)
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
const SCTLR_MMU_ON: u64 =
      (1 << 0)   // M   — MMU enable
    | (1 << 2)   // C   — Data cache enable
    | (1 << 3)   // SA  — SP alignment check
    | (1 << 12); // I   — Instruction cache enable

/// SCTLR_EL1.WXN (bit 19) — Write XOR Execute, for sub-phase 2
const SCTLR_WXN: u64 = 1 << 19;

// ─── Page table storage ────────────────────────────────────────────

// Linker-provided symbols for page table memory (in .page_tables section)
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
}

/// Get address of a linker symbol as usize
#[inline(always)]
fn sym_addr(sym: &u8) -> usize {
    sym as *const u8 as usize
}

/// Pointer to one of the 4 page tables (each 512 × u64 = 4096 bytes)
#[inline(always)]
fn table_ptr(index: usize) -> *mut u64 {
    unsafe {
        let base = sym_addr(&__page_tables_start);
        (base + index * 4096) as *mut u64
    }
}

/// Write a page table entry
#[inline(always)]
unsafe fn write_entry(table: *mut u64, index: usize, value: u64) {
    ptr::write_volatile(table.add(index), value);
}

// ─── Sub-phase 1: Identity map with 2 MiB blocks ──────────────────

/// Initialize page tables: identity map devices + RAM with 2 MiB blocks
///
/// Layout:
///   L1[0] → L2_device (covers 0x0000_0000 – 0x3FFF_FFFF)
///   L1[1] → L2_ram    (covers 0x4000_0000 – 0x7FFF_FFFF)
///
/// L2_device: map 0x0800_0000–0x09FF_FFFF as Device-nGnRnE (16 × 2MiB blocks)
/// L2_ram:    map 0x4000_0000–0x47FF_FFFF as Normal WB (64 × 2MiB blocks = 128 MiB)
unsafe fn init_tables_2mib() {
    let l1 = table_ptr(0);
    let l2_device = table_ptr(1);
    let l2_ram = table_ptr(2);

    // L1[0] → L2_device table
    write_entry(l1, 0, (l2_device as u64) | TABLE);
    // L1[1] → L2_ram table
    write_entry(l1, 1, (l2_ram as u64) | TABLE);

    // L2_device: map 2MiB blocks for device MMIO region
    // L1[0] covers 0x0000_0000 – 0x3FFF_FFFF, each L2 entry = 2MiB
    // UART0 at 0x0900_0000: L2 index = 0x0900_0000 / 0x20_0000 = 72
    // GIC  at 0x0800_0000: L2 index = 0x0800_0000 / 0x20_0000 = 64
    // Map indices 64..=72 to cover 0x0800_0000 – 0x09FF_FFFF
    for i in 64..=72 {
        let pa = (i as u64) * 0x20_0000; // 2 MiB aligned physical address
        write_entry(l2_device, i, pa | DEVICE_BLOCK);
    }

    // L2_ram: map 128 MiB of RAM starting at 0x4000_0000
    // 0x4000_0000 is at the start of the 1 GiB region covered by L1[1]
    // So L2 index 0 = 0x4000_0000, index 1 = 0x4020_0000, etc.
    for i in 0..64 {
        let pa = 0x4000_0000_u64 + (i as u64) * 0x20_0000;
        write_entry(l2_ram, i, pa | RAM_BLOCK);
    }
}

// ─── Sub-phase 2: Refine kernel 2MiB to 4KB pages with W^X ────────

/// Replace L2_ram[0] (first 2MiB at 0x4000_0000) with L3 table for fine-grained permissions
unsafe fn refine_kernel_pages() {
    let l2_ram = table_ptr(2);
    let l3_kernel = table_ptr(3);

    let text_start = sym_addr(&__text_start);
    let text_end = sym_addr(&__text_end);
    let rodata_start = sym_addr(&__rodata_start);
    let rodata_end = sym_addr(&__rodata_end);
    let data_start = sym_addr(&__data_start);
    let kernel_end = sym_addr(&__kernel_end);
    let user_stacks_start = sym_addr(&__user_stacks_start);
    let user_stacks_end = sym_addr(&__user_stacks_end);

    // Fill L3 table: 512 entries covering 0x4000_0000 – 0x401F_FFFF
    let base: usize = 0x4000_0000;
    for i in 0..512 {
        let pa = base + i * 4096;

        let desc = if pa >= user_stacks_start && pa < user_stacks_end {
            // User stacks: RW, EL0-accessible, non-executable
            (pa as u64) | USER_DATA_PAGE
        } else if pa >= text_start && pa < text_end {
            // Shared code (kernel + task): RO, executable by both EL1 and EL0
            // Both PXN=0 and UXN=0 so kernel handlers and EL0 tasks can execute.
            // AP = RO_EL0 (0b11) means both EL1 and EL0 can read.
            // WXN is satisfied because pages are RO (not writable).
            (pa as u64) | SHARED_CODE_PAGE
        } else if pa >= rodata_start && pa < rodata_end {
            // Read-only data: RO, non-executable
            (pa as u64) | KERNEL_RODATA_PAGE
        } else if pa >= data_start && pa < kernel_end {
            // Data + BSS + page tables + kernel stacks: RW, non-executable
            (pa as u64) | KERNEL_DATA_PAGE
        } else if pa < text_start {
            // Before kernel (DTB area etc): RW, non-executable
            (pa as u64) | KERNEL_DATA_PAGE
        } else {
            // Beyond kernel end within this 2MiB: invalid
            0
        };

        write_entry(l3_kernel, i, desc);
    }

    // Replace L2_ram[0] with pointer to L3 table
    write_entry(l2_ram, 0, (l3_kernel as u64) | TABLE);
}

/// Mark the stack guard page as invalid (causes Data Abort on stack overflow)
unsafe fn set_guard_page() {
    let l3_kernel = table_ptr(3);
    let guard_addr = sym_addr(&__stack_guard);
    let base: usize = 0x4000_0000;

    // Find which L3 entry corresponds to the guard page
    if guard_addr >= base && guard_addr < base + 512 * 4096 {
        let index = (guard_addr - base) / 4096;
        write_entry(l3_kernel, index, 0); // Invalid — Data Abort on access
    }
}

// ─── MMU enable sequence (called from assembly) ───────────────────

/// Full MMU initialization — called from boot.s after BSS clear
#[no_mangle]
pub unsafe extern "C" fn mmu_init() {
    // Sub-phase 1: Build 2MiB identity map
    init_tables_2mib();

    // Sub-phase 2: Refine first 2MiB to 4KB pages with W^X
    refine_kernel_pages();

    // Sub-phase 3: Stack guard page
    set_guard_page();

    // Flush page table writes to memory
    core::arch::asm!(
        "dsb ish",
        "isb",
        options(nomem, nostack)
    );
}

/// Enable MMU — called from assembly after mmu_init()
/// This is kept in Rust for the register constant values, but the actual
/// MSR sequence is in boot.s for precise control over instruction ordering.
#[no_mangle]
pub unsafe extern "C" fn mmu_get_config(out: *mut [u64; 4]) {
    let l1 = table_ptr(0);
    (*out)[0] = MAIR_VALUE;
    (*out)[1] = TCR_VALUE;
    (*out)[2] = l1 as u64; // TTBR0
    (*out)[3] = SCTLR_MMU_ON | SCTLR_WXN; // SCTLR bits to set
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

use crate::exception::TrapFrame;
use crate::uart_print;

// ─── Task state ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
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
}

// ─── Static task table ─────────────────────────────────────────────

/// 3 tasks: 0 = task_a, 1 = task_b, 2 = idle
const NUM_TASKS: usize = 3;

static mut TCBS: [Tcb; NUM_TASKS] = [EMPTY_TCB; NUM_TASKS];
static mut CURRENT: usize = 0;

/// Delay before auto-restarting a faulted task (100 ticks × 10ms = 1 second)
const RESTART_DELAY_TICKS: u64 = 100;

const EMPTY_TCB: Tcb = Tcb {
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
};

// ─── Public API ────────────────────────────────────────────────────

/// Initialize scheduler: set up TCBs for task_a, task_b, idle.
/// Must be called before enabling timer interrupts.
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

        // Schedule away to the next ready task
        schedule(frame);
    }
}

/// Restart a faulted task: zero context, reload entry point + stack, mark Ready.
/// Called from schedule() when restart delay has elapsed.
fn restart_task(task_idx: usize) {
    unsafe {
        if TCBS[task_idx].state != TaskState::Faulted {
            return;
        }

        let id = TCBS[task_idx].id;

        // Zero user stack (4KB) to prevent state leakage
        let ustack_top = TCBS[task_idx].user_stack_top;
        let ustack_base = (ustack_top - 4096) as *mut u8;
        core::ptr::write_bytes(ustack_base, 0, 4096);

        // Zero entire TrapFrame
        core::ptr::write_bytes(
            &mut TCBS[task_idx].context as *mut TrapFrame as *mut u8,
            0,
            core::mem::size_of::<TrapFrame>(),
        );

        // Reload entry point, stack, SPSR
        TCBS[task_idx].context.elr_el1 = TCBS[task_idx].entry_point;
        TCBS[task_idx].context.spsr_el1 = 0x000; // EL0t
        TCBS[task_idx].context.sp_el0 = ustack_top;

        TCBS[task_idx].state = TaskState::Ready;

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
pub fn bootstrap() -> ! {
    unsafe {
        TCBS[0].state = TaskState::Running;
        CURRENT = 0;

        let frame = &TCBS[0].context;

        // Load the task's context into registers and eret into EL0
        core::arch::asm!(
            // Set ELR_EL1 = task entry, SPSR_EL1 = 0x000 (EL0t)
            "msr elr_el1, {elr}",
            "msr spsr_el1, {spsr}",
            // Set SP_EL0 = user stack (task will use this at EL0)
            "msr sp_el0, {sp0}",
            // eret: CPU restores PSTATE from SPSR (EL0t), PC from ELR.
            // Task runs at EL0 with SP = SP_EL0 (user stack).
            "eret",
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

use crate::uart_print;

/// GIC INTID for EL1 Physical Timer (PPI 14)
pub const TIMER_INTID: u32 = 30;

/// Tick interval in ticks (computed at init)
static mut TICK_INTERVAL: u64 = 0;

/// Monotonic tick counter
static mut TICK_COUNT: u64 = 0;

/// Initialize timer for periodic ticks
/// `tick_ms` = interval in milliseconds (e.g., 10 for 10ms)
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

