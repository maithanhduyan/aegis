# Kế hoạch Phase E — Fault Isolation & Task Restart

> **Trạng thái: ✅ HOÀN THÀNH** Khi một task ở EL0 crash (Data Abort, Instruction Abort, FP trap, illegal syscall), kernel **không dừng toàn bộ hệ thống** nữa. Thay vào đó: in diagnostic → đánh dấu task là `Faulted` → dọn IPC → chuyển sang task khác → tự động restart task sau N ticks. Chia thành 3 sub-phase tăng dần, mỗi sub-phase boot được trên QEMU.

---

## Tại sao Phase E?

- **Hiện tại:** Mọi exception từ EL0 mà kernel không handle → `loop { wfe }` = **toàn bộ hệ thống dừng**. Một task lỗi kéo tất cả chết theo.
- **Yêu cầu safety:** DO-178C (máy bay), ISO 26262 (ô tô), IEC 62304 (y tế) đều bắt buộc **fault containment** — lỗi ở một component không được lan sang component khác.
- **Tác động lớn, thay đổi nhỏ:** Chỉ sửa exception handlers + scheduler + IPC cleanup. Không cần thêm section mới trong linker script, không cần sửa MMU.

---

## Phân tích hiện trạng

### Exception handlers hiện tại — tất cả đều halt vô điều kiện

| Handler | File | Phân biệt source? | Hành vi |
|---|---|---|---|
| `handle_instruction_abort` | `src/exception.rs` | ✅ EC `0x20` = lower EL, `0x21` = same EL | In lỗi → `loop { wfe }` — halt bất kể source |
| `handle_data_abort` | `src/exception.rs` | ✅ EC `0x24` = lower EL, `0x25` = same EL | In lỗi → `loop { wfe }` — halt bất kể source |
| `handle_fp_trap` | `src/exception.rs` | ❌ Không nhận `source` param | Hardcode "Kernel code attempted FP" → halt |
| `handle_unknown` | `src/exception.rs` | ⚠️ Nhận `source` nhưng chỉ print, vẫn halt | In lỗi → `loop { wfe }` |

**Nhận xét:** `handle_instruction_abort` và `handle_data_abort` đã có logic phân biệt lower-EL vs same-EL qua EC. Chỉ cần thêm nhánh: nếu lower-EL → fault task thay vì halt.

### TaskState enum — thiếu `Faulted`

```
Inactive = 0, Ready = 1, Running = 2, Blocked = 3
```

Cần thêm `Faulted = 4`. Scheduler hiện tại chỉ chuyển `Running → Ready`, bỏ qua `Blocked`. Logic này **tự nhiên đúng** cho `Faulted` — task ở trạng thái `Faulted` sẽ không bị scheduler chuyển thành `Ready`.

### TCB — thiếu trường cho restart

```
Tcb { context: TrapFrame, state: TaskState, id: u16, stack_top: u64 }
```

Entry point ban đầu chỉ lưu vào `context.elr_el1` — sau context switch đầu tiên, bị ghi đè bởi PC hiện tại. **Mất vĩnh viễn.** Tương tự `context.sp_el0` (user stack top ban đầu).

→ Cần thêm `entry_point: u64` và `user_stack_top: u64` vào `Tcb`.

### IPC — không có cleanup khi task fault

- `Endpoint { sender: Option<usize>, receiver: Option<usize> }` — single-slot, 2 endpoints.
- Khi task A fault mà task B đang `Blocked` chờ A trên endpoint → B **kẹt vĩnh viễn**. Không có timeout, không có cleanup.
- Khi task A fault mà chính A đang là `sender`/`receiver` trên endpoint → slot không được giải phóng → endpoint bị "kẹt".

→ Cần hàm `cleanup_faulted_task(task_idx)` trong `ipc.rs` để xóa task khỏi mọi endpoint slot và unblock partner.

### Idle task fallback

Trong `schedule()`, khi không tìm thấy task `Ready`:
```rust
if !found {
    next = 2; // idle
    TCBS[2].state = TaskState::Ready;
}
```

**Rủi ro:** Nếu idle task cũng fault → force-ready một task đã fault → undefined behavior. Cần guard: idle task không bao giờ được phép fault (nó chỉ chạy `wfi`), hoặc thêm kiểm tra.

---

## Các bước thực hiện

### E1 — Fault Handler: Task crash → đánh dấu Faulted, chuyển task

**Mục tiêu:** Khi task ở EL0 gây exception không xử lý được, kernel không halt — thay vào đó đánh dấu task là `Faulted` và chuyển sang task tiếp theo.

**Thay đổi:**

