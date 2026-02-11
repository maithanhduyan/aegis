/// AegisOS UART driver — PL011 on QEMU virt machine
///
/// UART0 data register at 0x0900_0000. Write-only for simplicity.
/// On host (non-AArch64), UART functions are no-ops for testing.

#[cfg(target_arch = "aarch64")]
use core::ptr;

/// UART0 PL011 data register on QEMU virt machine
#[cfg(target_arch = "aarch64")]
const UART0: *mut u8 = 0x0900_0000 as *mut u8;

/// Write a single byte to UART
#[cfg(target_arch = "aarch64")]
pub fn uart_write(byte: u8) {
    unsafe { ptr::write_volatile(UART0, byte) }
}

/// No-op on host (tests don't have UART)
#[cfg(not(target_arch = "aarch64"))]
pub fn uart_write(_byte: u8) {}

/// Print a string to UART
pub fn uart_print(s: &str) {
    for b in s.bytes() {
        uart_write(b);
    }
}

/// Print a u64 value as hexadecimal to UART
pub fn uart_print_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart_write(hex[nibble]);
    }
}
