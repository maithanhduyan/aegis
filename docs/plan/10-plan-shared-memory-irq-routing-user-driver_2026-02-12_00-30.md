# Kế hoạch Phase J — Shared Memory, Interrupt Routing & User-Mode Driver

> **Trạng thái: ✅ DONE** — Hoàn thành nợ kỹ thuật Shared Memory Grant (I³), thêm cơ chế interrupt routing từ kernel xuống EL0 task, cho phép map MMIO device vào user-space, và xây dựng proof-of-concept UART driver chạy ở EL0. Đây là bước biến AegisOS từ "kernel biết lập lịch" thành **microkernel thật sự** — nơi device driver chạy ở user-mode.

---

## Tại sao Phase J?

### Lỗ hổng hiện tại: "Kernel làm tất cả, task chỉ biết PING/PONG"

AegisOS sau Phase I có IPC đồng bộ, notification async, multi-sender queue, per-task address space, capability — nhưng tất cả device I/O (UART) vẫn do **kernel xử lý trực tiếp**. Task ở EL0 gọi `SYS_WRITE` → kernel copy byte ra UART. Task không bao giờ "chạm" phần cứng.

Trong hệ thống safety-critical thực tế, đây là vấn đề nghiêm trọng:

- **Tên lửa**: Sensor IMU gửi dữ liệu qua SPI → driver SPI phải chạy cách ly với hệ thống điều hướng. Nếu driver crash → kernel không bị ảnh hưởng.
- **Xe tự lái**: Camera driver lỗi → chỉ mất hình, hệ thống phanh vẫn hoạt động. Nếu driver nằm trong kernel → crash driver = crash cả xe.
- **Y tế**: Driver USB nhận dữ liệu từ cảm biến → lỗi driver không được làm hỏng module theo dõi nhịp tim.

Triết lý microkernel: **càng ít code trong kernel, càng an toàn**. Device driver PHẢI chạy ở user-mode.

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|---|---|
| 1 | Shared Memory Grant chưa implement (nợ Phase I³) | Task không thể chia sẻ dữ liệu lớn — chỉ truyền 32 bytes qua IPC |
| 2 | Tất cả IRQ xử lý trong kernel — EL0 task không nhận được hardware interrupt | Không thể viết user-mode driver |
| 3 | Device MMIO (UART, SPI, I2C...) chỉ kernel truy cập — EL0 task bị Permission Fault | Driver task không đọc/ghi được thanh ghi phần cứng |
| 4 | Không có mô hình user-mode driver | Kernel monolithic — mọi lỗi driver = lỗi kernel = hệ thống chết |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|---|---|---|
| **Shared Memory Grant** | Kernel cấp quyền 2 task chia sẻ vùng nhớ cụ thể (page-level) | #1 |
| **IRQ Bind + Routing** | Task đăng ký nhận interrupt → kernel chuyển IRQ thành notification | #2 |
| **Device MMIO Mapping** | Kernel map vùng MMIO device vào L3 page table của driver task | #3 |
| **UART User-Mode Driver** | Proof-of-concept: task ở EL0 trực tiếp đọc/ghi UART registers | #4 |

---

## Phân tích hiện trạng

### IPC & Notification (sau Phase I)

```
Syscalls: SYS_SEND(1), SYS_RECV(2), SYS_CALL(3), SYS_WRITE(4)
          SYS_NOTIFY(5), SYS_WAIT_NOTIFY(6)
          — Slots 7, 8 reserved nhưng CHƯA implement

Endpoint: 4 endpoints, multi-sender queue (FIFO, max 4 waiters)

Notification: u64 bitmask per task
  - SYS_NOTIFY: OR bits vào target.notify_pending, unblock nếu waiting
  - SYS_WAIT_NOTIFY: return pending bits hoặc block
  - Fire-and-forget, coalescing, 64 signal types
```

### Page Table (sau Phase H)

```
13 page tables tĩnh:
  [0]     = L2_device (shared) — device MMIO indices 64..=72 → DEVICE_BLOCK (AP_RW_EL1)
  [1..3]  = L1 per task
  [4..6]  = L2_ram per task
  [7..9]  = L3 per task — 512 entries × 4KB = first 2MiB fine-grained
  [10..12]= L1/L2/L3 kernel boot

L3 mapping cho first 2MiB (0x4000_0000..0x401F_FFFF):
  .text          → SHARED_CODE_PAGE  (RO, EL0+EL1 executable)
  .rodata        → KERNEL_RODATA_PAGE (RO, EL0+EL1, NX)
  .data/.bss     → KERNEL_DATA_PAGE  (RW, EL1 only, NX)
  .page_tables   → KERNEL_DATA_PAGE
  .task_stacks   → KERNEL_DATA_PAGE
  .user_stacks   → USER_DATA_PAGE (own) / KERNEL_DATA_PAGE (other)
  guard page     → 0 (unmapped)
```

### GIC & IRQ (sau Phase C)

```
GICv2: GICD 0x0800_0000, GICC 0x0801_0000
Hiện tại CHỈ enable INTID 30 (timer PPI)
IRQ dispatch: acknowledge → match INTID → chỉ có timer handler → EOI

exception_dispatch_irq():
  match intid {
      TIMER_INTID => tick_handler(frame),
      _ => uart_print("unhandled"), // ← tất cả device IRQ bị bỏ qua
  }
```

### Capability (sau Phase G+I)

