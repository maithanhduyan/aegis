/// AegisOS IRQ Routing Module
///
/// Routes hardware interrupts (SPIs, INTID ≥ 32) to user tasks via
/// the notification system. A task registers interest in an INTID
/// by calling SYS_IRQ_BIND; the kernel masks/unmasks the interrupt
/// and delivers a notification bit when it fires.
///
/// Flow:
///   1. Task → SYS_IRQ_BIND(intid, notify_bit) → kernel enables INTID
///   2. HW IRQ fires → irq_route() → notify task, mask INTID
///   3. Task handles device → SYS_IRQ_ACK(intid) → kernel unmasks INTID
///
/// Syscalls:
///   SYS_IRQ_BIND = 9:  register to receive IRQ as notification
///   SYS_IRQ_ACK  = 10: acknowledge IRQ handled, re-enable INTID

use crate::kernel::cell::KernelCell;
use crate::sched;
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

/// Maximum number of IRQ bindings (SPI slots)
pub const MAX_IRQ_BINDINGS: usize = 8;

/// Minimum INTID for user-bindable interrupts (SPIs start at 32)
pub const MIN_SPI_INTID: u32 = 32;

// ─── Error codes ───────────────────────────────────────────────────

pub const ERR_INVALID_INTID: u64 = 0xFFFF_1001;
pub const ERR_ALREADY_BOUND: u64 = 0xFFFF_1002;
pub const ERR_TABLE_FULL: u64 = 0xFFFF_1003;
pub const ERR_NOT_BOUND: u64 = 0xFFFF_1004;
pub const ERR_NOT_OWNER: u64 = 0xFFFF_1005;

// ─── IrqBinding struct ────────────────────────────────────────────

/// An IRQ-to-task binding — routes a hardware INTID to a notification.
#[derive(Clone, Copy, Debug)]
pub struct IrqBinding {
    /// Hardware interrupt ID (SPI, ≥ 32)
    pub intid: u32,
    /// Task receiving notifications when this IRQ fires
    pub task_id: usize,
    /// Which bit to OR into notify_pending
    pub notify_bit: u64,
    /// Whether this binding slot is in use
    pub active: bool,
    /// IRQ fired but task hasn't ACK'd yet (INTID masked)
    pub pending_ack: bool,
}

pub const EMPTY_BINDING: IrqBinding = IrqBinding {
    intid: 0,
    task_id: 0,
    notify_bit: 0,
    active: false,
    pending_ack: false,
};

// ─── Static binding table ──────────────────────────────────────────

pub static IRQ_BINDINGS: KernelCell<[IrqBinding; MAX_IRQ_BINDINGS]> =
    KernelCell::new([EMPTY_BINDING; MAX_IRQ_BINDINGS]);

// ─── Core operations ───────────────────────────────────────────────

/// Bind an IRQ (INTID) to a task's notification system.
///
/// Validates:
///   - INTID ≥ 32 (SPIs only; PPIs/SGIs are kernel-reserved)
///   - Not already bound by another task
///   - Table not full
///
/// On success: enables INTID in GIC, returns 0.
pub fn irq_bind(intid: u32, task_id: usize, notify_bit: u64) -> u64 {
    // Reject PPIs/SGIs (INTID < 32), including timer (INTID 30)
    if intid < MIN_SPI_INTID {
        uart_print("!!! IRQ: invalid INTID (< 32)\n");
        return ERR_INVALID_INTID;
    }

    // notify_bit must have exactly one bit set (or at least be non-zero)
    if notify_bit == 0 {
        uart_print("!!! IRQ: notify_bit is zero\n");
        return ERR_INVALID_INTID;
    }

    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        // Check for duplicate: same INTID already bound
        for i in 0..MAX_IRQ_BINDINGS {
            if (*IRQ_BINDINGS.get_mut())[i].active && (*IRQ_BINDINGS.get_mut())[i].intid == intid {
                uart_print("!!! IRQ: INTID already bound\n");
                return ERR_ALREADY_BOUND;
            }
        }

        // Find empty slot
        let mut slot: Option<usize> = None;
        for i in 0..MAX_IRQ_BINDINGS {
            if !(*IRQ_BINDINGS.get_mut())[i].active {
                slot = Some(i);
                break;
            }
        }

        let idx = match slot {
            Some(i) => i,
            None => {
                uart_print("!!! IRQ: binding table full\n");
                return ERR_TABLE_FULL;
            }
        };

        (*IRQ_BINDINGS.get_mut())[idx] = IrqBinding {
            intid,
            task_id,
            notify_bit,
            active: true,
            pending_ack: false,
        };

        // Enable this INTID in the GIC
        #[cfg(target_arch = "aarch64")]
        {
            crate::gic::enable_intid(intid);
        }

        uart_print("[AegisOS] IRQ BIND: INTID ");
        crate::uart_print_hex(intid as u64);
        uart_print(" -> task ");
        crate::uart_print_hex(task_id as u64);
        uart_print(", bit ");
        crate::uart_print_hex(notify_bit);
        uart_print("\n");
    }

    0 // success
}

