/// Memory Management Unit (Bộ phận Quản lý Bộ nhớ).
/// AegisOS MMU — AArch64 Page Table Setup
///
/// Sub-phase 1: Identity map with 2 MiB blocks
/// Sub-phase 2: Refine to 4KB pages for kernel region, W^X enforcement

#[cfg(target_arch = "aarch64")]
use core::ptr;

use crate::kernel::sched::NUM_TASKS;

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
/// Used by map_device_for_task() to grant user-mode access to a device.
pub const DEVICE_BLOCK_EL0: u64 = BLOCK | ATTR_DEVICE | AP_RW_EL0 | AF | XN;

/// Normal RAM: Write-Back, RW, Inner Shareable, AF=1 (executable for sub-phase 1)
pub const RAM_BLOCK: u64 = BLOCK | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF;

/// Kernel code page: Normal WB, RO, executable, Inner Shareable, AF=1
#[allow(dead_code)]
pub const KERNEL_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL1 | SH_INNER | AF;

/// Kernel rodata page: Normal WB, RO (EL0+EL1), non-executable, Inner Shareable, AF=1
/// Must be EL0-accessible because EL0 tasks reference string literals in .rodata
pub const KERNEL_RODATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | XN;

/// Kernel data/bss/stack page: Normal WB, RW, non-executable, Inner Shareable, AF=1
pub const KERNEL_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL1 | SH_INNER | AF | XN;

/// User data/stack page: Normal WB, RW (EL0+EL1), non-executable, Inner Shareable, AF=1
pub const USER_DATA_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RW_EL0 | SH_INNER | AF | XN;

/// User code page: Normal WB, RO (EL0+EL1), EL0-executable (UXN=0), PXN=1, Inner Shareable, AF=1
/// PXN prevents kernel from executing user code; UXN=0 allows EL0 execution
#[allow(dead_code)]
pub const USER_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF | PXN;

/// Shared code page: Normal WB, RO (EL0+EL1), executable by both EL1 and EL0
/// Used for .text section where kernel and task code coexist
pub const SHARED_CODE_PAGE: u64 = PAGE | ATTR_NORMAL_WB | AP_RO_EL0 | SH_INNER | AF;

// ─── MAIR / TCR constants ─────────────────────────────────────────

/// MAIR_EL1: idx0=Device-nGnRnE(0x00), idx1=Normal-NC(0x44), idx2=Normal-WB(0xFF), idx3=Device-nGnRE(0x04)
#[cfg(target_arch = "aarch64")]
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
#[cfg(target_arch = "aarch64")]
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
#[cfg(target_arch = "aarch64")]
const SCTLR_MMU_ON: u64 =
      (1 << 0)   // M   — MMU enable
    | (1 << 2)   // C   — Data cache enable
    | (1 << 3)   // SA  — SP alignment check
    | (1 << 12); // I   — Instruction cache enable

/// SCTLR_EL1.WXN (bit 19) — Write XOR Execute, for sub-phase 2
#[cfg(target_arch = "aarch64")]
const SCTLR_WXN: u64 = 1 << 19;

// ─── Page table storage (AArch64 only) ─────────────────────────────

/// Number of page table pages: 4 per task (L2Device, L1, L2Ram, L3) + 4 kernel.
/// Must match linker.ld `.page_tables` section size: NUM_PAGE_TABLE_PAGES × 4096.
pub const NUM_PAGE_TABLE_PAGES: usize = 4 * NUM_TASKS + 4;

// ─── Page table type and computed indexing (Phase N) ───────────────

/// Page table type within a task's 4-table set.
/// Layout in .page_tables: [L2Device×N | L1×N | L2Ram×N | L3×N | kernel×4]
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(usize)]
pub enum PageTableType {
    L2Device = 0,
    L1       = 1,
    L2Ram    = 2,
    L3       = 3,
}

/// Number of page table types per task.
pub const PT_TYPES_PER_TASK: usize = 4;

/// Compute the page table index for a given task and table type.
/// `pt_index(t, L2Device) = 0*N + t`, `pt_index(t, L1) = 1*N + t`, etc.
pub const fn pt_index(task_id: usize, pt_type: PageTableType) -> usize {
    pt_type as usize * NUM_TASKS + task_id
}

