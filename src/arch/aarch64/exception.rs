/// AegisOS Exception Handling — AArch64
///
/// Full context save/restore (288-byte TrapFrame), ESR_EL1 dispatch,
/// separate Sync/IRQ paths. TrapFrame layout is ABI-fixed for Phase C.

#[cfg(target_arch = "aarch64")]
use crate::uart_print;
#[cfg(target_arch = "aarch64")]
use crate::uart_print_hex;

// ─── TrapFrame: ABI-fixed layout, 288 bytes ────────────────────────

/// Saved CPU context on exception entry.
/// 36 × u64 = 288 bytes, 16-byte aligned.
/// This layout is shared between Rust and assembly — DO NOT reorder.
#[repr(C)]
pub struct TrapFrame {
    /// x0–x30 general-purpose registers (31 × 8 = 248 bytes)
    pub x: [u64; 31],      // offset   0..248
    /// Saved SP_EL0 (user stack pointer)
    pub sp_el0: u64,        // offset 248
    /// Saved ELR_EL1 (return address)
    pub elr_el1: u64,       // offset 256
    /// Saved SPSR_EL1 (saved processor state)
    pub spsr_el1: u64,      // offset 264
    /// Padding for 16-byte alignment
    pub _pad: [u64; 2],     // offset 272..288
}

/// TrapFrame size — must match assembly
#[allow(dead_code)]
pub const TRAPFRAME_SIZE: usize = 288;

