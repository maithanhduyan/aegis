/// Thời Khóa Biểu / Bộ lập lịch (Scheduler).
/// AegisOS Scheduler — Fixed-priority preemptive, 3 static tasks
///
/// Tasks run at EL0 (user mode). Each task has:
///   - A TrapFrame (saved/restored on context switch)
///   - Its own 4KB kernel stack (SP_EL1, in .task_stacks section)
///   - Its own 4KB user stack (SP_EL0, in .user_stacks section)
///   - A state (Ready, Running, Inactive)
///   - A priority (0 = lowest, 7 = highest)
///   - A time budget per epoch (0 = unlimited)
///   - A watchdog heartbeat interval (0 = disabled)
///
/// Context switch: timer IRQ → save frame → pick highest-priority Ready → switch SP_EL1 → load frame → eret to EL0

use crate::cap::CapBits;
use crate::exception::TrapFrame;
use crate::uart_print;

// ─── Task state ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum TaskState {
    Inactive = 0,
    Ready    = 1,
    Running  = 2,
    Blocked  = 3,
    Faulted  = 4,
    Exited   = 5,
}

// ─── Task Control Block ────────────────────────────────────────────

/// TCB — one per task. Context is saved/loaded during context switch.
#[repr(C)]
pub struct Tcb {
    pub context: TrapFrame,
    pub state: TaskState,
    pub id: u16,
    pub stack_top: u64,       // top of this task's kernel stack (SP_EL1)
    pub entry_point: u64,     // original entry address (for restart)
    pub user_stack_top: u64,  // original SP_EL0 top (for restart)
    pub fault_tick: u64,      // tick when task was marked Faulted
    pub caps: CapBits,        // capability bitmask (survives restart)
    pub ttbr0: u64,           // TTBR0_EL1 value (ASID << 48 | L1 base)
    pub notify_pending: u64,  // bitmask of pending notification bits
    pub notify_waiting: bool, // true if task is blocked in wait_notify
    // ─── Phase K fields ────────────────────────────────────────────
    pub priority: u8,            // current effective priority (0=lowest, 7=highest)
    pub base_priority: u8,       // original priority (before inheritance)
    pub time_budget: u64,        // max ticks per epoch (0 = unlimited)
    pub ticks_used: u64,         // ticks consumed in current epoch
    pub heartbeat_interval: u64, // max ticks between heartbeats (0 = disabled)
    pub last_heartbeat: u64,     // TICK_COUNT at last heartbeat
}

// ─── Static task table ─────────────────────────────────────────────

/// Task count: 0 = uart_driver, 1 = client, 2..6 = reserved, 7 = idle
pub const NUM_TASKS: usize = 8;

/// Index of the idle task (always the last task slot).
pub const IDLE_TASK_ID: usize = NUM_TASKS - 1;

use crate::kernel::cell::KernelCell;

pub static TCBS: KernelCell<[Tcb; NUM_TASKS]> = KernelCell::new([EMPTY_TCB; NUM_TASKS]);

/// Index of currently running task.
/// Encapsulated in KernelCell (Phase M1) — access via unsafe get()/get_mut().
pub static CURRENT: KernelCell<usize> = KernelCell::new(0);

/// Delay before auto-restarting a faulted task (100 ticks × 10ms = 1 second)
pub const RESTART_DELAY_TICKS: u64 = 100;

/// Phase K: Epoch length in ticks (100 ticks = 1 second)
pub const EPOCH_LENGTH: u64 = 100;

/// Phase K: Epoch tick counter — resets every EPOCH_LENGTH ticks.
/// Encapsulated in KernelCell (Phase M1) — access via unsafe get()/get_mut().
pub static EPOCH_TICKS: KernelCell<u64> = KernelCell::new(0);

/// Phase K: Watchdog scan interval (every 10 ticks = 100ms)
pub const WATCHDOG_SCAN_PERIOD: u64 = 10;

