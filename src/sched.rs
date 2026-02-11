/// Thời Khóa Biểu / Bộ lập lịch (Scheduler).
/// AegisOS Scheduler — Round-Robin, 3 static tasks
///
/// Tasks run at EL0 (user mode). Each task has:
///   - A TrapFrame (saved/restored on context switch)
///   - Its own 4KB kernel stack (SP_EL1, in .task_stacks section)
///   - Its own 4KB user stack (SP_EL0, in .user_stacks section)
///   - A state (Ready, Running, Inactive)
///
/// Context switch: timer IRQ → save frame → pick next Ready → switch SP_EL1 → load frame → eret to EL0

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
}

// ─── Static task table ─────────────────────────────────────────────

/// 3 tasks: 0 = task_a, 1 = task_b, 2 = idle
pub const NUM_TASKS: usize = 3;

pub static mut TCBS: [Tcb; NUM_TASKS] = [EMPTY_TCB; NUM_TASKS];
pub static mut CURRENT: usize = 0;

/// Delay before auto-restarting a faulted task (100 ticks × 10ms = 1 second)
pub const RESTART_DELAY_TICKS: u64 = 100;

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
};

// ─── Public API ────────────────────────────────────────────────────

/// Initialize scheduler: set up TCBs for task_a, task_b, idle.
/// Must be called before enabling timer interrupts.
#[cfg(target_arch = "aarch64")]
pub fn init(
    task_a_entry: u64,
    task_b_entry: u64,
    idle_entry: u64,
) {
    extern "C" {
        static __task_stacks_start: u8;  // kernel stacks (SP_EL1)
        static __user_stacks_start: u8;  // user stacks (SP_EL0)
    }

    let kstacks_base = unsafe { &__task_stacks_start as *const u8 as u64 };
    let ustacks_base = unsafe { &__user_stacks_start as *const u8 as u64 };

    // Each stack is 4KB. Stack grows downward, so top = base + (i+1)*4096
    // SPSR = 0x000 = EL0t: eret drops to EL0, uses SP_EL0
    // When exception from EL0 → EL1, CPU automatically uses SP_EL1
    unsafe {
        // Task 0: task_a
        TCBS[0].id = 0;
        TCBS[0].state = TaskState::Ready;
        TCBS[0].stack_top = kstacks_base + 1 * 4096;
        TCBS[0].entry_point = task_a_entry;
        TCBS[0].user_stack_top = ustacks_base + 1 * 4096;
        TCBS[0].context.elr_el1 = task_a_entry;
        TCBS[0].context.spsr_el1 = 0x000; // EL0t
        TCBS[0].context.sp_el0 = ustacks_base + 1 * 4096;

        // Task 1: task_b
        TCBS[1].id = 1;
        TCBS[1].state = TaskState::Ready;
        TCBS[1].stack_top = kstacks_base + 2 * 4096;
        TCBS[1].entry_point = task_b_entry;
        TCBS[1].user_stack_top = ustacks_base + 2 * 4096;
        TCBS[1].context.elr_el1 = task_b_entry;
        TCBS[1].context.spsr_el1 = 0x000; // EL0t
        TCBS[1].context.sp_el0 = ustacks_base + 2 * 4096;

        // Task 2: idle
        TCBS[2].id = 2;
        TCBS[2].state = TaskState::Ready;
        TCBS[2].stack_top = kstacks_base + 3 * 4096;
        TCBS[2].entry_point = idle_entry;
        TCBS[2].user_stack_top = ustacks_base + 3 * 4096;
        TCBS[2].context.elr_el1 = idle_entry;
        TCBS[2].context.spsr_el1 = 0x000; // EL0t
        TCBS[2].context.sp_el0 = ustacks_base + 3 * 4096;
    }

    uart_print("[AegisOS] scheduler ready (3 tasks, EL0)\n");
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
    unsafe {
        let old = CURRENT;

        // Save current task's context from the TrapFrame
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut TCBS[old].context as *mut TrapFrame,
            1,
        );

        // Mark old task as Ready (unless it's Blocked or Faulted)
        if TCBS[old].state == TaskState::Running {
            TCBS[old].state = TaskState::Ready;
        }

        // Auto-restart: check if any Faulted task has waited long enough
        let now = crate::timer::tick_count();
        for i in 0..NUM_TASKS {
            if TCBS[i].state == TaskState::Faulted
                && now.wrapping_sub(TCBS[i].fault_tick) >= RESTART_DELAY_TICKS
            {
                restart_task(i);
            }
        }

        // Round-robin: find next Ready task
        let mut next = (old + 1) % NUM_TASKS;
        let mut found = false;
        for _ in 0..NUM_TASKS {
            if TCBS[next].state == TaskState::Ready {
                found = true;
                break;
            }
            next = (next + 1) % NUM_TASKS;
        }

        if !found {
            // No ready task — force-restart idle (index 2) if faulted
            next = 2;
            if TCBS[2].state == TaskState::Faulted {
                restart_task(2);
            }
            TCBS[2].state = TaskState::Ready;
        }

        // Switch to new task
        TCBS[next].state = TaskState::Running;
        CURRENT = next;

        // Load new task's context into the frame.
        core::ptr::copy_nonoverlapping(
            &TCBS[next].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );

        // Phase H: Switch TTBR0 to the new task's page table
        #[cfg(target_arch = "aarch64")]
        {
            let ttbr0 = TCBS[next].ttbr0;
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
    unsafe { TCBS[CURRENT].id }
}

/// Set task state (used by IPC to block/unblock tasks)
pub fn set_task_state(task_idx: usize, state: TaskState) {
    if task_idx < NUM_TASKS {
        unsafe { TCBS[task_idx].state = state; }
    }
}

/// Get a register value from a task's saved context
pub fn get_task_reg(task_idx: usize, reg: usize) -> u64 {
    unsafe { TCBS[task_idx].context.x[reg] }
}

/// Set a register value in a task's saved context
pub fn set_task_reg(task_idx: usize, reg: usize, val: u64) {
    unsafe { TCBS[task_idx].context.x[reg] = val; }
}

/// Save the current TrapFrame into a task's TCB context
pub fn save_frame(task_idx: usize, frame: &TrapFrame) {
    unsafe {
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut TCBS[task_idx].context as *mut TrapFrame,
            1,
        );
    }
}

