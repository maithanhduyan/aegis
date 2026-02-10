## Plan: AegisOS Phase B — MMU + Page Table + Memory Model

Bật MMU trên AArch64 với identity mapping, thiết lập memory isolation đúng chuẩn safety-critical: W^X (Write XOR Execute), device memory tách biệt, guard page cho stack. Chia thành 4 sub-phase tăng dần độ phức tạp — mỗi phase đều build + boot được trên QEMU, không bao giờ "code mù" rồi mới test.

---

### Sub-phase 0 — Chuẩn bị linker script + page table storage

Cập nhật [linker.ld](linker.ld) để:

- Export thêm symbols ranh giới: `__text_start`, `__text_end`, `__rodata_start`, `__rodata_end`, `__data_start`, `__data_end` — cần để map đúng permission từng vùng
- Thêm section `.page_tables` (BSS, `ALIGN(4096)`) dành riêng 16 KiB (4 page table × 4096 bytes) — symbols `__page_tables_start`, `__page_tables_end`
- Đặt guard page symbol (`__stack_guard`) trước stack — 1 page 4KB invalid để bắt stack overflow

Tạo [src/mmu.rs](src/mmu.rs) với kiểu `PageTable` dùng `#[repr(C, align(4096))]` chứa `[u64; 512]`, cùng bộ hằng số descriptor (xem chi tiết phần Kỹ thuật bên dưới).

---

### Sub-phase 1 — Identity map 2 MiB blocks + bật MMU (milestone: UART vẫn in được)

Trong [src/mmu.rs](src/mmu.rs), viết hàm `init_page_tables()`:

- **L1 table** (512 entries): entry[0] → L2_device, entry[1] → L2_ram, còn lại invalid
- **L2_device** (covers `0x0000_0000–0x3FFF_FFFF`): map `0x0800_0000–0x09FF_FFFF` bằng 2 MiB block descriptor với AttrIndx=0 (Device-nGnRnE), AP=RW, XN=1, PXN=1, AF=1
- **L2_ram** (covers `0x4000_0000–0x7FFF_FFFF`): map 128 MiB RAM bằng 2 MiB block descriptor với AttrIndx=2 (Normal WB), AP=RW, SH=Inner Shareable, AF=1

Viết hàm `enable_mmu()` hoặc inline ASM trong [src/boot.s](src/boot.s):

1. `TLBI VMALLE1` + `DSB ISH` + `ISB` — xóa TLB cũ
2. Ghi `MAIR_EL1 = 0x00000000_04FF4400` — 4 loại memory attribute
3. Ghi `TCR_EL1` — T0SZ=25 (39-bit VA), 4KB granule, EPD1=1 (tắt TTBR1)
4. Ghi `TTBR0_EL1` = địa chỉ L1 table
5. `ISB`
6. Đọc `SCTLR_EL1`, bật bit M(0), C(2), I(12)
7. `ISB` — từ đây mọi instruction fetch đi qua MMU

Cập nhật [src/main.rs](src/main.rs): gọi `mmu::init()` ngay đầu `kernel_main()`, in thêm `[AegisOS] MMU enabled\n` — nếu UART vẫn hoạt động → **device mapping đúng**.

**Checkpoint:** `cargo build --release` + QEMU → thấy cả `[AegisOS] boot` VÀ `[AegisOS] MMU enabled`.

---

### Sub-phase 2 — Refine: W^X với 4KB pages cho kernel region

Thay L2 entry đầu tiên (2 MiB block ở `0x4000_0000`) bằng pointer tới **L3 table** (512 × 4KB pages):

- Pages `0x4000_0000–0x4007_FFFF` (trước kernel): invalid hoặc RW-NX (DTB area)
- Pages `__text_start–__text_end`: **RX** — AttrIndx=2, AP=RO (10), XN=0, PXN=0, AF=1
- Pages `__rodata_start–__rodata_end`: **RO-NX** — AttrIndx=2, AP=RO (10), XN=1, PXN=1, AF=1
- Pages `__data_start–__bss_end`: **RW-NX** — AttrIndx=2, AP=RW (00), XN=1, PXN=1, AF=1
- Pages stack: **RW-NX** — tương tự data
- Pages page_tables: **RW-NX** — tương tự data

Bật `SCTLR_EL1.WXN = 1` (bit 19) — hardware enforce W^X: bất kỳ page nào writable sẽ **tự động** trở thành non-executable.

