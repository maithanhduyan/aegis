/// Generic Interrupt Controller (Bộ điều khiển ngắt chung).
/// AegisOS GICv2 Driver — Minimal interrupt controller
///
/// QEMU virt machine GICv2 addresses:
///   GICD (Distributor):   0x0800_0000
///   GICC (CPU Interface): 0x0801_0000
use core::ptr;

// ─── Base addresses ────────────────────────────────────────────────

const GICD_BASE: usize = 0x0800_0000;
const GICC_BASE: usize = 0x0801_0000;

// ─── GICD register offsets ─────────────────────────────────────────

const GICD_CTLR: usize = 0x000;
const GICD_ISENABLER: usize = 0x100; // Set-enable (1 bit per INTID, registers of 32 bits)
const GICD_ICENABLER: usize = 0x180; // Clear-enable (write-1-to-disable, 1 bit per INTID)
const GICD_IPRIORITYR: usize = 0x400; // Priority (1 byte per INTID)

// ─── GICC register offsets ─────────────────────────────────────────

const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;

// ─── Helpers ───────────────────────────────────────────────────────

#[inline(always)]
fn gicd_write(offset: usize, val: u32) {
    unsafe { ptr::write_volatile((GICD_BASE + offset) as *mut u32, val) }
}

#[inline(always)]
fn gicd_read(offset: usize) -> u32 {
    unsafe { ptr::read_volatile((GICD_BASE + offset) as *const u32) }
}

#[inline(always)]
fn gicc_write(offset: usize, val: u32) {
    unsafe { ptr::write_volatile((GICC_BASE + offset) as *mut u32, val) }
}

#[inline(always)]
fn gicc_read(offset: usize) -> u32 {
    unsafe { ptr::read_volatile((GICC_BASE + offset) as *const u32) }
}

#[inline(always)]
fn gicd_write_byte(offset: usize, val: u8) {
    unsafe { ptr::write_volatile((GICD_BASE + offset) as *mut u8, val) }
}

// ─── Public API ────────────────────────────────────────────────────

/// Initialize GICv2: enable distributor + CPU interface, accept all priorities
pub fn init() {
    // 1. Disable distributor while configuring
    gicd_write(GICD_CTLR, 0);

    // 2. Enable distributor
    gicd_write(GICD_CTLR, 1);

    // 3. Set CPU interface: accept all priorities
    gicc_write(GICC_PMR, 0xFF);

    // 4. Enable CPU interface
    gicc_write(GICC_CTLR, 1);
}

/// Enable a specific interrupt ID
pub fn enable_intid(intid: u32) {
    // GICD_ISENABLER[n]: each register covers 32 INTIDs
    let reg_index = (intid / 32) as usize;
    let bit = 1u32 << (intid % 32);
    let offset = GICD_ISENABLER + reg_index * 4;

    let val = gicd_read(offset);
    gicd_write(offset, val | bit);
}

/// Disable (mask) a specific interrupt ID.
/// GICD_ICENABLER uses write-1-to-clear semantics — no read-modify-write needed.
pub fn disable_intid(intid: u32) {
    let reg_index = (intid / 32) as usize;
    let bit = 1u32 << (intid % 32);
    let offset = GICD_ICENABLER + reg_index * 4;
    gicd_write(offset, bit);
}

/// Set priority for a specific INTID (0 = highest, 0xFF = lowest)
pub fn set_priority(intid: u32, priority: u8) {
    // GICD_IPRIORITYR: 1 byte per INTID
    let offset = GICD_IPRIORITYR + intid as usize;
    gicd_write_byte(offset, priority);
}

/// Acknowledge IRQ — read GICC_IAR, returns INTID (1023 = spurious)
pub fn acknowledge() -> u32 {
    gicc_read(GICC_IAR) & 0x3FF // INTID is bits [9:0]
}

/// Signal End-Of-Interrupt for given INTID
pub fn end_interrupt(intid: u32) {
    gicc_write(GICC_EOIR, intid);
}

/// Spurious INTID constant
pub const INTID_SPURIOUS: u32 = 1023;