// ─── Exception vector table + save/restore macros ──────────────────

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(r#"
.section .text

/* ═══════════════════════════════════════════════════════════════════
 * Save all registers into a TrapFrame on the current SP.
 * After this macro: x0 = pointer to TrapFrame (= sp).
 * ═══════════════════════════════════════════════════════════════════ */
.macro SAVE_CONTEXT
    /* Allocate TrapFrame on stack */
    sub sp, sp, #288

    /* Save x0–x30 in pairs */
    stp x0,  x1,  [sp, #0]
    stp x2,  x3,  [sp, #16]
    stp x4,  x5,  [sp, #32]
    stp x6,  x7,  [sp, #48]
    stp x8,  x9,  [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30,      [sp, #240]

    /* Save SP_EL0 */
    mrs x9, sp_el0
    str x9, [sp, #248]

    /* Save ELR_EL1 + SPSR_EL1 */
    mrs x9,  elr_el1
    mrs x10, spsr_el1
    stp x9,  x10, [sp, #256]

    /* x0 = pointer to TrapFrame for Rust handler */
    mov x0, sp
.endm

/* ═══════════════════════════════════════════════════════════════════
 * Restore all registers from a TrapFrame on the current SP, then eret.
 * Used for same-EL exceptions (SP stays the same).
 * ═══════════════════════════════════════════════════════════════════ */
.macro RESTORE_CONTEXT
    /* Restore ELR_EL1 + SPSR_EL1 */
    ldp x9,  x10, [sp, #256]
    msr elr_el1, x9
    msr spsr_el1, x10

    /* Restore SP_EL0 */
    ldr x9, [sp, #248]
    msr sp_el0, x9

    /* Restore x0–x30 */
    ldp x0,  x1,  [sp, #0]
    ldp x2,  x3,  [sp, #16]
    ldp x4,  x5,  [sp, #32]
    ldp x6,  x7,  [sp, #48]
    ldp x8,  x9,  [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30,      [sp, #240]

    /* Deallocate TrapFrame */
    add sp, sp, #288

    /* Return from exception — restores PC from ELR, PSTATE from SPSR */
    eret
.endm

/* ═══════════════════════════════════════════════════════════════════
 * Save context for lower-EL (EL0→EL1) exceptions.
 * Load SP from the kernel boot stack top before saving, so all
 * exception handling uses the shared 16KB kernel stack regardless
 * of which task was running. Single-core, no nesting.
 * ═══════════════════════════════════════════════════════════════════ */
.macro SAVE_CONTEXT_LOWER
    /* When exception fires from EL0, CPU sets SP = SP_EL1 (whatever
     * it was when we last eret'd). We must switch to the shared kernel
     * boot stack (__stack_end) for handler execution.
     *
     * Problem: we need to clobber a register to load __stack_end,
     * but we haven't saved anything yet. Solution: save x9 to
     * TPIDR_EL1 (kernel scratch sysreg), switch SP, then save
     * all regs including the real x9 from TPIDR_EL1. */
    msr tpidr_el1, x9          /* stash x9 in kernel scratch reg */
    ldr x9, =__stack_end
    mov sp, x9

    /* Allocate TrapFrame on the kernel stack */
    sub sp, sp, #288

    /* Save x0–x8 */
    stp x0,  x1,  [sp, #0]
    stp x2,  x3,  [sp, #16]
    stp x4,  x5,  [sp, #32]
    stp x6,  x7,  [sp, #48]
    str x8,        [sp, #64]

    /* Recover and save the REAL x9 from TPIDR_EL1 */
    mrs x9, tpidr_el1
    str x9, [sp, #72]

    /* Save x10–x30 */
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30,      [sp, #240]

    /* Save SP_EL0 (user stack pointer) */
    mrs x9, sp_el0
    str x9, [sp, #248]

    /* Save ELR_EL1 + SPSR_EL1 */
    mrs x9,  elr_el1
    mrs x10, spsr_el1
    stp x9,  x10, [sp, #256]

    /* x0 = pointer to TrapFrame for Rust handler */
    mov x0, sp
.endm
.macro RESTORE_CONTEXT_LOWER
    /* Restore ELR_EL1 + SPSR_EL1 */
    ldp x9,  x10, [sp, #256]
    msr elr_el1, x9
    msr spsr_el1, x10

    /* Restore SP_EL0 (user stack for the target task) */
    ldr x9, [sp, #248]
    msr sp_el0, x9

    /* Restore x0–x30 */
    ldp x0,  x1,  [sp, #0]
    ldp x2,  x3,  [sp, #16]
    ldp x4,  x5,  [sp, #32]
    ldp x6,  x7,  [sp, #48]
    ldp x8,  x9,  [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30,      [sp, #240]

    /* Deallocate TrapFrame — SP now at kernel stack top (of OLD task) */
    add sp, sp, #288

    /* Return from exception */
    eret
.endm

/* ═══════════════════════════════════════════════════════════════════
 * Exception Vector Table — 2048-byte aligned, 16 entries × 128 bytes
 * ═══════════════════════════════════════════════════════════════════ */
.balign 2048
.global __exception_vectors
__exception_vectors:

/* ─── Group 0: Current EL with SP_EL0 ─────────────────────────── */
.balign 0x80
    b   _exc_sync_cur_sp0       /* Synchronous */
.balign 0x80
    b   _exc_irq_cur_sp0        /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_serror_stub        /* SError */

/* ─── Group 1: Current EL with SP_ELx ─────────────────────────── */
.balign 0x80
    b   _exc_sync_cur_spx       /* Synchronous */
.balign 0x80
    b   _exc_irq_cur_spx        /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_serror_stub        /* SError */

/* ─── Group 2: Lower EL, AArch64 ──────────────────────────────── */
.balign 0x80
    b   _exc_sync_lower64       /* Synchronous */
.balign 0x80
    b   _exc_irq_lower64        /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_serror_stub        /* SError */

/* ─── Group 3: Lower EL, AArch32 ──────────────────────────────── */
.balign 0x80
    b   _exc_fiq_stub           /* Synchronous (AArch32 not used) */
.balign 0x80
    b   _exc_fiq_stub           /* IRQ */
.balign 0x80
    b   _exc_fiq_stub           /* FIQ */
.balign 0x80
    b   _exc_fiq_stub           /* SError */

/* ═══════════════════════════════════════════════════════════════════
 * Synchronous exception handlers — save context, call Rust, restore
 * ═══════════════════════════════════════════════════════════════════ */

_exc_sync_cur_sp0:
    SAVE_CONTEXT
    mov x1, #0          /* source = 0: current EL, SP_EL0 */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT

_exc_sync_cur_spx:
    SAVE_CONTEXT
    mov x1, #1          /* source = 1: current EL, SP_ELx */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT

_exc_sync_lower64:
    SAVE_CONTEXT_LOWER
    mov x1, #2          /* source = 2: lower EL, AArch64 */
    bl  exception_dispatch_sync
    RESTORE_CONTEXT_LOWER

/* ═══════════════════════════════════════════════════════════════════
 * IRQ handlers — save context, call Rust, restore
 * ═══════════════════════════════════════════════════════════════════ */

_exc_irq_cur_sp0:
    SAVE_CONTEXT
    bl  exception_dispatch_irq
    RESTORE_CONTEXT

_exc_irq_cur_spx:
    SAVE_CONTEXT
    bl  exception_dispatch_irq
    RESTORE_CONTEXT

_exc_irq_lower64:
    SAVE_CONTEXT_LOWER
    bl  exception_dispatch_irq
    RESTORE_CONTEXT_LOWER

/* ═══════════════════════════════════════════════════════════════════
 * FIQ / SError stub — halt safely
 * ═══════════════════════════════════════════════════════════════════ */

_exc_fiq_stub:
    wfe
    b   _exc_fiq_stub

_exc_serror_stub:
    SAVE_CONTEXT
    bl  exception_dispatch_serror
    b   _exc_fiq_stub       /* halt after SError */
"#);

// ─── Rust exception dispatch ───────────────────────────────────────

/// Synchronous exception dispatch — called from assembly with TrapFrame pointer
/// x0 = &mut TrapFrame, x1 = source (0=cur/SP_EL0, 1=cur/SP_ELx, 2=lower64)
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn exception_dispatch_sync(frame: &mut TrapFrame, source: u64) {
    let esr: u64;
    // SAFETY: Reading ESR_EL1 is a read-only system register access at EL1.
    unsafe { core::arch::asm!("mrs {}, esr_el1", out(reg) esr, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;

    match ec {
        0x15 => handle_svc(frame, esr),
        0x20 | 0x21 => handle_instruction_abort(frame, esr, source),
        0x24 | 0x25 => handle_data_abort(frame, esr, source),
        0x07 => handle_fp_trap(frame, esr, source),
        _ => handle_unknown(frame, esr, ec, source),
    }
}

/// IRQ dispatch — acknowledge GIC, dispatch by INTID, EOI
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn exception_dispatch_irq(frame: &mut TrapFrame) {
    let intid = crate::gic::acknowledge();

    if intid == crate::gic::INTID_SPURIOUS {
        return; // spurious, ignore
    }

    match intid {
        crate::timer::TIMER_INTID => crate::timer::tick_handler(frame),
        _ => crate::irq::irq_route(intid, frame),
    }

    crate::gic::end_interrupt(intid);
}

/// SError dispatch — always fatal
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn exception_dispatch_serror(_frame: &mut TrapFrame) {
    uart_print("\n!!! SERROR (fatal) !!!\n");
    // Will halt in assembly stub after return
}

// ─── Individual exception handlers ─────────────────────────────────

/// SVC handler — dispatch syscalls by x7
#[cfg(target_arch = "aarch64")]
fn handle_svc(frame: &mut TrapFrame, _esr: u64) {
    let syscall_nr = frame.x[7];
    let ep_id = frame.x[6];

    // ─── Phase G: Capability check ─────────────────────────────────
    let required = crate::cap::cap_for_syscall(syscall_nr, ep_id);
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let task_caps = unsafe { (*crate::sched::TCBS.get_mut())[*crate::sched::CURRENT.get()].caps };

    if !crate::cap::cap_check(task_caps, required) {
        uart_print("!!! CAP DENIED: task ");
        // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
        uart_print_hex(unsafe { *crate::sched::CURRENT.get() } as u64);
        uart_print(" syscall #");
        uart_print_hex(syscall_nr);
        uart_print(" needs ");
        uart_print(crate::cap::cap_name(required));
        uart_print(" — faulting\n");
        crate::sched::fault_current_task(frame);
        return;
    }

    match syscall_nr {
        // SYS_YIELD = 0: voluntarily yield CPU
        0 => crate::sched::schedule(frame),
        // SYS_SEND = 1: send IPC message (ep_id in x6)
        1 => crate::ipc::sys_send(frame, frame.x[6] as usize),
        // SYS_RECV = 2: receive IPC message (ep_id in x6)
        2 => crate::ipc::sys_recv(frame, frame.x[6] as usize),
        // SYS_CALL = 3: send + receive (ep_id in x6)
        3 => crate::ipc::sys_call(frame, frame.x[6] as usize),
        // SYS_WRITE = 4: write buffer to UART (x0=buf, x1=len)
        4 => handle_sys_write(frame),
        // SYS_NOTIFY = 5: send notification to target (x6=target_id, x0=bitmask)
        5 => handle_notify(frame),
        // SYS_WAIT_NOTIFY = 6: wait for notification (returns pending bits in x0)
        6 => handle_wait_notify(frame),
        // SYS_GRANT_CREATE = 7: create shared memory grant (x0=grant_id, x6=peer_task_id)
        7 => handle_grant_create(frame),
        // SYS_GRANT_REVOKE = 8: revoke shared memory grant (x0=grant_id)
        8 => handle_grant_revoke(frame),
        // SYS_IRQ_BIND = 9: bind IRQ to notification (x0=intid, x1=notify_bit)
        9 => handle_irq_bind(frame),
        // SYS_IRQ_ACK = 10: acknowledge IRQ handled (x0=intid)
        10 => handle_irq_ack(frame),
        // SYS_DEVICE_MAP = 11: map device MMIO into user-space (x0=device_id)
        11 => handle_device_map(frame),
        // SYS_HEARTBEAT = 12: register watchdog heartbeat (x0=interval in ticks)
        12 => handle_heartbeat(frame),
        // SYS_EXIT = 13: graceful task exit (x0=exit_code)
        13 => crate::sched::sys_exit(frame, frame.x[0]),
        _ => {
            uart_print("!!! unknown syscall #");
            uart_print_hex(syscall_nr);
            uart_print(" — faulting task\n");
            crate::sched::fault_current_task(frame);
        }
    }
}

/// SYS_WRITE handler: write bytes to UART on behalf of EL0 task.
/// x0 = pointer to buffer, x1 = length in bytes.
/// Validates that the buffer pointer is in user-accessible memory.
#[cfg(target_arch = "aarch64")]
fn handle_sys_write(frame: &TrapFrame) {
    let buf_ptr = frame.x[0] as usize;
    let len = frame.x[1] as usize;

    let (valid, checked_len) = validate_write_args(buf_ptr, len);
    if !valid {
        if len > 0 {
            uart_print("!!! SYS_WRITE: bad pointer !!!\n");
        }
        return;
    }

    // Safe to read — copy bytes to UART
    for i in 0..checked_len {
        // SAFETY: buf_ptr validated by validate_write_args to be within user-accessible RAM range.
        let byte = unsafe { core::ptr::read_volatile((buf_ptr + i) as *const u8) };
        crate::uart_write(byte);
    }
}

/// SYS_NOTIFY handler: send async notification bits to a target task.
/// x6 = target task ID, x0 = notification bitmask.
/// OR-merges bits into target's notify_pending. If target is blocked
/// in wait_notify, unblock it immediately.
#[cfg(target_arch = "aarch64")]
fn handle_notify(frame: &mut TrapFrame) {
    let target_id = frame.x[6] as usize;
    let bits = frame.x[0];

    if target_id >= crate::sched::NUM_TASKS {
        uart_print("!!! SYS_NOTIFY: invalid target\n");
        frame.x[0] = 0xFFFF_DEAD;
        return;
    }

    if bits == 0 {
        return; // no-op
    }

    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        // OR-merge notification bits into target's pending mask
        (*crate::sched::TCBS.get_mut())[target_id].notify_pending |= bits;

        // If the target is blocked waiting for notifications, unblock it
        if (*crate::sched::TCBS.get_mut())[target_id].notify_waiting {
            (*crate::sched::TCBS.get_mut())[target_id].notify_waiting = false;

            // Deliver pending bits into the target's x0 and clear
            let pending = (*crate::sched::TCBS.get_mut())[target_id].notify_pending;
            (*crate::sched::TCBS.get_mut())[target_id].notify_pending = 0;
            crate::sched::set_task_reg(target_id, 0, pending);

            crate::sched::set_task_state(target_id, crate::sched::TaskState::Ready);
        }
    }
}

/// SYS_WAIT_NOTIFY handler: wait for notification bits.
/// If caller has pending bits: return immediately in x0 and clear.
/// Otherwise: block caller, set notify_waiting=true, schedule away.
#[cfg(target_arch = "aarch64")]
fn handle_wait_notify(frame: &mut TrapFrame) {
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        let current = *crate::sched::CURRENT.get();

        let pending = (*crate::sched::TCBS.get_mut())[current].notify_pending;
        if pending != 0 {
            // Notifications already pending — deliver immediately
            frame.x[0] = pending;
            (*crate::sched::TCBS.get_mut())[current].notify_pending = 0;
        } else {
            // No pending notifications — block and wait
            crate::sched::save_frame(current, frame);
            (*crate::sched::TCBS.get_mut())[current].notify_waiting = true;
            crate::sched::set_task_state(current, crate::sched::TaskState::Blocked);
            crate::sched::schedule(frame);
        }
    }
}

/// SYS_GRANT_CREATE handler: create shared memory grant.
/// x0 = grant_id, x6 = peer_task_id.
/// Returns result in x0 (0 = success, else error code).
#[cfg(target_arch = "aarch64")]
fn handle_grant_create(frame: &mut TrapFrame) {
    let grant_id = frame.x[0] as usize;
    let peer_id = frame.x[6] as usize;
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let current = unsafe { *crate::sched::CURRENT.get() };

    let result = crate::grant::grant_create(grant_id, current, peer_id);
    frame.x[0] = result;
}

/// SYS_GRANT_REVOKE handler: revoke shared memory grant.
/// x0 = grant_id.
/// Returns result in x0 (0 = success, else error code).
#[cfg(target_arch = "aarch64")]
fn handle_grant_revoke(frame: &mut TrapFrame) {
    let grant_id = frame.x[0] as usize;
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let current = unsafe { *crate::sched::CURRENT.get() };

    let result = crate::grant::grant_revoke(grant_id, current);
    frame.x[0] = result;
}

/// SYS_IRQ_BIND handler: bind IRQ INTID to notification bit.
/// x0 = intid, x1 = notify_bit.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
fn handle_irq_bind(frame: &mut TrapFrame) {
    let intid = frame.x[0] as u32;
    let notify_bit = frame.x[1];
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let current = unsafe { *crate::sched::CURRENT.get() };
    let result = crate::irq::irq_bind(intid, current, notify_bit);
    frame.x[0] = result;
}

/// SYS_IRQ_ACK handler: acknowledge IRQ handled, unmask INTID.
/// x0 = intid.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
fn handle_irq_ack(frame: &mut TrapFrame) {
    let intid = frame.x[0] as u32;
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let current = unsafe { *crate::sched::CURRENT.get() };
    let result = crate::irq::irq_ack(intid, current);
    frame.x[0] = result;
}

/// SYS_DEVICE_MAP handler: map device MMIO into user-space.
/// x0 = device_id (0 = UART0).
/// Returns result in x0 (0 = success, else error code).
#[cfg(target_arch = "aarch64")]
fn handle_device_map(frame: &mut TrapFrame) {
    let device_id = frame.x[0];
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let current = unsafe { *crate::sched::CURRENT.get() };
    // SAFETY: Called at EL1, device_id validated by match arm.
    let result = unsafe { crate::mmu::map_device_for_task(device_id, current) };
    frame.x[0] = result;
}

/// SYS_HEARTBEAT handler: register or refresh watchdog heartbeat.
/// x0 = heartbeat interval in ticks (0 = disable watchdog for this task).
/// Updates the task's heartbeat_interval and resets last_heartbeat to now.
#[cfg(target_arch = "aarch64")]
fn handle_heartbeat(frame: &mut TrapFrame) {
    let interval = frame.x[0];
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    let current = unsafe { *crate::sched::CURRENT.get() };
    crate::sched::record_heartbeat(current, interval);
    frame.x[0] = 0; // success
}

/// Instruction Abort handler — fault task if from lower EL, halt if from same EL
#[cfg(target_arch = "aarch64")]
fn handle_instruction_abort(frame: &mut TrapFrame, esr: u64, source: u64) {
    let far: u64;
    // SAFETY: Reading FAR_EL1 is a read-only system register access at EL1.
    unsafe { core::arch::asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;
    let ifsc = esr & 0x3F;

    uart_print("\n!!! INSTRUCTION ABORT !!!");
    if ec == 0x20 {
        // Lower EL (EL0 task) — recoverable fault
        uart_print(" [EL0 task]\n");
        uart_print("  IFSC: 0x");
        uart_print_hex(ifsc);
        print_fault_class(ifsc);
        uart_print("\n  FAR:  0x");
        uart_print_hex(far);
        uart_print("\n  ELR:  0x");
        uart_print_hex(frame.elr_el1);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal, halt
    uart_print(" [KERNEL]\n");
    uart_print("  Source: same EL\n");
    uart_print("  IFSC: 0x");
    uart_print_hex(ifsc);
    print_fault_class(ifsc);
    uart_print("\n  ESR:  0x");
    uart_print_hex(esr);
    uart_print("\n  FAR:  0x");
    uart_print_hex(far);
    uart_print("\n  ELR:  0x");
    uart_print_hex(frame.elr_el1);
    uart_print("\n  src:  ");
    uart_print_hex(source);
    uart_print("\n  HALTED.\n");
    // SAFETY: wfe is a hint instruction that idles the core until next event.
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Data Abort handler — fault task if from lower EL, halt if from same EL
#[cfg(target_arch = "aarch64")]
fn handle_data_abort(frame: &mut TrapFrame, esr: u64, source: u64) {
    let far: u64;
    // SAFETY: Reading FAR_EL1 is a read-only system register access at EL1.
    unsafe { core::arch::asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack)) };

    let ec = (esr >> 26) & 0x3F;
    let dfsc = esr & 0x3F;

    uart_print("\n!!! DATA ABORT !!!");
    if ec == 0x24 {
        // Lower EL (EL0 task) — recoverable fault
        uart_print(" [EL0 task]\n");
        uart_print("  DFSC: 0x");
        uart_print_hex(dfsc);
        print_fault_class(dfsc);
        let wnr = (esr >> 6) & 1;
        if wnr == 1 {
            uart_print("\n  Access: WRITE");
        } else {
            uart_print("\n  Access: READ");
        }
        uart_print("\n  FAR:  0x");
        uart_print_hex(far);
        uart_print("\n  ELR:  0x");
        uart_print_hex(frame.elr_el1);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal, halt
    uart_print(" [KERNEL]\n");
    uart_print("  Source: same EL\n");
    uart_print("  DFSC: 0x");
    uart_print_hex(dfsc);
    print_fault_class(dfsc);
    let wnr = (esr >> 6) & 1;
    if wnr == 1 {
        uart_print("\n  Access: WRITE");
    } else {
        uart_print("\n  Access: READ");
    }
    uart_print("\n  ESR:  0x");
    uart_print_hex(esr);
    uart_print("\n  FAR:  0x");
    uart_print_hex(far);
    uart_print("\n  ELR:  0x");
    uart_print_hex(frame.elr_el1);
    uart_print("\n  src:  ");
    uart_print_hex(source);
    uart_print("\n  HALTED.\n");
    // SAFETY: wfe is a hint instruction that idles the core until next event.
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// FP/SIMD trap — fault task if from lower EL, halt if from same EL
#[cfg(target_arch = "aarch64")]
fn handle_fp_trap(frame: &mut TrapFrame, esr: u64, source: u64) {
    uart_print("\n!!! FP/SIMD TRAP !!!");
    if source == 2 {
        // Lower EL (EL0 task) — recoverable
        uart_print(" [EL0 task]\n");
        uart_print("  Task attempted FP/SIMD instruction.\n");
        uart_print("  ESR: 0x");
        uart_print_hex(esr);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal
    uart_print(" [KERNEL]\n");
    uart_print("  Kernel code attempted FP instruction.\n");
    uart_print("  ESR: 0x");
    uart_print_hex(esr);
    uart_print("\n  HALTED.\n");
    // SAFETY: wfe is a hint instruction that idles the core until next event.
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Unknown/unhandled exception class — fault task if from lower EL, halt if same EL
#[cfg(target_arch = "aarch64")]
fn handle_unknown(frame: &mut TrapFrame, esr: u64, ec: u64, source: u64) {
    uart_print("\n!!! UNHANDLED EXCEPTION !!!");
    if source == 2 {
        // Lower EL (EL0 task) — recoverable
        uart_print(" [EL0 task]\n");
        uart_print("  EC:   0x");
        uart_print_hex(ec);
        uart_print("\n  ESR:  0x");
        uart_print_hex(esr);
        uart_print("\n  ELR:  0x");
        uart_print_hex(frame.elr_el1);
        uart_print("\n");
        crate::sched::fault_current_task(frame);
        return;
    }
    // Same EL (kernel) — fatal
    uart_print(" [KERNEL]\n");
    uart_print("  EC:   0x");
    uart_print_hex(ec);
    uart_print("\n  ESR:  0x");
    uart_print_hex(esr);
    uart_print("\n  ELR:  0x");
    uart_print_hex(frame.elr_el1);
    uart_print("\n  src:  ");
    uart_print_hex(source);
    uart_print("\n  HALTED.\n");
    // SAFETY: wfe is a hint instruction that idles the core until next event.
    loop { unsafe { core::arch::asm!("wfe") } }
}

/// Decode fault status code (DFSC/IFSC) bits [5:0] into human-readable class
#[cfg(target_arch = "aarch64")]
fn print_fault_class(fsc: u64) {
    let level = fsc & 0x3;
    match (fsc >> 2) & 0xF {
        0b0001 => {
            uart_print(" (Translation fault L");
            uart_print_hex(level);
            uart_print(")");
        }
        0b0010 => {
            uart_print(" (Access flag fault L");
            uart_print_hex(level);
            uart_print(")");
        }
        0b0011 => {
            uart_print(" (Permission fault L");
            uart_print_hex(level);
            uart_print(")");
        }
        _ => uart_print(" (other)"),
    }
}

// ─── Init ──────────────────────────────────────────────────────────

/// Install exception vector table — write VBAR_EL1
#[cfg(target_arch = "aarch64")]
pub fn init() {
    extern "C" {
        static __exception_vectors: u8;
    }
    // SAFETY: Writing VBAR_EL1 sets the exception vector base. __exception_vectors is linker-provided and 2KB-aligned.
    unsafe {
        let vbar = &__exception_vectors as *const u8 as u64;
        core::arch::asm!(
            "msr vbar_el1, {v}",
            "isb",
            v = in(reg) vbar,
            options(nomem, nostack)
        );
    }
}

// ─── Pure validation logic (testable on host) ──────────────────────

/// Validate a SYS_WRITE pointer+length from EL0.
/// Returns (valid, clamped_len). Pure function — no side effects.
pub fn validate_write_args(buf_ptr: usize, len: usize) -> (bool, usize) {
    if len == 0 || len > 256 {
        return (false, 0);
    }
    let buf_end = buf_ptr.wrapping_add(len);
    if buf_ptr < 0x4000_0000 || buf_end > 0x4800_0000 || buf_end < buf_ptr {
        return (false, 0);
    }
    (true, len)
}