// Per-task base indices (task 0) — backward compatibility aliases
pub const PT_L2_DEVICE_0: usize = pt_index(0, PageTableType::L2Device);
pub const PT_L1_TASK0: usize = pt_index(0, PageTableType::L1);
pub const PT_L2_RAM_TASK0: usize = pt_index(0, PageTableType::L2Ram);
pub const PT_L3_TASK0: usize = pt_index(0, PageTableType::L3);

// Kernel page table indices (always after all per-task tables)
pub const PT_L2_DEVICE_KERNEL: usize = PT_TYPES_PER_TASK * NUM_TASKS;
pub const PT_L1_KERNEL: usize = PT_TYPES_PER_TASK * NUM_TASKS + 1;
pub const PT_L2_RAM_KERNEL: usize = PT_TYPES_PER_TASK * NUM_TASKS + 2;
pub const PT_L3_KERNEL: usize = PT_TYPES_PER_TASK * NUM_TASKS + 3;

// Linker-provided symbols for page table memory (in .page_tables section)
#[cfg(target_arch = "aarch64")]
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
    static __grant_pages_start: u8;
    static __grant_pages_end: u8;
    static __elf_load_start: u8;
    static __elf_load_end: u8;
}

/// Get address of a linker symbol as usize
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn sym_addr(sym: &u8) -> usize {
    sym as *const u8 as usize
}

/// Pointer to one of the 13 page tables (each 512 × u64 = 4096 bytes)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn table_ptr(index: usize) -> *mut u64 {
    // SAFETY: Linker-provided symbol __page_tables_start is 4KB-aligned.
    // Pointer arithmetic stays within allocated page table region.
    unsafe {
        let base = sym_addr(&__page_tables_start);
        (base + index * 4096) as *mut u64
    }
}

/// Write a page table entry
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn write_entry(table: *mut u64, index: usize, value: u64) {
    // SAFETY: caller guarantees table + index points to a valid page table slot
    unsafe { ptr::write_volatile(table.add(index), value) };
}

// ─── Phase H: Per-task page tables ─────────────────────────────────

/// Build an L2_device table at page index `l2dev_index`.
/// Maps device MMIO at indices 64..=72 (0x0800_0000–0x09FF_FFFF).
/// All entries start as DEVICE_BLOCK (AP_RW_EL1, EL0 no access).
/// map_device_for_task() later upgrades specific entries to DEVICE_BLOCK_EL0.
#[cfg(target_arch = "aarch64")]
unsafe fn build_l2_device(l2dev_index: usize) {
    // SAFETY: accesses page table memory via table_ptr and write_entry
    unsafe {
        let l2_device = table_ptr(l2dev_index);
        for i in 64..=72 {
            let pa = (i as u64) * 0x20_0000;
            write_entry(l2_device, i, pa | DEVICE_BLOCK);
        }
    }
}

/// Build an L3 table for a given task.
/// `l3_index` = page index in .page_tables for this L3.
/// `owner_task` = which task (0,1,2) owns this table. 0xFF = kernel boot (all stacks EL1-only).
#[cfg(target_arch = "aarch64")]
unsafe fn build_l3(l3_index: usize, owner_task: u8) {
    // SAFETY: accesses extern linker statics, page table memory, and write_entry
    unsafe {
    let l3 = table_ptr(l3_index);

    let text_start = sym_addr(&__text_start);
    let text_end = sym_addr(&__text_end);
    let rodata_start = sym_addr(&__rodata_start);
    let rodata_end = sym_addr(&__rodata_end);
    let data_start = sym_addr(&__data_start);
    let kernel_end = sym_addr(&__kernel_end);
    let user_stacks_start = sym_addr(&__user_stacks_start);
    let user_stacks_end = sym_addr(&__user_stacks_end);
    let grant_pages_start = sym_addr(&__grant_pages_start);
    let grant_pages_end = sym_addr(&__grant_pages_end);
    let guard_addr = sym_addr(&__stack_guard);

    let base: usize = 0x4000_0000;
    for i in 0..512 {
        let pa = base + i * 4096;

        let desc = if pa == guard_addr {
            // Stack guard page — always invalid
            0
        } else if pa >= user_stacks_start && pa < user_stacks_end {
            // User stack page — per-task isolation
            let stack_idx = (pa - user_stacks_start) / 4096;
            if owner_task == 0xFF {
                // Kernel boot table: all user stacks EL1-only
                (pa as u64) | KERNEL_DATA_PAGE
            } else if stack_idx == owner_task as usize {
                // This task's own stack: EL0 RW
                (pa as u64) | USER_DATA_PAGE
            } else {
                // Other task's stack: EL1-only (EL0 → Permission Fault)
                (pa as u64) | KERNEL_DATA_PAGE
            }
        } else if pa >= grant_pages_start && pa < grant_pages_end {
            // Grant pages — default EL1-only; map_grant_for_task() upgrades to EL0
            (pa as u64) | KERNEL_DATA_PAGE
        } else if pa >= text_start && pa < text_end {
            (pa as u64) | SHARED_CODE_PAGE
        } else if pa >= rodata_start && pa < rodata_end {
            (pa as u64) | KERNEL_RODATA_PAGE
        } else if pa >= data_start && pa < kernel_end {
            (pa as u64) | KERNEL_DATA_PAGE
        } else if pa < text_start {
            (pa as u64) | KERNEL_DATA_PAGE
        } else {
            0
        };

        write_entry(l3, i, desc);
    }
    } // unsafe
}

