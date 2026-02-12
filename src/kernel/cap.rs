/// AegisOS Capability Module — Flat bitmask access control
///
/// Each task holds a `CapBits` (u64) bitmask in its TCB.
/// Before dispatching a syscall, the kernel checks that the task's
/// capability mask includes the required bit(s). Unauthorized
/// syscalls → fault (software defect in safety-critical context).
///
/// Design: flat u64 bitmask (not seL4 CSpace) — appropriate for
/// a static 3-task microkernel with no heap and no dynamic cap transfer.

// ─── Types ─────────────────────────────────────────────────────────

/// Capability bitmask — each bit grants one permission.
pub type CapBits = u64;

// ─── Capability bit constants ──────────────────────────────────────

/// Permission to send IPC on endpoint 0
pub const CAP_IPC_SEND_EP0: CapBits = 1 << 0;
/// Permission to receive IPC on endpoint 0
pub const CAP_IPC_RECV_EP0: CapBits = 1 << 1;
/// Permission to send IPC on endpoint 1
pub const CAP_IPC_SEND_EP1: CapBits = 1 << 2;
/// Permission to receive IPC on endpoint 1
pub const CAP_IPC_RECV_EP1: CapBits = 1 << 3;
/// Permission to write to UART (SYS_WRITE)
pub const CAP_WRITE: CapBits        = 1 << 4;
/// Permission to yield CPU (SYS_YIELD)
pub const CAP_YIELD: CapBits        = 1 << 5;
/// Permission to send notifications (SYS_NOTIFY)
pub const CAP_NOTIFY: CapBits       = 1 << 6;
/// Permission to wait for notifications (SYS_WAIT_NOTIFY)
pub const CAP_WAIT_NOTIFY: CapBits  = 1 << 7;
/// Permission to send IPC on endpoint 2
pub const CAP_IPC_SEND_EP2: CapBits = 1 << 8;
/// Permission to receive IPC on endpoint 2
pub const CAP_IPC_RECV_EP2: CapBits = 1 << 9;
/// Permission to send IPC on endpoint 3
pub const CAP_IPC_SEND_EP3: CapBits = 1 << 10;
/// Permission to receive IPC on endpoint 3
pub const CAP_IPC_RECV_EP3: CapBits = 1 << 11;
/// Permission to create shared memory grants (SYS_GRANT_CREATE)
pub const CAP_GRANT_CREATE: CapBits = 1 << 12;
/// Permission to revoke shared memory grants (SYS_GRANT_REVOKE)
pub const CAP_GRANT_REVOKE: CapBits = 1 << 13;
/// Permission to bind an IRQ to a notification (SYS_IRQ_BIND)
pub const CAP_IRQ_BIND: CapBits = 1 << 14;
/// Permission to acknowledge an IRQ (SYS_IRQ_ACK)
pub const CAP_IRQ_ACK: CapBits = 1 << 15;
/// Permission to map a device's MMIO into user-space (SYS_DEVICE_MAP)
pub const CAP_DEVICE_MAP: CapBits = 1 << 16;
/// Permission to register watchdog heartbeat (SYS_HEARTBEAT)
pub const CAP_HEARTBEAT: CapBits = 1 << 17;

// ─── Convenience combos ────────────────────────────────────────────

/// All capabilities (for privileged tasks)
pub const CAP_ALL: CapBits = CAP_IPC_SEND_EP0
    | CAP_IPC_RECV_EP0
    | CAP_IPC_SEND_EP1
    | CAP_IPC_RECV_EP1
    | CAP_WRITE
    | CAP_YIELD
    | CAP_NOTIFY
    | CAP_WAIT_NOTIFY
    | CAP_IPC_SEND_EP2
    | CAP_IPC_RECV_EP2
    | CAP_IPC_SEND_EP3
    | CAP_IPC_RECV_EP3
    | CAP_GRANT_CREATE
    | CAP_GRANT_REVOKE
    | CAP_IRQ_BIND
    | CAP_IRQ_ACK
    | CAP_DEVICE_MAP
    | CAP_HEARTBEAT;

/// No capabilities
pub const CAP_NONE: CapBits = 0;

// ─── Core functions ────────────────────────────────────────────────

/// Check whether `caps` includes all bits in `required`.
/// Returns `true` if the task has the required capability.
///
/// O(1), pure, no side effects — safe for use in hot path.
#[inline]
pub fn cap_check(caps: CapBits, required: CapBits) -> bool {
    (caps & required) == required
}

