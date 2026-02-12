//! AegisOS — Kernel library crate
//!
//! Re-exports kernel modules so they can be used by both the kernel
//! binary (main.rs) and host-side unit tests (tests/host_tests.rs).
//!
//! On AArch64: full kernel with asm, MMIO, linker symbols.
//! On host (x86_64): only pure logic available (types, constants, validation).

#![no_std]

// ─── Phase L module structure ──────────────────────────────────────

/// Architecture-specific code (boot, GIC, vectors, MMU, timer HW)
pub mod arch;

/// Portable kernel logic (IPC, capabilities, scheduler, grants, IRQ)
pub mod kernel;

/// Platform constants (MMIO addresses, memory map)
pub mod platform;

// ─── Modules still at root (will move to arch/ or kernel/ in L2) ──

pub mod uart;
pub mod mmu;
pub mod exception;
pub mod sched;
pub mod timer;
pub mod grant;
pub mod irq;

// ─── Backward-compatible re-exports ────────────────────────────────
// So existing `crate::ipc`, `crate::cap`, `crate::gic` paths keep working.

pub use kernel::ipc;
pub use kernel::cap;

#[cfg(target_arch = "aarch64")]
pub use arch::current::gic;

// Re-export common UART functions at crate root for convenience
pub use uart::{uart_write, uart_print, uart_print_hex};
