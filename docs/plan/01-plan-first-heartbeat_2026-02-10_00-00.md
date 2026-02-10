## Plan: AegisOS — First Heartbeat Boot

Triển khai bộ skeleton code tối thiểu để AegisOS khởi động thành công trên QEMU `virt` (AArch64), in được dòng `[AegisOS] boot` qua UART0 PL011. Workspace hiện tại **hoàn toàn trống** (chỉ có spec `docs/SEED1.md`), nên toàn bộ 6 file cần được tạo mới từ đầu. Sử dụng **phiên bản đã chỉnh sửa** (nửa sau của SEED1) vì đã sửa lỗi stack chồng code và BSS clear loop.

---

### Bước 1 — Tạo `Cargo.toml` (file gốc project)

Tạo [Cargo.toml](Cargo.toml) tại thư mục gốc với:

- `name = "aegis_os"`, `version = "0.1.0"`, `edition = "2024"`
- `[profile.release]` → `panic = "abort"`, `opt-level = "s"`, `lto = true`
- Không khai báo dependency nào (bare-metal `no_std`)
- **Lưu ý:** SEED1 không liệt kê file này nhưng bắt buộc phải có để `cargo build` hoạt động

---

### Bước 2 — Tạo Target Specification `aarch64-aegis.json`

Tạo [aarch64-aegis.json](aarch64-aegis.json) tại thư mục gốc, nội dung theo SEED1 §1:

- `llvm-target`: `aarch64-unknown-none`
- `panic-strategy`: `abort`, `disable-redzone`: `true`
- `features`: `+strict-align,+neon,+fp-armv8`
- Linker: `rust-lld` với flavor `ld.lld`

---

### Bước 3 — Tạo Linker Script `linker.ld` (phiên bản có stack riêng)

Tạo [linker.ld](linker.ld) tại thư mục gốc, dùng **phiên bản chỉnh sửa** (SEED1 nửa sau):

- `ENTRY(_start)`, base address `0x40080000`
- Sections: `.text` → `.rodata` → `.data` → `.bss` (với `__bss_start` / `__bss_end`)
- **Vùng stack riêng 16KB** (`__stack_start` / `__stack_end`) sau `.bss` — đây là điểm khác biệt quan trọng so với phiên bản đầu
- `/DISCARD/` cho `.comment`, `.eh_frame`, `.gcc_except_table`

---

### Bước 4 — Tạo Assembly Bootstub `src/boot.s`

Tạo [src/boot.s](src/boot.s), dùng **phiên bản ổn định** (SEED1 nửa sau):

- Park CPU core phụ (đọc `mpidr_el1`, chỉ core 0 chạy tiếp)
- Setup SP từ `__stack_end` (không phải `_start` — tránh chồng code)
- Clear BSS bằng loop `cmp`/`b.eq` (an toàn hơn `cbz`/`sub`)
- `bl kernel_main`, fallback `wfe` loop nếu return

---

### Bước 5 — Tạo Rust Kernel Entry `src/main.rs`

Tạo [src/main.rs](src/main.rs), dùng phiên bản tối giản (SEED1 §3 chỉnh sửa):

- `#![no_std]`, `#![no_main]`
- Hằng `UART0: *mut u8 = 0x0900_0000` (PL011 trên QEMU virt)
- Helper `uart_write(byte)` và `uart_print(s: &str)`
- `kernel_main() -> !`: in `\n[AegisOS] boot\n`, rồi loop `wfi`
- `#[panic_handler]`: in `PANIC\n`, rồi loop vô tận

---

### Bước 6 — Tạo Cargo Config `.cargo/config.toml`

Tạo [.cargo/config.toml](.cargo/config.toml):

- `[build]` → `target = "aarch64-aegis.json"`
- `[target.aarch64-aegis]` → `rustflags = ["-C", "link-arg=-Tlinker.ld"]`
- **Lưu ý:** dùng `[target.aarch64-aegis]` thay vì `[build]` cho `rustflags` — đúng theo phiên bản chỉnh sửa

---

### Kiểm tra — Build & Run trên QEMU

```
cargo build --release
qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel target/aarch64-aegis/release/aegis_os
```

Kết quả mong đợi: thấy `[AegisOS] boot` trên terminal. Nếu lỗi, debug bằng `qemu-system-aarch64 ... -S -s` + `gdb-multiarch`.

---

### Lưu ý quan trọng

1. **Rust nightly bắt buộc** — cần `rustup override set nightly` trong thư mục project vì dùng `#![no_std]` + `#![no_main]` trên custom target. Nên tạo thêm file `rust-toolchain.toml` với `channel = "nightly"` và `targets = []` (custom target không cần target install).
2. **Sau khi boot thành công → Phase B là MMU + Page Table**, không phải scheduler. Đây là quyết định thiết kế đúng cho safety-critical microkernel (memory isolation trước hết).
3. **Cần cài sẵn:** `qemu-system-aarch64`, `rust-lld` (đi kèm Rust nightly), và `cargo` trên máy host.
