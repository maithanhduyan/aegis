// AegisOS User Task — "logger" (Phase O)
//
// Receives IPC data from sensor task on endpoint 1, writes to UART.
// Demonstrates multi-ELF loading + cross-task IPC between user binaries.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libsyscall::{print, syscall_recv, syscall_yield};

// ─── Entry point ───────────────────────────────────────────────────

/// Logger task entry — receives sensor readings via IPC and logs to UART.
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    print("LOGGER:init ");

    loop {
        // Block waiting for IPC message on endpoint 1
        let reading = syscall_recv(1);

        // Log the received reading
        print("LOG:");
        // Simple hex digit output for the low nibble
        let digit = (reading & 0xF) as u8;
        let ch = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        libsyscall::syscall_write(&ch as *const u8, 1);
        print(" ");

        syscall_yield();
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
