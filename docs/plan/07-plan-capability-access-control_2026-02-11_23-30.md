# Kế hoạch Phase G — Capability-Based Access Control

> **Trạng thái: ✅ HOÀN THÀNH** — Thêm hệ thống "giấy phép" (capability) cho AegisOS: mỗi task chỉ được gọi syscall mà nó có quyền. Task không có quyền → bị fault + auto-restart. Đây là lớp kiểm soát truy cập bắt buộc cho safety-critical (DO-178C §5.3, IEC 62304 §5.3 — access control).

---

## Tại sao Phase G?

### Lỗ hổng hiện tại: "Ai cũng được làm mọi thứ"

Hiện tại AegisOS có 5 syscall (YIELD, SEND, RECV, CALL, WRITE) và 2 IPC endpoint. **Không có bất kỳ kiểm soát truy cập nào** — mọi task đều có thể:

- Gửi tin nhắn trên **bất kỳ endpoint nào** (SEND/RECV/CALL chỉ kiểm tra `ep_id < MAX_ENDPOINTS`)
- Ghi ra UART (SYS_WRITE chỉ kiểm tra pointer hợp lệ, không kiểm tra task có quyền không)
- Gọi YIELD thoải mái

Trong đời thật, điều này **cực kỳ nguy hiểm**. Tưởng tượng:
- Task chơi nhạc có thể gửi lệnh lên endpoint điều khiển phanh
- Task hiển thị UI có thể ghi UART debug — lộ thông tin nhạy cảm
- Task bị lỗi restart lại — vẫn giữ nguyên mọi quyền

### Giải pháp: Capability bitmask

Thay vì hệ thống CSpace phức tạp kiểu seL4 (quá nặng cho 3 task tĩnh, không heap), AegisOS dùng **bitmask capability** — mỗi task có một số `u64` (64 bit), mỗi bit đại diện một quyền cụ thể.

Ưu điểm:
- **Chỉ 8 byte/task** — tổng 24 byte cho 3 task
- **Kiểm tra = 1 phép AND** — `(caps & required) == required`, O(1)
- **Pure logic** — testable trên host, 0 phụ thuộc phần cứng
- **Mở rộng dễ** — 64 bit >> 5 syscall × 2 endpoint, dư sức cho tương lai

---

## Phân tích hiện trạng

### Syscall path hiện tại

```
EL0 task → SVC #0 → exception vector → handle_exception_lower()
  → ESR decode → handle_svc(frame, esr)
    → match frame.x[7] {
        0 => schedule(),           // YIELD — không kiểm tra gì
        1 => ipc::sys_send(...),   // SEND — chỉ check ep_id < MAX
        2 => ipc::sys_recv(...),   // RECV — chỉ check ep_id < MAX
        3 => ipc::sys_call(...),   // CALL — chỉ check ep_id < MAX
        4 => handle_write(...),    // WRITE — chỉ check pointer range
        _ => unknown syscall       // fault
      }
```

**Điểm chèn capability check:** Ngay trước `match`, sau khi đọc `frame.x[7]` — thêm kiểm tra `cap_check(task_caps, required_cap)`. Nếu fail → fault task.

### TCB hiện tại (`src/sched.rs`)

```
Tcb {
    context:        TrapFrame,  // 288B — ABI-locked
    state:          TaskState,  // 1B
    id:             u16,        // 2B
    stack_top:      u64,        // 8B — SP_EL1 (kernel stack)
    entry_point:    u64,        // 8B — for restart
    user_stack_top: u64,        // 8B — SP_EL0
    fault_tick:     u64,        // 8B — khi nào bị fault
}
```

**Thêm field mới:** `caps: u64` — nằm cuối struct, không ảnh hưởng offset các field cũ (repr(C)).

### IPC endpoint — không thay đổi

```
Endpoint { sender: Option<usize>, receiver: Option<usize> }
```

Capability kiểm soát **ai được dùng** endpoint, không thay đổi cơ chế IPC bên trong.

### Task initialization (`src/main.rs`)

Task được khởi tạo tĩnh trong `sched::init()` rồi `bootstrap()` eret vào EL0. Capability assignment thêm vào giữa `sched::init()` và `timer::init()`.

---

## Thiết kế capability

### Bảng bit capability

