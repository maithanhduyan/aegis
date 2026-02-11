# Kế hoạch Phase K — Priority Scheduler + Watchdog Heartbeat

> **Trạng thái: 📋 DRAFT** — Thay thế round-robin bằng priority-based scheduler với time budget enforcement, thêm watchdog heartbeat mechanism để phát hiện task "treo logic". Đây là bước quan trọng nhất để AegisOS đáp ứng yêu cầu deterministic timing của cả 3 tiêu chuẩn safety-critical (DO-178C, IEC 62304, ISO 26262).

---

## Tại sao Phase K?

### Lỗ hổng hiện tại: "Mọi task đều bình đẳng, không ai bị giám sát"

AegisOS sau Phase J có đầy đủ microkernel primitives: IPC, capability, per-task address space, shared memory, IRQ routing, user-mode driver. Nhưng scheduler vẫn là **round-robin thuần** — mọi task được CPU time bằng nhau, không phân biệt task phanh xe (safety-critical) với task hiển thị đồng hồ (non-critical).

Trong hệ thống safety-critical thực tế, đây là lỗ hổng **chết người**:

- **Tên lửa**: Task điều hướng INS/GPS cần chạy mỗi 10ms với deadline cứng. Nếu task telemetry chiếm CPU → tên lửa mất kiểm soát quỹ đạo. Round-robin không đảm bảo task nào chạy trước.
- **Xe tự lái**: Task xử lý phanh ABS (ASIL D) phải phản hồi trong 5ms. Task infotainment (QM) đang chạy loop nặng → scheduler round-robin cho infotainment tiếp tục → phanh trễ 10ms → tai nạn.
- **Y tế**: Máy thở cần điều chỉnh lưu lượng oxy mỗi chu kỳ hít thở (~3 giây). Task logging ghi file → chiếm CPU → máy thở không điều chỉnh kịp → bệnh nhân nguy hiểm.

Ngoài ra, hiện tại **không có cơ chế nào** phát hiện task bị "treo logic" (infinite loop không vi phạm memory). Task vẫn chạy, vẫn được schedule, nhưng không làm việc hữu ích — kernel không biết.

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|---|---|
| 1 | Round-robin không phân biệt priority — task critical và non-critical chia đều CPU | Task safety-critical có thể miss deadline → hệ thống mất an toàn |
| 2 | Không có time budget — task có thể chiếm CPU vô hạn trong mỗi epoch | Một task "nặng" ảnh hưởng tất cả task khác (temporal interference) |
| 3 | Không có watchdog/heartbeat — task treo logic không bị phát hiện | Task dừng hoạt động nhưng kernel vẫn schedule → lãng phí CPU + safety hazard |
| 4 | Không có priority inversion protection — task thấp giữ resource chặn task cao | Hiện tượng Mars Pathfinder: task thấp giữ IPC endpoint → task cao bị block vô thời hạn |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|---|---|---|
| **Priority Scheduler** | Mỗi task có priority (0–7). Scheduler luôn chọn task Ready có priority cao nhất | #1 |
| **Time Budget** | Mỗi task có budget (max ticks/epoch). Hết budget → forced yield đến epoch tiếp | #2 |
| **Watchdog Heartbeat** | Task khai báo heartbeat interval. Kernel monitor: không heartbeat đúng hạn → fault + restart | #3 |
| **Priority Inheritance (đơn giản)** | Khi task thấp giữ IPC endpoint mà task cao cần → tạm nâng priority task thấp | #4 |

---

## Phân tích hiện trạng

### Scheduler hiện tại (`src/sched.rs`)

```
Thuật toán: Round-robin thuần
  schedule(frame):
    1. Save context task hiện tại
    2. Mark old task = Ready (nếu Running)
    3. Auto-restart: scan tất cả Faulted task, restart nếu đủ delay
    4. Round-robin: next = (old + 1) % 3, tìm task Ready
    5. Nếu không có Ready → force idle (task 2)
    6. Load context task mới, switch TTBR0

Timer tick: 10ms (62.5 MHz CNTP trên QEMU)
Context switch: mỗi timer IRQ = mỗi 10ms

Không có: priority, time budget, epoch, preemption dựa trên priority
```

### TCB hiện tại