pub const EMPTY_TCB: Tcb = Tcb {
    context: TrapFrame {
        x: [0; 31],
        sp_el0: 0,
        elr_el1: 0,
        spsr_el1: 0,
        _pad: [0; 2],
    },
    state: TaskState::Inactive,
    id: 0,
    stack_top: 0,
    entry_point: 0,
    user_stack_top: 0,
    fault_tick: 0,
    caps: 0,
    ttbr0: 0,
    notify_pending: 0,
    notify_waiting: false,
    priority: 0,
    base_priority: 0,
    time_budget: 0,
    ticks_used: 0,
    heartbeat_interval: 0,
    last_heartbeat: 0,
};

// ─── Task metadata (Phase N) ───────────────────────────────────────

/// Static per-task boot configuration: capabilities, priority, budget.
/// Defined as a const array in main.rs; applied in kernel_main() loop.
pub struct TaskMetadata {
    pub caps: CapBits,
    pub priority: u8,
    pub time_budget: u64,
    pub heartbeat_interval: u64,
}

// ─── Public API ────────────────────────────────────────────────────

/// Initialize scheduler: set up TCBs for NUM_TASKS tasks.
/// `entries[i]` = entry point address for task i.
/// Must be called before enabling timer interrupts.
#[cfg(target_arch = "aarch64")]
pub fn init(entries: &[u64; NUM_TASKS]) {
    extern "C" {
        static __task_stacks_start: u8;  // kernel stacks (SP_EL1)
        static __user_stacks_start: u8;  // user stacks (SP_EL0)
    }

    // SAFETY: Linker-provided symbol, address taken for stack calculation.
    let kstacks_base = unsafe { &__task_stacks_start as *const u8 as u64 };
    // SAFETY: Linker-provided symbol, address taken for stack calculation.
    let ustacks_base = unsafe { &__user_stacks_start as *const u8 as u64 };

    // Each stack is 4KB. Stack grows downward, so top = base + (i+1)*4096
    // SPSR = 0x000 = EL0t: eret drops to EL0, uses SP_EL0
    // When exception from EL0 → EL1, CPU automatically uses SP_EL1
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..NUM_TASKS {
            (*TCBS.get_mut())[i].id = i as u16;
            (*TCBS.get_mut())[i].stack_top = kstacks_base + (i as u64 + 1) * 4096;
            (*TCBS.get_mut())[i].user_stack_top = ustacks_base + (i as u64 + 1) * 4096;
            if entries[i] != 0 {
                // Active task: set entry point and mark Ready
                (*TCBS.get_mut())[i].state = TaskState::Ready;
                (*TCBS.get_mut())[i].entry_point = entries[i];
                (*TCBS.get_mut())[i].context.elr_el1 = entries[i];
                (*TCBS.get_mut())[i].context.spsr_el1 = 0x000; // EL0t
                (*TCBS.get_mut())[i].context.sp_el0 = ustacks_base + (i as u64 + 1) * 4096;
            }
            // else: stays Inactive (from EMPTY_TCB)
        }
    }

    uart_print("[AegisOS] scheduler ready (");
    crate::uart_print_dec(NUM_TASKS as u64);
    uart_print(" tasks, priority-based, EL0)\n");
}

