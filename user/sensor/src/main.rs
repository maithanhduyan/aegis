// AegisOS User Task — "sensor" (Phase O)
//
// Simulated sensor: sends data via IPC to logger task on endpoint 1.
// Demonstrates multi-ELF loading + cross-task IPC between user binaries.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libsyscall::{print, syscall_send, syscall_yield};

// ─── Entry point ───────────────────────────────────────────────────

/// Sensor task entry — sends simulated sensor readings via IPC.
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    print("SENSOR:init ");

    let mut counter: u64 = 0;
    loop {
        // Send sensor reading on endpoint 1: x0=counter, x1=0xCAFE (tag)
        syscall_send(1, counter, 0xCAFE, 0, 0);
        print("S ");

        counter = counter.wrapping_add(1);
        syscall_yield();
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