1. **`src/sched.rs` — Thêm `Faulted` vào `TaskState`:**
   - Thêm variant `Faulted = 4` vào enum `TaskState`.

2. **`src/sched.rs` — Thêm trường TCB cho restart:**
   - Thêm `entry_point: u64` và `user_stack_top: u64` vào struct `Tcb` và `EMPTY_TCB`.
   - Trong `init()`: lưu entry point và user stack top vào trường mới (ngoài `context.elr_el1` / `context.sp_el0`).

3. **`src/sched.rs` — Thêm hàm `fault_current_task(frame)`:**
   - Lấy `CURRENT` task index.
   - In diagnostic: `"[AegisOS] TASK {id} FAULTED\n"`.
   - Set `TCBS[current].state = Faulted`.
   - Gọi `schedule(frame)` để chuyển sang task khác.
   - Nếu `schedule()` rơi vào fallback idle — đảm bảo idle task không bị override nếu đang `Faulted`.

4. **`src/exception.rs` — Sửa `handle_data_abort`:**
   - Nếu EC = `0x24` (lower EL): in diagnostic ngắn gọn → gọi `sched::fault_current_task(frame)` → **return** (không halt).
   - Nếu EC = `0x25` (same EL): giữ nguyên — in chi tiết + `loop { wfe }` (kernel bug, phải dừng).

5. **`src/exception.rs` — Sửa `handle_instruction_abort`:**
   - Nếu EC = `0x20` (lower EL): in diagnostic → `sched::fault_current_task(frame)` → return.
   - Nếu EC = `0x21` (same EL): giữ nguyên halt.

6. **`src/exception.rs` — Sửa `handle_fp_trap`:**
   - Thêm tham số `source: u64` vào signature.
   - Cập nhật caller (match arm EC `0x07` trong `exception_dispatch_sync`) truyền `source`.
   - Nếu source = lower EL (kiểm tra bằng tham số `source` từ asm): fault task + return.
   - Nếu source = same EL: giữ halt.

7. **`src/exception.rs` — Sửa `handle_unknown`:**
   - Nếu `source` cho thấy đây là lower-EL exception: fault task + return.
   - Nếu same-EL: giữ halt.

8. **`src/exception.rs` — Sửa `handle_svc` (invalid syscall number):**
   - Hiện tại nhánh `_` chỉ in warning. Đổi thành: fault task (illegal syscall = lỗi nghiêm trọng từ task).

**Checkpoint QEMU:** Thêm một "bad task" (task_c hoặc sửa tạm task_b) cố đọc kernel memory → Permission Fault → kernel in `TASK 1 FAULTED` → task_a + idle tiếp tục chạy → `A:PING A:PING ...` vẫn xuất hiện.

---

### E2 — IPC Cleanup: Giải phóng task fault khỏi IPC endpoint

**Mục tiêu:** Khi task fault, dọn sạch mọi slot IPC liên quan để partner không bị kẹt vĩnh viễn.

**Thay đổi:**

1. **`src/ipc.rs` — Thêm hàm `pub fn cleanup_task(task_idx: usize)`:**
   - Duyệt tất cả `ENDPOINTS[0..NUM_ENDPOINTS]`.
   - Nếu `ep.sender == Some(task_idx)` → clear thành `None`.
   - Nếu `ep.receiver == Some(task_idx)` → clear thành `None`.
   - Nếu partner đang `Blocked` chờ task này → set partner state = `Ready` (unblock). Partner sẽ được schedule lại — syscall sẽ retry hoặc nhận error.
   - **Quan trọng:** Partner nhận lại quyền chạy nhưng IPC message bị mất. Có thể set `x[0] = ERROR_PARTNER_FAULTED` trong partner context để partner biết.

2. **`src/sched.rs` — Gọi `ipc::cleanup_task(current)` trong `fault_current_task()`:**
   - Thêm gọi trước `schedule(frame)`.

**Checkpoint QEMU:** Task A send trên EP 0, Task B recv trên EP 0. Sửa tạm Task B để fault. Khi Task B fault → Task A bị unblock → Task A tiếp tục chạy (có thể in error hoặc retry send).

---

### E3 — Task Restart: Tự động khởi động lại task bị fault

**Mục tiêu:** Sau khi task fault, kernel tự động restart task sau N timer ticks (configurable). Task khởi động lại từ đầu với context sạch.

**Thay đổi:**

1. **`src/sched.rs` — Thêm `fault_tick: u64` vào `Tcb`:**
   - Khi task bị đánh dấu `Faulted`, lưu `TICK_COUNT` hiện tại vào `fault_tick`.
   - Thêm hằng số `RESTART_DELAY_TICKS: u64 = 100` (100 ticks × 10ms = 1 giây).

