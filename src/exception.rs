/// AegisOS Exception Handling — AArch64
///
/// Minimal exception vector table for catching Data Aborts, Instruction Aborts,
/// and other synchronous exceptions. Prints fault info via UART then halts.

use crate::uart_print;
use crate::uart_print_hex;

// ─── Exception vector table (assembly) ─────────────────────────────

core::arch::global_asm!(r#"
.section .text
.balign 2048
.global __exception_vectors
__exception_vectors:

// ─── Current EL with SP_EL0 ─────────────
.balign 0x80
    b   _sync_handler       // Synchronous
.balign 0x80
    b   _unhandled           // IRQ
.balign 0x80
    b   _unhandled           // FIQ
.balign 0x80
    b   _unhandled           // SError

// ─── Current EL with SP_ELx ─────────────
.balign 0x80
    b   _sync_handler       // Synchronous
.balign 0x80
    b   _unhandled           // IRQ
.balign 0x80
    b   _unhandled           // FIQ
.balign 0x80
    b   _unhandled           // SError

// ─── Lower EL, AArch64 ─────────────────
.balign 0x80
    b   _sync_handler       // Synchronous
.balign 0x80
    b   _unhandled           // IRQ
.balign 0x80
    b   _unhandled           // FIQ
.balign 0x80
    b   _unhandled           // SError

// ─── Lower EL, AArch32 ─────────────────
.balign 0x80
    b   _unhandled           // Synchronous
.balign 0x80
    b   _unhandled           // IRQ
.balign 0x80
    b   _unhandled           // FIQ
.balign 0x80
    b   _unhandled           // SError

// ─── Handlers ───────────────────────────

_sync_handler:
    // Save x0-x2, x30 on stack
    stp x0, x1, [sp, #-16]!
    stp x2, x30, [sp, #-16]!

    // Read ESR_EL1 and FAR_EL1
    mrs x0, esr_el1
    mrs x1, far_el1
    mrs x2, elr_el1
    bl  sync_exception_handler

    // Halt after exception
    b   _unhandled

_unhandled:
    wfe
    b   _unhandled
"#);

// ─── Rust exception handler ────────────────────────────────────────

/// Called from assembly vector table on synchronous exception
#[no_mangle]
pub extern "C" fn sync_exception_handler(esr: u64, far: u64, elr: u64) {
    let ec = (esr >> 26) & 0x3F; // Exception Class

    uart_print("\n!!! EXCEPTION !!!\n");

    match ec {
        0x20 => uart_print("  Type: Instruction Abort (lower EL)\n"),
        0x21 => uart_print("  Type: Instruction Abort (same EL)\n"),
        0x24 => uart_print("  Type: Data Abort (lower EL)\n"),
        0x25 => uart_print("  Type: Data Abort (same EL)\n"),
        0x15 => uart_print("  Type: SVC call\n"),
        0x00 => uart_print("  Type: Unknown reason\n"),
        _ => uart_print("  Type: Other\n"),
    }

    uart_print("  ESR_EL1: 0x");
    uart_print_hex(esr);
    uart_print("\n  FAR_EL1: 0x");
    uart_print_hex(far);
    uart_print("\n  ELR_EL1: 0x");
    uart_print_hex(elr);
    uart_print("\n  HALTED.\n");
}

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
