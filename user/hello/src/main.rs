// AegisOS User Task — "hello" ELF demo (Phase L5 → O2 refactored)
//
// Minimal EL0 task that proves end-to-end ELF loading.
// Uses libsyscall for all syscall wrappers (single source of truth).

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use libsyscall::{print, syscall_yield, syscall_exit};

// ─── Entry point ───────────────────────────────────────────────────

/// User task entry — prints "L5:ELF ", yields a few times, then exits gracefully.
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    print("L5:ELF ");
    // Yield a few times to show task is alive
    syscall_yield();
    syscall_yield();
    // Graceful exit — kernel should log "task 2 exited (code=0)"
    syscall_exit(0);
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
