# Kế hoạch Phase I — Enhanced IPC: Notifications + Shared Memory Grant

> **Trạng thái: 📋 DRAFT** — Mở rộng IPC từ register-only (32 bytes) sang hệ thống hoàn chỉnh: async notification (tín hiệu không chặn), shared memory grant (chia sẻ vùng nhớ có kiểm soát giữa 2 task), multi-sender queue. Đây là nền tảng cho user-mode device driver (Phase J) và interrupt routing.

---

## Tại sao Phase I?

### Lỗ hổng hiện tại: "IPC quá yếu cho công việc thật"

Phase H đã cách ly bộ nhớ hoàn toàn — mỗi task có bản đồ riêng. Nhưng cách ly quá tốt lại tạo ra vấn đề mới:

**Hai task không có cách nào chia sẻ dữ liệu lớn.**

IPC hiện tại chỉ truyền được **32 bytes** (4 thanh ghi × 8 bytes). Đủ cho tin nhắn "PING/PONG", nhưng hoàn toàn không đủ cho:

- Camera gửi hình ảnh cho chương trình nhận diện → hàng triệu bytes
- Cảm biến gửi dữ liệu liên tục cho bộ lọc Kalman → hàng nghìn bytes/giây
- Driver nhận gói mạng → chuyển cho ứng dụng → hàng trăm bytes mỗi gói

Ngoài ra, IPC hiện tại hoàn toàn **đồng bộ** (blocking). Nếu task A gửi tin nhắn mà task B chưa sẵn sàng → A bị chặn. Điều này nguy hiểm cho hệ thống thời gian thực:

- Task điều khiển phanh **không bao giờ được phép bị chặn** chờ task phát nhạc
- Timer interrupt cần **thông báo** cho nhiều task mà không đợi ai

### Ba vấn đề cần giải quyết

| # | Vấn đề | Ảnh hưởng |
|---|---|---|
| 1 | Register-only payload (32B max) | Không truyền được dữ liệu lớn |
| 2 | Blocking-only IPC | Task quan trọng bị chặn bởi task kém quan trọng |
| 3 | Single-slot endpoint (1 sender, 1 receiver) | Không hỗ trợ mô hình nhiều client gửi đến 1 server |

### Giải pháp: 3 cơ chế mới

| Cơ chế | Mô tả | Giải quyết |
|---|---|---|
| **Notification** | Tín hiệu async (u64 bitmask), không chặn sender | Vấn đề #2 |
| **Shared Memory Grant** | Kernel cấp quyền cho 2 task chia sẻ 1 vùng nhớ cụ thể | Vấn đề #1 |
| **Multi-sender Queue** | Endpoint chấp nhận nhiều sender xếp hàng | Vấn đề #3 |

---

## Phân tích hiện trạng

### IPC hiện tại (sau Phase H)

```
Endpoint {
    sender:   Option<usize>,    // 1 task chờ gửi (hoặc None)
    receiver: Option<usize>,    // 1 task chờ nhận (hoặc None)
}

ENDPOINTS: [Endpoint; 2]   // chỉ 2 endpoint

Syscalls:
  SYS_SEND (1): gửi x[0..3] → block nếu không có receiver
  SYS_RECV (2): nhận x[0..3] → block nếu không có sender
  SYS_CALL (3): send + recv atomic
```

### Capability hiện tại

```rust
CAP_YIELD         = 1 << 0    // SYS_YIELD
CAP_IPC_SEND_EP0  = 1 << 1    // SYS_SEND/CALL trên EP0
CAP_IPC_RECV_EP0  = 1 << 2    // SYS_RECV trên EP0
CAP_IPC_SEND_EP1  = 1 << 3    // SYS_SEND/CALL trên EP1
CAP_IPC_RECV_EP1  = 1 << 4    // SYS_RECV trên EP1
CAP_WRITE         = 1 << 5    // SYS_WRITE
// bits 6..63: chưa dùng → CÓ THỂ MỞ RỘNG
```

### Hạn chế cần khắc phục

1. **Endpoint cố định 2 slots** — không thể thêm endpoint runtime
2. **Single sender/receiver** — nếu 2 task cùng gửi vào EP0 → task thứ 2 bị bỏ qua
3. **Không có notification** — mọi IPC đều blocking
4. **Không chia sẻ bộ nhớ** — Phase H cách ly hoàn toàn, nhưng đôi khi cần chia sẻ có kiểm soát

---

## Thiết kế Phase I