/// Acknowledge an IRQ, allowing the kernel to unmask it.
///
/// The task must be the one that received the notification.
/// Clears pending_ack and re-enables the INTID in the GIC.
pub fn irq_ack(intid: u32, task_id: usize) -> u64 {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if (*IRQ_BINDINGS.get_mut())[i].active
                && (*IRQ_BINDINGS.get_mut())[i].intid == intid
            {
                if (*IRQ_BINDINGS.get_mut())[i].task_id != task_id {
                    uart_print("!!! IRQ ACK: not the bound task\n");
                    return ERR_NOT_OWNER;
                }

                if !(*IRQ_BINDINGS.get_mut())[i].pending_ack {
                    // Already ACK'd or never fired — no-op
                    return 0;
                }

                (*IRQ_BINDINGS.get_mut())[i].pending_ack = false;

                // Re-enable (unmask) the INTID in GIC
                #[cfg(target_arch = "aarch64")]
                {
                    crate::gic::enable_intid(intid);
                }

                return 0;
            }
        }
    }

    // No binding found for this INTID
    ERR_NOT_BOUND
}

/// Route a hardware IRQ to the bound task (called from exception handler).
///
/// Looks up the INTID in the binding table. If bound:
///   - OR notify_bit into task's notify_pending
///   - If task is waiting on notifications → unblock it
///   - Set pending_ack = true
///   - Mask the INTID until task calls SYS_IRQ_ACK
///
/// If not bound, prints a warning and ignores.
#[cfg(target_arch = "aarch64")]
pub fn irq_route(intid: u32, _frame: &mut crate::exception::TrapFrame) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if (*IRQ_BINDINGS.get_mut())[i].active && (*IRQ_BINDINGS.get_mut())[i].intid == intid {
                let tid = (*IRQ_BINDINGS.get_mut())[i].task_id;
                let bit = (*IRQ_BINDINGS.get_mut())[i].notify_bit;

                // OR notification bit into task's pending mask
                (*sched::TCBS.get_mut())[tid].notify_pending |= bit;

                // If task is waiting for notifications, unblock it
                if (*sched::TCBS.get_mut())[tid].notify_waiting {
                    (*sched::TCBS.get_mut())[tid].notify_waiting = false;
                    (*sched::TCBS.get_mut())[tid].state = sched::TaskState::Ready;
                    // Deliver pending bits into x0
                    let pending = (*sched::TCBS.get_mut())[tid].notify_pending;
                    (*sched::TCBS.get_mut())[tid].context.x[0] = pending;
                    (*sched::TCBS.get_mut())[tid].notify_pending = 0;
                }

                // Mark pending ACK — INTID stays masked until task ACKs
                (*IRQ_BINDINGS.get_mut())[i].pending_ack = true;

                // Mask this INTID until ACK
                crate::gic::disable_intid(intid);

                return;
            }
        }

        // No binding found — log and ignore
        uart_print("!!! IRQ INTID=");
        crate::uart_print_hex(intid as u64);
        uart_print(" (unbound, ignored)\n");
    }
}

