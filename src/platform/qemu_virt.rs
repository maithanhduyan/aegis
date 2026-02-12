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

// ─── ELF Load Region (Phase O) ────────────────────────────────────

/// Base address for ELF load region (matches linker.ld .elf_load)
pub const ELF_LOAD_BASE: u64 = 0x4010_0000;

/// Per-task ELF slot size (16 KiB)
pub const ELF_LOAD_SIZE_PER_TASK: usize = 16 * 1024;

/// Number of ELF task slots (tasks 2–7, slot 0..5)
pub const MAX_ELF_TASKS: usize = 6;

/// First task ID that can hold an ELF binary (tasks 0–1 are kernel tasks)
pub const ELF_FIRST_TASK_ID: usize = 2;

/// Compute the load address for ELF slot `slot` (0-indexed).
/// slot 0 → task 2 at 0x4010_0000, slot 1 → task 3 at 0x4010_4000, etc.
pub const fn elf_load_addr(slot: usize) -> u64 {
    ELF_LOAD_BASE + (slot as u64) * (ELF_LOAD_SIZE_PER_TASK as u64)
}

// ─── Kani formal verification proofs ───────────────────────────────

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proof P1: elf_load_addr produces non-overlapping regions
    /// within the ELF load area for all valid slot pairs.
    ///
    /// For all slots i, j ∈ [0, MAX_ELF_TASKS) where i ≠ j:
    /// - [addr(i), addr(i)+SIZE) ∩ [addr(j), addr(j)+SIZE) = ∅
    /// - addr(slot) ≥ ELF_LOAD_BASE
    /// - addr(slot) + SIZE ≤ ELF_LOAD_BASE + total_region_size
    #[kani::proof]
    fn elf_load_addr_no_overlap() {
        let i: usize = kani::any();
        let j: usize = kani::any();
        kani::assume(i < MAX_ELF_TASKS);
        kani::assume(j < MAX_ELF_TASKS);
        kani::assume(i != j);

        let addr_i = elf_load_addr(i);
        let addr_j = elf_load_addr(j);
        let size = ELF_LOAD_SIZE_PER_TASK as u64;

        // Each address must be ≥ base
        assert!(addr_i >= ELF_LOAD_BASE, "addr must be >= ELF_LOAD_BASE");
        assert!(addr_j >= ELF_LOAD_BASE, "addr must be >= ELF_LOAD_BASE");

        // Each region must fit within the total ELF area
        let total_size = (MAX_ELF_TASKS as u64) * size;
        assert!(
            addr_i + size <= ELF_LOAD_BASE + total_size,
            "region must fit within ELF area"
        );
        assert!(
            addr_j + size <= ELF_LOAD_BASE + total_size,
            "region must fit within ELF area"
        );

        // Regions must not overlap: [i, i+size) ∩ [j, j+size) = ∅
        assert!(
            addr_i + size <= addr_j || addr_j + size <= addr_i,
            "ELF regions must not overlap"
        );
    }
}