### I1 — Notification (Tín hiệu async)

#### Khái niệm

Notification = **tín hiệu nhẹ, không blocking** dùng bitmask u64. Mỗi task có một `notification word` (u64). Bit nào bật = có tín hiệu loại đó.

Hoạt động giống **chuông cửa**: ai cũng có thể bấm chuông (set bit), người trong nhà tùy lúc mới ra mở cửa (poll hoặc wait).

#### Thiết kế dữ liệu

```rust
// Thêm vào Tcb (sched.rs)
pub struct Tcb {
    // ...existing fields...
    pub ttbr0: u64,
    pub notify_pending: u64,   // ← MỚI: bitmask tín hiệu đang chờ
    pub notify_waiting: bool,  // ← MỚI: task đang chờ notification?
}
```

#### Syscall mới

| Syscall | Số | Tham số | Hành vi |
|---|---|---|---|
| `SYS_NOTIFY` | 5 | x6 = target_task_id, x0 = bitmask | OR bitmask vào `notify_pending` của target. Nếu target đang `notify_waiting` → unblock. **Không block sender.** |
| `SYS_WAIT_NOTIFY` | 6 | (không tham số) | Nếu `notify_pending != 0`: trả kết quả trong x0, clear pending, trở về ngay. Nếu `== 0`: block cho đến khi có notification. |

#### Capability mới

```rust
CAP_NOTIFY        = 1 << 6    // Cho phép gọi SYS_NOTIFY
CAP_WAIT_NOTIFY   = 1 << 7    // Cho phép gọi SYS_WAIT_NOTIFY
```

#### Luồng hoạt động

```
Task A (sender):                     Task B (receiver):
  SYS_NOTIFY(target=1, bits=0x01)      SYS_WAIT_NOTIFY()
  → không block, tiếp tục chạy          → nếu pending != 0: trả ngay
  → OR 0x01 vào B.notify_pending        → nếu pending == 0: block
  → nếu B đang wait → unblock B         → khi unblock: x0 = pending bits
                                         → clear pending
```

**Đặc điểm quan trọng:**
- **Fire-and-forget** — sender không bao giờ bị block
- **Coalescing** — nhiều notify trước khi wait → tất cả OR lại thành 1 bitmask
- **64 loại tín hiệu** — đủ cho interrupt routing, timer events, fault alerts

---

### I2 — Multi-sender Queue cho Endpoint

#### Vấn đề

Endpoint hiện tại: `sender: Option<usize>` — chỉ 1 task chờ gửi. Nếu task thứ 2 cũng muốn gửi → không có chỗ.

#### Giải pháp: Circular queue

```rust
const MAX_WAITERS: usize = 4;  // tối đa 4 task chờ trên 1 endpoint

pub struct Endpoint {
    pub sender_queue: [Option<usize>; MAX_WAITERS],
    pub sender_head: usize,
    pub sender_count: usize,
    pub receiver: Option<usize>,  // receiver vẫn single-slot (1 server pattern)
}
```

**Tại sao receiver vẫn single-slot?** Mô hình microkernel điển hình là **nhiều client → 1 server**. Server recv từ endpoint, xử lý, trả lời. Chỉ 1 server recv tại một thời điểm.

#### Thay đổi logic IPC

- `sys_send()`: nếu không có receiver → push vào `sender_queue` (thay vì chỉ set `sender`)
- `sys_recv()`: nếu có sender trong queue → pop đầu tiên (FIFO), deliver message
- `cleanup_task()`: scan toàn bộ `sender_queue`, xóa task faulted

---

### I3 — Shared Memory Grant

#### Khái niệm

Kernel cấp quyền cho 2 task **chia sẻ một vùng nhớ cụ thể** — cả hai đều map vùng đó là `AP_RW_EL0` trong bảng trang riêng. Vùng này gọi là **grant region**.

#### Dùng tĩnh (Phase I) — không cần allocator

Vì AegisOS không có heap, grant region được **cấp phát tĩnh trong linker script**:

```ld
/* linker.ld — thêm section */
.grant_pages (NOLOAD) : ALIGN(4096) {
    __grant_pages_start = .;
    . = . + 2 * 4096;      /* 2 grant pages × 4KB = 8KB */
    __grant_pages_end = .;
} > RAM
```

2 grant pages: grant 0 (4KB) và grant 1 (4KB). Mỗi grant page có thể được chia sẻ giữa 2 task.

#### Cấu trúc dữ liệu

