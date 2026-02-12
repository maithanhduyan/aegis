/// AegisOS MMU — Host Stub
///
/// On host (x86_64): only constants, types, and stub functions.
/// On AArch64: this file is NOT compiled — the full implementation
/// lives in arch/aarch64/mmu.rs and is loaded via `#[path]` in lib.rs.

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
pub const DEVICE_BLOCK_EL0: u64 = BLOCK | ATTR_DEVICE | AP_RW_EL0 | AF | XN;

/// Normal RAM: Write-Back, RW, Inner Shareable, AF=1
pub const RAM_BLOCK: u64 = BLOCK | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF;

/// Kernel code page: Normal WB, RO, executable, Inner Shareable, AF=1
#[allow(dead_code)]
pub const KERNEL_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL1 | SH_INNER | AF;

/// Kernel rodata page: RO EL0-accessible, non-executable
pub const KERNEL_RODATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | XN;

/// Kernel data/bss/stack page: Normal WB, RW, non-executable
pub const KERNEL_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF | XN;

/// User data/stack page: Normal WB, RW (EL0+EL1), non-executable
pub const USER_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL0 | SH_INNER | AF | XN;

/// User code page: Normal WB, RO (EL0+EL1), EL0-executable (UXN=0), PXN=1
#[allow(dead_code)]
pub const USER_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | PXN;

/// Shared code page: Normal WB, RO (EL0+EL1), executable by both EL1 and EL0
pub const SHARED_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF;

// ─── Page table storage constants ──────────────────────────────────

/// Number of page table pages
pub const NUM_PAGE_TABLE_PAGES: usize = 16;

// Page indices
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

// ─── Phase J3: Device MMIO mapping ─────────────────────────────────

/// Device registry — whitelisted devices that can be mapped for EL0 tasks.
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
pub const MAX_DEVICE_ID: usize = 0;

// Error codes for map_device_for_task
pub const DEVICE_MAP_ERR_INVALID_ID: u64 = 0xFFFF_2001;
pub const DEVICE_MAP_ERR_INVALID_TASK: u64 = 0xFFFF_2002;

// ─── Host-stub functions ───────────────────────────────────────────

/// Get physical address of L1 page table for a task (host: fake addresses).
pub fn page_table_base(task_id: usize) -> u64 {
    0x8000_0000_u64 + (task_id as u64) * 0x1_0000
}

/// Get the TTBR0_EL1 value for a task: (ASID << 48) | page_table_base.
pub fn ttbr0_for_task(task_id: usize, asid: u16) -> u64 {
    let base = page_table_base(task_id);
    ((asid as u64) << 48) | base
}

/// Host-test stub for map_device_for_task
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

// ─── Phase L4: Page attribute manipulation ─────────────────────────

/// Error: invalid task_id for set_page_attr
pub const PAGE_ATTR_ERR_INVALID_TASK: u64 = 0xFFFF_3001;
/// Error: vaddr outside L3-mapped range
pub const PAGE_ATTR_ERR_OUT_OF_RANGE: u64 = 0xFFFF_3002;

/// Host-test stub for set_page_attr — validates params, no actual write.
pub fn set_page_attr(task_id: usize, vaddr: u64, _template: u64) -> u64 {
    if task_id >= 3 {
        return PAGE_ATTR_ERR_INVALID_TASK;
    }
    let base: u64 = 0x4000_0000;
    if vaddr < base || vaddr >= base + 512 * 4096 {
        return PAGE_ATTR_ERR_OUT_OF_RANGE;
    }
    0 // success
}