```rust
pub struct Tcb {
    pub context: TrapFrame,      // 288 bytes, ABI-locked
    pub state: TaskState,        // Inactive/Ready/Running/Blocked/Faulted
    pub id: u16,
    pub stack_top: u64,          // kernel SP_EL1
    pub entry_point: u64,        // for restart
    pub user_stack_top: u64,     // SP_EL0 for restart
    pub fault_tick: u64,         // when task faulted
    pub caps: CapBits,           // u64 capability bitmask
    pub ttbr0: u64,              // TTBR0_EL1 (ASID<<48 | L1 base)
    pub notify_pending: u64,     // notification bitmask
    pub notify_waiting: bool,    // blocked in wait_notify?
}
```

### Timer hiện tại (`src/timer.rs`)

```
CNTP_EL0: Physical timer, EL0 accessible
INTID 30 (PPI), 10ms interval
Frequency: 62.5 MHz (QEMU virt default)

tick_handler(frame):
  TICK_COUNT += 1
  timer::rearm()
  sched::schedule(frame)

Không có: epoch counter, budget tracking, watchdog scan
```

### IPC hiện tại (`src/ipc.rs`)

```
4 endpoints, multi-sender FIFO queue (max 4 waiters)
Blocking: task gọi SEND khi receiver chưa sẵn sàng → Blocked
          task gọi RECV khi sender chưa gửi → Blocked

Không có: priority inheritance khi blocking
Rủi ro: Task priority 0 (thấp) giữ EP receiver slot → Task priority 7 (cao)
         gọi SEND → blocked → không chạy → deadline miss
```

### Capability bits còn trống

```
Bit 0–16:  ĐÃ DÙNG (17 bits)
Bit 17–63: TRỐNG (47 bits)
Phase K cần: ~2 bits (CAP_HEARTBEAT, có thể CAP_SET_PRIORITY)
```

---

## Thiết kế Phase K

### K1 — Priority Scheduler

#### Khái niệm

Thay thế round-robin bằng **fixed-priority preemptive scheduler**. Mỗi task có priority (0 = thấp nhất, 7 = cao nhất). Scheduler luôn chọn task Ready có priority cao nhất. Nếu cùng priority → round-robin trong nhóm đó.

Hình ảnh: Round-robin giống **xếp hàng mua kem** — ai đến trước mua trước, bất kể đói hay no. Priority scheduler giống **phòng cấp cứu bệnh viện** — bệnh nhân nặng nhất được khám trước, dù đến sau.

#### Thiết kế dữ liệu

**Thêm field vào TCB** (không thay đổi TrapFrame 288 bytes):

```rust
pub struct Tcb {
    // ... existing fields giữ nguyên ...
    pub priority: u8,            // MỚI: 0 (thấp nhất) – 7 (cao nhất)
    pub base_priority: u8,       // MỚI: priority gốc (trước inheritance)
}
```

**Thay đổi `schedule()` algorithm:**

```
schedule(frame):
  1. Save context task hiện tại
  2. Mark old = Ready (nếu Running)
  3. Auto-restart scan (giữ nguyên)
  4. --- MỚI: Priority-based selection ---
     best_prio = -1
     best_idx = NONE
     scan_start = (old + 1) % NUM_TASKS   // round-robin tiebreaker
     for offset in 0..NUM_TASKS:
       idx = (scan_start + offset) % NUM_TASKS
       if TCBS[idx].state == Ready && TCBS[idx].priority > best_prio:
         best_prio = TCBS[idx].priority
         best_idx = idx
  5. Nếu best_idx == NONE → force idle
  6. Switch context + TTBR0
```

**Gán priority mặc định trong `kernel_main()`:**

