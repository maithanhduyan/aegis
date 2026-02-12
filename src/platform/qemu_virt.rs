/// AegisOS — QEMU virt machine platform constants
///
/// Memory-mapped I/O addresses and memory map for the QEMU
/// `virt` machine with Cortex-A53. These constants are referenced
/// by arch drivers and kernel modules.

// ─── GIC (Generic Interrupt Controller) ────────────────────────────

/// GIC Distributor base address
pub const GICD_BASE: usize = 0x0800_0000;

/// GIC CPU Interface base address
pub const GICC_BASE: usize = 0x0801_0000;

// ─── UART ──────────────────────────────────────────────────────────

/// PL011 UART0 data register address
pub const UART0_BASE: usize = 0x0900_0000;

// ─── RAM ───────────────────────────────────────────────────────────

/// Physical RAM base address (QEMU virt)
pub const RAM_BASE: usize = 0x4000_0000;

/// Kernel load address
pub const KERNEL_BASE: usize = 0x4008_0000;

// ─── Timer ─────────────────────────────────────────────────────────

/// GIC INTID for EL1 Physical Timer (PPI 14 → INTID 30)
pub const TIMER_INTID: u32 = 30;

/// Default tick interval in milliseconds
pub const TICK_MS: u32 = 10;

/// Timer frequency on QEMU virt (Hz)
pub const TIMER_FREQ_HZ: u64 = 62_500_000;
