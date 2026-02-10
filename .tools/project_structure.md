# Cấu trúc Dự án như sau:

```
./
├── .cargo
│   └── config.toml
├── Cargo.toml
├── aarch64-aegis.json
├── linker.ld
├── rust-toolchain.toml
└── src
    ├── boot.s
    └── main.rs
```

# Danh sách chi tiết các file:

## File ./aarch64-aegis.json:
```json
{
    "arch": "aarch64",
    "crt-objects-fallback": "false",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32",
    "disable-redzone": true,
    "features": "+v8a,+strict-align,+neon,+fp-armv8",
    "linker": "rust-lld",
    "linker-flavor": "gnu-lld",
    "llvm-target": "aarch64-unknown-none",
    "max-atomic-width": 128,
    "panic-strategy": "abort",
    "relocation-model": "static",
    "target-pointer-width": 64
}

```

## File ./Cargo.toml:
```
[package]
name = "aegis_os"
version = "0.1.0"
edition = "2021"
authors = ["AegisOS Team"]
description = "Safety-critical AArch64 microkernel"

[profile.release]
panic = "abort"
opt-level = "s"
lto = true

```

## File ./linker.ld:
```
ENTRY(_start)

SECTIONS
{
    . = 0x40080000;

    .text : {
        KEEP(*(.text._start))
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data   : { *(.data*)   }

    .bss : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    }

    . = ALIGN(16);
    __stack_start = .;
    . += 0x4000;        /* 16 KB stack */
    __stack_end = .;

    /DISCARD/ : { *(.comment*) *(.eh_frame*) *(.gcc_except_table*) }
}

```

## File ./rust-toolchain.toml:
```
[toolchain]
channel = "nightly"

```

## File ./src\boot.s:
```asm
.section .text._start
.global _start

_start:
    /* Chỉ core 0 chạy, các core khác park */
    mrs x0, mpidr_el1
    and x0, x0, #3
    cbz x0, 1f

0:
    wfe
    b 0b

1:
    /* Setup stack pointer từ vùng stack riêng */
    ldr x0, =__stack_end
    mov sp, x0

    /* Clear BSS */
    ldr x0, =__bss_start
    ldr x1, =__bss_end

2:
    cmp x0, x1
    b.eq 3f
    str xzr, [x0], #8
    b 2b

3:
    bl kernel_main

4:
    wfe
    b 4b

```

## File ./src\main.rs:
```rust
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

```

