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

// ─── Pure functions for Kani verification ──────────────────────────

/// Pure copy_message: copy MSG_REGS elements from src to dst array.
/// Mirrors copy_message() but operates on explicit arrays.
#[cfg(kani)]
fn copy_message_pure(src: &[u64; MSG_REGS], dst: &[u64; MSG_REGS]) -> [u64; MSG_REGS] {
    let mut out = *dst;
    let mut i = 0;
    while i < MSG_REGS {
        out[i] = src[i];
        i += 1;
    }
    out
}

/// Pure cleanup: remove task_id from all sender queues and receiver slots.
/// Mirrors cleanup_task() but operates on explicit endpoint state.
#[cfg(kani)]
fn cleanup_pure(
    sender_tasks: &mut [[usize; MAX_WAITERS]; MAX_ENDPOINTS],
    sender_heads: &mut [usize; MAX_ENDPOINTS],
    sender_counts: &mut [usize; MAX_ENDPOINTS],
    receivers: &mut [Option<usize>; MAX_ENDPOINTS],
    task_id: usize,
) {
    let mut ep = 0;
    while ep < MAX_ENDPOINTS {
        // Remove from sender queue (inline remove logic)
        let old_count = sender_counts[ep];
        let old_head = sender_heads[ep];
        let mut new_tasks = [0usize; MAX_WAITERS];
        let mut new_count = 0usize;
        let mut i = 0;
        while i < old_count {
            let idx = (old_head + i) % MAX_WAITERS;
            if sender_tasks[ep][idx] != task_id {
                new_tasks[new_count] = sender_tasks[ep][idx];
                new_count += 1;
            }
            i += 1;
        }
        sender_tasks[ep] = new_tasks;
        sender_heads[ep] = 0;
        sender_counts[ep] = new_count;

        // Remove from receiver slot
        if receivers[ep] == Some(task_id) {
            receivers[ep] = None;
        }

        ep += 1;
    }
}

// ─── Kani formal verification proofs ───────────────────────────────

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proof 1: SenderQueue overflow safety.
    /// For all sequences of push/pop on SenderQueue (MAX_WAITERS=4):
    /// - push when full → returns false (no corruption)
    /// - pop when empty → returns None (no panic)
    /// - count is always in [0, MAX_WAITERS]
    /// - head is always in [0, MAX_WAITERS)
    #[kani::proof]
    #[kani::unwind(5)] // MAX_WAITERS=4, need 5 iterations
    fn ipc_queue_no_overflow() {
        let mut q = SenderQueue::new();

        // Push up to MAX_WAITERS elements — all should succeed
        let mut i: usize = 0;
        while i < MAX_WAITERS {
            let task: usize = kani::any();
            let ok = q.push(task);
            assert!(ok, "push should succeed when count < MAX_WAITERS");
            assert!(q.count == i + 1, "count should increment");
            assert!(q.count <= MAX_WAITERS, "count must not exceed MAX_WAITERS");
            assert!(q.head < MAX_WAITERS, "head must be valid index");
            i += 1;
        }

        // Queue is full — next push must fail
        let extra: usize = kani::any();
        let overflow = q.push(extra);
        assert!(!overflow, "push when full must return false");
        assert!(q.count == MAX_WAITERS, "count must stay at MAX_WAITERS after failed push");

        // Pop all elements — should succeed
        let mut j: usize = 0;
        while j < MAX_WAITERS {
            let result = q.pop();
            assert!(result.is_some(), "pop should succeed when count > 0");
            assert!(q.count == MAX_WAITERS - 1 - j, "count should decrement");
            assert!(q.head < MAX_WAITERS, "head must remain valid");
            j += 1;
        }

        // Queue is empty — pop must return None
        let empty = q.pop();
        assert!(empty.is_none(), "pop when empty must return None");
        assert!(q.count == 0, "count must be 0 after draining");
    }

    /// Proof 2: IPC message integrity.
    /// For all message payloads (x0–x3), copy_message_pure transfers
    /// the exact values from sender to receiver without corruption.
    #[kani::proof]
    fn ipc_message_integrity() {
        let src: [u64; MSG_REGS] = [kani::any(), kani::any(), kani::any(), kani::any()];
        let dst: [u64; MSG_REGS] = [kani::any(), kani::any(), kani::any(), kani::any()];

        let result = copy_message_pure(&src, &dst);

        // Every register must match the source exactly
        assert_eq!(result[0], src[0], "x0 must match sender");
        assert_eq!(result[1], src[1], "x1 must match sender");
        assert_eq!(result[2], src[2], "x2 must match sender");
        assert_eq!(result[3], src[3], "x3 must match sender");

        // Original source must be unmodified
        // (Rust borrow rules guarantee this, but Kani confirms the logic)
    }

    /// Proof 3: Cleanup completeness.
    /// For all task_id ∈ [0, NUM_TASKS), after cleanup_pure:
    /// - No sender queue in any endpoint contains task_id
    /// - No receiver slot in any endpoint references task_id
    #[kani::proof]
    #[kani::unwind(5)] // MAX_ENDPOINTS=4, MAX_WAITERS=4
    fn ipc_cleanup_completeness() {
        let task_id: usize = kani::any();
        kani::assume(task_id < sched::NUM_TASKS);

        // Set up symbolic endpoint state
        let mut sender_tasks = [[0usize; MAX_WAITERS]; MAX_ENDPOINTS];
        let mut sender_heads = [0usize; MAX_ENDPOINTS];
        let mut sender_counts = [0usize; MAX_ENDPOINTS];
        let mut receivers = [None::<usize>; MAX_ENDPOINTS];

        // Initialize with constrained symbolic values
        let mut ep = 0;
        while ep < MAX_ENDPOINTS {
            sender_counts[ep] = kani::any();
            kani::assume(sender_counts[ep] <= MAX_WAITERS);
            sender_heads[ep] = kani::any();
            kani::assume(sender_heads[ep] < MAX_WAITERS);

            let mut w = 0;
            while w < MAX_WAITERS {
                sender_tasks[ep][w] = kani::any();
                kani::assume(sender_tasks[ep][w] < sched::NUM_TASKS);
                w += 1;
            }

            let has_recv: bool = kani::any();
            if has_recv {
                let recv_id: usize = kani::any();
                kani::assume(recv_id < sched::NUM_TASKS);
                receivers[ep] = Some(recv_id);
            }

            ep += 1;
        }

        // Perform cleanup
        cleanup_pure(
            &mut sender_tasks,
            &mut sender_heads,
            &mut sender_counts,
            &mut receivers,
            task_id,
        );

        // Verify: task_id is NOT in any sender queue
        let mut ep2 = 0;
        while ep2 < MAX_ENDPOINTS {
            let mut i = 0;
            while i < sender_counts[ep2] {
                let idx = (sender_heads[ep2] + i) % MAX_WAITERS;
                assert!(
                    sender_tasks[ep2][idx] != task_id,
                    "cleanup must remove task from all sender queues"
                );
                i += 1;
            }

            // Verify: task_id is NOT a receiver
            assert!(
                receivers[ep2] != Some(task_id),
                "cleanup must remove task from all receiver slots"
            );

            ep2 += 1;
        }
    }
}
