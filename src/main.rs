#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

// Boot assembly — inline vào binary thông qua global_asm!
core::arch::global_asm!(include_str!("boot.s"));

/// UART0 PL011 data register trên QEMU virt machine
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

fn uart_write(byte: u8) {
    unsafe { ptr::write_volatile(UART0, byte) }
}

fn uart_print(s: &str) {
    for b in s.bytes() {
        uart_write(b);
    }
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart_print("\n[AegisOS] boot\n");

    loop {
        unsafe { core::arch::asm!("wfi") }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}
