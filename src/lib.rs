//! AegisOS — Kernel library crate
//!
//! Re-exports kernel modules so they can be used by both the kernel
//! binary (main.rs) and host-side unit tests (tests/host_tests.rs).
//!
//! On AArch64: full kernel with asm, MMIO, linker symbols.
//! On host (x86_64): only pure logic available (types, constants, validation).

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

// ─── Phase L module structure ──────────────────────────────────────

/// Architecture-specific code (boot, GIC, vectors, MMU, timer HW)
pub mod arch;

/// Portable kernel logic (IPC, capabilities, scheduler, grants, IRQ)
pub mod kernel;

/// Platform constants (MMIO addresses, memory map)
pub mod platform;

// ─── UART: stays at root (tiny, dual-cfg) ──────────────────────────

pub mod uart;

// ─── MMU + Exception: full arch version on AArch64, host stub otherwise ─

#[cfg(target_arch = "aarch64")]
#[path = "arch/aarch64/mmu.rs"]
pub mod mmu;

#[cfg(not(target_arch = "aarch64"))]
pub mod mmu; // reads src/mmu.rs (host-only stub)

#[cfg(target_arch = "aarch64")]
#[path = "arch/aarch64/exception.rs"]
pub mod exception;

#[cfg(not(target_arch = "aarch64"))]
pub mod exception; // reads src/exception.rs (host-only stub)

// ─── Backward-compatible re-exports ────────────────────────────────
// So existing `crate::ipc`, `crate::cap`, `crate::gic`, `crate::sched`,
// `crate::timer`, `crate::grant`, `crate::irq` paths keep working.

pub use kernel::ipc;
pub use kernel::cap;
pub use kernel::sched;
pub use kernel::timer;
pub use kernel::grant;
pub use kernel::irq;
pub use kernel::elf;
pub use kernel::log;

#[cfg(target_arch = "aarch64")]
pub use arch::current::gic;

// Re-export common UART functions at crate root for convenience
pub use uart::{uart_write, uart_print, uart_print_hex, uart_print_dec};
