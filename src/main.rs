#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

mod mmu;
mod exception;
mod gic;
mod timer;
mod sched;
mod ipc;

// Boot assembly — inline vào binary thông qua global_asm!
core::arch::global_asm!(include_str!("boot.s"));

/// UART0 PL011 data register trên QEMU virt machine
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

pub fn uart_write(byte: u8) {
    unsafe { ptr::write_volatile(UART0, byte) }
}

pub fn uart_print(s: &str) {
    for b in s.bytes() {
        uart_write(b);
    }
}

/// Print a u64 value as hexadecimal
pub fn uart_print_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart_write(hex[nibble]);
    }
}

// ─── Syscall wrappers ──────────────────────────────────────────────

/// SYS_YIELD (syscall #0): voluntarily yield the CPU to the next task.
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
/// msg[0..4] in x0..x3, ep_id in x6, syscall# in x7.
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
/// Returns msg[0] (first message word).
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
/// Returns msg[0] from reply.
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

// ─── Task entry points ─────────────────────────────────────────────

/// Task A (client): send "PING" on EP 0, receive reply
#[no_mangle]
pub extern "C" fn task_a_entry() -> ! {
    loop {
        // Send PING (msg[0] = 0x50494E47 = "PING" in ASCII hex)
        uart_print("A:PING ");
        syscall_call(0, 0x50494E47, 0, 0, 0);
    }
}

/// Task B (server): receive on EP 0, send PONG reply
#[no_mangle]
pub extern "C" fn task_b_entry() -> ! {
    loop {
        // Receive message
        let _msg = syscall_recv(0);
        uart_print("B:PONG ");
        // Reply by sending back on same endpoint
        syscall_send(0, 0x504F4E47, 0, 0, 0); // "PONG"
    }
}

/// Idle task: just wfi in a loop
#[no_mangle]
pub extern "C" fn idle_entry() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

// ─── Kernel main ───────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart_print("\n[AegisOS] boot\n");
    uart_print("[AegisOS] MMU enabled (identity map)\n");
    uart_print("[AegisOS] W^X enforced (WXN + 4KB pages)\n");

    // Install exception vector table
    exception::init();
    uart_print("[AegisOS] exceptions ready\n");

    // Initialize GIC + enable timer interrupt
    gic::init();
    gic::set_priority(timer::TIMER_INTID, 0);
    gic::enable_intid(timer::TIMER_INTID);

    // Initialize scheduler with task entry points
    sched::init(
        task_a_entry as *const () as u64,
        task_b_entry as *const () as u64,
        idle_entry as *const () as u64,
    );

    // Start timer: 10ms periodic tick (IRQ still masked — won't fire yet)
    timer::init(10);

    uart_print("[AegisOS] bootstrapping into task_a...\n");

    // Bootstrap: load task_a context and eret into it — never returns.
    // The eret restores SPSR with IRQ unmasked (0x345), so interrupts
    // become active exactly when task_a starts executing.
    sched::bootstrap();
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}
