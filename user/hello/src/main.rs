// AegisOS User Task — "hello" ELF demo (Phase L5)
//
// Minimal EL0 task that proves end-to-end ELF loading:
//   1. Built as a separate Cargo crate → ELF64 binary
//   2. Embedded in kernel via include_bytes!
//   3. Loaded by kernel's ELF loader into .elf_load pages
//   4. Runs at EL0 with USER_CODE_PAGE permissions
//   5. Communicates with kernel via syscalls only

#![no_std]
#![no_main]

use core::panic::PanicInfo;

// ─── Syscall wrappers (duplicated — user binary has no access to kernel crate) ──

/// SYS_WRITE (syscall #4): write string to UART via kernel.
#[inline(always)]
fn syscall_write(buf: *const u8, len: usize) {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") buf as u64,
            in("x1") len as u64,
            in("x7") 4u64,
            options(nomem, nostack)
        );
    }
}

/// SYS_YIELD (syscall #0): voluntarily yield the CPU.
#[inline(always)]
fn syscall_yield() {
    unsafe {
        core::arch::asm!(
            "mov x7, #0",
            "svc #0",
            out("x7") _,
            options(nomem, nostack)
        );
    }
}

// ─── Entry point ───────────────────────────────────────────────────

/// User task entry — prints "L5:ELF " then yields in a loop.
/// This function is the ELF entry point (_start).
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    let msg = b"L5:ELF ";
    syscall_write(msg.as_ptr(), msg.len());
    loop {
        syscall_yield();
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