```rust
// Trong module mới: src/grant.rs

pub const MAX_GRANTS: usize = 2;
pub const GRANT_PAGE_SIZE: usize = 4096;

pub struct Grant {
    pub owner: Option<usize>,     // task tạo grant
    pub peer: Option<usize>,      // task được chia sẻ
    pub phys_addr: u64,           // physical address của grant page
    pub active: bool,
}

pub static mut GRANTS: [Grant; MAX_GRANTS] = [EMPTY_GRANT; MAX_GRANTS];
```

#### Syscall mới

| Syscall | Số | Tham số | Hành vi |
|---|---|---|---|
| `SYS_GRANT_CREATE` | 7 | x0 = grant_id, x6 = peer_task_id | Map grant page vào **cả hai** task's L3 page table với `AP_RW_EL0`. Cả owner và peer đều đọc/ghi được. |
| `SYS_GRANT_REVOKE` | 8 | x0 = grant_id | Unmap grant page từ peer's L3 (set entry = `AP_RW_EL1`). Owner vẫn giữ access. |

#### Cơ chế page table update

Khi `SYS_GRANT_CREATE(grant_id=0, peer=1)`:

1. Tìm grant page 0 address (từ `__grant_pages_start`)
2. Tìm L3 entry index cho grant page address trong L3 tables
3. Trong L3 của task owner: set entry = `phys_addr | USER_DATA_PAGE` (AP_RW_EL0)
4. Trong L3 của task peer: set entry = `phys_addr | USER_DATA_PAGE` (AP_RW_EL0)
5. `tlbi aside1, ASID_owner` + `tlbi aside1, ASID_peer` + `dsb ish` + `isb`

Khi `SYS_GRANT_REVOKE(grant_id=0)`:

1. Trong L3 của peer: set entry = `phys_addr | KERNEL_DATA_PAGE` (AP_RW_EL1 — EL0 no access)
2. `tlbi aside1, ASID_peer` + `dsb ish` + `isb`

#### Capability

```rust
CAP_GRANT_CREATE  = 1 << 8    // Cho phép tạo grant
CAP_GRANT_REVOKE  = 1 << 9    // Cho phép thu hồi grant
```

#### Constraint quan trọng

- Grant page nằm **sau** `.user_stacks` → phải nằm trong vùng L3 cover (first 2MiB: `0x4000_0000..0x401F_FFFF`)
- Linker phải đặt `.grant_pages` trước guard page
- Khi task fault + restart: grant **bị thu hồi tự động** (revoke tất cả grant liên quan đến task faulted)

---

### I4 — Mở rộng Endpoint lên 4

Tăng `MAX_ENDPOINTS` từ 2 lên 4. Thêm capability bits cho EP2, EP3:

```rust
CAP_IPC_SEND_EP2  = 1 << 10
CAP_IPC_RECV_EP2  = 1 << 11
CAP_IPC_SEND_EP3  = 1 << 12
CAP_IPC_RECV_EP3  = 1 << 13
```

Cập nhật `cap_for_syscall()` để dispatch EP2, EP3.

---

## Tóm tắt thay đổi theo file

| File | Thay đổi | Sub-phase |
|---|---|---|
| `src/sched.rs` | Thêm `notify_pending: u64`, `notify_waiting: bool` vào Tcb + EMPTY_TCB | I1 |
| `src/ipc.rs` | Multi-sender queue, tăng MAX_ENDPOINTS=4, sửa sys_send/recv/call + cleanup | I2, I4 |
| `src/exception.rs` | Dispatch SYS_NOTIFY(5), SYS_WAIT_NOTIFY(6), SYS_GRANT_CREATE(7), SYS_GRANT_REVOKE(8) trong `handle_svc` | I1, I3 |
| `src/cap.rs` | Thêm 8 capability bits: NOTIFY, WAIT_NOTIFY, GRANT_CREATE, GRANT_REVOKE, EP2/EP3 SEND/RECV | I1, I3, I4 |
| **MỚI** `src/grant.rs` | Module quản lý shared memory grants | I3 |
| `src/lib.rs` | `pub mod grant;` | I3 |
| `src/main.rs` | Cập nhật task caps, demo notification + grant trong task_a/task_b, UART messages | I1–I4 |
| `linker.ld` | Thêm `.grant_pages` section (8KB, 2 pages) | I3 |
| `src/mmu.rs` | Grant page ban đầu mapped `AP_RW_EL1` (kernel only), hàm `map_grant_for_task()` / `unmap_grant_for_task()` để update L3 entries runtime | I3 |
| `tests/host_tests.rs` | ~15 tests mới: notification, multi-sender queue, grant, expanded caps | I1–I4 |
| `tests/qemu_boot_test.sh` | Thêm checkpoints cho notification + grant | I1–I4 |
| `tests/qemu_boot_test.ps1` | Tương tự | I1–I4 |

