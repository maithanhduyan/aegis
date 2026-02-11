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