```
Task 0 (UART driver):  priority = 6  (driver cần responsive)
Task 1 (client):       priority = 4  (application)
Task 2 (idle):         priority = 0  (thấp nhất, luôn)
```

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/sched.rs` | Sửa | Thêm `priority: u8`, `base_priority: u8` vào `Tcb`. Đổi thuật toán `schedule()` từ round-robin sang priority-based. Cập nhật `EMPTY_TCB`. Thêm `set_task_priority()` |
| `src/main.rs` | Sửa | Gán `priority` cho mỗi task trong `kernel_main()` |
| `tests/host_tests.rs` | Sửa | Cập nhật tests scheduler: thêm tests priority selection, cùng priority round-robin |

#### Checkpoint K1

UART output:
```
[AegisOS] scheduler ready (3 tasks, priority-based, EL0)
```

---

### K2 — Time Budget Enforcement

#### Khái niệm

Mỗi task có **time budget** = số ticks tối đa được chạy trong mỗi **epoch**. Khi task hết budget → bị đánh dấu `BudgetExhausted` → không được schedule cho đến khi epoch mới bắt đầu. Epoch = N ticks (ví dụ: 100 ticks = 1 giây).

Hình ảnh: Budget giống **tiền tiêu vặt hàng tuần**. Mỗi tuần (epoch) em được 100.000đ. Tiêu hết thì phải đợi tuần sau. Task "nặng" tiêu hết budget → không thể chiếm CPU của task khác.

#### Thiết kế dữ liệu

**Thêm field vào TCB:**

```rust
pub struct Tcb {
    // ... existing + K1 fields ...
    pub time_budget: u64,        // MỚI: max ticks per epoch (0 = unlimited)
    pub ticks_used: u64,         // MỚI: ticks đã dùng trong epoch hiện tại
}
```

**Thêm epoch tracking trong timer module:**

```rust
// Trong src/timer.rs hoặc src/sched.rs
pub static mut EPOCH_TICKS: u64 = 0;
pub const EPOCH_LENGTH: u64 = 100;  // 100 ticks = 1 giây
```

**Logic trong tick_handler:**

```
tick_handler(frame):
  TICK_COUNT += 1
  EPOCH_TICKS += 1

  // Kiểm tra budget task đang chạy
  TCBS[CURRENT].ticks_used += 1
  if TCBS[CURRENT].time_budget > 0
     && TCBS[CURRENT].ticks_used >= TCBS[CURRENT].time_budget:
    // Budget hết → không schedule task này nữa trong epoch
    TCBS[CURRENT].state = Ready  // vẫn Ready nhưng budget exhausted
    // (schedule() sẽ skip nó)

  // Epoch reset
  if EPOCH_TICKS >= EPOCH_LENGTH:
    EPOCH_TICKS = 0
    for task in TCBS:
      task.ticks_used = 0   // reset budget

  timer::rearm()
  sched::schedule(frame)
```

**Schedule() thêm budget check:**

```
// Trong priority selection:
if TCBS[idx].state == Ready
   && TCBS[idx].priority > best_prio
   && (TCBS[idx].time_budget == 0
       || TCBS[idx].ticks_used < TCBS[idx].time_budget):
  // Task này eligible
```

**Gán budget mặc định:**

```
Task 0 (UART driver):  budget = 0   (unlimited — driver phải responsive)
Task 1 (client):       budget = 50  (50/100 ticks = 50% CPU max)
Task 2 (idle):         budget = 0   (unlimited — chỉ chạy khi không ai khác)
```

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/sched.rs` | Sửa | Thêm `time_budget: u64`, `ticks_used: u64` vào `Tcb`. Thêm budget check vào `schedule()`. Thêm `EPOCH_TICKS`, `EPOCH_LENGTH`. Thêm epoch reset logic |
| `src/timer.rs` | Sửa | Thêm budget tracking vào `tick_handler()`: `TCBS[CURRENT].ticks_used += 1`. Epoch reset khi `EPOCH_TICKS >= EPOCH_LENGTH` |
| `src/main.rs` | Sửa | Gán `time_budget` cho mỗi task |
| `tests/host_tests.rs` | Sửa | Tests budget exhaustion, epoch reset, unlimited budget |

#### Checkpoint K2

UART output:
```
[AegisOS] time budget enforcement enabled (epoch=100 ticks)
```

---

### K3 — Watchdog Heartbeat

#### Khái niệm