/// Stub for host tests — irq_route requires TrapFrame which is AArch64-only.
#[cfg(not(target_arch = "aarch64"))]
pub fn irq_route_test(intid: u32, task_id: usize) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if (*IRQ_BINDINGS.get_mut())[i].active && (*IRQ_BINDINGS.get_mut())[i].intid == intid {
                let tid = (*IRQ_BINDINGS.get_mut())[i].task_id;
                let bit = (*IRQ_BINDINGS.get_mut())[i].notify_bit;

                (*sched::TCBS.get_mut())[tid].notify_pending |= bit;

                if (*sched::TCBS.get_mut())[tid].notify_waiting {
                    (*sched::TCBS.get_mut())[tid].notify_waiting = false;
                    (*sched::TCBS.get_mut())[tid].state = sched::TaskState::Ready;
                    let pending = (*sched::TCBS.get_mut())[tid].notify_pending;
                    (*sched::TCBS.get_mut())[tid].context.x[0] = pending;
                    (*sched::TCBS.get_mut())[tid].notify_pending = 0;
                }

                (*IRQ_BINDINGS.get_mut())[i].pending_ack = true;
                // No GIC on host
                return;
            }
        }
    }
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Clean up all IRQ bindings for a faulted/restarted task.
/// If binding has pending_ack, re-enable the INTID (unmask orphaned IRQ).
pub fn irq_cleanup_task(task_id: usize) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..MAX_IRQ_BINDINGS {
            if (*IRQ_BINDINGS.get_mut())[i].active && (*IRQ_BINDINGS.get_mut())[i].task_id == task_id {
                // If IRQ was masked waiting for ACK, unmask it
                if (*IRQ_BINDINGS.get_mut())[i].pending_ack {
                    #[cfg(target_arch = "aarch64")]
                    {
                        crate::gic::enable_intid((*IRQ_BINDINGS.get_mut())[i].intid);
                    }
                }

                uart_print("[AegisOS] IRQ cleanup: unbind INTID ");
                crate::uart_print_hex((*IRQ_BINDINGS.get_mut())[i].intid as u64);
                uart_print(" from task ");
                crate::uart_print_hex(task_id as u64);
                uart_print("\n");

                // Disable the INTID since no one is listening
                #[cfg(target_arch = "aarch64")]
                {
                    crate::gic::disable_intid((*IRQ_BINDINGS.get_mut())[i].intid);
                }

                (*IRQ_BINDINGS.get_mut())[i] = EMPTY_BINDING;
            }
        }
    }
}

// ─── Pure functions for Kani verification (Phase P) ────────────────

/// Pure irq_bind: validate inputs, find slot, return slot index.
/// Mirrors irq_bind() logic but operates on explicit array.
/// Does NOT touch globals, GIC, or UART.
// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert
#[cfg(kani)]
pub fn irq_bind_pure(
    table: &[IrqBinding; MAX_IRQ_BINDINGS],
    intid: u32,
    task_id: usize,
    notify_bit: u64,
) -> Result<usize, u64> {
    if intid < MIN_SPI_INTID {
        return Err(ERR_INVALID_INTID);
    }
    if notify_bit == 0 {
        return Err(ERR_INVALID_INTID);
    }

    // Check for duplicate INTID
    let mut i: usize = 0;
    while i < MAX_IRQ_BINDINGS {
        if table[i].active && table[i].intid == intid {
            return Err(ERR_ALREADY_BOUND);
        }
        i += 1;
    }

    // Find empty slot
    let mut slot: Option<usize> = None;
    let mut j: usize = 0;
    while j < MAX_IRQ_BINDINGS {
        if !table[j].active {
            slot = Some(j);
            break;
        }
        j += 1;
    }

    match slot {
        Some(idx) => Ok(idx),
        None => Err(ERR_TABLE_FULL),
    }
}

/// Pure irq_route: find binding for INTID, return (task_id, notify_bit).
/// Mirrors irq_route() lookup logic (no side effects).
// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert
#[cfg(kani)]
pub fn irq_route_pure(
    table: &[IrqBinding; MAX_IRQ_BINDINGS],
    intid: u32,
) -> Option<(usize, u64)> {
    let mut i: usize = 0;
    while i < MAX_IRQ_BINDINGS {
        if table[i].active && table[i].intid == intid {
            return Some((table[i].task_id, table[i].notify_bit));
        }
        i += 1;
    }
    None
}

/// Pure irq_cleanup: remove all bindings for a task.
/// Mirrors irq_cleanup_task() logic — returns new array state.
// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert
#[cfg(kani)]
pub fn irq_cleanup_pure(
    table: &[IrqBinding; MAX_IRQ_BINDINGS],
    task_id: usize,
) -> [IrqBinding; MAX_IRQ_BINDINGS] {
    let mut result = *table;
    let mut i: usize = 0;
    while i < MAX_IRQ_BINDINGS {
        if result[i].active && result[i].task_id == task_id {
            result[i] = EMPTY_BINDING;
        }
        i += 1;
    }
    result
}