```
12/64 bits đã dùng:
  Bit 0-3:   IPC_SEND/RECV EP0/EP1
  Bit 4:     WRITE
  Bit 5:     YIELD
  Bit 6-7:   NOTIFY / WAIT_NOTIFY
  Bit 8-11:  IPC_SEND/RECV EP2/EP3
  Bit 12-63: TRỐNG (52 bits)
```

### Memory Layout hiện tại (linker.ld)

```
0x4008_0000  .text (kernel + task code)
             .rodata
             .data
             .bss
             .page_tables    (13 × 4KB = 52KB)
             .task_stacks    (3 × 4KB = 12KB)
             .user_stacks    (3 × 4KB = 12KB)
             ── guard page   (4KB, unmapped)
             ── boot stack   (16KB)
             __kernel_end
```

---

## Thiết kế Phase J

### J1 — Shared Memory Grant (Hoàn thành nợ I³)

#### Khái niệm

Grant = kernel cho phép 2 task **chia sẻ một vùng nhớ vật lý cụ thể**. Cả hai task đều map vùng đó thành `AP_RW_EL0` trong L3 page table riêng. Khi revoke → peer mất quyền truy cập (entry chuyển về `AP_RW_EL1`).

Hình ảnh: Grant giống một **tấm bảng trắng trong phòng họp**. Người chủ phòng (owner) mời người khác (peer) vào viết lên bảng. Khi xong, người chủ khóa cửa lại — peer không vào được nữa.

#### Thiết kế dữ liệu

**Module mới: `src/grant.rs`**

```rust
pub const MAX_GRANTS: usize = 2;

pub struct Grant {
    pub owner: Option<usize>,     // task tạo grant (None = chưa dùng)
    pub peer: Option<usize>,      // task được chia sẻ
    pub phys_addr: u64,           // physical address của grant page
    pub active: bool,             // đang active?
}

pub static mut GRANTS: [Grant; MAX_GRANTS] = [EMPTY_GRANT; MAX_GRANTS];
```

**Linker section mới: `.grant_pages`**

```ld
/* Chèn sau .user_stacks, trước guard page */
. = ALIGN(4096);
__grant_pages_start = .;
.grant_pages (NOLOAD) : {
    . += 2 * 4096;      /* 2 grant pages × 4KB = 8KB */
}
__grant_pages_end = .;
```

**Hàm mới trong `src/mmu.rs`:**

```rust
// Map grant page vào L3 của task → AP_RW_EL0
pub unsafe fn map_grant_for_task(grant_phys: u64, task_id: usize);

// Unmap grant page từ L3 của task → AP_RW_EL1 (EL0 no access)
pub unsafe fn unmap_grant_for_task(grant_phys: u64, task_id: usize);
```

Cơ chế: tính L3 entry index từ physical address → update entry → `tlbi aside1is, <ASID>` + `dsb ish` + `isb`.

#### Syscall mới

| # | Tên | x7 | x6 | x0 | Mô tả |
|---|---|---|---|---|---|
| 7 | `SYS_GRANT_CREATE` | 7 | peer_task_id | grant_id | Map grant page vào L3 của cả owner và peer với `AP_RW_EL0` |
| 8 | `SYS_GRANT_REVOKE` | 8 | — | grant_id | Unmap grant page từ L3 của peer. Owner giữ access |

#### Capability mới

| Bit | Tên | Mô tả |
|---|---|---|
| 12 | `CAP_GRANT_CREATE` | Quyền tạo shared memory grant |
| 13 | `CAP_GRANT_REVOKE` | Quyền thu hồi grant |

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `linker.ld` | Sửa | Thêm `.grant_pages` section (8KB) sau `.user_stacks`, trước guard page. Symbols: `__grant_pages_start`, `__grant_pages_end` |
| `src/grant.rs` | **Tạo mới** | `Grant` struct, `GRANTS` static, `grant_create()`, `grant_revoke()`, `grant_cleanup_task()` |
| `src/lib.rs` | Sửa | Thêm `pub mod grant;` |
| `src/mmu.rs` | Sửa | Thêm `map_grant_for_task()` và `unmap_grant_for_task()`. Trong `build_l3()`: grant pages ban đầu = `KERNEL_DATA_PAGE`. Thêm `extern` symbols cho `__grant_pages_start/__grant_pages_end` |
| `src/cap.rs` | Sửa | Thêm `CAP_GRANT_CREATE = 1 << 12`, `CAP_GRANT_REVOKE = 1 << 13`. Cập nhật `CAP_ALL`. Thêm case trong `cap_for_syscall()` |
| `src/exception.rs` | Sửa | Thêm case `7 => handle_grant_create(frame)`, `8 => handle_grant_revoke(frame)` trong `handle_svc` |
| `src/sched.rs` | Sửa | Trong `fault_current_task()` hoặc `restart_task()`: gọi `grant::cleanup_task()` để revoke tất cả grant liên quan |
| `src/main.rs` | Sửa | Syscall wrappers `syscall_grant_create()`, `syscall_grant_revoke()`. Cập nhật task caps. Demo grant trong task_a/task_b |

#### Ràng buộc J1

1. **Grant page PHẢI nằm trong vùng L3 cover** — first 2MiB (`0x4000_0000..0x401F_FFFF`). Linker PHẢI đặt `.grant_pages` trước guard page → kiểm tra `__grant_pages_end < __stack_guard`
2. **TLB invalidate** bắt buộc sau mỗi page table update: `tlbi aside1is, <ASID>` chỉ flush ASID cụ thể
3. **Grant cleanup khi fault** — task bị fault → tất cả grant liên quan bị revoke tự động. Nếu không → stale mapping
4. **Single-core** → không cần lock, nhưng IRQ disabled trong SVC handler → safe