**Checkpoint:** build + boot → in `[AegisOS] W^X enforced\n`. Nếu kernel code bị map sai permission → crash ngay → dễ debug.

---

### Sub-phase 3 — Guard pages + unmap vùng không dùng

Trong L3 table:

- **Guard page trước stack** (`__stack_guard`): để entry = 0 (invalid) — nếu stack overflow → Data Abort thay vì ghi đè bộ nhớ âm thầm
- **Guard page sau stack** (nếu có): tương tự
- **Unmap tất cả vùng RAM không thuộc kernel**: entry = 0 — truy cập vùng trống → fault ngay

Tạo [src/exception.rs](src/exception.rs) — exception vector table tối thiểu:

- Thiết lập `VBAR_EL1` trỏ tới vector table (aligned 2048 bytes)
- Handler cho Synchronous Exception (Data Abort / Instruction Abort): đọc `ESR_EL1`, `FAR_EL1`, in ra UART rồi halt
- Còn lại (IRQ, FIQ, SError): halt an toàn

**Checkpoint:** build + boot → in `[AegisOS] memory isolation active\n`. Cố tình truy cập vùng unmapped (test) → thấy abort message trên UART.

---

### Chi tiết kỹ thuật quan trọng

**Hằng số descriptor (dùng trong `mmu.rs`):**

| Hằng | Giá trị | Mô tả |
|---|---|---|
| `VALID` | `0b1` | Bit 0 — entry hợp lệ |
| `TABLE` | `0b11` | L1/L2 table descriptor |
| `BLOCK` | `0b01` | L1 (1GiB) / L2 (2MiB) block |
| `PAGE` | `0b11` | L3 page descriptor |
| `ATTR_DEVICE` | `0 << 2` | MAIR index 0 = Device-nGnRnE |
| `ATTR_NORMAL_NC` | `1 << 2` | MAIR index 1 = Normal Non-Cacheable |
| `ATTR_NORMAL_WB` | `2 << 2` | MAIR index 2 = Normal Write-Back |
| `AP_RW_EL1` | `0b00 << 6` | EL1 Read-Write |
| `AP_RO_EL1` | `0b10 << 6` | EL1 Read-Only |
| `SH_INNER` | `0b11 << 8` | Inner Shareable |
| `AF` | `1 << 10` | Access Flag — **BẮT BUỘC = 1** (Cortex-A53 không có HW AF) |
| `PXN` | `1 << 53` | Privileged Execute Never |
| `UXN` | `1 << 54` | Unprivileged Execute Never |

**System registers:**

| Register | Giá trị | Ghi chú |
|---|---|---|
| `MAIR_EL1` | `0x00000000_04FF4400` | idx0=Device, idx1=NC, idx2=WB, idx3=Device-nGnRE |
| `TCR_EL1` | T0SZ=25, TG0=4KB, EPD1=1, IPS=48-bit | 39-bit VA, 3-level walk, chỉ dùng TTBR0 |
| `SCTLR_EL1` | Set bits M(0), C(2), SA(3), I(12), WXN(19) | Phase 1 không bật WXN, Phase 2 mới bật |
| `TTBR0_EL1` | Physical addr của L1 table | Phải align 4096 bytes |

**QEMU virt memory map liên quan:**

| Địa chỉ | Kích thước | Dùng cho |
|---|---|---|
| `0x0800_0000` | 128 KiB | GIC (distributor + CPU interface) |
| `0x0900_0000` | 64 KiB | UART0 PL011 |
| `0x4000_0000` | 128 MiB (mặc định) | RAM — kernel load tại `0x4008_0000` |

---

### Lưu ý quan trọng

1. **Mỗi sub-phase phải boot được trên QEMU** — không viết hết rồi mới test. Nếu crash ở sub-phase 1, sub-phase 2 sẽ thừa.
2. **AF=1 bắt buộc** trên mọi descriptor hợp lệ — Cortex-A53 không tự set Access Flag, quên = fault toàn bộ.
3. **Identity map code bật MMU** — instruction ngay sau `MSR SCTLR_EL1` dùng VA để fetch, nếu VA≠PA không mapped → crash ngay.
4. **Barrier pattern:** mọi ghi system register → `ISB`; mọi ghi page table + TLBI → `DSB ISH` + `ISB`.
5. **Phase tiếp theo sau Phase B:** Exception handling đầy đủ → IPC → Scheduler — theo đúng thứ tự microkernel.
