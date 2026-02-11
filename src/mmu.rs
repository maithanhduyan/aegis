/// Memory Management Unit (Bộ phận Quản lý Bộ nhớ).
/// AegisOS MMU — AArch64 Page Table Setup
///
/// Sub-phase 1: Identity map with 2 MiB blocks
/// Sub-phase 2: Refine to 4KB pages for kernel region, W^X enforcement

#[cfg(target_arch = "aarch64")]
use core::ptr;

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
}

/// Get address of a linker symbol as usize
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn sym_addr(sym: &u8) -> usize {
    sym as *const u8 as usize
}

/// Pointer to one of the 4 page tables (each 512 × u64 = 4096 bytes)
#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn table_ptr(index: usize) -> *mut u64 {
    unsafe {
        let base = sym_addr(&__page_tables_start);
        (base + index * 4096) as *mut u64
    }
}

/// Write a page table entry
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn write_entry(table: *mut u64, index: usize, value: u64) {
    ptr::write_volatile(table.add(index), value);
}

// ─── Sub-phase 1: Identity map with 2 MiB blocks ──────────────────

/// Initialize page tables: identity map devices + RAM with 2 MiB blocks
///
/// Layout:
#[cfg(target_arch = "aarch64")]
///   L1[0] → L2_device (covers 0x0000_0000 – 0x3FFF_FFFF)
///   L1[1] → L2_ram    (covers 0x4000_0000 – 0x7FFF_FFFF)
///
/// L2_device: map 0x0800_0000–0x09FF_FFFF as Device-nGnRnE (16 × 2MiB blocks)
/// L2_ram:    map 0x4000_0000–0x47FF_FFFF as Normal WB (64 × 2MiB blocks = 128 MiB)
unsafe fn init_tables_2mib() {
    let l1 = table_ptr(0);
    let l2_device = table_ptr(1);
    let l2_ram = table_ptr(2);

    // L1[0] → L2_device table
    write_entry(l1, 0, (l2_device as u64) | TABLE);
    // L1[1] → L2_ram table
    write_entry(l1, 1, (l2_ram as u64) | TABLE);

    // L2_device: map 2MiB blocks for device MMIO region
    // L1[0] covers 0x0000_0000 – 0x3FFF_FFFF, each L2 entry = 2MiB
    // UART0 at 0x0900_0000: L2 index = 0x0900_0000 / 0x20_0000 = 72
    // GIC  at 0x0800_0000: L2 index = 0x0800_0000 / 0x20_0000 = 64
    // Map indices 64..=72 to cover 0x0800_0000 – 0x09FF_FFFF
    for i in 64..=72 {
        let pa = (i as u64) * 0x20_0000; // 2 MiB aligned physical address
        write_entry(l2_device, i, pa | DEVICE_BLOCK);
    }

    // L2_ram: map 128 MiB of RAM starting at 0x4000_0000
    // 0x4000_0000 is at the start of the 1 GiB region covered by L1[1]
    // So L2 index 0 = 0x4000_0000, index 1 = 0x4020_0000, etc.
    for i in 0..64 {
        let pa = 0x4000_0000_u64 + (i as u64) * 0x20_0000;
        write_entry(l2_ram, i, pa | RAM_BLOCK);
    }
}

// ─── Sub-phase 2: Refine kernel 2MiB to 4KB pages with W^X ────────

/// Replace L2_ram[0] (first 2MiB at 0x4000_0000) with L3 table for fine-grained permissions
#[cfg(target_arch = "aarch64")]
unsafe fn refine_kernel_pages() {
    let l2_ram = table_ptr(2);
    let l3_kernel = table_ptr(3);

    let text_start = sym_addr(&__text_start);
    let text_end = sym_addr(&__text_end);
    let rodata_start = sym_addr(&__rodata_start);
    let rodata_end = sym_addr(&__rodata_end);
    let data_start = sym_addr(&__data_start);
    let kernel_end = sym_addr(&__kernel_end);
    let user_stacks_start = sym_addr(&__user_stacks_start);
    let user_stacks_end = sym_addr(&__user_stacks_end);

    // Fill L3 table: 512 entries covering 0x4000_0000 – 0x401F_FFFF
    let base: usize = 0x4000_0000;
    for i in 0..512 {
        let pa = base + i * 4096;

        let desc = if pa >= user_stacks_start && pa < user_stacks_end {
            // User stacks: RW, EL0-accessible, non-executable
            (pa as u64) | USER_DATA_PAGE
        } else if pa >= text_start && pa < text_end {
            // Shared code (kernel + task): RO, executable by both EL1 and EL0
            // Both PXN=0 and UXN=0 so kernel handlers and EL0 tasks can execute.
            // AP = RO_EL0 (0b11) means both EL1 and EL0 can read.
            // WXN is satisfied because pages are RO (not writable).
            (pa as u64) | SHARED_CODE_PAGE
        } else if pa >= rodata_start && pa < rodata_end {
            // Read-only data: RO, non-executable
            (pa as u64) | KERNEL_RODATA_PAGE
        } else if pa >= data_start && pa < kernel_end {
            // Data + BSS + page tables + kernel stacks: RW, non-executable
            (pa as u64) | KERNEL_DATA_PAGE
        } else if pa < text_start {
            // Before kernel (DTB area etc): RW, non-executable
            (pa as u64) | KERNEL_DATA_PAGE
        } else {
            // Beyond kernel end within this 2MiB: invalid
            0
        };

        write_entry(l3_kernel, i, desc);
    }

    // Replace L2_ram[0] with pointer to L3 table
    write_entry(l2_ram, 0, (l3_kernel as u64) | TABLE);
}

/// Mark the stack guard page as invalid (causes Data Abort on stack overflow)
#[cfg(target_arch = "aarch64")]
unsafe fn set_guard_page() {
    let l3_kernel = table_ptr(3);
    let guard_addr = sym_addr(&__stack_guard);
    let base: usize = 0x4000_0000;

    // Find which L3 entry corresponds to the guard page
    if guard_addr >= base && guard_addr < base + 512 * 4096 {
        let index = (guard_addr - base) / 4096;
        write_entry(l3_kernel, index, 0); // Invalid — Data Abort on access
    }
}

// ─── MMU enable sequence (called from assembly) ───────────────────

/// Full MMU initialization — called from boot.s after BSS clear
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub unsafe extern "C" fn mmu_init() {
    // Sub-phase 1: Build 2MiB identity map
    init_tables_2mib();

    // Sub-phase 2: Refine first 2MiB to 4KB pages with W^X
    refine_kernel_pages();

    // Sub-phase 3: Stack guard page
    set_guard_page();

    // Flush page table writes to memory
    core::arch::asm!(
        "dsb ish",
        "isb",
        options(nomem, nostack)
    );
}

/// Enable MMU — called from assembly after mmu_init()
/// This is kept in Rust for the register constant values, but the actual
/// MSR sequence is in boot.s for precise control over instruction ordering.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub unsafe extern "C" fn mmu_get_config(out: *mut [u64; 4]) {
    let l1 = table_ptr(0);
    (*out)[0] = MAIR_VALUE;
    (*out)[1] = TCR_VALUE;
    (*out)[2] = l1 as u64; // TTBR0
    (*out)[3] = SCTLR_MMU_ON | SCTLR_WXN; // SCTLR bits to set
}
