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
use crate::kernel::cell::KernelCell;
use crate::sched::{self, TaskState};
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

#[allow(dead_code)]
pub const SYS_SEND: u64 = 1;
#[allow(dead_code)]
pub const SYS_RECV: u64 = 2;
#[allow(dead_code)]
pub const SYS_CALL: u64 = 3;

pub const MAX_ENDPOINTS: usize = 4;
pub const MSG_REGS: usize = 4; // x[0]..x[3]
pub const MAX_WAITERS: usize = 4; // max senders queued per endpoint

// ─── Endpoint ──────────────────────────────────────────────────────

/// Circular queue for sender waiters on an endpoint.
pub struct SenderQueue {
    pub tasks: [usize; MAX_WAITERS],
    pub head: usize, // index of next task to dequeue
    pub count: usize, // number of tasks in queue
}

impl SenderQueue {
    pub const fn new() -> Self {
        SenderQueue { tasks: [0; MAX_WAITERS], head: 0, count: 0 }
    }

    /// Push a task index. Returns false if queue is full.
    pub fn push(&mut self, task: usize) -> bool {
        if self.count >= MAX_WAITERS { return false; }
        let tail = (self.head + self.count) % MAX_WAITERS;
        self.tasks[tail] = task;
        self.count += 1;
        true
    }

    /// Pop the next sender. Returns None if empty.
    pub fn pop(&mut self) -> Option<usize> {
        if self.count == 0 { return None; }
        let task = self.tasks[self.head];
        self.head = (self.head + 1) % MAX_WAITERS;
        self.count -= 1;
        Some(task)
    }

    /// Remove a specific task from the queue (for cleanup).
    pub fn remove(&mut self, task: usize) {
        // Rebuild queue without the target task
        let old_count = self.count;
        let old_head = self.head;
        let mut new_tasks = [0usize; MAX_WAITERS];
        let mut new_count = 0usize;
        for i in 0..old_count {
            let idx = (old_head + i) % MAX_WAITERS;
            if self.tasks[idx] != task {
                new_tasks[new_count] = self.tasks[idx];
                new_count += 1;
            }
        }
        self.tasks = new_tasks;
        self.head = 0;
        self.count = new_count;
    }

    /// Check if a task is in the queue.
    pub fn contains(&self, task: usize) -> bool {
        for i in 0..self.count {
            let idx = (self.head + i) % MAX_WAITERS;
            if self.tasks[idx] == task { return true; }
        }
        false
    }
}

/// An IPC endpoint. Multiple senders can queue, one receiver waits.
pub struct Endpoint {
    /// Circular queue of tasks blocked waiting to send on this endpoint
    pub sender_queue: SenderQueue,
    /// Task blocked waiting to receive on this endpoint (None = no waiter)
    pub receiver: Option<usize>,
}

pub const EMPTY_EP: Endpoint = Endpoint {
    sender_queue: SenderQueue { tasks: [0; MAX_WAITERS], head: 0, count: 0 },
    receiver: None,
};

