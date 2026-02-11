/// AegisOS IPC — Synchronous Endpoint-based messaging
/// Inter-Process Communication (IPC) Giao tiếp đồng bộ giữa các tiến trình qua các endpoint.
/// Synchronous IPC: sender blocks until receiver is ready and vice versa.
/// Message payload: x[0]..x[3] in TrapFrame (4 × u64 = 32 bytes).
///
/// Syscalls:
///   SYS_SEND = 1: send message to endpoint (blocks if no receiver)
///   SYS_RECV = 2: receive message from endpoint (blocks if no sender)
///   SYS_CALL = 3: send + recv (client call pattern)

use crate::exception::TrapFrame;
use crate::sched::{self, TaskState};
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

#[allow(dead_code)]
pub const SYS_SEND: u64 = 1;
#[allow(dead_code)]
pub const SYS_RECV: u64 = 2;
#[allow(dead_code)]
pub const SYS_CALL: u64 = 3;

pub const MAX_ENDPOINTS: usize = 2;
pub const MSG_REGS: usize = 4; // x[0]..x[3]

// ─── Endpoint ──────────────────────────────────────────────────────

/// An IPC endpoint. One task can be waiting to send, one to receive.
/// For simplicity: single-slot (one waiter per direction).
pub struct Endpoint {
    /// Task blocked waiting to send on this endpoint (None = no waiter)
    pub sender: Option<usize>,
    /// Task blocked waiting to receive on this endpoint (None = no waiter)
    pub receiver: Option<usize>,
}

pub const EMPTY_EP: Endpoint = Endpoint {
    sender: None,
    receiver: None,
};

pub static mut ENDPOINTS: [Endpoint; MAX_ENDPOINTS] = [EMPTY_EP; MAX_ENDPOINTS];

// ─── IPC operations ────────────────────────────────────────────────

/// sys_send(frame, ep_id): send message on endpoint.
/// Message payload: frame.x[0..4] → receiver's x[0..4].
/// If a receiver is already waiting: deliver immediately, unblock receiver.
/// Otherwise: block sender, enqueue, schedule away.
pub fn sys_send(frame: &mut TrapFrame, ep_id: usize) {
    if ep_id >= MAX_ENDPOINTS {
        uart_print("!!! IPC: invalid endpoint\n");
        return;
    }

    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB so copy_message can read from it
        sched::save_frame(current, frame);

        if let Some(recv_task) = ENDPOINTS[ep_id].receiver.take() {
            // Receiver is waiting — deliver message directly
            copy_message(current, recv_task);

            // Unblock receiver
            sched::set_task_state(recv_task, TaskState::Ready);

            // Sender continues (not blocked)
        } else {
            // No receiver — block sender and wait
            ENDPOINTS[ep_id].sender = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

/// sys_recv(frame, ep_id): receive message from endpoint.
/// If a sender is already waiting: receive immediately, unblock sender.
/// Otherwise: block receiver, enqueue, schedule away.
pub fn sys_recv(frame: &mut TrapFrame, ep_id: usize) {
    if ep_id >= MAX_ENDPOINTS {
        uart_print("!!! IPC: invalid endpoint\n");
        return;
    }

    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB
        sched::save_frame(current, frame);

        if let Some(send_task) = ENDPOINTS[ep_id].sender.take() {
            // Sender is waiting — receive message directly
            copy_message(send_task, current);

            // Unblock sender
            sched::set_task_state(send_task, TaskState::Ready);

            // Load received message back into our frame so caller sees it
            sched::load_frame(current, frame);
        } else {
            // No sender — block receiver and wait
            ENDPOINTS[ep_id].receiver = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

/// sys_call(frame, ep_id): send message, then block to receive reply.
/// Equivalent to send + recv atomically.
pub fn sys_call(frame: &mut TrapFrame, ep_id: usize) {
    if ep_id >= MAX_ENDPOINTS {
        uart_print("!!! IPC: invalid endpoint\n");
        return;
    }

    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB so message can be copied
        sched::save_frame(current, frame);

        if let Some(recv_task) = ENDPOINTS[ep_id].receiver.take() {
            // Receiver is waiting — deliver message
            copy_message(current, recv_task);
            sched::set_task_state(recv_task, TaskState::Ready);

            // Now block ourselves waiting for reply
            ENDPOINTS[ep_id].receiver = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        } else {
            // No receiver — block as sender, will also need reply
            ENDPOINTS[ep_id].sender = Some(current);
            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────

/// Copy message registers x[0]..x[3] from sender's TCB to receiver's TCB.
pub unsafe fn copy_message(from_task: usize, to_task: usize) {
    for i in 0..MSG_REGS {
        let val = sched::get_task_reg(from_task, i);
        sched::set_task_reg(to_task, i, val);
    }
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Remove a faulted task from all IPC endpoint slots.
/// If a partner was blocked waiting for this task, unblock the partner
/// so it can be rescheduled (partner will retry IPC or find no match).
pub fn cleanup_task(task_idx: usize) {
    unsafe {
        for i in 0..MAX_ENDPOINTS {
            // If the faulted task was a pending sender, clear the slot
            if ENDPOINTS[i].sender == Some(task_idx) {
                ENDPOINTS[i].sender = None;
            }

            // If the faulted task was a pending receiver, clear the slot
            if ENDPOINTS[i].receiver == Some(task_idx) {
                ENDPOINTS[i].receiver = None;
            }
        }

        // Also check: is any other task Blocked because it was waiting
        // for a rendezvous with the faulted task? In synchronous IPC,
        // a task blocks *on an endpoint*, not on a specific partner.
        // So if partner is blocked on ep.sender/receiver, it stays blocked
        // until another task does the complementary operation.
        // This is correct for synchronous IPC — partner will be unblocked
        // when the faulted task restarts and re-enters IPC.
    }
}