/// Schedule: save current context, pick next Ready task, load its context.
/// Called from timer IRQ handler with the current TrapFrame.
///
/// The trick: we modify `*frame` in-place. When the IRQ handler does
/// RESTORE_CONTEXT, it restores the NEW task's registers, and `eret`
/// jumps to the new task's ELR — completing the context switch.
///
/// SP_EL1 switching: We update CURRENT_KSTACK with the new task's kernel
/// stack top. The RESTORE_CONTEXT_EL0 macro reads this and sets SP
/// before eret, so the next exception from EL0 uses the correct stack.
pub fn schedule(frame: &mut TrapFrame) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    // ptr::copy_nonoverlapping: src and dst are valid pointers to non-overlapping TrapFrame-sized memory within static TCBS array.
    // Inline asm (msr ttbr0_el1): switches address space to new task's page table. Called at EL1.
    unsafe {
        let old = *CURRENT.get();

        // Save current task's context from the TrapFrame
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut (*TCBS.get_mut())[old].context as *mut TrapFrame,
            1,
        );

        // Mark old task as Ready (unless it's Blocked or Faulted)
        if (*TCBS.get_mut())[old].state == TaskState::Running {
            (*TCBS.get_mut())[old].state = TaskState::Ready;
        }

        // Auto-restart: check if any Faulted task has waited long enough
        let now = crate::timer::tick_count();
        for i in 0..NUM_TASKS {
            if (*TCBS.get_mut())[i].state == TaskState::Faulted
                && now.wrapping_sub((*TCBS.get_mut())[i].fault_tick) >= RESTART_DELAY_TICKS
            {
                restart_task(i);
            }
        }

        // Phase K: Priority-based selection with budget check.
        // Scan all tasks, pick the Ready task with highest priority
        // that still has budget remaining. Round-robin tiebreaker.
        let mut best_prio: i16 = -1;
        let mut next = IDLE_TASK_ID; // default to idle
        let mut found = false;
        for offset in 0..NUM_TASKS {
            let idx = (old + 1 + offset) % NUM_TASKS;
            if (*TCBS.get_mut())[idx].state == TaskState::Ready {
                // Check time budget (0 = unlimited)
                let budget_ok = (*TCBS.get_mut())[idx].time_budget == 0
                    || (*TCBS.get_mut())[idx].ticks_used < (*TCBS.get_mut())[idx].time_budget;
                if budget_ok && ((*TCBS.get_mut())[idx].priority as i16) > best_prio {
                    best_prio = (*TCBS.get_mut())[idx].priority as i16;
                    next = idx;
                    found = true;
                }
            }
        }

        if !found {
            // No ready task with budget — force idle
            next = IDLE_TASK_ID;
            if (*TCBS.get_mut())[IDLE_TASK_ID].state == TaskState::Faulted {
                restart_task(IDLE_TASK_ID);
            }
            (*TCBS.get_mut())[IDLE_TASK_ID].state = TaskState::Ready;
        }

        // Switch to new task
        (*TCBS.get_mut())[next].state = TaskState::Running;
        *CURRENT.get_mut() = next;

        // Load new task's context into the frame.
        core::ptr::copy_nonoverlapping(
            &(*TCBS.get_mut())[next].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );

        // Phase H: Switch TTBR0 to the new task's page table
        #[cfg(target_arch = "aarch64")]
        {
            let ttbr0 = (*TCBS.get_mut())[next].ttbr0;
            core::arch::asm!(
                "msr ttbr0_el1, {val}",
                "isb",
                val = in(reg) ttbr0,
                options(nomem, nostack)
            );
        }
    }
}

/// Get current task ID
pub fn current_task_id() -> u16 {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe { (*TCBS.get_mut())[*CURRENT.get()].id }
}

/// Set task state (used by IPC to block/unblock tasks)
pub fn set_task_state(task_idx: usize, state: TaskState) {
    if task_idx < NUM_TASKS {
        // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
        unsafe { (*TCBS.get_mut())[task_idx].state = state; }
    }
}

/// Get a register value from a task's saved context
pub fn get_task_reg(task_idx: usize, reg: usize) -> u64 {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe { (*TCBS.get_mut())[task_idx].context.x[reg] }
}

/// Set a register value in a task's saved context
pub fn set_task_reg(task_idx: usize, reg: usize, val: u64) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe { (*TCBS.get_mut())[task_idx].context.x[reg] = val; }
}

/// Save the current TrapFrame into a task's TCB context
pub fn save_frame(task_idx: usize, frame: &TrapFrame) {
    // SAFETY: src and dst are valid pointers to non-overlapping TrapFrame-sized memory within static TCBS array.
    unsafe {
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut (*TCBS.get_mut())[task_idx].context as *mut TrapFrame,
            1,
        );
    }
}