2. **`src/sched.rs` — Thêm hàm `pub fn restart_task(task_idx: usize)`:**
   - Guard: chỉ restart nếu `state == Faulted`.
   - Zero toàn bộ `context` (TrapFrame) — clear 288 bytes.
   - Set `context.elr_el1 = entry_point` (từ trường mới).
   - Set `context.spsr_el1 = 0x000` (EL0t).
   - Set `context.sp_el0 = user_stack_top` (từ trường mới).
   - Zero user stack memory: ghi 4096 byte zero vào `user_stack_top - 4096` (ngăn rò rỉ state cũ).
   - Set `state = Ready`.
   - In `"[AegisOS] TASK {id} RESTARTED\n"`.

3. **`src/sched.rs` — Kiểm tra restart trong `schedule()`:**
   - Trước vòng round-robin, duyệt tất cả task: nếu `state == Faulted` và `TICK_COUNT - fault_tick >= RESTART_DELAY_TICKS` → gọi `restart_task(i)`.
   - Task vừa restart sẽ có `state = Ready` → round-robin sẽ pick nó lên.

4. **`src/sched.rs` — Bảo vệ idle task:**
   - Trong `schedule()` fallback: nếu idle task (index 2) đang `Faulted`, gọi `restart_task(2)` ngay lập tức (không chờ delay) — hệ thống phải luôn có ít nhất idle task chạy.

5. **`src/timer.rs` — Export `TICK_COUNT`:**
   - Thêm `pub fn tick_count() -> u64` để `sched.rs` đọc được tick hiện tại (hoặc truyền tick vào `schedule`).

**Checkpoint QEMU:** Task B fault → in `TASK 1 FAULTED` → 1 giây sau in `TASK 1 RESTARTED` → `B:PONG` xuất hiện lại → hệ thống tự hồi phục hoàn toàn.

---

## Tóm tắt thay đổi theo file

| File | Thay đổi | Sub-phase |
|---|---|---|
| `src/sched.rs` | `Faulted` state, `entry_point`/`user_stack_top`/`fault_tick` fields, `fault_current_task()`, `restart_task()`, kiểm tra restart trong `schedule()` | E1 + E3 |
| `src/exception.rs` | Sửa 4 handler: lower-EL → fault task + return, same-EL → giữ halt. Thêm `source` cho `handle_fp_trap`. Sửa invalid syscall → fault. | E1 |
| `src/ipc.rs` | `cleanup_task()` — xóa task khỏi endpoint slots, unblock partner | E2 |
| `src/timer.rs` | Export `tick_count()` | E3 |
| `src/main.rs` | (Test) Tạm sửa task để trigger fault — verify checkpoint | E1, E2, E3 |
| `linker.ld` | Không thay đổi | — |
| `src/mmu.rs` | Không thay đổi | — |

---

## Điểm cần lưu ý

1. **Same-EL fault = kernel bug, phải halt.** Không bao giờ "fault and continue" cho lỗi kernel — đó là undefined behavior. Chỉ lower-EL (EL0 task) fault mới được recover.

2. **IPC error propagation:** Khi partner bị fault và bị xóa khỏi endpoint, task còn lại được unblock nhưng message bị mất. Có 2 lựa chọn:
   - **(A)** Set `x[0] = ERROR_CODE` trong partner context → partner cần kiểm tra return value. Phức tạp hơn nhưng đúng hơn.
   - **(B)** Chỉ unblock, partner sẽ retry send/recv → gặp endpoint trống → block lại → chờ task restart. Đơn giản hơn, phù hợp cho AegisOS hiện tại.
   - **Đề xuất:** Chọn **(B)** cho đơn giản. Phase F có thể thêm error codes.

3. **User stack zeroing:** Khi restart, nên zero user stack 4KB để ngăn rò rỉ dữ liệu từ lần chạy trước. Đây là yêu cầu security/safety — tiêu chuẩn IEC 62304 yêu cầu "clean state on restart".

4. **Restart delay tunable:** 100 ticks (1 giây) là giá trị mặc định hợp lý. Có thể thay đổi thành const hoặc per-task config sau. Quá nhanh → restart loop nếu bug deterministic. Quá chậm → hệ thống thiếu task quá lâu.

5. **Idle task bất tử:** Idle task (index 2) không bao giờ nên fault (chỉ chạy `wfi`). Nhưng phòng trường hợp, `schedule()` fallback force-restart idle ngay lập tức nếu nó `Faulted`.