Task khai báo **heartbeat interval** (số ticks giữa hai lần heartbeat). Mỗi khi task gọi `SYS_HEARTBEAT` (syscall #12), kernel ghi lại timestamp. Trong `tick_handler()`, kernel scan tất cả task: nếu task có heartbeat_interval > 0 và `now - last_heartbeat > heartbeat_interval` → task bị coi là "treo" → fault + restart.

Hình ảnh: Watchdog giống **bảo vệ đêm** đi tuần. Mỗi phòng (task) phải bật đèn (heartbeat) mỗi N phút. Nếu bảo vệ đi qua mà đèn tắt quá lâu → gõ cửa báo động (fault + restart).

#### Syscall mới

| # | Tên | x7 | x6 | x0 | Mô tả |
|---|---|---|---|---|---|
| 12 | `SYS_HEARTBEAT` | 12 | — | interval (ticks) | Khai báo hoặc cập nhật heartbeat. interval=0 để tắt watchdog cho task này. Mỗi lần gọi cũng reset timer |

#### Capability mới

| Bit | Tên | Mô tả |
|---|---|---|
| 17 | `CAP_HEARTBEAT` | Quyền sử dụng SYS_HEARTBEAT |

#### Thiết kế dữ liệu

**Thêm field vào TCB:**

```rust
pub struct Tcb {
    // ... existing + K1 + K2 fields ...
    pub heartbeat_interval: u64,  // MỚI: max ticks giữa 2 heartbeat (0 = disabled)
    pub last_heartbeat: u64,      // MỚI: TICK_COUNT lần cuối heartbeat
}
```

**Logic trong tick_handler (scan mỗi WATCHDOG_SCAN_PERIOD ticks):**

```
// Scan mỗi 10 ticks (100ms) để giảm overhead
const WATCHDOG_SCAN_PERIOD: u64 = 10;

if TICK_COUNT % WATCHDOG_SCAN_PERIOD == 0:
  for i in 0..NUM_TASKS:
    if TCBS[i].heartbeat_interval > 0
       && TCBS[i].state != Faulted
       && TCBS[i].state != Inactive
       && (TICK_COUNT - TCBS[i].last_heartbeat) > TCBS[i].heartbeat_interval:
      uart_print("[AegisOS] WATCHDOG: task X missed heartbeat\n")
      // Fault task — schedule() sẽ auto-restart sau RESTART_DELAY_TICKS
      TCBS[i].state = Faulted
      TCBS[i].fault_tick = TICK_COUNT
      cleanup_task(i)  // IPC, grant, IRQ cleanup
```

**SYS_HEARTBEAT handler (trong exception.rs handle_svc):**

```
case 12:  // SYS_HEARTBEAT
  check CAP_HEARTBEAT
  interval = frame.x[0]
  TCBS[current].heartbeat_interval = interval
  TCBS[current].last_heartbeat = TICK_COUNT
```

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/sched.rs` | Sửa | Thêm `heartbeat_interval: u64`, `last_heartbeat: u64` vào `Tcb`. Cập nhật `EMPTY_TCB`. Reset heartbeat fields trong `restart_task()` |
| `src/timer.rs` | Sửa | Thêm watchdog scan vào `tick_handler()`. Mỗi `WATCHDOG_SCAN_PERIOD` ticks, scan tất cả task |
| `src/exception.rs` | Sửa | Thêm case `12 => handle_heartbeat(frame)` trong `handle_svc` |
| `src/cap.rs` | Sửa | Thêm `CAP_HEARTBEAT = 1 << 17`. Cập nhật `CAP_ALL`, `cap_for_syscall()`, `cap_name()` |
| `src/main.rs` | Sửa | Thêm syscall wrapper `syscall_heartbeat()`. Gán `CAP_HEARTBEAT` cho task 0 và 1. Demo heartbeat trong task entries. Gán `heartbeat_interval` mặc định |
| `tests/host_tests.rs` | Sửa | Tests heartbeat set/reset, watchdog trigger, disabled watchdog, cap check |

#### Checkpoint K3

UART output:
```
[AegisOS] watchdog heartbeat enabled
```

---

### K4 — Priority Inheritance (Đơn giản)

#### Khái niệm

Khi task priority cao bị **blocked trên IPC** (SEND/RECV/CALL) do task priority thấp chưa sẵn sàng → kernel **tạm nâng priority** task thấp lên bằng task cao. Khi task thấp hoàn thành IPC → priority trở về `base_priority`.

Đây là giải pháp đơn giản cho **priority inversion** — hiện tượng Mars Pathfinder (1997) nổi tiếng.

Hình ảnh: Bạn nhỏ lớp 1 (priority thấp) đang dùng phòng lab. Thầy hiệu trưởng (priority cao) cần phòng. Thay vì để thầy đợi, trường cho bạn lớp 1 **ưu tiên dọn phòng xong** (tạm nâng priority) → thầy vào sớm hơn.

#### Thiết kế

**Khi task X (priority cao) bị block do IPC đến task Y (priority thấp):**

```
// Trong ipc.rs, khi task X gọi SEND/CALL và bị Blocked:
if TCBS[X].priority > TCBS[Y].priority:
  TCBS[Y].priority = TCBS[X].priority   // tạm nâng Y
```

**Khi task Y hoàn thành IPC (RECV completes, SEND unblocks):**

```
// Khôi phục priority gốc
TCBS[Y].priority = TCBS[Y].base_priority
```

**Khi task Y bị fault:**

```
// Trong fault_current_task():
TCBS[Y].priority = TCBS[Y].base_priority  // đảm bảo khôi phục
```

#### Ràng buộc K4

- **Single-level inheritance only** — không hỗ trợ chuỗi A → B → C. Đủ cho 3 task.
- **Chỉ áp dụng cho IPC blocking** — không áp dụng cho WAIT_NOTIFY (notification là async).
- `base_priority` không bao giờ thay đổi sau khi gán.

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/ipc.rs` | Sửa | Khi block sender/receiver: check priority, nâng nếu cần. Khi unblock: khôi phục `base_priority` |
| `src/sched.rs` | Sửa | Trong `fault_current_task()`: khôi phục `priority = base_priority` trước cleanup |
| `tests/host_tests.rs` | Sửa | Tests priority inheritance: task thấp nâng khi task cao block, khôi phục sau IPC, khôi phục sau fault |

#### Checkpoint K4

Không có UART checkpoint riêng — K4 là logic nội bộ. Xác nhận qua host unit tests.

---

### K5 — Tests & Boot Checkpoints

#### Host unit tests mới (ước lượng: ~15 tests)

| # | Test case | Sub-phase | Mô tả |
|---|---|---|---|
| 1 | `test_priority_highest_selected` | K1 | Task priority 7 luôn được chọn trước priority 4 |
| 2 | `test_priority_same_roundrobin` | K1 | Cùng priority → round-robin giữa chúng |
| 3 | `test_priority_skip_faulted` | K1 | Task priority cao nhưng Faulted → bỏ qua |
| 4 | `test_priority_skip_blocked` | K1 | Task priority cao nhưng Blocked → bỏ qua |
| 5 | `test_priority_idle_lowest` | K1 | Idle (priority 0) chỉ chạy khi không ai Ready |
| 6 | `test_budget_exhausted_skip` | K2 | Task hết budget → không schedule trong epoch |
| 7 | `test_budget_unlimited` | K2 | Budget = 0 → không giới hạn |
| 8 | `test_budget_epoch_reset` | K2 | Epoch mới → ticks_used reset về 0 |
| 9 | `test_budget_partial_use` | K2 | Task dùng 30/50 budget → vẫn eligible |
| 10 | `test_heartbeat_set` | K3 | SYS_HEARTBEAT ghi interval + last_heartbeat |
| 11 | `test_heartbeat_miss_faults` | K3 | Task không heartbeat quá interval → Faulted |
| 12 | `test_heartbeat_disabled` | K3 | interval=0 → watchdog không scan task này |
| 13 | `test_heartbeat_reset_on_restart` | K3 | Task restart → heartbeat fields reset |
| 14 | `test_cap_heartbeat` | K3 | `cap_for_syscall(12, _) == CAP_HEARTBEAT` |
| 15 | `test_priority_inheritance_basic` | K4 | Task thấp nâng priority khi task cao block trên IPC |
| 16 | `test_priority_inheritance_restore` | K4 | Priority khôi phục sau IPC hoàn thành |
| 17 | `test_priority_inheritance_fault_restore` | K4 | Priority khôi phục khi task bị fault |

#### QEMU boot checkpoints mới

| # | Checkpoint UART output |
|---|---|
| 16 | `[AegisOS] scheduler ready (3 tasks, priority-based, EL0)` |
| 17 | `[AegisOS] time budget enforcement enabled` |
| 18 | `[AegisOS] watchdog heartbeat enabled` |

**Lưu ý:** Checkpoint 5 hiện tại (`[AegisOS] scheduler ready`) sẽ thay đổi nội dung thành checkpoint 16. Cần cập nhật `tests/qemu_boot_test.sh`.

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|---|---|---|
| 1 | TrapFrame = 288 bytes | ABI-locked | Không thay đổi — K thêm field vào TCB, KHÔNG vào TrapFrame |
| 2 | No heap — budget/heartbeat data phải static | Bất biến AegisOS | Tất cả field mới trong `Tcb` struct (static array) |
| 3 | No FP/SIMD | CPACR_EL1.FPEN=0 | Budget tính bằng integer ticks, không cần float |
| 4 | NUM_TASKS = 3 cố định | Static allocation | Priority scheme đơn giản, không cần ready queue phức tạp |
| 5 | Single-core | Không cần lock | Mọi scheduler logic chạy trong interrupt handler (IRQ disabled) |
| 6 | Timer tick = 10ms | Resolution | Budget và heartbeat tính bằng bội số 10ms. Đủ cho most safety-critical (phanh ABS cần ~5ms → 1 tick gần đủ) |
| 7 | Watchdog scan overhead | Performance | Scan 3 tasks mỗi 10 ticks = O(3) mỗi 100ms — negligible |
| 8 | Priority range 0–7 | 3 bits đủ cho 3 tasks | Mở rộng đến 255 nếu tăng NUM_TASKS tương lai |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|---|
| 1 | Priority starvation — task thấp không bao giờ chạy | 🟡 Trung bình | 🟡 Task thấp "đói" | Time budget giới hạn task cao. Idle task (priority 0) luôn chạy khi không ai cần CPU |
| 2 | Priority inheritance deadlock — 2 task nâng lẫn nhau | 🟢 Thấp | 🔴 Cả 2 blocked vĩnh viễn | Single-level inheritance only + 3 tasks → cycle không thể xảy ra (A blocks on B, B blocks on C — C không block trên A vì C là idle) |
| 3 | Watchdog false positive — task hợp lệ bị fault do busy | 🟡 Trung bình | 🟡 Task restart không cần thiết | Heartbeat interval đủ dài (>= 2× expected loop time). Task không đăng ký heartbeat thì không bị scan |
| 4 | Epoch reset race — task đang chạy bị reset budget giữa chừng | 🟢 Thấp | 🟢 Nhỏ | Reset xảy ra trong tick_handler → IRQ disabled → atomic. Task chỉ mất 1 tick tối đa |
| 5 | Scheduler thay đổi ảnh hưởng QEMU boot output | 🟡 Trung bình | 🟡 Checkpoint fail | Cập nhật qemu_boot_test.sh cùng lúc. Task order có thể thay đổi → checkpoint string cần flexible |
| 6 | TCB size tăng ảnh hưởng cache | 🟢 Thấp | 🟢 Negligible | Thêm ~34 bytes (2×u8 + 4×u64). TCB vẫn < 400 bytes. 3 TCBs < 1.2KB |

---

## Backward Compatibility

### Thay đổi breaking

| Thay đổi | Ảnh hưởng | Giải pháp |
|---|---|---|
| Scheduler không còn round-robin thuần | Task order có thể khác | Gán priority phù hợp. Round-robin vẫn hoạt động trong cùng priority group |
| Checkpoint `[AegisOS] scheduler ready` thay đổi nội dung | QEMU test fail | Cập nhật `qemu_boot_test.sh` |
| Task có thể bị "starved" nếu priority thấp | Behavior change | Time budget đảm bảo fairness tối thiểu |

### Không thay đổi (backward compatible)

- Syscall ABI 0–11 giữ nguyên
- TrapFrame 288 bytes giữ nguyên
- Capability bits 0–16 giữ nguyên
- IPC, notification, grant, IRQ routing giữ nguyên
- Memory layout (linker.ld) KHÔNG thay đổi
- Page table layout KHÔNG thay đổi

---

## Test Plan

### Host unit tests mới (ước lượng: ~17 tests)

Xem chi tiết tại [K5 — Tests](#k5--tests--boot-checkpoints).

### QEMU boot checkpoints

Sau Phase K: **18 checkpoints** (15 cũ + 3 mới, với checkpoint 5 thay đổi nội dung).

---

## Thứ tự triển khai

| Bước | Sub-phase | Phụ thuộc | Checkpoint xác nhận | Risk |
|---|---|---|---|---|
| 1 | **K1: Priority Scheduler** | Không — thay đổi nội bộ `schedule()` | QEMU boot + `[AegisOS] scheduler ready (3 tasks, priority-based, EL0)` + 5 host tests | 🟢 Thấp |
| 2 | **K2: Time Budget** | K1 (cần priority selection trước) | + `[AegisOS] time budget enforcement enabled` + 4 host tests | 🟢 Thấp |
| 3 | **K3: Watchdog Heartbeat** | K1 (scan trong tick_handler) | + `[AegisOS] watchdog heartbeat enabled` + 4 host tests + syscall #12 | 🟡 Trung bình (syscall mới + cap bit mới) |
| 4 | **K4: Priority Inheritance** | K1 + IPC blocking | + 3 host tests. Không có UART checkpoint riêng | 🟡 Trung bình (IPC logic change) |
| 5 | **K5: Tests** | K1-K4 | `cargo test` ~152 tests pass (135+17). QEMU 18 checkpoints pass | 🟢 Thấp |

---

## Syscall ABI sau Phase K

| # | Tên | x7 | x0 | Mô tả | Mới? |
|---|---|---|---|---|---|
| 0 | SYS_YIELD | 0 | — | Nhường CPU | |
| 1 | SYS_SEND | 1 | msg[0] | Gửi IPC | |
| 2 | SYS_RECV | 2 | msg[0] (out) | Nhận IPC | |
| 3 | SYS_CALL | 3 | msg[0..3] | Send+Recv atomic | |
| 4 | SYS_WRITE | 4 | ptr | Ghi UART | |
| 5 | SYS_NOTIFY | 5 | bitmask | Gửi notification | |
| 6 | SYS_WAIT_NOTIFY | 6 | pending (out) | Chờ notification | |
| 7 | SYS_GRANT_CREATE | 7 | grant_id | Tạo grant | |
| 8 | SYS_GRANT_REVOKE | 8 | grant_id | Thu hồi grant | |
| 9 | SYS_IRQ_BIND | 9 | intid | Đăng ký IRQ | |
| 10 | SYS_IRQ_ACK | 10 | intid | ACK IRQ | |
| 11 | SYS_DEVICE_MAP | 11 | device_id | Map MMIO | |
| **12** | **SYS_HEARTBEAT** | **12** | **interval** | **Khai báo heartbeat** | **✅ MỚI** |

---

## Capability Bitmap sau Phase K

```
Bit  0–16:  Giữ nguyên từ Phase J
Bit 17:     CAP_HEARTBEAT       ← MỚI (K3)
Bit 18–63:  Reserved (46 bits còn trống)
```

Tổng: **18/64 bits đã dùng**.

---

## Tổng kết chi phí

| Metric | Giá trị |
|---|---|
| File mới | 0 |
| File sửa | 6 (`sched.rs`, `timer.rs`, `exception.rs`, `cap.rs`, `main.rs`, `ipc.rs`) |
| Dòng code thêm (ước lượng) | ~200 kernel + ~100 test |
| Bộ nhớ thêm | ~102 bytes (34 bytes × 3 TCBs) + ~16 bytes static (epoch counters) |
| Tests mới | ~17 |
| Tổng tests | ~152 (135 + 17) |
| Syscalls mới | 1 (SYS_HEARTBEAT) |
| Tổng syscalls | 13 (0–12) |
| Capability bits mới | 1 (bit 17: CAP_HEARTBEAT) |
| Tổng capability bits | 18/64 |
| QEMU checkpoints mới | 3 |
| Tổng checkpoints | 18 |

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|---|---|---|
| **DO-178C** §6.3.3 | Partitioning — Temporal | Priority scheduler + time budget = **temporal partitioning**. Task safety-critical (DAL A) chạy trước task non-critical (DAL E). Time budget ngăn một task chiếm CPU vô hạn |
| **DO-178C** §6.4.4.2 | Scheduling determinism | Fixed-priority preemptive scheduling cho phép phân tích WCET. Round-robin KHÔNG đáp ứng được — priority scheduling là yêu cầu tối thiểu |
| **ISO 26262** Part 6 §7.4.5 | Freedom from interference — Temporal | Time budget enforcement = temporal isolation giữa ASIL D (phanh) và QM (infotainment). Priority inheritance ngăn priority inversion (Mars Pathfinder scenario) |
| **ISO 26262** Part 6 §7.4.11 | Monitoring — Alive supervision | Watchdog heartbeat = alive supervision mechanism. Task không heartbeat đúng hạn → fault + restart. Trực tiếp map vào ISO 26262 watchdog requirement |
| **IEC 62304** §5.3.5 | Software Architecture — Timing | Priority scheduler cho phép khai báo timing constraint cho mỗi software unit. Heartbeat = liveness monitor cho Class C software |
| **IEC 62304** §5.5.3 | Software Unit Verification | 17 unit tests cover priority logic, budget exhaustion, heartbeat, inheritance |

---

## Sơ đồ tổng quan Scheduler sau Phase K

```
                    Timer IRQ (mỗi 10ms)
                           │
                           ▼
              ┌─────────────────────────┐
              │     tick_handler()      │
              │                         │
              │  1. TICK_COUNT++         │
              │  2. Budget tracking:    │
              │     ticks_used++        │
              │  3. Epoch reset check   │
              │  4. Watchdog scan       │
              │     (mỗi 10 ticks)      │
              │  5. schedule(frame)     │
              └───────────┬─────────────┘
                          │
                          ▼
              ┌─────────────────────────┐
              │  schedule() — Priority  │
              │                         │
              │  Scan tất cả tasks:     │
              │  ┌──────────────────┐   │
              │  │ Task 0 (prio=6)  │   │ ← UART driver
              │  │ budget=unlimited │   │
              │  │ heartbeat=50     │   │
              │  ├──────────────────┤   │
              │  │ Task 1 (prio=4)  │   │ ← Client app
              │  │ budget=50 ticks  │   │
              │  │ heartbeat=100    │   │
              │  ├──────────────────┤   │
              │  │ Task 2 (prio=0)  │   │ ← Idle (WFI)
              │  │ budget=unlimited │   │
              │  │ heartbeat=0 (off)│   │
              │  └──────────────────┘   │
              │                         │
              │  Chọn: Ready + highest  │
              │  priority + budget OK   │
              │                         │
              │  Tiebreak: round-robin  │
              └───────────┬─────────────┘
                          │
                          ▼
              ┌─────────────────────────┐
              │  Context switch:        │
              │  Save old TrapFrame     │
              │  Load new TrapFrame     │
              │  Switch TTBR0           │
              │  eret → EL0             │
              └─────────────────────────┘
```

---

## So sánh trước/sau Phase K

| Khía cạnh | Trước (Phase J) | Sau (Phase K) |
|---|---|---|
| Scheduler | Round-robin | Fixed-priority preemptive |
| Task priority | Không có | 0–7 (8 levels) |
| CPU budget | Không giới hạn | Time budget per epoch |
| Starvation protection | Không | Budget limits + idle guarantee |
| Liveness monitoring | Không | Watchdog heartbeat |
| Priority inversion | Không xử lý | Single-level inheritance |
| Timing determinism | Không | Deterministic priority + budget |
| Safety certification gap | Lớn (temporal) | Đáp ứng cơ bản DO-178C/ISO 26262/IEC 62304 |

---

## Bước tiếp theo đề xuất

1. [ ] **Review kế hoạch** → phản hồi/chỉnh sửa (đặc biệt K4 priority inheritance scope)
2. [ ] **Triển khai K1** (Priority Scheduler) — rủi ro thấp, thay đổi nội bộ `schedule()` (handoff → Aegis-Agent)
3. [ ] **Triển khai K2** (Time Budget) — cần test epoch reset kỹ (handoff → Aegis-Agent)
4. [ ] **Triển khai K3** (Watchdog Heartbeat) — syscall mới + cap bit mới (handoff → Aegis-Agent)
5. [ ] **Triển khai K4** (Priority Inheritance) — sửa IPC blocking logic (handoff → Aegis-Agent)
6. [ ] **Triển khai K5** (Tests) — 17 host tests + 3 QEMU checkpoints (handoff → Aegis-Agent)
7. [ ] **Viết blog #11** — giải thích priority scheduling và watchdog cho học sinh lớp 5 (handoff → Aegis-StoryTeller)
8. [ ] **Chạy test suite đầy đủ** — ~152 host tests + 18 QEMU checkpoints (handoff → Aegis-Tester)
9. [ ] **Cập nhật `copilot-instructions.md`** — reflect 13 syscalls, 18 capability bits, priority scheduler