// ─── Kani formal verification proofs (Phase P) ────────────────────

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proof 4: Route always delivers correct (task_id, notify_bit) for bound INTID.
    /// Constrained: intid 32–127, task_id < NUM_TASKS.
    #[kani::proof]
    #[kani::unwind(9)] // MAX_IRQ_BINDINGS=8, loop needs 9
    fn irq_route_correctness() {
        let mut table = [EMPTY_BINDING; MAX_IRQ_BINDINGS];

        // Set up one known binding
        let slot: usize = kani::any();
        kani::assume(slot < MAX_IRQ_BINDINGS);
        let intid: u32 = kani::any();
        kani::assume(intid >= MIN_SPI_INTID && intid <= 127);
        let task_id: usize = kani::any();
        kani::assume(task_id < crate::sched::NUM_TASKS);
        let notify_bit: u64 = kani::any();
        kani::assume(notify_bit != 0);

        table[slot] = IrqBinding {
            intid,
            task_id,
            notify_bit,
            active: true,
            pending_ack: false,
        };

        // Route should find this binding
        let result = irq_route_pure(&table, intid);
        assert!(result.is_some(), "route must find bound INTID");
        let (tid, bit) = result.unwrap();
        assert_eq!(tid, task_id, "route must return correct task_id");
        assert_eq!(bit, notify_bit, "route must return correct notify_bit");
    }

    /// Proof 5: After cleanup, no active binding references the cleaned task.
    /// Constrained: task_id < NUM_TASKS, intid 32–127.
    #[kani::proof]
    #[kani::unwind(9)] // MAX_IRQ_BINDINGS=8, loop needs 9
    fn irq_no_orphaned_binding() {
        let task_id: usize = kani::any();
        kani::assume(task_id < crate::sched::NUM_TASKS);

        // Symbolic initial state (constrained)
        let mut table = [EMPTY_BINDING; MAX_IRQ_BINDINGS];
        let mut i: usize = 0;
        while i < MAX_IRQ_BINDINGS {
            table[i].active = kani::any();
            if table[i].active {
                table[i].intid = kani::any();
                kani::assume(table[i].intid >= MIN_SPI_INTID && table[i].intid <= 127);
                table[i].task_id = kani::any();
                kani::assume(table[i].task_id < crate::sched::NUM_TASKS);
                table[i].notify_bit = kani::any();
                kani::assume(table[i].notify_bit != 0);
                table[i].pending_ack = kani::any();
            }
            i += 1;
        }

        // Perform cleanup
        let result = irq_cleanup_pure(&table, task_id);

        // PROPERTY: no active binding references task_id
        let mut j: usize = 0;
        while j < MAX_IRQ_BINDINGS {
            if result[j].active {
                assert!(
                    result[j].task_id != task_id,
                    "cleanup must remove all bindings for task"
                );
            }
            j += 1;
        }
    }

    /// Proof 6: Cannot bind the same INTID twice.
    /// Constrained: intid 32–127.
    #[kani::proof]
    #[kani::unwind(9)] // MAX_IRQ_BINDINGS=8, loop needs 9
    fn irq_bind_no_duplicate_intid() {
        let mut table = [EMPTY_BINDING; MAX_IRQ_BINDINGS];

        // First bind succeeds
        let intid: u32 = kani::any();
        kani::assume(intid >= MIN_SPI_INTID && intid <= 127);
        let task1: usize = kani::any();
        kani::assume(task1 < crate::sched::NUM_TASKS);
        let bit1: u64 = kani::any();
        kani::assume(bit1 != 0);

        let slot1 = irq_bind_pure(&table, intid, task1, bit1);
        assert!(slot1.is_ok(), "first bind should succeed");
        let idx1 = slot1.unwrap();

        // Apply the bind
        table[idx1] = IrqBinding {
            intid,
            task_id: task1,
            notify_bit: bit1,
            active: true,
            pending_ack: false,
        };

        // Second bind with same INTID must fail
        let task2: usize = kani::any();
        kani::assume(task2 < crate::sched::NUM_TASKS);
        let bit2: u64 = kani::any();
        kani::assume(bit2 != 0);

        let slot2 = irq_bind_pure(&table, intid, task2, bit2);
        assert!(slot2.is_err(), "duplicate INTID bind must fail");
        assert_eq!(slot2.unwrap_err(), ERR_ALREADY_BOUND);
    }
}