| Bit | Hằng số | Quyền |
|-----|---------|-------|
| 0 | `CAP_IPC_SEND_EP0` | Gửi trên endpoint 0 |
| 1 | `CAP_IPC_RECV_EP0` | Nhận trên endpoint 0 |
| 2 | `CAP_IPC_SEND_EP1` | Gửi trên endpoint 1 |
| 3 | `CAP_IPC_RECV_EP1` | Nhận trên endpoint 1 |
| 4 | `CAP_WRITE` | Dùng SYS_WRITE (UART output) |
| 5 | `CAP_YIELD` | Dùng SYS_YIELD |
| 6–63 | Dự trữ | Timer, memory grant, cap delegation... |

### Hàm kiểm tra (pure, testable)

```
cap_check(caps: u64, required: u64) -> bool
    = (caps & required) == required
```

### Hàm mapping syscall → required capability

```
cap_for_syscall(syscall_nr: u64, ep_id: u64) -> u64
    YIELD  → CAP_YIELD
    SEND   → CAP_IPC_SEND_EP{ep_id}
    RECV   → CAP_IPC_RECV_EP{ep_id}
    CALL   → CAP_IPC_SEND_EP{ep_id} | CAP_IPC_RECV_EP{ep_id}
    WRITE  → CAP_WRITE
```

### Hành vi khi bị từ chối

**Fault task** — đây là lựa chọn phù hợp safety-critical:
- Gọi syscall không có quyền = **software defect** (lỗi thiết kế)
- In UART: `"[AegisOS] CAP DENIED: task {id}, syscall {nr}"`
- Gọi `fault_current_task()` → TaskState::Faulted → auto-restart sau 100 ticks (1 giây)
- Sau restart, task **vẫn giữ capability** (capability = chính sách tĩnh, không phải runtime state)

### Phân bổ capability cho 3 task hiện tại

| Task | ID | Vai trò | Capabilities |
|------|----|---------|-------------|
| task_a | 0 | Client PING | `CAP_IPC_SEND_EP0 \| CAP_IPC_RECV_EP0 \| CAP_WRITE \| CAP_YIELD` |
| task_b | 1 | Server PONG | `CAP_IPC_SEND_EP0 \| CAP_IPC_RECV_EP0 \| CAP_WRITE \| CAP_YIELD` |
| idle | 2 | WFI loop | `CAP_YIELD` |

→ idle task **không thể** gọi IPC hay WRITE — đúng nguyên tắc least privilege.

---

## Các bước thực hiện

### G1 — Tạo module `cap.rs` + thêm `caps` vào TCB

**Mục tiêu:** Định nghĩa capability system, thêm field vào TCB, chưa enforcement.

**Thay đổi:**

1. **Tạo `src/cap.rs`:**
   - Type alias `pub type CapBits = u64`
   - 6 hằng số capability (`CAP_IPC_SEND_EP0` ... `CAP_YIELD`)
   - Hàm `pub fn cap_check(caps: CapBits, required: CapBits) -> bool`
   - Hàm `pub fn cap_for_syscall(syscall_nr: u64, ep_id: u64) -> CapBits`
   - Hàm `pub fn cap_name(cap: CapBits) -> &'static str` (cho UART debug output)

2. **Sửa `src/sched.rs`:**
   - Thêm `pub caps: u64` vào `Tcb` (cuối struct, sau `fault_tick`)
   - Cập nhật `EMPTY_TCB`: `caps: 0`
   - **Không ảnh hưởng TrapFrame** (caps nằm ngoài TrapFrame)

3. **Sửa `src/lib.rs`:**
   - Thêm `pub mod cap;`

4. **Sửa `src/main.rs`:**
   - Trong `kernel_main()`, sau `sched::init()`, gán capability cho từng task

**Checkpoint:** Build thành công. `cargo test` pass (tests cũ không bị break). QEMU boot bình thường — capability chưa enforce, kernel chạy y hệt.

---

### G2 — Enforce capability trong `handle_svc`

**Mục tiêu:** Mọi syscall phải qua capability check. Task không có quyền → fault.

**Thay đổi:**

1. **Sửa `src/exception.rs` — `handle_svc()`:**
   - Trước `match syscall_nr`: đọc `caps` từ TCB hiện tại
   - Tính `required = cap::cap_for_syscall(syscall_nr, ep_id)`
   - Nếu `!cap::cap_check(caps, required)`:
     - In `"[AegisOS] CAP DENIED: task {id}, syscall {nr}"`
     - Gọi `fault_current_task()` + `schedule()` + return
   - Nếu pass → tiếp tục dispatch bình thường

