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
    SAVE_CONTEXT
    mov x1, #2          /* source = 2: lower EL, AArch64 */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT

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
    SAVE_CONTEXT
    bl  exception_dispatch_irq
    RESTORE_CONTEXT

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
        0x07 => handle_fp_trap(frame, esr),
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
        _ => {
            uart_print("!!! unknown syscall #");
            uart_print_hex(syscall_nr);
            uart_print(" !!!\n");
        }
    }
}

/// Instruction Abort handler — print fault details, halt
fn handle_instruction_abort(frame: &mut TrapFrame, esr: u64, source: u64) {
    let far: u64;
    unsafe { core::arch::asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;
    let ifsc = esr & 0x3F; // Instruction Fault Status Code

    uart_print("\n!!! INSTRUCTION ABORT !!!\n");
    if ec == 0x20 {
        uart_print("  Source: lower EL\n");
    } else {
        uart_print("  Source: same EL\n");
    }
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

/// Data Abort handler — print fault details, halt
fn handle_data_abort(frame: &mut TrapFrame, esr: u64, source: u64) {
    let far: u64;
    unsafe { core::arch::asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;
    let dfsc = esr & 0x3F; // Data Fault Status Code

    uart_print("\n!!! DATA ABORT !!!\n");
    if ec == 0x24 {
        uart_print("  Source: lower EL\n");
    } else {
        uart_print("  Source: same EL\n");
    }
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

/// FP/SIMD trap — kernel should not use FP
fn handle_fp_trap(_frame: &mut TrapFrame, esr: u64) {
    uart_print("\n!!! FP/SIMD TRAP !!!\n");
    uart_print("  Kernel code attempted FP instruction.\n");
    uart_print("  ESR: 0x");
    uart_print_hex(esr);
    uart_print("\n  HALTED.\n");
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Unknown/unhandled exception class
fn handle_unknown(frame: &mut TrapFrame, esr: u64, ec: u64, source: u64) {
    uart_print("\n!!! UNHANDLED EXCEPTION !!!\n");
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