#### Checkpoint J1

UART output:
```
[AegisOS] grant system ready
[AegisOS] GRANT: task 0 → task 1 (grant 0)
```
Task A ghi dữ liệu vào grant page → notify Task B → Task B đọc đúng giá trị từ grant page.

---

### J2 — Interrupt Routing (Kernel → User Task)

#### Khái niệm

Hiện tại: tất cả hardware interrupt được xử lý trực tiếp bởi kernel. EL0 task không biết khi nào device có sự kiện.

Interrupt routing = **kernel nhận IRQ, chuyển thành notification gửi cho task đã đăng ký**. Task đó là user-mode driver.

Hình ảnh: Kernel giống **lễ tân khách sạn**. Khi có khách (interrupt) đến, lễ tân không tự xử lý — lễ tân gọi điện lên phòng (notification) cho nhân viên phụ trách (driver task). Nhân viên xử lý xong, gọi lại lễ tân báo "xong rồi" (IRQ ACK).

#### Thiết kế dữ liệu

```rust
// Trong src/irq.rs (module mới) hoặc mở rộng src/gic.rs

pub const MAX_IRQ_BINDINGS: usize = 8;  // tối đa 8 IRQ bound

pub struct IrqBinding {
    pub intid: u32,          // hardware INTID (ví dụ: 33 = UART0 IRQ)
    pub task_id: usize,      // task nhận notification
    pub notify_bit: u64,     // bit nào trong notify_pending
    pub active: bool,        // binding đang active?
    pub pending_ack: bool,   // IRQ đã gửi notification nhưng chưa ACK
}

pub static mut IRQ_BINDINGS: [IrqBinding; MAX_IRQ_BINDINGS] = [EMPTY_BINDING; MAX_IRQ_BINDINGS];
```

#### Luồng hoạt động

```
1. User task (driver): SYS_IRQ_BIND(intid=33, notify_bit=0x01)
   → Kernel: validate INTID, check CAP_IRQ_BIND
   → Kernel: gic::enable_intid(33), lưu binding
   → Return success

2. Hardware fires IRQ (INTID=33):
   → exception_dispatch_irq():
     → acknowledge() → intid=33
     → lookup IRQ_BINDINGS → found: task_id=0, notify_bit=0x01
     → TCBS[0].notify_pending |= 0x01
     → if notify_waiting → unblock
     → set pending_ack = true (mask IRQ cho đến khi ACK)
     → end_interrupt(33)

3. Driver task wakes up: SYS_WAIT_NOTIFY → x0 = 0x01
   → Driver xử lý device (đọc/ghi MMIO registers)
   → SYS_IRQ_ACK(intid=33)
   → Kernel: clear pending_ack, unmask IRQ

4. Lặp lại từ bước 2
```

#### Syscall mới

| # | Tên | x7 | x6 | x0 | x1 | Mô tả |
|---|---|---|---|---|---|---|
| 9 | `SYS_IRQ_BIND` | 9 | — | intid | notify_bit | Đăng ký nhận IRQ. Kernel enable INTID trong GIC |
| 10 | `SYS_IRQ_ACK` | 10 | — | intid | — | Báo kernel đã xử lý xong IRQ. Kernel unmask INTID |

#### Capability mới

| Bit | Tên | Mô tả |
|---|---|---|
| 14 | `CAP_IRQ_BIND` | Quyền đăng ký nhận interrupt routing |
| 15 | `CAP_IRQ_ACK` | Quyền acknowledge interrupt (thường cùng task với BIND) |

#### Bảo vệ quan trọng

**IRQ masking giữa notification và ACK:**
- Khi kernel gửi notification cho driver → **mask INTID** (disable trong GICD) → tránh interrupt storm
- Khi driver gọi `SYS_IRQ_ACK` → **unmask INTID** (re-enable) → sẵn sàng cho interrupt tiếp
- Nếu driver bị fault trước khi ACK → `irq_cleanup_task()` unmask + unbind