2. **Edge case: unknown syscall (nr > 4):**
   - Hiện tại đã fault. Giữ nguyên — capability check chạy trước, nhưng unknown syscall vẫn fault dù có cap.

3. **Edge case: CALL = SEND + RECV:**
   - `cap_for_syscall(SYS_CALL, ep_id)` trả về `CAP_IPC_SEND_EPx | CAP_IPC_RECV_EPx` — cần cả 2 bit.

**Checkpoint:** QEMU boot thành công. task_a/task_b vẫn PING/PONG (chúng có đủ cap). idle task vẫn chạy WFI + YIELD (có `CAP_YIELD`).

---

### G3 — Capability không bị mất khi restart

**Mục tiêu:** Đảm bảo `restart_task()` không xoá capability.

**Thay đổi:**

1. **Kiểm tra `src/sched.rs` — `restart_task()`:**
   - Hiện tại chỉ reset `context` (TrapFrame) + `state` + `entry_point` + `user_stack_top`
   - `caps` nằm ngoài scope reset → **tự động giữ nguyên, không cần sửa gì**
   - Nếu `restart_task` zeroes toàn bộ TCB → cần giữ lại `caps` trước khi zeroes

2. **Xác nhận bằng test:** Viết test kiểm tra caps survive restart.

**Checkpoint:** Test xác nhận capability tồn tại sau restart.

---

### G4 — QEMU verification: idle task bị deny IPC

**Mục tiêu:** Chứng minh enforcement hoạt động trên QEMU thật.

**Thay đổi:**

1. **Thêm feature flag `test-cap-deny` trong `Cargo.toml`:**
   - Khi active: idle task (task 2) cố gọi `sys_send(0, ...)` → bị CAP DENIED
   - Expected UART output: `"[AegisOS] CAP DENIED: task 2, syscall 1"`

2. **Sửa idle entry (gated by `#[cfg(feature = "test-cap-deny")]`):**
   - Trước WFI loop: gọi `sys_send(0, 42, 0, 0, 0)` → bị deny
   - Không ảnh hưởng build mặc định

3. **Cập nhật `qemu_boot_test.sh` / `.ps1`:**
   - Thêm optional `--features test-cap-deny` test pass
   - Kiểm tra output có dòng `CAP DENIED`

**Checkpoint:** QEMU output hiển thị `CAP DENIED` cho idle task. task_a/task_b vẫn PING/PONG bình thường.

---

### G5 — Viết unit tests cho capability

**Mục tiêu:** ~12 test mới trong `tests/host_tests.rs`, nhóm **Capability**.

**Test cases:**

| # | Test name | Mô tả |
|---|-----------|-------|
| 1 | `cap_check_single_bit_set` | `cap_check(0x01, 0x01) == true` |
| 2 | `cap_check_single_bit_unset` | `cap_check(0x00, 0x01) == false` |
| 3 | `cap_check_multi_required_all_present` | `cap_check(0x07, 0x05) == true` |
| 4 | `cap_check_multi_required_partial` | `cap_check(0x01, 0x05) == false` |
| 5 | `cap_check_zero_required` | `cap_check(0x00, 0x00) == true` (no cap needed) |
| 6 | `cap_for_syscall_yield` | Trả về `CAP_YIELD` |
| 7 | `cap_for_syscall_send_ep0` | Trả về `CAP_IPC_SEND_EP0` |
| 8 | `cap_for_syscall_recv_ep1` | Trả về `CAP_IPC_RECV_EP1` |
| 9 | `cap_for_syscall_call_needs_both` | CALL cần SEND + RECV |
| 10 | `cap_for_syscall_write` | Trả về `CAP_WRITE` |
| 11 | `cap_survives_restart` | Gán caps → fault → caps vẫn còn |
| 12 | `cap_empty_denies_all_ipc` | `caps = 0` → mọi IPC cap check fail |

**Cập nhật `reset_test_state()`:** Reset `caps` trong mỗi TCB về 0.

**Checkpoint:** `cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1` — 67 tests pass (55 cũ + 12 mới).

---

## Tóm tắt thay đổi theo file