### Không thay đổi

- `src/boot.s` — boot flow giữ nguyên
- `src/gic.rs`, `src/timer.rs` — giữ nguyên (interrupt routing là Phase J)

---

## Các bước thực hiện

### I1 — Notification System (async tín hiệu)

1. **Sửa `src/sched.rs`**: Thêm `notify_pending: u64` và `notify_waiting: bool` vào `Tcb` + `EMPTY_TCB`
2. **Sửa `src/cap.rs`**: Thêm `CAP_NOTIFY = 1 << 6`, `CAP_WAIT_NOTIFY = 1 << 7`
3. **Sửa `src/exception.rs`**: Thêm case `5 => handle_notify(frame)` và `6 => handle_wait_notify(frame)` vào `handle_svc`
4. **Implement `handle_notify()`**: Đọc target_id từ `x6`, bitmask từ `x0`. OR vào target's `notify_pending`. Nếu target đang `notify_waiting` → unblock (set Ready, clear `notify_waiting`)
5. **Implement `handle_wait_notify()`**: Nếu caller's `notify_pending != 0` → set `x0 = pending`, clear pending, return. Nếu `== 0` → set `notify_waiting = true`, block, schedule away
6. **Sửa `src/main.rs`**: Thêm `syscall_notify()` và `syscall_wait_notify()` wrapper cho EL0. Demo trong `task_a` hoặc thêm task behavior
7. **Cập nhật caps** trong `kernel_main()` cho các task

**Checkpoint:** Build + QEMU boot. Task A notify Task B, Task B nhận notification. UART hiển thị kết quả.

---

### I2 — Multi-sender Queue

1. **Sửa `src/ipc.rs`**: Đổi `sender: Option<usize>` → `sender_queue: [Option<usize>; MAX_WAITERS]` + `sender_head`/`sender_count`
2. **Sửa `sys_send()`**: Nếu không có receiver → push vào queue (FIFO). Nếu queue đầy → trả lỗi (set x0 = error code)
3. **Sửa `sys_recv()`**: Nếu có sender trong queue → pop front, deliver
4. **Sửa `cleanup_task()`**: Scan toàn bộ sender_queue, xóa faulted task entries, compact lại
5. **Unit tests**: Test 3 task cùng send vào 1 endpoint, receiver nhận theo thứ tự FIFO

**Checkpoint:** Build + QEMU. 2 task cùng gửi vào EP0, server nhận đúng thứ tự.

---

### I3 — Shared Memory Grant

1. **Sửa `linker.ld`**: Thêm `.grant_pages` section (2×4KB) sau `.user_stacks`, trước guard page. Thêm `__grant_pages_start`, `__grant_pages_end` symbols
2. **Sửa `src/mmu.rs`**: Trong `build_l3()`, grant pages ban đầu = `KERNEL_DATA_PAGE` (EL0 no access). Thêm hàm `pub unsafe fn map_grant_for_task(grant_phys: u64, task_id: usize)` và `pub unsafe fn unmap_grant_for_task(grant_phys: u64, task_id: usize)` — update L3 entry + TLB invalidate
3. **Tạo `src/grant.rs`**: `Grant` struct, `GRANTS` static, `grant_create()`, `grant_revoke()`, `grant_cleanup_task()`
4. **Sửa `src/cap.rs`**: Thêm `CAP_GRANT_CREATE`, `CAP_GRANT_REVOKE`
5. **Sửa `src/exception.rs`**: Dispatch `SYS_GRANT_CREATE(7)`, `SYS_GRANT_REVOKE(8)` trong `handle_svc`
6. **Sửa `src/sched.rs`**: Trong `fault_current_task()` hoặc `restart_task()`, gọi `grant::cleanup_task()` để revoke tất cả grant liên quan
7. **Sửa `src/main.rs`**: Demo: Task A tạo grant, ghi dữ liệu vào grant page, notify Task B. Task B đọc dữ liệu từ grant page, xác nhận
8. **Unit tests**: Grant create/revoke, access after revoke, cleanup on fault