**INTID validation:**
- Timer INTID 30 KHÔNG được phép bind (kernel-reserved)
- Chỉ cho phép bind SPIs (INTID 32+) — PPIs và SGIs là kernel-reserved
- Mỗi INTID chỉ bind cho 1 task (không multi-bind)

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/irq.rs` | **Tạo mới** | `IrqBinding` struct, `IRQ_BINDINGS` static, `irq_bind()`, `irq_ack()`, `irq_route()`, `irq_cleanup_task()` |
| `src/lib.rs` | Sửa | Thêm `pub mod irq;` |
| `src/gic.rs` | Sửa | Thêm `pub fn disable_intid(intid: u32)` để mask interrupt. Thêm `pub fn is_enabled(intid: u32) -> bool` |
| `src/exception.rs` | Sửa | Trong `exception_dispatch_irq`: thay `_ => uart_print("unhandled")` bằng `_ => irq::irq_route(intid, frame)` — lookup binding, gửi notification. Thêm case `9 => handle_irq_bind(frame)`, `10 => handle_irq_ack(frame)` trong `handle_svc` |
| `src/cap.rs` | Sửa | Thêm `CAP_IRQ_BIND = 1 << 14`, `CAP_IRQ_ACK = 1 << 15`. Cập nhật `cap_for_syscall()` |
| `src/sched.rs` | Sửa | Trong `fault_current_task()`: gọi `irq::irq_cleanup_task()` |
| `src/main.rs` | Sửa | Syscall wrappers `syscall_irq_bind()`, `syscall_irq_ack()` |

#### Checkpoint J2

UART output:
```
[AegisOS] IRQ routing ready (max 8 bindings)
[AegisOS] IRQ BIND: INTID 33 → task 0, bit 0x01
```
Khi UART interrupt fire → task nhận notification → driver xử lý.

---

### J3 — Device MMIO Mapping cho EL0

#### Khái niệm

Hiện tại: device MMIO (UART `0x0900_0000`, GIC `0x0800_0000`) được map trong L2_device với `AP_RW_EL1` — EL0 task không truy cập được. Permission Fault nếu EL0 cố đọc/ghi.

Phase J3 cho phép kernel **map một vùng MMIO device cụ thể vào per-task L2_device** với `AP_RW_EL0`, nhưng CHỈ cho task có capability `CAP_DEVICE_MAP`.

#### Thách thức kiến trúc

L2_device hiện tại là **shared** (page index 0) — tất cả task và kernel dùng chung. Nếu sửa entry trong L2_device → ảnh hưởng tất cả.

**Giải pháp: Per-task L2_device**

Tạo thêm 3 L2_device tables (1 per task) — mỗi task có bản copy riêng. Ban đầu giống nhau (device = EL1 only). Khi `SYS_DEVICE_MAP` → sửa entry trong L2_device riêng của task đó.

Tuy nhiên, thêm 3 pages = 12KB → tổng page tables = 16 pages (từ 13).

**Cách tối ưu hơn: Reuse L2_device shared + capability check trong fault handler**

Thay vì thêm page tables, giữ L2_device shared nhưng khi task có `CAP_DEVICE_MAP` cho MMIO cụ thể → ta sửa **L2_device entry** cho vùng đó thành `AP_RW_EL0`. Vấn đề: ảnh hưởng TẤT CẢ task.

**Quyết định thiết kế: Per-task L2_device (3 pages thêm)**

Lý do: Safety-critical — isolation phải absolute. Task A là UART driver → chỉ A thấy UART MMIO. Task B không bao giờ truy cập được UART dù cùng share L2_device.

```
Page tables sau Phase J:
  [0]     = L2_device cho task 0 (thay vì shared)
  [1]     = L2_device cho task 1
  [2]     = L2_device cho task 2
  [3..5]  = L1 per task
  [6..8]  = L2_ram per task
  [9..11] = L3 per task
  [12]    = L2_device kernel boot (EL1 only — mọi device accessible)
  [13]    = L1 kernel boot
  [14]    = L2_ram kernel boot
  [15]    = L3 kernel boot
  Total: 16 pages (tăng từ 13)
```

#### Syscall mới

| # | Tên | x7 | x6 | x0 | Mô tả |
|---|---|---|---|---|---|
| 11 | `SYS_DEVICE_MAP` | 11 | — | device_id | Map MMIO region của device vào L2_device riêng của caller. device_id: 0=UART |

#### Capability mới

| Bit | Tên | Mô tả |
|---|---|---|
| 16 | `CAP_DEVICE_MAP` | Quyền map device MMIO vào user-space |

#### Device registry (static)

```rust
// Trong src/device.rs hoặc src/irq.rs

pub const DEVICE_UART: u64 = 0;

pub struct DeviceInfo {
    pub l2_index: usize,    // L2 entry index (e.g., 72 cho UART)
    pub intid: u32,         // hardware interrupt ID (33 cho UART0 trên QEMU virt)
    pub name: &'static str,
}

pub const DEVICES: &[DeviceInfo] = &[
    DeviceInfo { l2_index: 72, intid: 33, name: "UART0" },
    // Thêm device ở đây trong tương lai
];
```

#### Cơ chế page table update

Khi `SYS_DEVICE_MAP(device_id=0)` (UART):
1. Lookup `DEVICES[0]` → `l2_index = 72`
2. Trong L2_device **của caller task**: sửa entry 72 từ `DEVICE_BLOCK | AP_RW_EL1` → `DEVICE_BLOCK_EL0`
3. `DEVICE_BLOCK_EL0 = BLOCK | ATTR_DEVICE | AP_RW_EL0 | AF | XN` — EL0 readable/writable, non-executable
4. TLB invalidate: `tlbi aside1is, <ASID>` + `dsb ish` + `isb`

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/mmu.rs` | Sửa lớn | Thay đổi layout: per-task L2_device (3 pages thêm). `NUM_PAGE_TABLE_PAGES = 16`. Thêm `DEVICE_BLOCK_EL0` descriptor. Hàm `map_device_for_task(device_id, task_id)`. Cập nhật tất cả `PT_*` constants |
| `linker.ld` | Sửa | `.page_tables` tăng từ `13 * 4096` lên `16 * 4096` (thêm 12KB) |
| `src/exception.rs` | Sửa | Case `11 => handle_device_map(frame)` trong `handle_svc` |
| `src/cap.rs` | Sửa | Thêm `CAP_DEVICE_MAP = 1 << 16`. Cập nhật `cap_for_syscall()` |
| `src/main.rs` | Sửa | Syscall wrapper `syscall_device_map()` |

#### Ràng buộc J3

1. **GIC MMIO KHÔNG BAO GIỜ map cho EL0** — chỉ UART và device an toàn. GIC L2 indices 64-66 luôn `AP_RW_EL1`
2. **Descriptor phải là `ATTR_DEVICE`** — device memory không cache. Sai attribute → data corruption
3. **XN bắt buộc** — device MMIO không bao giờ executable (W^X vẫn đảm bảo)
4. **Page table layout thay đổi** → tất cả `PT_*` constants trong `mmu.rs` phải cập nhật → high risk

