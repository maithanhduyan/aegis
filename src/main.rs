// AegisOS — Kernel binary entry point
// This entire file is AArch64-only. When building for host tests (x86_64),
// the content is gated off and only the lib crate is tested.

// On AArch64: full kernel binary with boot asm, syscall wrappers, tasks
#![cfg_attr(target_arch = "aarch64", no_std)]
#![cfg_attr(target_arch = "aarch64", no_main)]

// On host (x86_64): empty bin that does nothing (tests use --lib --test)
#![cfg_attr(not(target_arch = "aarch64"), allow(unused))]

#[cfg(target_arch = "aarch64")]
use core::panic::PanicInfo;

#[cfg(target_arch = "aarch64")]
use aegis_os::uart_print;
#[cfg(target_arch = "aarch64")]
use aegis_os::exception;
#[cfg(target_arch = "aarch64")]
use aegis_os::sched;
#[cfg(target_arch = "aarch64")]
use aegis_os::timer;
#[cfg(target_arch = "aarch64")]
use aegis_os::gic;

// Boot assembly — inline vào binary thông qua global_asm!
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(include_str!("boot.s"));

// ─── Syscall wrappers ──────────────────────────────────────────────

/// SYS_YIELD (syscall #0): voluntarily yield the CPU to the next task.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_yield() {
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
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_send(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") 1u64, // SYS_SEND
            options(nomem, nostack)
        );
    }
}

/// SYS_RECV (syscall #2): receive message from endpoint.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_recv(ep_id: u64) -> u64 {
    let msg0: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") 2u64, // SYS_RECV
            lateout("x0") msg0,
            options(nomem, nostack)
        );
    }
    msg0
}

/// SYS_CALL (syscall #3): send message then wait for reply.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_call(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) -> u64 {
    let reply0: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") 3u64, // SYS_CALL
            lateout("x0") reply0,
            options(nomem, nostack)
        );
    }
    reply0
}

/// SYS_WRITE (syscall #4): write string to UART via kernel.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_write(buf: *const u8, len: usize) {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") buf as u64,
            in("x1") len as u64,
            in("x7") 4u64, // SYS_WRITE
            options(nomem, nostack)
        );
    }
}

/// Print a string from EL0 via SYS_WRITE syscall
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn user_print(s: &str) {
    syscall_write(s.as_ptr(), s.len());
}

// ─── Task entry points ─────────────────────────────────────────────

/// Task A (client): send "PING" on EP 0, receive reply
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn task_a_entry() -> ! {
    loop {
        user_print("A:PING ");
        syscall_call(0, 0x50494E47, 0, 0, 0);
    }
}

/// Task B (server): receive on EP 0, send PONG reply
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn task_b_entry() -> ! {
    loop {
        let _msg = syscall_recv(0);
        user_print("B:PONG ");
        syscall_send(0, 0x504F4E47, 0, 0, 0); // "PONG"
    }
}

/// Idle task: just wfi in a loop
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn idle_entry() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

// ─── Kernel main ───────────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart_print("\n[AegisOS] boot\n");
    uart_print("[AegisOS] MMU enabled (identity map)\n");
    uart_print("[AegisOS] W^X enforced (WXN + 4KB pages)\n");

    exception::init();
    uart_print("[AegisOS] exceptions ready\n");

    gic::init();
    gic::set_priority(timer::TIMER_INTID, 0);
    gic::enable_intid(timer::TIMER_INTID);

    sched::init(
        task_a_entry as *const () as u64,
        task_b_entry as *const () as u64,
        idle_entry as *const () as u64,
    );

    // ─── Phase G: Assign capabilities ──────────────────────────────
    unsafe {
        use aegis_os::cap::*;
        // Task 0 (task_a): PING client — needs CALL on EP0 + WRITE + YIELD
        sched::TCBS[0].caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD;
        // Task 1 (task_b): PONG server — needs RECV/SEND on EP0 + WRITE + YIELD
        sched::TCBS[1].caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD;
        // Task 2 (idle): only needs YIELD (WFI loop)
        sched::TCBS[2].caps = CAP_YIELD;
    }
    uart_print("[AegisOS] capabilities assigned\n");

    timer::init(10);

    uart_print("[AegisOS] bootstrapping into task_a (EL0)...\n");
    sched::bootstrap();
}

#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}

// On host target: provide a main() so the bin target compiles
#[cfg(not(target_arch = "aarch64"))]
fn main() {}