**Checkpoint:** Build + QEMU. Task A ghi "HELLO" vào grant page, Task B đọc đúng "HELLO". UART xác nhận.

---

### I4 — Mở rộng Endpoint + Capability

1. **Sửa `src/ipc.rs`**: `MAX_ENDPOINTS = 4`
2. **Sửa `src/cap.rs`**: Thêm `CAP_IPC_SEND_EP2/3`, `CAP_IPC_RECV_EP2/3`. Cập nhật `cap_for_syscall()`
3. **Unit tests**: Capability check cho EP2, EP3

**Checkpoint:** Build + test. Tất cả tests pass.

---

### I5 — Tests + Boot Checkpoints

1. **~15 unit tests mới** trong `tests/host_tests.rs`:
   - Notification: pending OR, wait returns pending, clear after wait, no-block sender
   - Multi-sender: queue FIFO order, queue full rejection, cleanup removes from queue
   - Grant: create maps both tasks, revoke unmaps peer, cleanup on fault
   - Expanded caps: EP2/EP3 capability bits, notify/grant caps
2. **Boot checkpoints**: `"[AegisOS] notification system ready"`, `"[AegisOS] grant system ready"`, `"[AegisOS] endpoints: 4"`
3. **Cập nhật `reset_test_state()`**: Reset `notify_pending`, `notify_waiting`, `GRANTS`

**Checkpoint:** `cargo test` — ~94 tests pass (79 cũ + 15 mới). QEMU boot — tất cả checkpoints pass.

---

## Sơ đồ tổng quan sau Phase I

```
                    ┌─────────────────────────────────┐
                    │         EL0 Task A               │
                    │  ┌───────┐  ┌───────┐            │
                    │  │ Stack │  │ Grant │ ← shared   │
                    │  └───────┘  │ Page  │   with B   │
                    │             └───┬───┘            │
                    └─────────────────┼────────────────┘
                                      │ SYS_GRANT_CREATE
                    ┌─────────────────┼────────────────┐
                    │         EL1 Kernel                │
                    │                                   │
                    │  ┌──────────────┐                 │
                    │  │ Notification │ ← async signal  │
                    │  │ u64 bitmask  │                 │
                    │  └──────────────┘                 │
                    │                                   │
                    │  ┌──────────────────────┐         │
                    │  │ Endpoint Queue       │         │
                    │  │ [S0, S1, S2, S3] → R │         │
                    │  └──────────────────────┘         │
                    │                                   │
                    │  ┌──────────────┐                 │
                    │  │ Grant Table  │ ← 2 entries     │
                    │  │ owner + peer │                  │
                    │  └──────────────┘                 │
                    └─────────────────┼────────────────┘
                                      │
                    ┌─────────────────┼────────────────┐
                    │         EL0 Task B               │
                    │  ┌───────┐  ┌───────┐            │
                    │  │ Stack │  │ Grant │ ← shared   │
                    │  └───────┘  │ Page  │   with A   │
                    │             └───────┘             │
                    └──────────────────────────────────┘
```

---

## Syscall ABI sau Phase I

| # | Tên | x7 | x6 | x0–x3 | Hướng |
|---|---|---|---|---|---|
| 0 | SYS_YIELD | 0 | — | — | — |
| 1 | SYS_SEND | 1 | ep_id | message payload | → |
| 2 | SYS_RECV | 2 | ep_id | message payload | ← |
| 3 | SYS_CALL | 3 | ep_id | send → recv | ↔ |
| 4 | SYS_WRITE | 4 | — | x0=ptr, x1=len | → |
| 5 | SYS_NOTIFY | 5 | target_task | x0=bitmask | → |
| 6 | SYS_WAIT_NOTIFY | 6 | — | x0=pending (out) | ← |
| 7 | SYS_GRANT_CREATE | 7 | peer_task | x0=grant_id | → |
| 8 | SYS_GRANT_REVOKE | 8 | — | x0=grant_id | → |

---

## Capability Bitmap sau Phase I

```
Bit  0: CAP_YIELD
Bit  1: CAP_IPC_SEND_EP0
Bit  2: CAP_IPC_RECV_EP0
Bit  3: CAP_IPC_SEND_EP1
Bit  4: CAP_IPC_RECV_EP1
Bit  5: CAP_WRITE
Bit  6: CAP_NOTIFY          ← MỚI
Bit  7: CAP_WAIT_NOTIFY     ← MỚI
Bit  8: CAP_GRANT_CREATE    ← MỚI
Bit  9: CAP_GRANT_REVOKE    ← MỚI
Bit 10: CAP_IPC_SEND_EP2    ← MỚI
Bit 11: CAP_IPC_RECV_EP2    ← MỚI
Bit 12: CAP_IPC_SEND_EP3    ← MỚI
Bit 13: CAP_IPC_RECV_EP3    ← MỚI
Bits 14..63: Reserved
```