#### Checkpoint J3

UART output:
```
[AegisOS] device MMIO mapping ready
[AegisOS] DEVICE MAP: UART0 → task 0
```
Task 0 (UART driver) ghi trực tiếp vào `0x0900_0000` (UART DR) → ký tự xuất hiện trên terminal.

---

### J4 — UART User-Mode Driver (Proof of Concept)

#### Khái niệm

Đây là "final exam" cho J1-J3. Chứng minh toàn bộ cơ chế hoạt động end-to-end:

1. Task 0 trở thành **UART driver** (EL0)
2. Task 0 bind UART IRQ (INTID 33) + map UART MMIO
3. Task 1 muốn in chuỗi → gửi IPC đến Task 0
4. Task 0 nhận IPC → ghi trực tiếp vào UART registers

#### Luồng hoạt động chi tiết

```
Boot:
  kernel_main():
    - Gán caps cho task 0: CAP_IRQ_BIND | CAP_IRQ_ACK | CAP_DEVICE_MAP |
                           CAP_IPC_RECV_EP0 | CAP_WRITE | ...
    - Gán caps cho task 1: CAP_IPC_SEND_EP0 | CAP_WRITE | CAP_YIELD | ...
    - bootstrap() → task 0 chạy

Task 0 (UART driver, EL0):
  uart_driver_entry():
    1. SYS_DEVICE_MAP(device_id=0)        → map UART MMIO vào user-space
    2. SYS_IRQ_BIND(intid=33, bit=0x01)   → đăng ký nhận UART interrupt
    3. Loop:
       a. SYS_RECV(ep_id=0)               → nhận request từ client task
       b. Đọc message payload (x0 = buf_addr trong grant page, x1 = len)
       c. Ghi từng byte ra UART DR (0x0900_0000) trực tiếp (volatile write)
       d. SYS_SEND(ep_id=0, status)        → trả kết quả cho client

Task 1 (client, EL0):
  client_entry():
    1. Ghi chuỗi "Hello from user driver!\n" vào grant page
    2. SYS_CALL(ep_id=0, grant_addr, len, 0, 0)
    3. Lặp lại
```

#### Task entry points mới

```rust
// Thay thế task_a_entry / task_b_entry hiện tại

fn uart_driver_entry() -> ! {
    // Map UART MMIO
    syscall_device_map(DEVICE_UART);
    // Bind UART IRQ (INTID 33 trên QEMU virt)
    syscall_irq_bind(33, 0x01);
    user_print("UART-DRV: ready\n");
    loop {
        let msg = syscall_recv(0);      // nhận request
        // Ghi bytes trực tiếp ra UART
        // ... (dùng volatile write tới 0x0900_0000)
        syscall_send(0, 0x4F4B, 0, 0, 0); // reply "OK"
    }
}

fn client_entry() -> ! {
    loop {
        syscall_call(0, /* ... */);
        // Yield một lúc
        for _ in 0..1000 { syscall_yield(); }
    }
}
```

#### Lưu ý UART trên QEMU virt