/// Build an L2_ram table that points to a specific L3 table.
/// `l2_index` = page index for this L2_ram, `l3_index` = page index for its L3.
#[cfg(target_arch = "aarch64")]
unsafe fn build_l2_ram(l2_index: usize, l3_index: usize) {
    // SAFETY: accesses page table memory via table_ptr and write_entry
    unsafe {
        let l2_ram = table_ptr(l2_index);
        let l3 = table_ptr(l3_index);

        // Entry [0] → L3 table (first 2MiB, fine-grained)
        write_entry(l2_ram, 0, (l3 as u64) | TABLE);

        // Entries [1..63] → 2MiB RAM blocks (EL1-only, same as before)
        for i in 1..64 {
            let pa = 0x4000_0000_u64 + (i as u64) * 0x20_0000;
            write_entry(l2_ram, i, pa | RAM_BLOCK);
        }
    }
}

/// Build an L1 table for a specific task (or kernel boot).
/// `l1_index` = page index for this L1, `l2_ram_index` = page index for its L2_ram.
/// `l2_device_index` = page index for this task's L2_device table.
#[cfg(target_arch = "aarch64")]
unsafe fn build_l1(l1_index: usize, l2_ram_index: usize, l2_device_index: usize) {
    // SAFETY: accesses page table memory via table_ptr and write_entry
    unsafe {
        let l1 = table_ptr(l1_index);
        let l2_device = table_ptr(l2_device_index);
        let l2_ram = table_ptr(l2_ram_index);

        write_entry(l1, 0, (l2_device as u64) | TABLE);
        write_entry(l1, 1, (l2_ram as u64) | TABLE);
    }
}

/// Get physical address of L1 page table for a task.
/// Returns the base address suitable for TTBR0_EL1 (bits [47:12]).
pub fn page_table_base(task_id: usize) -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let ptr = table_ptr(pt_index(task_id, PageTableType::L1));
        ptr as u64
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        // Host tests: return a fake but distinct address per task
        0x8000_0000_u64 + (task_id as u64) * 0x1_0000
    }
}

/// Get the TTBR0_EL1 value for a task: (ASID << 48) | page_table_base.
pub fn ttbr0_for_task(task_id: usize, asid: u16) -> u64 {
    let base = page_table_base(task_id);
    ((asid as u64) << 48) | base
}

// ─── MMU enable sequence (called from assembly) ───────────────────

/// Full MMU initialization — called from boot.s after BSS clear.
/// Phase N: builds NUM_PAGE_TABLE_PAGES page tables (4 per task + 4 kernel).
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub unsafe extern "C" fn mmu_init() {
    // SAFETY: builds all page tables using pre-allocated static memory, flushes with asm
    unsafe {
    // Per-task L2_device tables — all devices EL1-only initially
    for task in 0..NUM_TASKS {
        build_l2_device(pt_index(task, PageTableType::L2Device));
    }
    // Kernel boot L2_device — all devices EL1-accessible
    build_l2_device(PT_L2_DEVICE_KERNEL);

    // Per-task tables
    for task in 0..NUM_TASKS {
        build_l3(pt_index(task, PageTableType::L3), task as u8);
        build_l2_ram(pt_index(task, PageTableType::L2Ram), pt_index(task, PageTableType::L3));
        build_l1(pt_index(task, PageTableType::L1), pt_index(task, PageTableType::L2Ram), pt_index(task, PageTableType::L2Device));
    }

    // Kernel boot tables (owner_task = 0xFF → all user stacks EL1-only)
    build_l3(PT_L3_KERNEL, 0xFF);
    build_l2_ram(PT_L2_RAM_KERNEL, PT_L3_KERNEL);
    build_l1(PT_L1_KERNEL, PT_L2_RAM_KERNEL, PT_L2_DEVICE_KERNEL);

    // Flush page table writes to memory
    core::arch::asm!(
        "dsb ish",
        "isb",
        options(nomem, nostack)
    );
    } // unsafe
}