/// Map a syscall number + endpoint ID to the required capability bit(s).
///
/// Syscall ABI: x7 = syscall_nr, x6 = endpoint_id.
/// Returns 0 if the syscall/endpoint combo is unrecognized (caller
/// should treat as "no cap can grant this" → fault).
pub fn cap_for_syscall(syscall_nr: u64, ep_id: u64) -> CapBits {
    match syscall_nr {
        // SYS_YIELD = 0
        0 => CAP_YIELD,
        // SYS_SEND = 1
        1 => match ep_id {
            0 => CAP_IPC_SEND_EP0,
            1 => CAP_IPC_SEND_EP1,
            2 => CAP_IPC_SEND_EP2,
            3 => CAP_IPC_SEND_EP3,
            _ => 0, // invalid endpoint
        },
        // SYS_RECV = 2
        2 => match ep_id {
            0 => CAP_IPC_RECV_EP0,
            1 => CAP_IPC_RECV_EP1,
            2 => CAP_IPC_RECV_EP2,
            3 => CAP_IPC_RECV_EP3,
            _ => 0,
        },
        // SYS_CALL = 3: needs both send and recv on the endpoint
        3 => match ep_id {
            0 => CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0,
            1 => CAP_IPC_SEND_EP1 | CAP_IPC_RECV_EP1,
            2 => CAP_IPC_SEND_EP2 | CAP_IPC_RECV_EP2,
            3 => CAP_IPC_SEND_EP3 | CAP_IPC_RECV_EP3,
            _ => 0,
        },
        // SYS_WRITE = 4
        4 => CAP_WRITE,
        // SYS_NOTIFY = 5
        5 => CAP_NOTIFY,
        // SYS_WAIT_NOTIFY = 6
        6 => CAP_WAIT_NOTIFY,
        // SYS_GRANT_CREATE = 7
        7 => CAP_GRANT_CREATE,
        // SYS_GRANT_REVOKE = 8
        8 => CAP_GRANT_REVOKE,
        // SYS_IRQ_BIND = 9
        9 => CAP_IRQ_BIND,
        // SYS_IRQ_ACK = 10
        10 => CAP_IRQ_ACK,
        // SYS_DEVICE_MAP = 11
        11 => CAP_DEVICE_MAP,
        // SYS_HEARTBEAT = 12
        12 => CAP_HEARTBEAT,
        // Unknown syscall — no valid cap
        _ => 0,
    }
}

/// Return a human-readable name for a single capability bit.
/// Used for UART debug output when denying a syscall.
pub fn cap_name(cap: CapBits) -> &'static str {
    match cap {
        CAP_IPC_SEND_EP0 => "IPC_SEND_EP0",
        CAP_IPC_RECV_EP0 => "IPC_RECV_EP0",
        CAP_IPC_SEND_EP1 => "IPC_SEND_EP1",
        CAP_IPC_RECV_EP1 => "IPC_RECV_EP1",
        CAP_WRITE         => "WRITE",
        CAP_YIELD         => "YIELD",
        CAP_NOTIFY        => "NOTIFY",
        CAP_WAIT_NOTIFY   => "WAIT_NOTIFY",
        CAP_IPC_SEND_EP2  => "IPC_SEND_EP2",
        CAP_IPC_RECV_EP2  => "IPC_RECV_EP2",
        CAP_IPC_SEND_EP3  => "IPC_SEND_EP3",
        CAP_IPC_RECV_EP3  => "IPC_RECV_EP3",
        CAP_GRANT_CREATE  => "GRANT_CREATE",
        CAP_GRANT_REVOKE  => "GRANT_REVOKE",
        CAP_IRQ_BIND      => "IRQ_BIND",
        CAP_IRQ_ACK       => "IRQ_ACK",
        CAP_DEVICE_MAP    => "DEVICE_MAP",
        CAP_HEARTBEAT     => "HEARTBEAT",
        CAP_ALL           => "ALL",
        CAP_NONE          => "NONE",
        _                 => "UNKNOWN",
    }
}

// ─── Kani formal verification proofs ───────────────────────────────

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove: cap_check is a pure bitwise AND — always consistent.
    /// Property 1: cap_check(caps, 0) is always true (vacuous).
    /// Property 2: cap_check(0, required) is false for any non-zero required.
    /// Property 3: cap_check is exactly `(caps & required) == required`.
    #[kani::proof]
    fn cap_check_bitwise_correctness() {
        let caps: u64 = kani::any();
        let required: u64 = kani::any();

        // Definitional equivalence
        assert_eq!(cap_check(caps, required), (caps & required) == required);

        // Vacuous truth: zero required always passes
        assert!(cap_check(caps, 0));

        // Zero caps fails for any non-zero required
        if required != 0 {
            assert!(!cap_check(0, required));
        }
    }

    /// Prove: cap_for_syscall never panics and returns only valid cap bits.
    /// For all valid syscall numbers (0..=12) and endpoints (0..=3),
    /// the returned bitmask is a subset of CAP_ALL (0x3FFFF).
    #[kani::proof]
    fn cap_for_syscall_no_panic_and_bounded() {
        let nr: u64 = kani::any();
        let ep: u64 = kani::any();
        kani::assume(nr <= 12);
        kani::assume(ep <= 3);

        let result = cap_for_syscall(nr, ep);

        // Result must be a subset of defined capability bits
        assert!(
            result & !CAP_ALL == 0,
            "cap_for_syscall returned bits outside CAP_ALL"
        );
    }
}