- UART0 PL011: base = `0x0900_0000`
- Data Register (DR) = offset 0x00: ghi byte → xuất ký tự
- Flag Register (FR) = offset 0x18: bit 5 (TXFF) = TX FIFO full
- Interrupt: INTID 33 (SPI 1) — UART combined interrupt
- Cần enable UART interrupt trong UART IMSC register (offset 0x38)

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/main.rs` | Sửa lớn | Thay task_a_entry → uart_driver_entry, task_b_entry → client_entry. Cập nhật caps. Thêm UART register constants cho user-space |
| `src/main.rs` | Sửa | `kernel_main`: thêm `gic::enable_intid(33)` ban đầu (hoặc để IRQ_BIND tự enable) |

#### Backward compatibility

- **SYS_WRITE (syscall 4) vẫn hoạt động** — kernel UART output cho debug/panic
- Task không có `CAP_DEVICE_MAP` vẫn dùng `SYS_WRITE` bình thường
- UART driver chỉ là thêm một con đường I/O, không xóa con đường cũ

#### Checkpoint J4

UART output:
```
[AegisOS] boot
[AegisOS] UART user-mode driver enabled
UART-DRV: ready
CLIENT: Hello from user-mode driver!
```
Ký tự "Hello from user-mode driver!" được ghi trực tiếp bởi EL0 task (không qua SYS_WRITE).

---

### J5 — Tests & Boot Checkpoints

#### Host unit tests mới (ước lượng: ~20 tests)

| # | Test case | Sub-phase | Mô tả |
|---|---|---|---|
| 1 | `test_grant_create_sets_active` | J1 | Tạo grant → active=true, owner/peer đúng |
| 2 | `test_grant_create_duplicate_rejected` | J1 | Tạo grant đã active → fail |
| 3 | `test_grant_revoke_clears_peer` | J1 | Revoke → peer=None, active=false |
| 4 | `test_grant_revoke_nonexistent` | J1 | Revoke grant chưa active → no-op |
| 5 | `test_grant_cleanup_on_fault` | J1 | Task fault → tất cả grant liên quan bị revoke |
| 6 | `test_grant_cleanup_both_owner_and_peer` | J1 | Fault task là owner → revoke. Fault task là peer → revoke |
| 7 | `test_irq_bind_success` | J2 | Bind INTID 33 → binding active, task/bit đúng |
| 8 | `test_irq_bind_timer_rejected` | J2 | Bind INTID 30 (timer) → rejected |
| 9 | `test_irq_bind_ppi_rejected` | J2 | Bind INTID < 32 (PPIs) → rejected |
| 10 | `test_irq_bind_duplicate_rejected` | J2 | Bind INTID đã bound → rejected |
| 11 | `test_irq_bind_max_bindings` | J2 | Bind 8 IRQs → OK. Bind thứ 9 → full |
| 12 | `test_irq_route_sends_notification` | J2 | Simulate IRQ route → task.notify_pending set đúng bit |
| 13 | `test_irq_route_unblocks_waiting` | J2 | Task đang wait_notify + IRQ route → task unblocked |
| 14 | `test_irq_ack_clears_pending` | J2 | ACK → pending_ack=false |
| 15 | `test_irq_cleanup_on_fault` | J2 | Fault task → unbind all IRQs, unmask |
| 16 | `test_cap_grant_create` | J1 | cap_for_syscall(7, _) == CAP_GRANT_CREATE |
| 17 | `test_cap_grant_revoke` | J1 | cap_for_syscall(8, _) == CAP_GRANT_REVOKE |
| 18 | `test_cap_irq_bind` | J2 | cap_for_syscall(9, _) == CAP_IRQ_BIND |
| 19 | `test_cap_irq_ack` | J2 | cap_for_syscall(10, _) == CAP_IRQ_ACK |
| 20 | `test_cap_device_map` | J3 | cap_for_syscall(11, _) == CAP_DEVICE_MAP |

#### QEMU boot checkpoints mới

| # | Checkpoint UART output |
|---|---|
| 13 | `[AegisOS] grant system ready` |
| 14 | `[AegisOS] IRQ routing ready` |
| 15 | `[AegisOS] UART user-mode driver enabled` |
| 16 | `UART-DRV: ready` |

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|---|---|---|
| 1 | No heap — grant/IRQ tables phải static | Bất biến AegisOS | `static mut GRANTS`, `static mut IRQ_BINDINGS` với kích thước cố định |
| 2 | TrapFrame = 288 bytes | ABI-locked | Không thay đổi — J không thêm field vào TrapFrame |
| 3 | Grant pages trong vùng L3 (first 2MiB) | L3 chỉ cover `0x4000_0000..0x401F_FFFF` | Linker đặt `.grant_pages` trước guard page |
| 4 | W^X — device MMIO phải XN | Bất biến | `DEVICE_BLOCK_EL0` có `XN` bits set |
| 5 | Timer INTID 30 = kernel-reserved | Kernel cần timer để schedule | `irq_bind()` reject INTID < 32 |
| 6 | GIC MMIO không bao giờ map cho EL0 | GIC control = kernel-only | `map_device_for_task()` whitelist devices, GIC không nằm trong list |
| 7 | Page table layout change (13→16 pages) | Per-task L2_device | Cập nhật linker.ld `.page_tables` size + tất cả `PT_*` constants |
| 8 | No FP/SIMD | CPACR_EL1.FPEN=0 | Không dùng floating point trong driver logic |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|---|
| 1 | Page table layout change gây boot failure | 🔴 Cao | 🔴 Không boot được | Test J3 rất kỹ — backup PT constants cũ, rollback nếu fail. Thực hiện J1/J2 trước (không thay đổi PT layout) |
| 2 | UART INTID sai trên QEMU virt | 🟡 Trung bình | 🟡 IRQ không fire | Verify INTID bằng QEMU `-d int` tracing. UART0 trên virt = SPI 1 = INTID 33 (confirmed by QEMU source) |
| 3 | TLB stale sau grant/MMIO map | 🟡 Trung bình | 🔴 Task truy cập sai dữ liệu | `tlbi aside1is` + `dsb ish` + `isb` sau MỌI page table update. Test bằng access pattern verify |
| 4 | Grant page overflow ngoài 2MiB | 🟢 Thấp | 🔴 Unmapped access | Static check: `assert!(__grant_pages_end < 0x401F_FFFF)` trong mmu_init hoặc build-time linker check |
| 5 | IRQ storm nếu driver không ACK kịp | 🟡 Trung bình | 🟡 CPU bão hoà | Mask INTID sau khi route notification, chờ ACK mới unmask. Timeout → auto-unmask sau N ticks |
| 6 | UART user driver conflict với kernel SYS_WRITE | 🟡 Trung bình | 🟡 Output lẫn lộn | Phase J: kernel vẫn dùng SYS_WRITE cho boot messages. Driver task output riêng. Tương lai: mutex hoặc output serialization |

---

## Tổng kết thay đổi theo file

| File | J1 | J2 | J3 | J4 | Mức thay đổi |
|---|---|---|---|---|---|
| `linker.ld` | `.grant_pages` | — | `.page_tables` 16×4K | — | 🟡 Trung bình |
| `src/grant.rs` | **Tạo mới** | — | — | — | 🟢 Mới |
| `src/irq.rs` | — | **Tạo mới** | — | — | 🟢 Mới |
| `src/lib.rs` | `mod grant` | `mod irq` | — | — | 🟢 Nhỏ |
| `src/mmu.rs` | `map/unmap_grant` | — | Per-task L2_device, `DEVICE_BLOCK_EL0`, layout 16 pages | — | 🔴 Lớn |
| `src/cap.rs` | +2 bits | +2 bits | +1 bit | — | 🟡 Trung bình |
| `src/exception.rs` | +2 cases SVC | +2 cases SVC, sửa IRQ dispatch | +1 case SVC | — | 🟡 Trung bình |
| `src/gic.rs` | — | +`disable_intid()` | — | — | 🟢 Nhỏ |
| `src/sched.rs` | cleanup grant | cleanup IRQ | — | — | 🟢 Nhỏ |
| `src/main.rs` | +wrappers, caps | +wrappers, caps | +wrapper | Task entries mới | 🔴 Lớn |
| `tests/host_tests.rs` | +6 tests | +9 tests | +1 test | +4 tests | 🟡 Trung bình |
| `tests/qemu_boot_test.*` | +1 checkpoint | +1 checkpoint | +1 checkpoint | +1 checkpoint | 🟢 Nhỏ |

---

## Thứ tự triển khai

| Bước | Sub-phase | Phụ thuộc | Checkpoint xác nhận | Risk |
|---|---|---|---|---|
| 1 | **J1: Shared Memory Grant** | Không — thiết kế sẵn từ Plan 09 | QEMU boot + `[AegisOS] grant system ready` + task A/B chia sẻ dữ liệu qua grant page | 🟢 Thấp |
| 2 | **J2: IRQ Routing** | J1 (grant cho driver buffer — optional), notification (✅ đã có) | + `[AegisOS] IRQ routing ready` + IRQ bind thành công + ~9 host tests | 🟡 Trung bình |
| 3 | **J3: Device MMIO Mapping** | J2 (driver cần cả IRQ + MMIO) | + `[AegisOS] device MMIO mapping ready` + Task ghi trực tiếp UART register | 🔴 Cao (page table layout change) |
| 4 | **J4: UART User-Mode Driver** | J2 + J3 | + `UART-DRV: ready` + client output qua user-mode driver | 🟡 Trung bình |
| 5 | **J5: Tests** | J1-J4 | `cargo test` ~114 tests pass (94+20). QEMU 16 checkpoints pass | 🟢 Thấp |

**Lưu ý thứ tự J3 (page table change):**
J3 là bước rủi ro cao nhất vì thay đổi `NUM_PAGE_TABLE_PAGES` từ 13→16 ảnh hưởng toàn bộ `PT_*` constants và linker layout. **Backup + test kỹ sau mỗi constant change.** Có thể chia J3 thành:
- J3a: Thêm 3 page tables vào linker, cập nhật constants, verify boot vẫn OK (KHÔNG thay đổi functionality)
- J3b: Implement `SYS_DEVICE_MAP` + `DEVICE_BLOCK_EL0` + per-task L2_device logic

---

## Syscall ABI sau Phase J

| # | Tên | x7 | x6 | x0 | x1 | Hướng |
|---|---|---|---|---|---|---|
| 0 | SYS_YIELD | 0 | — | — | — | — |
| 1 | SYS_SEND | 1 | ep_id | msg[0] | msg[1] | → |
| 2 | SYS_RECV | 2 | ep_id | msg[0] | msg[1] | ← |
| 3 | SYS_CALL | 3 | ep_id | msg[0..3] | | ↔ |
| 4 | SYS_WRITE | 4 | — | ptr | len | → |
| 5 | SYS_NOTIFY | 5 | target_id | bitmask | — | → |
| 6 | SYS_WAIT_NOTIFY | 6 | — | pending (out) | — | ← |
| 7 | **SYS_GRANT_CREATE** | 7 | peer_id | grant_id | — | → |
| 8 | **SYS_GRANT_REVOKE** | 8 | — | grant_id | — | → |
| 9 | **SYS_IRQ_BIND** | 9 | — | intid | notify_bit | → |
| 10 | **SYS_IRQ_ACK** | 10 | — | intid | — | → |
| 11 | **SYS_DEVICE_MAP** | 11 | — | device_id | — | → |

---

## Capability Bitmap sau Phase J

```
Bit  0: CAP_IPC_SEND_EP0
Bit  1: CAP_IPC_RECV_EP0
Bit  2: CAP_IPC_SEND_EP1
Bit  3: CAP_IPC_RECV_EP1
Bit  4: CAP_WRITE
Bit  5: CAP_YIELD
Bit  6: CAP_NOTIFY
Bit  7: CAP_WAIT_NOTIFY
Bit  8: CAP_IPC_SEND_EP2
Bit  9: CAP_IPC_RECV_EP2
Bit 10: CAP_IPC_SEND_EP3
Bit 11: CAP_IPC_RECV_EP3
Bit 12: CAP_GRANT_CREATE    ← MỚI (J1)
Bit 13: CAP_GRANT_REVOKE    ← MỚI (J1)
Bit 14: CAP_IRQ_BIND        ← MỚI (J2)
Bit 15: CAP_IRQ_ACK         ← MỚI (J2)
Bit 16: CAP_DEVICE_MAP      ← MỚI (J3)
Bits 17..63: Reserved (47 bits)
```

---

## Tổng kết chi phí

| Metric | Giá trị |
|---|---|
| File mới | 2 (`src/grant.rs`, `src/irq.rs`) |
| File sửa | 10 (`mmu.rs`, `cap.rs`, `exception.rs`, `gic.rs`, `sched.rs`, `lib.rs`, `main.rs`, `linker.ld`, `host_tests.rs`, boot test scripts) |
| Dòng code thêm (ước lượng) | ~400 kernel + ~200 test |
| Bộ nhớ thêm | 8 KiB grant pages + 12 KiB page tables (3 L2_device) + ~200B static (Grant/IRQ tables) |
| Tests mới | ~20 |
| Tổng tests | ~114 (94 cũ + 20 mới) |
| Syscalls mới | 5 (GRANT_CREATE, GRANT_REVOKE, IRQ_BIND, IRQ_ACK, DEVICE_MAP) |
| Tổng syscalls | 12 (0-11) |
| Capability bits mới | 5 (bits 12-16) |
| Tổng capability bits | 17/64 |
| QEMU checkpoints mới | 4 |
| Tổng checkpoints | 16 |

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|---|---|---|
| **DO-178C** §6.3.3 | Partitioning Integrity | Driver chạy ở EL0 + per-task page table + capability = spatial partitioning. IRQ routing = temporal isolation (driver chỉ xử lý khi có notification) |
| **DO-178C** §6.4.4 | Resource Usage | Shared memory grant với revoke = controlled resource sharing, audit trail qua UART log |
| **ISO 26262** Part 6 §7.4.4 | Freedom from interference — Spatial | Per-task L2_device: chỉ driver task thấy MMIO device. Các task khác → Permission Fault |
| **ISO 26262** Part 6 §7.4.5 | Freedom from interference — Temporal | IRQ masking giữa notification và ACK → driver không bị interrupt storm. Timer scheduling vẫn hoạt động |
| **IEC 62304** §5.3.2 | Software Architecture — Interfaces | Syscall ABI 7-11 = formal interface giữa driver (EL0) và kernel (EL1). Document đầy đủ |
| **IEC 62304** §5.5.3 | Software Unit Verification | 20 unit tests cover grant logic, IRQ binding, capability checks |

---

## Memory Layout sau Phase J

```
0x4008_0000  .text           (kernel + task code, RX)
             .rodata         (RO, EL0 readable)
             .data           (RW, EL1 only)
             .bss            (RW, EL1 only)
             .page_tables    (16 × 4KB = 64KB)    ← tăng từ 52KB
             .task_stacks    (3 × 4KB = 12KB)
             .user_stacks    (3 × 4KB = 12KB)
             .grant_pages    (2 × 4KB = 8KB)       ← MỚI
             ── guard page   (4KB, unmapped)
             ── boot stack   (16KB)
             __kernel_end