/// Load a task's TCB context into the TrapFrame
pub fn load_frame(task_idx: usize, frame: &mut TrapFrame) {
    // SAFETY: src and dst are valid pointers to non-overlapping TrapFrame-sized memory within static TCBS array.
    unsafe {
        core::ptr::copy_nonoverlapping(
            &(*TCBS.get_mut())[task_idx].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );
    }
}

/// Cleanup all resources held by a task: IPC endpoints, grants, IRQ bindings,
/// watchdog, and priority inheritance. Shared by fault_current_task() and sys_exit().
///
/// SAFETY: Caller must ensure single-core kernel context with interrupts masked.
pub unsafe fn cleanup_task_resources(task_idx: usize) {
    // SAFETY: Single-core kernel, interrupts masked. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        // Restore base priority (undo any inheritance)
        (*TCBS.get_mut())[task_idx].priority = (*TCBS.get_mut())[task_idx].base_priority;

        // Disable watchdog monitoring
        (*TCBS.get_mut())[task_idx].heartbeat_interval = 0;
    }

    // Clean up IPC endpoints — unblock any partner waiting for this task
    crate::ipc::cleanup_task(task_idx);

    // Clean up shared memory grants — revoke all grants involving this task
    crate::grant::cleanup_task(task_idx);

    // Clean up IRQ bindings — unbind all IRQs owned by this task
    crate::irq::irq_cleanup_task(task_idx);
}

/// Mark the currently running task as Faulted, cleanup IPC, and schedule away.
/// Called from exception handlers when a lower-EL fault is recoverable.
pub fn fault_current_task(frame: &mut TrapFrame) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        let current = *CURRENT.get();
        let id = (*TCBS.get_mut())[current].id;

        uart_print("[AegisOS] TASK ");
        crate::uart_print_hex(id as u64);
        uart_print(" FAULTED\n");

        (*TCBS.get_mut())[current].state = TaskState::Faulted;
        (*TCBS.get_mut())[current].fault_tick = crate::timer::tick_count();

        // Cleanup all task resources (IPC, grants, IRQ, watchdog, priority)
        cleanup_task_resources(current);

        // Schedule away to the next ready task
        schedule(frame);
    }
}

/// Handle SYS_EXIT syscall: gracefully terminate the current task.
/// Unlike fault_current_task(), sets state to Exited (no auto-restart).
pub fn sys_exit(frame: &mut TrapFrame, exit_code: u64) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        let current = *CURRENT.get();
        let id = (*TCBS.get_mut())[current].id;

        uart_print("[AegisOS] task ");
        crate::uart_print_dec(id as u64);
        uart_print(" exited (code=");
        crate::uart_print_dec(exit_code);
        uart_print(")\n");

        (*TCBS.get_mut())[current].state = TaskState::Exited;

        // Cleanup all task resources (IPC, grants, IRQ, watchdog, priority)
        cleanup_task_resources(current);

        // Schedule away — Exited tasks are never auto-restarted
        schedule(frame);
    }
}

/// Restart a faulted task: zero context, reload entry point + stack, mark Ready.
/// Called from schedule() when restart delay has elapsed.
pub fn restart_task(task_idx: usize) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    // ptr::write_bytes: pointer targets valid TrapFrame/stack memory within static TCBS array and linker-placed sections.
    unsafe {
        if (*TCBS.get_mut())[task_idx].state != TaskState::Faulted {
            return;
        }

        let id = (*TCBS.get_mut())[task_idx].id;

        // Zero user stack (4KB) to prevent state leakage
        // Only on AArch64 — on host tests, user_stack_top is a fake address
        #[cfg(target_arch = "aarch64")]
        {
            let ustack_top = (*TCBS.get_mut())[task_idx].user_stack_top;
            let ustack_base = (ustack_top - 4096) as *mut u8;
            core::ptr::write_bytes(ustack_base, 0, 4096);
        }

        // Zero entire TrapFrame
        core::ptr::write_bytes(
            &mut (*TCBS.get_mut())[task_idx].context as *mut TrapFrame as *mut u8,
            0,
            core::mem::size_of::<TrapFrame>(),
        );

        // Reload entry point, stack, SPSR
        (*TCBS.get_mut())[task_idx].context.elr_el1 = (*TCBS.get_mut())[task_idx].entry_point;
        (*TCBS.get_mut())[task_idx].context.spsr_el1 = 0x000; // EL0t
        (*TCBS.get_mut())[task_idx].context.sp_el0 = (*TCBS.get_mut())[task_idx].user_stack_top;

        (*TCBS.get_mut())[task_idx].state = TaskState::Ready;
        (*TCBS.get_mut())[task_idx].notify_pending = 0;
        (*TCBS.get_mut())[task_idx].notify_waiting = false;

        // Phase K: Reset scheduling state on restart
        (*TCBS.get_mut())[task_idx].priority = (*TCBS.get_mut())[task_idx].base_priority;
        (*TCBS.get_mut())[task_idx].ticks_used = 0;
        (*TCBS.get_mut())[task_idx].last_heartbeat = crate::timer::tick_count();

        uart_print("[AegisOS] TASK ");
        crate::uart_print_hex(id as u64);
        uart_print(" RESTARTED\n");
    }
}