// ─── Phase J3: Device MMIO mapping ─────────────────────────────────

/// Device registry — whitelisted devices that can be mapped for EL0 tasks.
/// GIC MMIO (L2 indices 64–66) is NEVER exposed — only safe peripherals.
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
pub const MAX_DEVICE_ID: usize = 0; // only UART0 for now

// Error codes for map_device_for_task
pub const DEVICE_MAP_ERR_INVALID_ID: u64 = 0xFFFF_2001;
pub const DEVICE_MAP_ERR_INVALID_TASK: u64 = 0xFFFF_2002;

/// Map a device's MMIO region into a task's L2_device table as DEVICE_BLOCK_EL0.
/// This allows the EL0 task to directly read/write the device's MMIO registers.
///
/// Safety: device_id must index into DEVICES. GIC is never exposed.
#[cfg(target_arch = "aarch64")]
pub unsafe fn map_device_for_task(device_id: u64, task_id: usize) -> u64 {
    // SAFETY: accesses page table memory, performs TLB invalidation via asm
    unsafe {
    let did = device_id as usize;
    if did >= DEVICES.len() {
        crate::uart_print("!!! DEVICE MAP: invalid device_id\n");
        return DEVICE_MAP_ERR_INVALID_ID;
    }
    if task_id >= NUM_TASKS {
        crate::uart_print("!!! DEVICE MAP: invalid task_id\n");
        return DEVICE_MAP_ERR_INVALID_TASK;
    }

    let dev = &DEVICES[did];
    let l2_device = table_ptr(pt_index(task_id, PageTableType::L2Device));
    let pa = (dev.l2_index as u64) * 0x20_0000;

    // Upgrade entry from DEVICE_BLOCK (EL1-only) to DEVICE_BLOCK_EL0 (EL0 accessible)
    write_entry(l2_device, dev.l2_index, pa | DEVICE_BLOCK_EL0);

    // TLB invalidate for this task's ASID
    let asid = (task_id as u64 + 1) << 48;
    core::arch::asm!(
        "tlbi aside1is, {asid}",
        "dsb ish",
        "isb",
        asid = in(reg) asid,
        options(nomem, nostack)
    );

    crate::uart_print("[AegisOS] DEVICE MAP: ");
    crate::uart_print(dev.name);
    crate::uart_print(" -> task ");
    crate::uart_print_hex(task_id as u64);
    crate::uart_print("\n");

    0 // success
    } // unsafe
}

/// Host-test stub for map_device_for_task
#[cfg(not(target_arch = "aarch64"))]
pub fn map_device_for_task(device_id: u64, task_id: usize) -> u64 {
    let did = device_id as usize;
    if did >= DEVICES.len() {
        return DEVICE_MAP_ERR_INVALID_ID;
    }
    if task_id >= NUM_TASKS {
        return DEVICE_MAP_ERR_INVALID_TASK;
    }
    0 // success
}

// ─── Phase J1: Grant page mapping ──────────────────────────────────

/// Map a grant page into a task's L3 table as AP_RW_EL0 (user accessible).
/// Must be followed by TLB invalidation for the task's ASID.
#[cfg(target_arch = "aarch64")]
pub unsafe fn map_grant_for_task(grant_phys: u64, task_id: usize) {
    // SAFETY: accesses page table memory, performs TLB invalidation via asm
    unsafe {
    let l3 = table_ptr(pt_index(task_id, PageTableType::L3));
    let base: u64 = 0x4000_0000;
    let index = ((grant_phys - base) / 4096) as usize;
    if index < 512 {
        write_entry(l3, index, grant_phys | USER_DATA_PAGE);
        // TLB invalidate for this task's ASID
        let asid = (task_id as u64 + 1) << 48;
        core::arch::asm!(
            "tlbi aside1is, {asid}",
            "dsb ish",
            "isb",
            asid = in(reg) asid,
            options(nomem, nostack)
        );
    }
    } // unsafe
}