```

---

## Sơ đồ tổng quan sau Phase J

```
              ┌─────────────────────────────────────────────┐
              │  EL0 — Task 0 (UART Driver)                 │
              │  ┌─────────┐  ┌────────────┐  ┌──────────┐ │
              │  │ User    │  │ Grant      │  │ UART     │ │
              │  │ Stack   │  │ Page 0     │  │ MMIO     │ │
              │  │ (4KB)   │  │ (shared)   │  │ 0x0900.. │ │
              │  └─────────┘  └─────┬──────┘  └────┬─────┘ │
              │                     │               │       │
              │  SYS_IRQ_BIND ──────┼───────────────┤       │
              │  SYS_DEVICE_MAP ────┼───────────────┘       │
              └─────────────────────┼───────────────────────┘
                                    │
          ┌─────────────────────────┼───────────────────────┐
          │  EL1 — Kernel                                   │
          │                         │                       │
          │  ┌──────────────┐  ┌────┴─────────┐             │
          │  │ IRQ Bindings │  │ Grant Table  │             │
          │  │ INTID→Task   │  │ owner + peer │             │
          │  │ notify_bit   │  │ phys_addr    │             │
          │  └──────┬───────┘  └──────────────┘             │
          │         │                                       │
          │   IRQ fires (INTID 33)                          │
          │    → lookup binding                             │
          │    → TCBS[0].notify_pending |= bit              │
          │    → mask INTID (chờ ACK)                       │
          │         │                                       │
          │  ┌──────┴──────────────────────────────┐        │
          │  │ GICv2: GICD 0x0800_0000             │        │
          │  │        enable/disable/ack per INTID  │        │
          │  └─────────────────────────────────────┘        │
          └─────────────────────────────────────────────────┘
                                    │
              ┌─────────────────────┼───────────────────────┐
              │  EL0 — Task 1 (Client)                      │
              │  ┌─────────┐  ┌────┴──────┐                 │
              │  │ User    │  │ Grant     │                 │
              │  │ Stack   │  │ Page 0    │                 │
              │  │ (4KB)   │  │ (shared)  │                 │
              │  └─────────┘  └───────────┘                 │
              │                                             │
              │  SYS_CALL(EP0, buf, len) ──→ Task 0 recv    │
              └─────────────────────────────────────────────┘
```

---

## Bước tiếp theo đề xuất

1. [ ] **Review kế hoạch** → phản hồi/chỉnh sửa (đặc biệt J3 page table layout change)
2. [ ] **Triển khai J1** (Shared Memory Grant) — rủi ro thấp nhất, thiết kế sẵn từ Plan 09 (handoff → Aegis-Agent)
3. [ ] **Triển khai J2** (IRQ Routing) — cần test INTID 33 trên QEMU virt
4. [ ] **Triển khai J3** (Device MMIO Mapping) — chia thành J3a (layout change) + J3b (functionality)
5. [ ] **Triển khai J4** (UART User-Mode Driver PoC) — integration test cho J1+J2+J3
6. [ ] **Triển khai J5** (Tests) — 20 host tests + 4 QEMU checkpoints
7. [ ] **Viết blog #10** — giải thích interrupt routing và user-mode driver cho học sinh lớp 5 (handoff → Aegis-StoryTeller)
8. [ ] **Chạy test suite đầy đủ** — ~114 host tests + 16 QEMU checkpoints (handoff → Aegis-Tester)
9. [ ] **Cập nhật `copilot-instructions.md`** — reflect 12 syscalls, 17 capability bits, 16 page tables
