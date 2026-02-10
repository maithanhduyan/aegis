#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

mod mmu;
mod exception;

// Boot assembly — inline vào binary thông qua global_asm!
core::arch::global_asm!(include_str!("boot.s"));

/// UART0 PL011 data register trên QEMU virt machine
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

fn uart_write(byte: u8) {
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

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // MMU was enabled by boot.s with W^X + guard page before we get here
    uart_print("\n[AegisOS] boot\n");
    uart_print("[AegisOS] MMU enabled (identity map)\n");
    uart_print("[AegisOS] W^X enforced (WXN + 4KB pages)\n");

    // Install exception vector table
    exception::init();
    uart_print("[AegisOS] memory isolation active\n");

    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}
