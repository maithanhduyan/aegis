//! AegisOS User-Space Syscall Library
//!
//! Single source of truth for all syscall wrappers + constants.
//! Every user binary depends on this crate instead of duplicating asm.
//!
//! Syscall ABI: x7 = syscall number, x6 = endpoint ID, x0–x3 = payload.

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

// ─── Syscall Numbers ───────────────────────────────────────────────

pub const SYS_YIELD: u64 = 0;
pub const SYS_SEND: u64 = 1;
pub const SYS_RECV: u64 = 2;
pub const SYS_CALL: u64 = 3;
pub const SYS_WRITE: u64 = 4;
pub const SYS_NOTIFY: u64 = 5;
pub const SYS_WAIT_NOTIFY: u64 = 6;
pub const SYS_GRANT_CREATE: u64 = 7;
pub const SYS_GRANT_REVOKE: u64 = 8;
pub const SYS_IRQ_BIND: u64 = 9;
pub const SYS_IRQ_ACK: u64 = 10;
pub const SYS_DEVICE_MAP: u64 = 11;
pub const SYS_HEARTBEAT: u64 = 12;
pub const SYS_EXIT: u64 = 13;

// ─── Syscall Wrappers ──────────────────────────────────────────────

/// SYS_YIELD (syscall #0): voluntarily yield the CPU.
#[inline(always)]
pub fn syscall_yield() {
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "mov x7, #0",
            "svc #0",
            out("x7") _,
            options(nomem, nostack)
        );
    }
}

/// SYS_SEND (syscall #1): send message on endpoint.
#[inline(always)]
pub fn syscall_send(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) {
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") SYS_SEND,
            options(nomem, nostack)
        );
    }
}

/// SYS_RECV (syscall #2): receive message from endpoint.
/// Returns first message register (x0).
#[inline(always)]
pub fn syscall_recv(ep_id: u64) -> u64 {
    let msg0: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") SYS_RECV,
            lateout("x0") msg0,
            options(nomem, nostack)
        );
    }
    msg0
}

/// SYS_RECV variant returning first two message registers (x0, x1).
#[inline(always)]
pub fn syscall_recv2(ep_id: u64) -> (u64, u64) {
    let msg0: u64;
    let msg1: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") SYS_RECV,
            lateout("x0") msg0,
            lateout("x1") msg1,
            options(nomem, nostack)
        );
    }
    (msg0, msg1)
}

/// SYS_CALL (syscall #3): send message then wait for reply.
#[inline(always)]
pub fn syscall_call(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) -> u64 {
    let reply0: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") SYS_CALL,
            lateout("x0") reply0,
            options(nomem, nostack)
        );
    }
    reply0
}

/// SYS_WRITE (syscall #4): write string to UART via kernel.
#[inline(always)]
pub fn syscall_write(buf: *const u8, len: usize) {
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") buf as u64,
            in("x1") len as u64,
            in("x7") SYS_WRITE,
            options(nomem, nostack)
        );
    }
}

/// Print a string from EL0 via SYS_WRITE syscall.
#[inline(always)]
pub fn print(s: &str) {
    syscall_write(s.as_ptr(), s.len());
}

/// SYS_NOTIFY (syscall #5): send notification bitmask to target task.
#[inline(always)]
pub fn syscall_notify(target_id: u64, bits: u64) {
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") bits,
            in("x6") target_id,
            in("x7") SYS_NOTIFY,
            options(nomem, nostack)
        );
    }
}

/// SYS_WAIT_NOTIFY (syscall #6): block until notification arrives.
/// Returns the pending bitmask in x0.
#[inline(always)]
pub fn syscall_wait_notify() -> u64 {
    let bits: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x7") SYS_WAIT_NOTIFY,
            lateout("x0") bits,
            options(nomem, nostack)
        );
    }
    bits
}

/// SYS_GRANT_CREATE (syscall #7): create shared memory grant.
/// x0 = grant_id, x6 = peer_task_id. Returns result in x0.
#[inline(always)]
pub fn syscall_grant_create(grant_id: u64, peer_task_id: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") grant_id,
            in("x6") peer_task_id,
            in("x7") SYS_GRANT_CREATE,
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_GRANT_REVOKE (syscall #8): revoke shared memory grant.
/// x0 = grant_id. Returns result in x0.
#[inline(always)]
pub fn syscall_grant_revoke(grant_id: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") grant_id,
            in("x7") SYS_GRANT_REVOKE,
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_IRQ_BIND (syscall #9): bind an IRQ INTID to a notification bit.
/// x0 = intid (≥32, SPIs only), x1 = notify_bit. Returns result in x0.
#[inline(always)]
pub fn syscall_irq_bind(intid: u64, notify_bit: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") intid,
            in("x1") notify_bit,
            in("x7") SYS_IRQ_BIND,
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_IRQ_ACK (syscall #10): acknowledge an IRQ handled, re-enable INTID.
/// x0 = intid. Returns result in x0.
#[inline(always)]
pub fn syscall_irq_ack(intid: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") intid,
            in("x7") SYS_IRQ_ACK,
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_DEVICE_MAP (syscall #11): map device MMIO into user-space.
/// x0 = device_id (0 = UART0). Returns result in x0.
#[inline(always)]
pub fn syscall_device_map(device_id: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") device_id,
            in("x7") SYS_DEVICE_MAP,
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_HEARTBEAT (syscall #12): register/refresh watchdog heartbeat.
/// x0 = heartbeat interval in ticks (0 = disable). Returns result in x0.
#[inline(always)]
pub fn syscall_heartbeat(interval: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") interval,
            in("x7") SYS_HEARTBEAT,
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_EXIT (syscall #13): graceful task exit.
/// x0 = exit_code. Task is cleaned up and never returns.
#[inline(always)]
pub fn syscall_exit(code: u64) -> ! {
    // SAFETY: SVC triggers synchronous exception handled by kernel.
    // Kernel sets TaskState::Exited and schedules away — this never returns.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") code,
            in("x7") SYS_EXIT,
            options(nomem, nostack, noreturn)
        );
    }
}
