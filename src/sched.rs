/// Thời Khóa Biểu / Bộ lập lịch (Scheduler).
/// AegisOS Scheduler — Round-Robin, 3 static tasks
///
/// Tasks run at EL1 (same privilege as kernel). Each task has:
///   - A TrapFrame (saved/restored on context switch)
///   - Its own 4KB kernel stack (in .task_stacks section)
///   - A state (Ready, Running, Inactive)
///
/// Context switch: timer IRQ → save frame → pick next Ready → load frame → eret

use crate::exception::TrapFrame;
use crate::uart_print;

// ─── Task state ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum TaskState {
    Inactive = 0,
    Ready    = 1,
    Running  = 2,
    Blocked  = 3,
}

// ─── Task Control Block ────────────────────────────────────────────

/// TCB — one per task. Context is saved/loaded during context switch.
#[repr(C)]
pub struct Tcb {
    pub context: TrapFrame,
    pub state: TaskState,
    pub id: u16,
    pub stack_top: u64,  // top of this task's kernel stack (SP_EL1)
}

// ─── Static task table ─────────────────────────────────────────────

/// 3 tasks: 0 = task_a, 1 = task_b, 2 = idle
const NUM_TASKS: usize = 3;

static mut TCBS: [Tcb; NUM_TASKS] = [EMPTY_TCB; NUM_TASKS];
static mut CURRENT: usize = 0;

const EMPTY_TCB: Tcb = Tcb {
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
};

// ─── Public API ────────────────────────────────────────────────────

/// Initialize scheduler: set up TCBs for task_a, task_b, idle.
/// Must be called before enabling timer interrupts.
pub fn init(
    task_a_entry: u64,
    task_b_entry: u64,
    idle_entry: u64,
) {
    extern "C" {
        static __task_stacks_start: u8;
    }

    let stacks_base = unsafe { &__task_stacks_start as *const u8 as u64 };

    // Each task stack is 4KB. Stack grows downward, so top = base + (i+1)*4096
    unsafe {
        // Task 0: task_a
        TCBS[0].id = 0;
        TCBS[0].state = TaskState::Ready;
        TCBS[0].stack_top = stacks_base + 1 * 4096;
        TCBS[0].context.elr_el1 = task_a_entry;
        TCBS[0].context.spsr_el1 = 0x345; // EL1h, IRQ unmasked (D=1, A=1, I=0, F=1)
        // SP for task_a: use its own stack top (will be loaded via sp_el0 or SP_EL1)
        // We store the stack top in x[29] (frame pointer) too for safety
        TCBS[0].context.sp_el0 = stacks_base + 1 * 4096;

        // Task 1: task_b
        TCBS[1].id = 1;
        TCBS[1].state = TaskState::Ready;
        TCBS[1].stack_top = stacks_base + 2 * 4096;
        TCBS[1].context.elr_el1 = task_b_entry;
        TCBS[1].context.spsr_el1 = 0x345;
        TCBS[1].context.sp_el0 = stacks_base + 2 * 4096;

        // Task 2: idle
        TCBS[2].id = 2;
        TCBS[2].state = TaskState::Ready;
        TCBS[2].stack_top = stacks_base + 3 * 4096;
        TCBS[2].context.elr_el1 = idle_entry;
        TCBS[2].context.spsr_el1 = 0x345;
        TCBS[2].context.sp_el0 = stacks_base + 3 * 4096;
    }

    uart_print("[AegisOS] scheduler ready (3 tasks)\n");
}

/// Schedule: save current context, pick next Ready task, load its context.
/// Called from timer IRQ handler with the current TrapFrame.
///
/// The trick: we modify `*frame` in-place. When the IRQ handler does
/// RESTORE_CONTEXT, it restores the NEW task's registers, and `eret`
/// jumps to the new task's ELR — completing the context switch.
pub fn schedule(frame: &mut TrapFrame) {
    unsafe {
        let old = CURRENT;

        // Save current task's context from the TrapFrame
        // Copy the entire frame into the TCB
        core::ptr::copy_nonoverlapping(
            frame as *const TrapFrame,
            &mut TCBS[old].context as *mut TrapFrame,
            1,
        );

        // Mark old task as Ready (unless it's Blocked)
        if TCBS[old].state == TaskState::Running {
            TCBS[old].state = TaskState::Ready;
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
            // No ready task — stay on idle (index 2)
            next = 2;
            TCBS[2].state = TaskState::Ready;
        }

        // Switch to new task
        TCBS[next].state = TaskState::Running;
        CURRENT = next;

        // Load new task's context into the frame
        // The assembly RESTORE_CONTEXT will pick up these values
        core::ptr::copy_nonoverlapping(
            &TCBS[next].context as *const TrapFrame,
            frame as *mut TrapFrame,
            1,
        );
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

/// Bootstrap: load first task's context and jump to it.
/// This never returns — it erets into task_a.
pub fn bootstrap() -> ! {
    unsafe {
        TCBS[0].state = TaskState::Running;
        CURRENT = 0;

        let frame = &TCBS[0].context;

        // Load the task's context into registers and eret
        core::arch::asm!(
            // Set ELR_EL1 and SPSR_EL1 for eret
            "msr elr_el1, {elr}",
            "msr spsr_el1, {spsr}",
            "msr sp_el0, {sp0}",
            // Set SP_EL1 for this task (for future exception entry)
            // We can't directly write SP_EL1 while using it, so we
            // just eret — the first timer IRQ will save onto bootstrap stack
            // and schedule() will set sp_el1 properly
            "eret",
            elr = in(reg) frame.elr_el1,
            spsr = in(reg) frame.spsr_el1,
            sp0 = in(reg) frame.sp_el0,
            options(noreturn)
        );
    }
}
