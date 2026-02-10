🧠 **B3K**: **ĐÃ XÁC NHẬN. HỆ THỐNG KHÓA MỤC TIÊU.** 🎯

Chào mừng Commander. Bạn không chỉ viết code, bạn đang xây dựng một **Pháo đài Số (Digital Fortress)**.

Tôi xác nhận 4 tham số chiến lược của bạn:
1.  **Architecture:** **AArch64 (ARMv8-A)** ✅ (Chuẩn công nghiệp hàng không/vũ trụ).
2.  **Platform:** **QEMU Virt** ✅ (Môi trường kiểm thử tiêu chuẩn trước khi lên silicon).
3.  **Kernel Design:** **Microkernel cực nhỏ** ✅ (Giảm tối đa TCB - Trusted Computing Base).
4.  **Endgame:** **Formal Verification** ✅ (Chứng minh toán học là đích đến cuối cùng).

---

### 🛡️ CHIẾN DỊCH KHỞI ĐỘNG: "THE FIRST HEARTBEAT"

Chúng ta sẽ không nói suông nữa. Dưới đây là bộ **Skeleton Code** tối thiểu để AegisOS cất tiếng khóc chào đời trên QEMU.

Hãy tạo cấu trúc thư mục và các file sau **NGAY LẬP TỨC**.

#### 1. Target Specification: `aarch64-aegis.json`
Rust cần biết chúng ta đang target vào bare-metal ARM64.

```json
{
    "llvm-target": "aarch64-unknown-none",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "executables": true,
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "panic-strategy": "abort",
    "disable-redzone": true,
    "features": "+strict-align,+neon,+fp-armv8"
}
```

#### 2. Linker Script: `layout.ld`
QEMU `virt` machine load kernel tại địa chỉ `0x40080000`. Chúng ta phải báo cho linker biết điều đó.

```ld
ENTRY(_start)

SECTIONS
{
    . = 0x40080000; /* Điểm bắt đầu bộ nhớ RAM cho Kernel trên QEMU virt */

    .text : {
        KEEP(*(.text._start))
        *(.text._start_arguments) /* Constants for boot */
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    
    .bss : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    }

    /DISCARD/ : { *(.comment*) *(.eh_frame*) *(.gcc_except_table*) }
}
```

#### 3. The Assembly Bootstub: `src/boot.s`
Nhiệm vụ: Park các CPU core phụ (chúng ta chỉ dùng 1 core để start), thiết lập stack, và nhảy vào Rust.

```asm
.section .text._start

.global _start

_start:
    /* Chỉ cho phép CPU core 0 (Master) chạy tiếp, các core khác ngủ */
    mrs     x0, mpidr_el1
    and     x0, x0, #3
    cbz     x0, master_core

park_cpu:
    wfe
    b       park_cpu

master_core:
    /* Thiết lập Stack Pointer (SP) tại địa chỉ an toàn (trước kernel image) */
    ldr     x0, =_start
    mov     sp, x0

    /* Xóa BSS (Khởi tạo biến static về 0) - Bước này quan trọng cho Rust! */
    ldr     x0, =__bss_start
    ldr     x1, =__bss_end
    sub     x1, x1, x0
    cbz     x1, jump_rust
    
clear_bss:
    str     xzr, [x0], #8
    sub     x1, x1, #8
    cbnz    x1, clear_bss

jump_rust:
    /* Nhảy vào hàm kernel_main của Rust */
    bl      kernel_main

    /* Nếu Rust return (không bao giờ xảy ra), treo máy an toàn */
    b       park_cpu
```

#### 4. The Brain (Rust): `src/main.rs`
Đây là nơi bạn in ký tự đầu tiên ra UART.
*Lưu ý: Trên QEMU virt, địa chỉ UART0 PL011 là `0x0900_0000`.*

```rust
#![no_std]
#![no_main]
#![feature(asm_const)]

use core::panic::PanicInfo;
use core::ptr;

// Địa chỉ UART0 trên QEMU virt machine
const UART0_DR: *mut u8 = 0x0900_0000 as *mut u8;

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // Gửi lời chào từ AegisOS
    // Đây là driver UART "nguyên thủy" nhất để chứng minh sự sống
    let message = b"\n[AegisOS] SYSTEM BOOT... OK.\n[AegisOS] SAFETY PROTOCOLS... ACTIVE.\n";
    
    for &byte in message {
        unsafe {
            ptr::write_volatile(UART0_DR, byte);
        }
    }

    // Vòng lặp vô tận - Trái tim của OS
    loop {
        // Wait for interrupt (tiết kiệm năng lượng CPU)
        unsafe { core::arch::asm!("wfi") };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Trong trường hợp panic, in '!' và treo máy
    unsafe { ptr::write_volatile(UART0_DR, b'!' as u8) };
    loop {
        unsafe { core::arch::asm!("wfe") };
    }
}
```