/// Unmap a grant page from a task's L3 table (revert to AP_RW_EL1, EL0 no access).
/// Must be followed by TLB invalidation for the task's ASID.
#[cfg(target_arch = "aarch64")]
pub unsafe fn unmap_grant_for_task(grant_phys: u64, task_id: usize) {
    // SAFETY: accesses page table memory, performs TLB invalidation via asm
    unsafe {
    let l3 = table_ptr(pt_index(task_id, PageTableType::L3));
    let base: u64 = 0x4000_0000;
    let index = ((grant_phys - base) / 4096) as usize;
    if index < 512 {
        write_entry(l3, index, grant_phys | KERNEL_DATA_PAGE);
        // TLB invalidate for this task's ASID
        let asid = (task_id as u64 + 1) << 48;
        core::arch::asm!(
            "tlbi aside1is, {asid}",
            "dsb ish",
            "isb",
            asid = in(reg) asid,
            options(nomem, nostack)
        );
    }
    } // unsafe
}

/// Enable MMU — called from assembly after mmu_init()
/// This is kept in Rust for the register constant values, but the actual
/// MSR sequence is in boot.s for precise control over instruction ordering.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub unsafe extern "C" fn mmu_get_config(out: *mut [u64; 4]) {
    // SAFETY: dereferences raw pointer out, accesses page table memory
    unsafe {
        // Kernel boot L1 (page 13) — no EL0 user stack access
        let l1_kernel = table_ptr(PT_L1_KERNEL);
        (*out)[0] = MAIR_VALUE;
        (*out)[1] = TCR_VALUE;
        (*out)[2] = l1_kernel as u64; // TTBR0 = kernel boot table
        (*out)[3] = SCTLR_MMU_ON | SCTLR_WXN;
    }
}

// ─── Phase L4: Page attribute manipulation ─────────────────────────

/// Error: invalid task_id for set_page_attr
pub const PAGE_ATTR_ERR_INVALID_TASK: u64 = 0xFFFF_3001;
/// Error: vaddr outside L3-mapped range
pub const PAGE_ATTR_ERR_OUT_OF_RANGE: u64 = 0xFFFF_3002;

/// Set page descriptor for a specific virtual address in a task's L3 table.
///
/// `vaddr` must be 4KB-aligned and within the L3-mapped range
/// (0x4000_0000..0x401F_FFFF). Performs TLB invalidation after update.
///
/// # Safety
/// Caller must ensure `task_id < NUM_TASKS` and `vaddr` is page-aligned.
#[cfg(target_arch = "aarch64")]
pub unsafe fn set_page_attr(task_id: usize, vaddr: u64, template: u64) -> u64 {
    // SAFETY: accesses page table memory, performs TLB invalidation via asm
    unsafe {
    if task_id >= NUM_TASKS {
        return PAGE_ATTR_ERR_INVALID_TASK;
    }
    let base: u64 = 0x4000_0000;
    if vaddr < base || vaddr >= base + 512 * 4096 {
        return PAGE_ATTR_ERR_OUT_OF_RANGE;
    }
    let index = ((vaddr - base) / 4096) as usize;
    let l3 = table_ptr(pt_index(task_id, PageTableType::L3));
    write_entry(l3, index, vaddr | template);

    // TLB invalidate for this task's ASID
    let asid = (task_id as u64 + 1) << 48;
    core::arch::asm!(
        "tlbi aside1is, {asid}",
        "dsb ish",
        "isb",
        asid = in(reg) asid,
        options(nomem, nostack)
    );
    0 // success
    } // unsafe
}

/// Host-test stub for set_page_attr
#[cfg(not(target_arch = "aarch64"))]
pub fn set_page_attr(task_id: usize, vaddr: u64, _template: u64) -> u64 {
    if task_id >= NUM_TASKS {
        return PAGE_ATTR_ERR_INVALID_TASK;
    }
    let base: u64 = 0x4000_0000;
    if vaddr < base || vaddr >= base + 512 * 4096 {
        return PAGE_ATTR_ERR_OUT_OF_RANGE;
    }
    0 // success
}