// ─── Phase K helper functions ──────────────────────────────────────

/// Reset all tasks' ticks_used to 0 at the start of a new epoch.
/// Called from timer tick_handler when EPOCH_TICKS reaches EPOCH_LENGTH.
pub fn epoch_reset() {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        *EPOCH_TICKS.get_mut() = 0;
        for i in 0..NUM_TASKS {
            if (*TCBS.get_mut())[i].state != TaskState::Inactive
                && (*TCBS.get_mut())[i].state != TaskState::Exited
            {
                (*TCBS.get_mut())[i].ticks_used = 0;
            }
        }
    }
}

/// Scan all tasks for watchdog heartbeat violations.
/// If a task has heartbeat_interval > 0 and hasn't sent a heartbeat
/// within that interval, mark it Faulted (will auto-restart after delay).
pub fn watchdog_scan() {
    let now = crate::timer::tick_count();
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..NUM_TASKS {
            let hb = (*TCBS.get_mut())[i].heartbeat_interval;
            if hb == 0 {
                continue; // watchdog disabled for this task
            }
            if (*TCBS.get_mut())[i].state == TaskState::Faulted
                || (*TCBS.get_mut())[i].state == TaskState::Inactive
                || (*TCBS.get_mut())[i].state == TaskState::Exited
            {
                continue; // already faulted, inactive, or exited
            }
            let elapsed = now.wrapping_sub((*TCBS.get_mut())[i].last_heartbeat);
            if elapsed > hb {
                #[cfg(target_arch = "aarch64")]
                {
                    uart_print("[AegisOS] WATCHDOG: task ");
                    crate::uart_print_hex((*TCBS.get_mut())[i].id as u64);
                    uart_print(" missed heartbeat\n");
                }
                (*TCBS.get_mut())[i].state = TaskState::Faulted;
                (*TCBS.get_mut())[i].fault_tick = now;
                (*TCBS.get_mut())[i].priority = (*TCBS.get_mut())[i].base_priority;
                crate::ipc::cleanup_task(i);
            }
        }
    }
}

/// Set a task's effective priority (used for priority inheritance).
/// Does nothing if task_idx is out of range.
pub fn set_task_priority(task_idx: usize, priority: u8) {
    if task_idx < NUM_TASKS {
        // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
        unsafe { (*TCBS.get_mut())[task_idx].priority = priority; }
    }
}

/// Get a task's current effective priority.
pub fn get_task_priority(task_idx: usize) -> u8 {
    if task_idx < NUM_TASKS {
        // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
        unsafe { (*TCBS.get_mut())[task_idx].priority }
    } else {
        0
    }
}

/// Get a task's base (original) priority.
pub fn get_task_base_priority(task_idx: usize) -> u8 {
    if task_idx < NUM_TASKS {
        // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
        unsafe { (*TCBS.get_mut())[task_idx].base_priority }
    } else {
        0
    }
}