/// Load a task's TCB context into the TrapFrame
pub fn load_frame(task_idx: usize, frame: &mut TrapFrame) {
    unsafe {
        core::ptr::copy_nonoverlapping(
            &TCBS[task_idx].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );
    }
}

/// Mark the currently running task as Faulted, cleanup IPC, and schedule away.
/// Called from exception handlers when a lower-EL fault is recoverable.
pub fn fault_current_task(frame: &mut TrapFrame) {
    unsafe {
        let current = CURRENT;
        let id = TCBS[current].id;

        uart_print("[AegisOS] TASK ");
        crate::uart_print_hex(id as u64);
        uart_print(" FAULTED\n");

        TCBS[current].state = TaskState::Faulted;
        TCBS[current].fault_tick = crate::timer::tick_count();

        // Clean up IPC endpoints — unblock any partner waiting for this task
        crate::ipc::cleanup_task(current);

        // Clean up shared memory grants — revoke all grants involving this task
        crate::grant::cleanup_task(current);

        // Clean up IRQ bindings — unbind all IRQs owned by this task
        crate::irq::irq_cleanup_task(current);

        // Schedule away to the next ready task
        schedule(frame);
    }
}

/// Restart a faulted task: zero context, reload entry point + stack, mark Ready.
/// Called from schedule() when restart delay has elapsed.
pub fn restart_task(task_idx: usize) {
    unsafe {
        if TCBS[task_idx].state != TaskState::Faulted {
            return;
        }

        let id = TCBS[task_idx].id;

        // Zero user stack (4KB) to prevent state leakage
        // Only on AArch64 — on host tests, user_stack_top is a fake address
        #[cfg(target_arch = "aarch64")]
        {
            let ustack_top = TCBS[task_idx].user_stack_top;
            let ustack_base = (ustack_top - 4096) as *mut u8;
            core::ptr::write_bytes(ustack_base, 0, 4096);
        }

        // Zero entire TrapFrame
        core::ptr::write_bytes(
            &mut TCBS[task_idx].context as *mut TrapFrame as *mut u8,
            0,
            core::mem::size_of::<TrapFrame>(),
        );

        // Reload entry point, stack, SPSR
        TCBS[task_idx].context.elr_el1 = TCBS[task_idx].entry_point;
        TCBS[task_idx].context.spsr_el1 = 0x000; // EL0t
        TCBS[task_idx].context.sp_el0 = TCBS[task_idx].user_stack_top;

        TCBS[task_idx].state = TaskState::Ready;
        TCBS[task_idx].notify_pending = 0;
        TCBS[task_idx].notify_waiting = false;

        uart_print("[AegisOS] TASK ");
        crate::uart_print_hex(id as u64);
        uart_print(" RESTARTED\n");
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
    unsafe {
        TCBS[0].state = TaskState::Running;
        CURRENT = 0;

        let frame = &TCBS[0].context;
        let ttbr0 = TCBS[0].ttbr0;

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