| File | Thay đổi | Sub-phase |
|---|---|---|
| `src/cap.rs` | **MỚI** — `CapBits`, 6 hằng số, `cap_check()`, `cap_for_syscall()`, `cap_name()` | G1 |
| `src/sched.rs` | Thêm `caps: u64` vào `Tcb`, cập nhật `EMPTY_TCB` | G1 |
| `src/lib.rs` | Thêm `pub mod cap;` | G1 |
| `src/main.rs` | Gán capability cho 3 task trong `kernel_main()` | G1 |
| `src/exception.rs` | Thêm capability check trong `handle_svc()` trước dispatch | G2 |
| `Cargo.toml` | Thêm feature `test-cap-deny` | G4 |
| `src/main.rs` | Idle task entry gated bởi `test-cap-deny` feature | G4 |
| `tests/qemu_boot_test.sh` | Thêm optional cap-deny test pass | G4 |
| `tests/qemu_boot_test.ps1` | Tương tự cho Windows | G4 |
| `tests/host_tests.rs` | Thêm ~12 capability tests, cập nhật `reset_test_state()` | G5 |

### Không thay đổi:
- `src/ipc.rs` — cơ chế IPC giữ nguyên, chỉ thêm gate ở tầng trên (handle_svc)
- `src/mmu.rs`, `src/gic.rs`, `src/timer.rs`, `src/uart.rs` — không liên quan
- `linker.ld` — 8 byte thêm vào BSS, không cần section mới
- `src/boot.s` — không thay đổi

---

## Điểm cần lưu ý

1. **TCB size tăng 8 byte.** `caps: u64` thêm vào cuối struct `Tcb`. Offset các field cũ **không thay đổi** (repr(C)). 4 test TrapFrame layout hiện tại vẫn pass vì TrapFrame nằm ở offset 0 và không bị ảnh hưởng.

2. **Capability = chính sách tĩnh.** Gán 1 lần trong `kernel_main()`, tồn tại suốt đời task, survive qua restart. Không có cơ chế runtime grant/revoke ở Phase G — để Phase H nếu cần.

3. **CALL = SEND + RECV.** `sys_call()` cần cả 2 bit capability. Nếu task chỉ có SEND mà không có RECV → cap denied.

4. **`cap_for_syscall` xử lý unknown ep_id.** Nếu `ep_id >= MAX_ENDPOINTS` → trả capability bất khả thi (không bao giờ match) → cap denied trước khi IPC code chạy. Double protection.

5. **UART output khi cap denied.** Dùng `uart_print!` (kernel-only) — task đang ở EL1 context (trong handler), an toàn.

6. **Backward compatibility.** Nếu `caps = 0xFFFF_FFFF_FFFF_FFFF` (tất cả bit bật), task có mọi quyền → hành vi y hệt trước Phase G. Có thể dùng hằng `CAP_ALL` cho backward compat khi debug.

7. **DO-178C mapping:**
   - Capability definition = §5.3.2 (Software Architecture — access control)
   - Capability enforcement = §5.3.3 (Software Detailed Design — least privilege)
   - Cap tests = §6.4.2.2 (Low-level Testing — new test cases)
   - Cap QEMU test = §6.4.3 (Integration Testing — denial scenario)

---

## Tổng kết chi phí

| Metric | Giá trị |
|--------|---------|
| File mới | 1 (`src/cap.rs`, ~50 dòng) |
| File sửa | 5 (`sched.rs`, `lib.rs`, `main.rs`, `exception.rs`, `host_tests.rs`) |
| Dòng code thêm | ~80 dòng kernel + ~120 dòng test |
| Bộ nhớ thêm | 24 byte BSS (3 × 8) |
| Tests mới | ~12 |
| Tổng tests sau Phase G | ~67 (55 + 12) |
| Risk | **Thấp** — capability check là pure logic, nằm trước dispatch, không sửa IPC/scheduler core |

---

## Đề xuất hành động tiếp theo

1. **Bắt đầu G1** — Tạo `src/cap.rs` với hằng số + `cap_check()` + `cap_for_syscall()`. Thêm `caps: u64` vào `Tcb`. Verify build + test cũ pass.

2. **Tiếp G2** — Chèn capability check vào `handle_svc()`. Verify QEMU boot vẫn PING/PONG.

3. **G3** — Xác nhận `restart_task()` không xoá caps. Viết test nhanh.

4. **G4** — Thêm `test-cap-deny` feature, verify idle task bị deny trên QEMU.

5. **G5** — Viết 12 unit tests. Verify 67/67 pass.

6. **Sau G5** — Viết blog #07. Cập nhật plan F trạng thái → ✅ DONE. Lên kế hoạch Phase H (Per-Task Address Space hoặc Capability Delegation).
