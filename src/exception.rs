/// AegisOS Exception Handling — Host Stub
///
/// On host (x86_64): only TrapFrame struct and pure validation logic.
/// On AArch64: this file is NOT compiled — the full implementation
/// lives in arch/aarch64/exception.rs and is re-exported as
/// `crate::exception` via lib.rs.

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