pub static ENDPOINTS: KernelCell<[Endpoint; MAX_ENDPOINTS]> = KernelCell::new([EMPTY_EP; MAX_ENDPOINTS]);

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

    // SAFETY: Single-core kernel, interrupts masked during kernel execution.
    // No concurrent access on uniprocessor QEMU virt.
    // Accesses KernelCell ENDPOINTS; calls sched functions that access TCBS/CURRENT.
    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB so copy_message can read from it
        sched::save_frame(current, frame);

        if let Some(recv_task) = (*ENDPOINTS.get_mut())[ep_id].receiver.take() {
            // Receiver is waiting — deliver message directly
            copy_message(current, recv_task);

            // Phase K4: Restore receiver's base priority if it was boosted
            sched::restore_base_priority(recv_task);

            // Unblock receiver
            sched::set_task_state(recv_task, TaskState::Ready);

            // Sender continues (not blocked)
        } else {
            // No receiver — enqueue sender and block
            if !(*ENDPOINTS.get_mut())[ep_id].sender_queue.push(current) {
                uart_print("!!! IPC: sender queue full\n");
                return;
            }
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

    // SAFETY: Single-core kernel, interrupts masked during kernel execution.
    // No concurrent access on uniprocessor QEMU virt.
    // Accesses KernelCell ENDPOINTS; calls sched functions that access TCBS/CURRENT.
    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB
        sched::save_frame(current, frame);

        if let Some(send_task) = (*ENDPOINTS.get_mut())[ep_id].sender_queue.pop() {
            // Sender is waiting — receive message directly
            copy_message(send_task, current);

            // Phase K4: Restore sender's base priority if it was boosted
            sched::restore_base_priority(send_task);

            // Unblock sender
            sched::set_task_state(send_task, TaskState::Ready);

            // Load received message back into our frame so caller sees it
            sched::load_frame(current, frame);
        } else {
            // No sender — block receiver and wait
            (*ENDPOINTS.get_mut())[ep_id].receiver = Some(current);
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

    // SAFETY: Single-core kernel, interrupts masked during kernel execution.
    // No concurrent access on uniprocessor QEMU virt.
    // Accesses KernelCell ENDPOINTS; calls sched functions that access TCBS/CURRENT.
    unsafe {
        let current = sched::current_task_id() as usize;

        // Save current frame to TCB so message can be copied
        sched::save_frame(current, frame);

        if let Some(recv_task) = (*ENDPOINTS.get_mut())[ep_id].receiver.take() {
            // Receiver is waiting — deliver message
            copy_message(current, recv_task);

            // Phase K4: Restore receiver's base priority if it was boosted
            sched::restore_base_priority(recv_task);

            sched::set_task_state(recv_task, TaskState::Ready);

            // Now block ourselves waiting for reply
            (*ENDPOINTS.get_mut())[ep_id].receiver = Some(current);

            // Phase K4: Boost the receiver if our priority is higher
            // (prevents priority inversion during call-reply pattern)
            maybe_boost_priority(current, recv_task);

            sched::set_task_state(current, TaskState::Blocked);
            sched::schedule(frame);
        } else {
            // No receiver — enqueue as sender, will also need reply
            if !(*ENDPOINTS.get_mut())[ep_id].sender_queue.push(current) {
                uart_print("!!! IPC: sender queue full\n");
                return;
            }
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

// ─── Phase K4: Priority Inheritance helper ─────────────────────────

/// Boost a target task's priority if the current task has higher priority.
/// This prevents priority inversion: if a high-priority task is waiting
/// on a low-priority task, the low-priority task inherits the higher
/// priority temporarily so it can complete its work faster.
fn maybe_boost_priority(blocker: usize, holder: usize) {
    let blocker_prio = sched::get_task_priority(blocker);
    let holder_prio = sched::get_task_priority(holder);
    if blocker_prio > holder_prio {
        sched::set_task_priority(holder, blocker_prio);
    }
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Remove a faulted task from all IPC endpoint slots.
/// If a partner was blocked waiting for this task, unblock the partner
/// so it can be rescheduled (partner will retry IPC or find no match).
pub fn cleanup_task(task_idx: usize) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution.
    // No concurrent access on uniprocessor QEMU virt.
    // Accesses KernelCell ENDPOINTS to clear faulted task from all endpoint slots.
    unsafe {
        for i in 0..MAX_ENDPOINTS {
            // If the faulted task was a pending sender, remove from queue
            (*ENDPOINTS.get_mut())[i].sender_queue.remove(task_idx);

            // If the faulted task was a pending receiver, clear the slot
            if (*ENDPOINTS.get_mut())[i].receiver == Some(task_idx) {
                (*ENDPOINTS.get_mut())[i].receiver = None;
            }
        }
    }
}