#### 5. Config Cargo: `.cargo/config.toml`

```toml
[build]
target = "aarch64-aegis.json"
rustflags = ["-C", "link-arg=-Tlayout.ld"]
```

---

### 🚦 LỆNH KHỞI ĐỘNG (LAUNCH SEQUENCE)

Sau khi tạo các file trên, hãy chạy lệnh sau trong terminal:

```bash
# 1. Build kernel
cargo build --release

# 2. Chạy QEMU
qemu-system-aarch64 \
    -machine virt \
    -cpu cortex-a53 \
    -nographic \
    -kernel target/aarch64-aegis/release/aegis_os
```

---

### 👁️ NHIỆM VỤ CỦA BẠN NGAY BÂY GIỜ

1.  **Copy-paste** code trên.
2.  **Run**.
3.  Báo cáo lại: Bạn có thấy dòng chữ **`[AegisOS] SYSTEM BOOT... OK.`** không?

Nếu thấy dòng chữ đó, **AegisOS đã chính thức ra đời**. 
Nếu lỗi, chúng ta debug. 
Nếu thành công, chúng ta ăn mừng 30 giây và chuyển sang **Phase B (Memory Management)**.

**COMMANDER, EXECUTE!** 🚀

---
Tốt. Giữ nhịp độ nhưng cần chỉnh lại vài chi tiết kỹ thuật để lần boot đầu tiên ổn định hơn và tránh lỗi khó debug.

Dưới đây là **phiên bản tối thiểu, chính xác hơn cho QEMU `virt` + AArch64**, đã loại bỏ một số bẫy thường gặp.

---

# 1. Sửa điểm quan trọng trong boot ASM

Stack không nên đặt trùng vùng code. Nên cấp một vùng stack riêng.

**`linker.ld`** thêm stack:

```ld
ENTRY(_start)

SECTIONS
{
    . = 0x40080000;

    .text : {
        KEEP(*(.text._start))
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data : { *(.data*) }

    .bss : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    }

    . = ALIGN(16);
    __stack_start = .;
    . += 0x4000;   /* 16 KB stack */
    __stack_end = .;
}
```

---

# 2. Boot assembly ổn định hơn

**`boot.s`**

```asm
.section .text._start
.global _start

_start:
    /* Chỉ core0 chạy */
    mrs x0, mpidr_el1
    and x0, x0, #3
    cbz x0, 1f

0:
    wfe
    b 0b

1:
    /* setup stack */
    ldr x0, =__stack_end
    mov sp, x0

    /* clear BSS */
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

---

# 3. Rust kernel entry tối giản (ổn định)

**`main.rs`**

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

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

---

# 4. Cargo config

`.cargo/config.toml`

```toml
[build]
target = "aarch64-aegis.json"

[target.aarch64-aegis]
rustflags = [
  "-C", "link-arg=-Tlinker.ld",
]
```

---

# 5. Build

```bash
cargo build --release
```

---

# 6. Chạy QEMU (chuẩn)

```bash
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a53 \
  -nographic \
  -kernel target/aarch64-aegis/release/aegis_os
```

Nếu mọi thứ đúng, bạn sẽ thấy:

```
[AegisOS] boot
```

Đó là nhịp tim đầu tiên.

---

# 7. Nếu không boot

90% lỗi sẽ là:

* sai địa chỉ load
* stack sai
* BSS chưa clear
* linker script lỗi
* QEMU dùng sai CPU

Cách debug:

```
qemu-system-aarch64 ... -S -s
gdb-multiarch kernel.elf
```

---

# 8. Khi dòng chữ xuất hiện → chuyển Phase B

Ngay khi UART in được:

bước tiếp theo **không phải scheduler**
mà là:

**MMU + page table + memory model**

Nếu kernel không có memory isolation đúng từ đầu, mọi thứ sau sẽ sai.

---

# 9. Xác nhận trạng thái

Chỉ cần trả lời:

* Boot thành công chưa?
* Có thấy dòng `[AegisOS] boot` không?

Nếu có → sang thiết kế **memory subsystem chuẩn cho hệ thống an toàn cấp sinh mạng**.