/// Restore a task's priority to its base priority (undo inheritance).
pub fn restore_base_priority(task_idx: usize) {
    if task_idx < NUM_TASKS {
        // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
        unsafe { (*TCBS.get_mut())[task_idx].priority = (*TCBS.get_mut())[task_idx].base_priority; }
    }
}

/// Record a heartbeat for the current task.
pub fn record_heartbeat(task_idx: usize, interval: u64) {
    if task_idx < NUM_TASKS {
        // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
        unsafe {
            (*TCBS.get_mut())[task_idx].heartbeat_interval = interval;
            (*TCBS.get_mut())[task_idx].last_heartbeat = crate::timer::tick_count();
        }
    }
}

/// Bootstrap: load first task's context and eret into EL0.
/// This never returns — it erets into task_a at EL0.
///
/// SPSR = 0x000 (EL0t) means eret drops to EL0 using SP_EL0.
/// SP_EL1 stays at __stack_end (the shared kernel boot stack).
/// SAVE_CONTEXT_LOWER reloads SP to __stack_end on every exception
/// entry, so the bootstrap SP value doesn't matter after this point.
#[cfg(target_arch = "aarch64")]
pub fn bootstrap() -> ! {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    // Inline asm: sets TTBR0_EL1, ELR_EL1, SPSR_EL1, SP_EL0 and erets into EL0 user task. Called at EL1.
    unsafe {
        (*TCBS.get_mut())[0].state = TaskState::Running;
        *CURRENT.get_mut() = 0;

        let frame = &(*TCBS.get_mut())[0].context;
        let ttbr0 = (*TCBS.get_mut())[0].ttbr0;

        // Load the task's context into registers and eret into EL0
        core::arch::asm!(
            // Phase H: Switch TTBR0 to task 0's per-task page table
            "msr ttbr0_el1, {ttbr0}",
            "isb",
            // Set ELR_EL1 = task entry, SPSR_EL1 = 0x000 (EL0t)
            "msr elr_el1, {elr}",
            "msr spsr_el1, {spsr}",
            // Set SP_EL0 = user stack (task will use this at EL0)
            "msr sp_el0, {sp0}",
            // eret: CPU restores PSTATE from SPSR (EL0t), PC from ELR.
            // Task runs at EL0 with SP = SP_EL0 (user stack).
            "eret",
            ttbr0 = in(reg) ttbr0,
            elr = in(reg) frame.elr_el1,
            spsr = in(reg) frame.spsr_el1,
            sp0 = in(reg) frame.sp_el0,
            options(noreturn)
        );
    }
}

// ─── Pure functions for Kani verification (Phase P) ────────────────

/// Pure watchdog check: should a task be faulted based on heartbeat timing?
/// Returns true if interval > 0 AND elapsed > interval.
/// Mirrors the inner check in watchdog_scan().
// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert
#[cfg(kani)]
pub fn watchdog_should_fault(interval: u64, elapsed: u64) -> bool {
    interval > 0 && elapsed > interval
}

/// Pure epoch reset: zero ticks_used for all non-Inactive/Exited tasks.
/// Returns new ticks_used array. Mirrors epoch_reset() logic.
// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert
#[cfg(kani)]
pub fn epoch_reset_pure(
    states: &[TaskState; NUM_TASKS],
    ticks_used: &[u64; NUM_TASKS],
) -> [u64; NUM_TASKS] {
    let mut result = *ticks_used;
    let mut i: usize = 0;
    while i < NUM_TASKS {
        if states[i] != TaskState::Inactive && states[i] != TaskState::Exited {
            result[i] = 0;
        }
        i += 1;
    }
    result
}

// ─── Kani formal verification proofs ───────────────────────────────