---

## Tổng kết chi phí

| Metric | Giá trị |
|---|---|
| File mới | 1 (`src/grant.rs`) |
| File sửa | 10 (`sched.rs`, `ipc.rs`, `exception.rs`, `cap.rs`, `lib.rs`, `main.rs`, `linker.ld`, `mmu.rs`, `host_tests.rs`, boot test scripts) |
| Dòng code thêm | ~250 kernel + ~150 test |
| Bộ nhớ thêm | 8 KiB BSS (2 grant pages) + ~100B static (Grant table, queue arrays) |
| Tests mới | ~15 |
| Tổng tests | ~94 (79 cũ + 15 mới) |
| Syscalls mới | 4 (NOTIFY, WAIT_NOTIFY, GRANT_CREATE, GRANT_REVOKE) |
| Tổng syscalls | 9 |
| Risk | **Trung bình** — Notification đơn giản (OR bitmask). Grant phức tạp hơn (runtime page table update + TLB invalidate). Multi-sender queue cần cẩn thận race condition (nhưng single-core nên OK). |

---

## Điểm cần lưu ý

1. **Grant page phải trong vùng L3** — first 2MiB (`0x4000_0000..0x401F_FFFF`). Nếu linker đặt `.grant_pages` vượt quá 2MiB offset → không có L3 entry → phải dùng L2 2MiB block mapping → KHÔNG đủ fine-grained. **Cần kiểm tra linker output.**

2. **TLB invalidate khi grant** — Dùng `tlbi aside1is, <ASID>` để chỉ flush TLB entries của task bị ảnh hưởng, không flush toàn bộ. Hiệu quả hơn `tlbi vmalle1`.

3. **Notification là nền tảng cho interrupt routing (Phase J)** — Khi device interrupt xảy ra, kernel handler sẽ `SYS_NOTIFY` task driver. Task driver `SYS_WAIT_NOTIFY` rồi xử lý device.

4. **Grant cleanup trên fault** — Khi task bị fault, tất cả grant liên quan phải bị revoke. Nếu không, task restart có thể truy cập grant page mà peer đã giải phóng → stale mapping.

5. **Single-core simplicity** — Không cần lock/atomic cho queue operations vì chỉ có 1 core. Interrupt handler chạy trên cùng core, nhưng `schedule()` được gọi với IRQ disabled (trong handler) → no preemption during IPC logic → safe.

6. **Memory layout sau Phase I:**
   ```
   0x4008_0000  .text (kernel + task code)
                .rodata
                .data
                .bss
                .page_tables (13 × 4KB = 52KB)
                .task_stacks (3 × 4KB)
                .user_stacks (3 × 4KB)
                .grant_pages (2 × 4KB) ← MỚI
                guard page (4KB)
                boot stack (16KB)
   ```

---

## Đề xuất hành động tiếp theo

1. **Bắt đầu I1 (Notification)** — Thêm `notify_pending` + `notify_waiting` vào TCB. Implement SYS_NOTIFY + SYS_WAIT_NOTIFY. Verify trên QEMU. **Đây là sub-phase an toàn nhất — không đụng page table, không đụng memory layout.**

2. **Tiếp I2 (Multi-sender queue)** — Refactor Endpoint struct. Sửa sys_send/recv. Test FIFO order. **Low risk — chỉ thay đổi data structure.**

3. **I3 (Shared Memory Grant)** — Thêm `.grant_pages` vào linker. Implement grant module. Runtime page table update + TLB invalidate. **Đây là sub-phase rủi ro cao nhất — cần test kỹ trên QEMU.**

4. **I4 (Expand Endpoints)** — Tăng MAX_ENDPOINTS, thêm caps. **Trivial.**

5. **I5 (Tests + checkpoints)** — Viết 15 unit tests, cập nhật boot test scripts. **Verify tất cả pass.**

6. **Sau Phase I** — Viết blog #09. Lên kế hoạch Phase J (Interrupt Routing + User-Mode Device Driver). Đây là nơi notification + grant kết hợp để tạo driver framework hoàn chỉnh.