/// Pure scheduling decision — mirrors the logic in schedule() but takes
/// all state as parameters instead of reading globals. This enables
/// Kani to exhaustively verify scheduling properties.
#[cfg(kani)]
fn pick_next_task_pure(
    is_eligible: &[bool; NUM_TASKS],
    priorities: &[u8; NUM_TASKS],
    old: usize,
) -> usize {
    let mut best_prio: i16 = -1;
    let mut next = IDLE_TASK_ID;
    let mut found = false;
    let mut offset: usize = 0;
    while offset < NUM_TASKS {
        let idx = (old + 1 + offset) % NUM_TASKS;
        if is_eligible[idx] && (priorities[idx] as i16) > best_prio {
            best_prio = priorities[idx] as i16;
            next = idx;
            found = true;
        }
        offset += 1;
    }
    if !found {
        next = IDLE_TASK_ID;
    }
    next
}

/// Pure restart logic — mirrors restart_task() but operates on explicit
/// state fields instead of globals.
#[cfg(kani)]
fn restart_task_pure(
    state: TaskState,
    entry_point: u64,
    user_stack_top: u64,
    base_priority: u8,
    now: u64,
) -> (TaskState, u64, u64, u64, u8, u64, u64, bool) {
    if state != TaskState::Faulted {
        // No-op: return sentinel values indicating no restart
        return (state, 0, 0, 0, 0, 0, 0, false);
    }
    // After restart:
    (
        TaskState::Ready,     // new state
        entry_point,          // elr_el1 = entry_point
        0x000,                // spsr_el1 = EL0t
        user_stack_top,       // sp_el0 restored
        base_priority,        // priority = base
        0,                    // ticks_used = 0
        now,                  // last_heartbeat = now
        true,                 // restart performed
    )
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove: scheduling always returns a valid task index (< NUM_TASKS).
    /// When no task is eligible, returns IDLE_TASK_ID.
    /// When a task IS eligible, the returned task IS eligible.
    #[kani::proof]
    #[kani::unwind(9)] // NUM_TASKS=8, loop needs unwind bound = 9
    fn schedule_idle_guarantee() {
        let mut is_eligible = [false; NUM_TASKS];
        let mut priorities = [0u8; NUM_TASKS];
        let old: usize = kani::any();
        kani::assume(old < NUM_TASKS);

        let mut i: usize = 0;
        while i < NUM_TASKS {
            is_eligible[i] = kani::any();
            priorities[i] = kani::any();
            kani::assume(priorities[i] <= 7);
            i += 1;
        }

        let next = pick_next_task_pure(&is_eligible, &priorities, old);

        // PROPERTY 1: result is always a valid task index
        assert!(next < NUM_TASKS, "scheduler returned invalid index");

        // PROPERTY 2: if nothing is eligible, returns IDLE_TASK_ID
        let any_eligible = is_eligible[0] || is_eligible[1]
            || is_eligible[2] || is_eligible[3]
            || is_eligible[4] || is_eligible[5]
            || is_eligible[6] || is_eligible[7];
        if !any_eligible {
            assert_eq!(next, IDLE_TASK_ID, "no eligible tasks but didn't pick idle");
        }

        // PROPERTY 3: if something is eligible, the picked task IS eligible
        if any_eligible {
            assert!(is_eligible[next], "picked an ineligible task");
        }
    }

    /// Prove: restart_task state machine transitions are correct.
    /// - Only Faulted tasks get restarted.
    /// - After restart: state=Ready, context restored from entry/stack,
    ///   priority=base, ticks=0, heartbeat=now.
    #[kani::proof]
    fn restart_task_state_machine() {
        let entry_point: u64 = kani::any();
        let user_stack_top: u64 = kani::any();
        let base_priority: u8 = kani::any();
        kani::assume(base_priority <= 7);
        let now: u64 = kani::any();

        // Test Faulted → Ready transition
        let (state, elr, spsr, sp, prio, ticks, hb, did_restart) =
            restart_task_pure(
                TaskState::Faulted,
                entry_point,
                user_stack_top,
                base_priority,
                now,
            );
        assert!(did_restart, "Faulted task should restart");
        assert_eq!(state, TaskState::Ready);
        assert_eq!(elr, entry_point, "ELR must match entry point");
        assert_eq!(spsr, 0x000, "SPSR must be EL0t");
        assert_eq!(sp, user_stack_top, "SP must match user stack");
        assert_eq!(prio, base_priority, "priority must be base");
        assert_eq!(ticks, 0, "ticks_used must be zero");
        assert_eq!(hb, now, "heartbeat must be current time");

        // Test non-Faulted states are no-ops
        let (s2, _, _, _, _, _, _, did2) =
            restart_task_pure(TaskState::Ready, entry_point, user_stack_top, base_priority, now);
        assert!(!did2, "Ready task should not restart");
        assert_eq!(s2, TaskState::Ready, "state unchanged");

        let (s3, _, _, _, _, _, _, did3) =
            restart_task_pure(TaskState::Blocked, entry_point, user_stack_top, base_priority, now);
        assert!(!did3, "Blocked task should not restart");
        assert_eq!(s3, TaskState::Blocked, "state unchanged");

        let (s4, _, _, _, _, _, _, did4) =
            restart_task_pure(TaskState::Running, entry_point, user_stack_top, base_priority, now);
        assert!(!did4, "Running task should not restart");
        assert_eq!(s4, TaskState::Running, "state unchanged");

        let (s5, _, _, _, _, _, _, did5) =
            restart_task_pure(TaskState::Inactive, entry_point, user_stack_top, base_priority, now);
        assert!(!did5, "Inactive task should not restart");
        assert_eq!(s5, TaskState::Inactive, "state unchanged");

        let (s6, _, _, _, _, _, _, did6) =
            restart_task_pure(TaskState::Exited, entry_point, user_stack_top, base_priority, now);
        assert!(!did6, "Exited task should not restart");
        assert_eq!(s6, TaskState::Exited, "state unchanged");
    }

    // ─── Phase P proofs: watchdog + budget ─────────────────────────

    /// Proof: If a task doesn't heartbeat within its interval, watchdog detects it.
    /// And if the task heartbeats within the interval, watchdog does NOT fault it.
    #[kani::proof]
    fn watchdog_violation_detection() {
        let interval: u64 = kani::any();
        let elapsed: u64 = kani::any();

        let should_fault = watchdog_should_fault(interval, elapsed);

        if interval == 0 {
            // Watchdog disabled — never fault
            assert!(!should_fault, "disabled watchdog must not fault");
        } else if elapsed > interval {
            // Violation — must fault
            assert!(should_fault, "missed heartbeat must trigger fault");
        } else {
            // Within interval — must NOT fault
            assert!(!should_fault, "timely heartbeat must not trigger fault");
        }
    }

    /// Proof: After epoch reset, every non-Inactive/Exited task has ticks_used = 0.
    /// Inactive and Exited tasks are unaffected.
    #[kani::proof]
    #[kani::unwind(9)] // NUM_TASKS=8, loop needs 9
    fn budget_epoch_reset_fairness() {
        let mut states = [TaskState::Inactive; NUM_TASKS];
        let mut ticks_used = [0u64; NUM_TASKS];

        // Symbolic task states and ticks
        let mut i: usize = 0;
        while i < NUM_TASKS {
            let s: u8 = kani::any();
            kani::assume(s <= 5); // TaskState variants 0..=5
            states[i] = match s {
                0 => TaskState::Inactive,
                1 => TaskState::Ready,
                2 => TaskState::Running,
                3 => TaskState::Blocked,
                4 => TaskState::Faulted,
                _ => TaskState::Exited,
            };
            ticks_used[i] = kani::any();
            i += 1;
        }

        let result = epoch_reset_pure(&states, &ticks_used);

        // Verify properties
        let mut j: usize = 0;
        while j < NUM_TASKS {
            if states[j] == TaskState::Inactive || states[j] == TaskState::Exited {
                // PROPERTY: Inactive/Exited tasks are NOT reset
                assert_eq!(
                    result[j], ticks_used[j],
                    "Inactive/Exited ticks must be preserved"
                );
            } else {
                // PROPERTY: All other tasks get ticks_used = 0
                assert_eq!(
                    result[j], 0,
                    "Active task ticks_used must be zero after epoch reset"
                );
            }
            j += 1;
        }
    }
}
