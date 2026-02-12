# Kế hoạch Phase O — Multi-ELF & User Ecosystem

> **Trạng thái: ✅ APPROVED** — Đồng thuận đạt 2026-02-12 ([final_consensus](../discussions/phase-o-multi-elf-user-ecosystem/final_consensus_2026-02-12.md)). Kích hoạt 5 task slots trống (2–6) bằng cách load nhiều ELF binary, tạo thư viện syscall dùng chung (`libsyscall`), thêm `SYS_EXIT` cho vòng đời task hoàn chỉnh, mở rộng ELF load region, build system docs, và tiếp tục formal verification cho IPC data integrity. Phase này biến AegisOS từ "demo 2 task + 1 ELF" thành "hệ thống 6 task thật với user ecosystem".

---

## Tại sao Phase O?

### Lỗ hổng/Hạn chế hiện tại: "5/8 task slots trống — hệ thống mạnh nhưng chưa dùng hết sức"

Phase N đã mở rộng AegisOS từ 3 lên 8 tasks, wrap 100% globals trong KernelCell, và chứng minh 6 tính chất bằng Kani. Nhưng thực tế chỉ **3/8 tasks** đang hoạt động (uart_driver, client, idle/ELF demo). 5 slots (tasks 2–6) vẫn `Inactive` — giống trường học 8 phòng nhưng chỉ mở 3 lớp.

**Ví dụ thực tế:** Xe tự lái cần 8 tasks: camera, LiDAR, bản đồ, tay lái, phanh, telemetry, cập nhật, sức khỏe (Blog #14). AegisOS hiện chỉ minh họa được: driver + client + demo. Không thể mô phỏng kịch bản thật nếu không kích hoạt các task slots còn lại.

Đồng thời, user binary `user/hello` là monolithic — duplicate syscall wrappers, fixed load address, không có cơ chế tái sử dụng code. Mỗi binary mới phải copy-paste ~50 dòng syscall asm. Đây là anti-pattern cho safety-critical: code duplicated = bug duplicated.

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|---|---|
| 1 | 5/8 task slots `Inactive` (tasks 2–6) | Không thể mô phỏng hệ thống thật; Phase N scale vô nghĩa nếu không dùng |
| 2 | Chỉ 1 ELF load region (12 KiB tại `0x4010_0000`) | Không thể load nhiều binary vào nhiều task |
| 3 | User binary duplicate syscall wrappers (~50 dòng asm/binary) | Code lặp → bug lặp, không đạt DO-178C "single source of truth" |
| 4 | Không có `SYS_EXIT` — task chỉ fault hoặc loop | Không thể test vòng đời task hoàn chỉnh; DO-178C §5.6 yêu cầu graceful shutdown |
| 5 | Kani chưa verify IPC state machine | Deadlock risk chưa chứng minh — DO-333 model checking incomplete |
| 6 | `include_bytes!` chỉ embed 1 binary tĩnh | Không flexible; muốn nhiều binary phải sửa kernel source |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|---|---|---|
| O1: Multi-ELF Loading | Mở rộng ELF load region + per-task load addresses | #1, #2, #6 |
| O2: `libsyscall` User Library | Crate dùng chung cho user binaries | #3 |
| O3: SYS_EXIT Task Lifecycle | Syscall #13 cho graceful task exit | #4 |
| O4: Kani IPC Proofs | Formal verification queue overflow + message integrity + cleanup | #5 |
| O5: Build System Docs | README build order + `scripts/build-all.sh` | #1, #6 |

---

## Phân tích hiện trạng

### ELF Loading hiện tại

```
Luồng hiện tại:
1. user/hello → cargo build → ELF binary tại user/hello/target/.../hello
2. kernel main.rs: static USER_ELF: &[u8] = include_bytes!("../user/hello/target/.../hello");
3. parse_elf64(USER_ELF) → ElfInfo { entry, segments }
4. load_elf_segments() → copy vào __elf_load_start (0x4010_0000), 12 KiB max
5. Cache maintenance (dc cvau + ic iallu)
6. Override TCBS[IDLE_TASK_ID].entry_point = elf_entry
7. Set page attributes per segment (USER_CODE_PAGE / USER_DATA_PAGE)

Vấn đề:
- Chỉ 1 vùng load duy nhất (__elf_load_start → __elf_load_end)
- ELF override vào IDLE_TASK_ID (task 7) — hardcoded
- Không có cơ chế load binary thứ 2 vào task 2–6
```

### User binary hiện tại (`user/hello`)

```
user/hello/
├── Cargo.toml        — standalone #![no_std] crate
├── linker.ld         — ENTRY(_start), links at 0x4010_0000
├── aarch64-user.json — custom target spec
└── src/main.rs       — 62 lines: syscall_write(), syscall_yield(), _start(), panic_handler()

Syscall wrappers duplicated:
- syscall_write(): 10 lines inline asm
- syscall_yield(): 8 lines inline asm
→ Mỗi binary mới phải copy 18+ lines
→ Nếu syscall ABI thay đổi → sửa ở N chỗ
```

### Linker memory layout (liên quan)

```
.elf_load (NOLOAD):     0x4010_0000, 3 pages (12 KiB)
.grant_pages (NOLOAD):  trước .elf_load, 2 pages (8 KiB)

Khoảng trống sau __elf_load_end → __stack_guard:
  __elf_load_end = 0x4010_0000 + 0x3000 = 0x4010_3000
  __stack_guard  = aligned sau __elf_load_end

Có thể mở rộng .elf_load mà không ảnh hưởng sections trước.
```

### Task metadata hiện tại

```rust
// kernel_main() → TASK_META const array
const TASK_META: [TaskMetadata; 8] = [
    // Task 0: uart_driver — priority 6, caps đầy đủ
    // Task 1: client — priority 4, budget 50 ticks
    // Task 2–6: INACTIVE (caps=0, priority=0)
    // Task 7: idle/ELF demo — priority 5, budget 2
];

// sched::init() nhận entries: &[u64; NUM_TASKS]
// Task 2–6 entries = 0 → state = Inactive
```

---

## Thiết kế Phase O

### O1 — Multi-ELF Loading: Kích hoạt Tasks 2–6

#### Khái niệm

Hiện AegisOS chỉ load 1 ELF binary vào 1 vùng nhớ cố định. Phase O mở rộng thành **nhiều vùng load**, mỗi task có địa chỉ load riêng. Giống như trường học có 8 phòng — mỗi phòng có bàn ghế riêng, không dùng chung.

#### O1a — Mở rộng ELF Load Region

**Hiện tại:** 1 region × 12 KiB (3 pages) tại `0x4010_0000`

**Đề xuất:** 6 regions × 16 KiB (4 pages) cho tasks 2–7. Tổng: 96 KiB.

| Task | Load address | Size | Dùng cho |
|---|---|---|---|
| 2 | `0x4010_0000` | 16 KiB | ELF user task A |
| 3 | `0x4010_4000` | 16 KiB | ELF user task B |
| 4 | `0x4010_8000` | 16 KiB | ELF user task C |
| 5 | `0x4010_C000` | 16 KiB | ELF user task D |
| 6 | `0x4011_0000` | 16 KiB | ELF user task E |
| 7 | `0x4011_4000` | 16 KiB | ⛔ Reserved — IDLE task (`idle_entry`, không load ELF) |

**Lý do 16 KiB/task:**
- 4 pages = đủ cho user binary có .text + .rodata + .data + .bss riêng
- Tổng 96 KiB = 0.07% RAM — không đáng kể
- Tương lai có thể tăng nếu cần

**Thay đổi linker.ld:**
```
.elf_load (NOLOAD) : {
    . += 6 * 16 * 1024;   /* 6 tasks × 16 KiB = 96 KiB */
}
```

**Constants mới (`platform/qemu_virt.rs` hoặc `elf.rs`):**
```
pub const ELF_LOAD_BASE: u64 = 0x4010_0000;
pub const ELF_LOAD_SIZE_PER_TASK: usize = 16 * 1024;  // 16 KiB
pub const MAX_ELF_TASKS: usize = 6;  // tasks 2–7

pub const fn elf_load_addr(slot: usize) -> u64 {
    ELF_LOAD_BASE + (slot as u64) * ELF_LOAD_SIZE_PER_TASK as u64
}
```

**Compile-time safety (`const_assert!`):**

Mỗi `include_bytes!` binary phải kèm `const_assert!` kiểm tra kích thước ≤ 16 KiB:
```rust
const_assert!(USER_HELLO.len() <= ELF_LOAD_SIZE_PER_TASK);
const_assert!(USER_SENSOR.len() <= ELF_LOAD_SIZE_PER_TASK);
const_assert!(USER_LOGGER.len() <= ELF_LOAD_SIZE_PER_TASK);
```

> **Migration trigger:** Khi vượt 5 ELF binaries → chuyển sang template-based linker.ld generation. Document trong README.

**Manual linker.ld per-binary:**

Mỗi user crate có `linker.ld` riêng với slot-specific load address. 3 files × ~25 dòng = ~75 dòng — chấp nhận duplicate để tránh build script complexity.

#### O1b — Multi-Binary Embedding

**Hiện tại:** `include_bytes!("../user/hello/target/.../hello")` — 1 binary

**Đề xuất:** Embed nhiều binary + load function tái sử dụng

```rust
// Mỗi binary là một user crate riêng
static USER_HELLO: &[u8] = include_bytes!("../user/hello/target/.../hello");
static USER_SENSOR: &[u8] = include_bytes!("../user/sensor/target/.../sensor");
static USER_LOGGER: &[u8] = include_bytes!("../user/logger/target/.../logger");
// ...

// Load function tổng quát (thay vì inline trong kernel_main)
fn load_elf_to_task(task_id: usize, elf_data: &[u8]) -> Result<u64, &'static str>;
```

**Hàm `load_elf_to_task()` sẽ:**
1. Tính load address từ `elf_load_addr(task_id - 2)` (slot 0–5 → task 2–7)
2. `parse_elf64()` + `load_elf_segments()`
3. Cache maintenance
4. Set page attributes per segment
5. Update `TCBS[task_id].entry_point` + `TCBS[task_id].context.elr_el1`
6. Set task state = `Ready`

#### O1c — Demo User Tasks

Tạo 2–3 user crate mới minh họa các khả năng khác nhau:

| User crate | Task slot | Chức năng | Syscalls dùng |
|---|---|---|---|
| `user/hello` (có sẵn) | 2 | In "L5:ELF", yield | WRITE, YIELD |
| `user/sensor` (mới) | 3 | Mô phỏng sensor: gửi dữ liệu qua IPC | SEND, YIELD, HEARTBEAT |
| `user/logger` (mới) | 4 | Nhận IPC từ sensor, ghi UART | RECV, WRITE, YIELD |

**Task 7 = IDLE thuần:**
- `IDLE_TASK_ID = 7`, entry = `idle_entry()` (`wfi` loop)
- **Không load ELF** vào task 7 — dual-role vi phạm separation of concerns
- Priority: 0, budget: 0 (infinite availability, scheduler fallback)
- `user/hello` ELF di chuyển từ task 7 → task 2

**Lý do 3 binaries:**
- Chứng minh multi-ELF loading hoạt động
- Chứng minh IPC giữa 2 ELF user tasks (sensor → logger)
- `user/hello` giữ nguyên (backward compatible) — load vào task 2 slot

#### File cần thay đổi (O1)

| File | Thao tác | Chi tiết |
|---|---|---|
| `linker.ld` | Sửa | `.elf_load: . += 6 * 16 * 1024` (12 KiB → 96 KiB) |
| `src/platform/qemu_virt.rs` | Sửa | Thêm ELF_LOAD_BASE, ELF_LOAD_SIZE_PER_TASK, elf_load_addr() |
| `src/kernel/elf.rs` | Sửa | Thêm `load_elf_to_task()` reusable function |
| `src/arch/aarch64/mmu.rs` | Sửa | Map 6 ELF regions thay vì 1 (per-task page attributes) |
| `src/main.rs` | Sửa lớn | Multi-binary `include_bytes!` + `const_assert!`, loop load, TASK_META cho tasks 2–6; task 7 = idle only |
| `user/sensor/` | Tạo mới | Sensor demo crate + `linker.ld` (slot 1: `0x4010_4000`) |
| `user/logger/` | Tạo mới | Logger demo crate + `linker.ld` (slot 2: `0x4010_8000`) |
| `user/hello/linker.ld` | Sửa | Load address → slot 0: `0x4010_0000` (task 2) |
| `user/sensor/linker.ld` | Tạo mới | Load address → slot 1: `0x4010_4000` (task 3) |
| `user/logger/linker.ld` | Tạo mới | Load address → slot 2: `0x4010_8000` (task 4) |
| `tests/host_tests.rs` | Sửa | Test cho elf_load_addr(), load_elf_to_task() |

#### Checkpoint O1

```
[AegisOS] task 2 loaded from ELF (entry=0x40100000)
[AegisOS] task 3 loaded from ELF (entry=0x40104000)
[AegisOS] task 4 loaded from ELF (entry=0x40108000)
[AegisOS] 3 ELF user tasks loaded, task 7 = IDLE
```

---

### O2 — `libsyscall`: User Syscall Library

#### Khái niệm

Thay vì mỗi user binary copy-paste syscall wrappers, tạo một **shared library crate** (`user/libsyscall`) chứa tất cả syscall wrappers + types + constants. Giống "sách giáo khoa chung" cho tất cả học sinh — thay vì mỗi em photo bài riêng.

#### Thiết kế

**User workspace:** Tạo `user/Cargo.toml` workspace tách biệt hoàn toàn khỏi kernel workspace, giải quyết target mismatch (kernel `aarch64-aegis.json` vs user `aarch64-user.json`). Shared `user/aarch64-user.json` cho tất cả user crates.

```
user/
├── Cargo.toml          ← NEW: workspace = ["libsyscall", "hello", "sensor", "logger"]
├── aarch64-user.json   ← MOVED: shared custom target (từ user/hello/)
│
├── libsyscall/         ← NEW: shared syscall library
│   ├── Cargo.toml      ← #![no_std], lib crate
│   └── src/lib.rs      ← All 14 syscall wrappers + types (bao gồm SYS_EXIT)
│
├── hello/
│   ├── Cargo.toml      ← depends on libsyscall
│   ├── linker.ld       ← slot 0: 0x4010_0000
│   └── src/main.rs     ← ~15 lines: use libsyscall::*; fn _start() {...}
│
├── sensor/             ← NEW
│   ├── Cargo.toml
│   ├── linker.ld       ← slot 1: 0x4010_4000
│   └── src/main.rs
│
└── logger/             ← NEW
    ├── Cargo.toml
    ├── linker.ld       ← slot 2: 0x4010_8000
    └── src/main.rs
```

**Lợi ích workspace:**
- Unified `Cargo.lock` cho tất cả user crates
- `cargo clippy --workspace` kiểm tra tất cả cùng lúc
- ABI consistency khi `libsyscall` thay đổi — tất cả binary tự rebuild

**`libsyscall` exports:**

```rust
// Syscall numbers
pub const SYS_YIELD: u64 = 0;
pub const SYS_SEND: u64 = 1;
// ... tất cả 13 syscalls

// Syscall wrappers (inline asm, aarch64 only)
pub fn syscall_yield();
pub fn syscall_send(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64);
pub fn syscall_recv(ep_id: u64) -> u64;
pub fn syscall_recv2(ep_id: u64) -> (u64, u64);
pub fn syscall_call(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) -> u64;
pub fn syscall_write(buf: *const u8, len: usize);
pub fn syscall_notify(target: u64, bits: u64);
pub fn syscall_wait_notify() -> u64;
pub fn syscall_grant_create(grant_id: u64, peer: u64);
pub fn syscall_grant_revoke(grant_id: u64);
pub fn syscall_irq_bind(intid: u64, notify_bit: u64) -> u64;
pub fn syscall_irq_ack(intid: u64) -> u64;
pub fn syscall_device_map(device_id: u64) -> u64;
pub fn syscall_heartbeat(interval: u64) -> u64;
pub fn syscall_exit(code: u64);  // NEW — Phase O3

// Convenience macros
pub macro print($msg:expr) { syscall_write($msg.as_ptr(), $msg.len()) }
```

**Lợi ích:**
- **Single source of truth** — sửa ABI 1 chỗ, tất cả binary tự cập nhật
- **DO-178C §5.5 traceability** — 1 module = 1 spec = 1 test set
- Giảm code user binary từ ~62 dòng → ~15 dòng (hello)

#### File cần thay đổi (O2)

| File | Thao tác | Chi tiết |
|---|---|---|
| `user/Cargo.toml` | Tạo mới | Workspace: `members = ["libsyscall", "hello", "sensor", "logger"]` |
| `user/aarch64-user.json` | Di chuyển | Từ `user/hello/aarch64-user.json` → `user/aarch64-user.json` (shared) |
| `user/libsyscall/` | Tạo mới | Cargo.toml + src/lib.rs (14 syscall wrappers) |
| `user/hello/Cargo.toml` | Sửa | Thêm `libsyscall = { path = "../libsyscall" }` |
| `user/hello/src/main.rs` | Sửa | Xóa 18 dòng syscall duplicates, dùng `use libsyscall::*` |
| `user/sensor/` | Tạo mới | Dùng libsyscall |
| `user/logger/` | Tạo mới | Dùng libsyscall |

#### Checkpoint O2

```
user/hello rebuilt with libsyscall → same UART output "L5:ELF "
```

---

### O3 — SYS_EXIT: Task Lifecycle hoàn chỉnh

#### Khái niệm

Hiện tasks chỉ có 2 cách kết thúc: **loop forever** hoặc **fault**. Không có cơ chế "xong việc rồi, tôi muốn dừng" — giống nhà máy không có nút tắt, chỉ có rút phích điện.

`SYS_EXIT` cho phép task tự kết thúc gracefully. Kernel chuyển task sang trạng thái mới `Exited`, giải phóng tài nguyên IPC/grant.

> **⚠️ Quyết định: SYS_EXIT only, NO SYS_KILL.**
> Không implement SYS_KILL, không reserve bit/placeholder. Lý do:
> - KILL cần authority-based design mà Phase O chưa có context
> - KILL = security attack surface (task A kill task B = DoS vector)
> - Safety-critical tasks self-exit hoặc watchdog restart — cover 95% lifecycle
> - Revisit khi có supervisor pattern (future phase)

#### Syscall mới

| # | Tên | x7 | x6 | x0 | Mô tả |
|---|---|---|---|---|---|
| 13 | SYS_EXIT | 13 | — | exit_code | Task tự kết thúc, kernel cleanup |

#### Capability mới

| Bit | Tên | Mô tả |
|---|---|---|
| 18 | `CAP_EXIT` | Quyền gọi SYS_EXIT (mọi task đều nên có) |

#### TaskState mới

```rust
pub enum TaskState {
    Inactive = 0,
    Ready    = 1,
    Running  = 2,
    Blocked  = 3,
    Faulted  = 4,
    Exited   = 5,  // ← NEW: graceful exit, no auto-restart
}
```

**Khác biệt Faulted vs Exited:**
- `Faulted` → auto-restart sau 100 ticks (lỗi bất ngờ, cần hồi phục)
- `Exited` → **không** auto-restart (task chủ động dừng, tôn trọng ý định)

#### Exit handler logic

**Extract `cleanup_task_resources(task_id)` helper** — reuse logic từ `fault_current_task()`:

```
cleanup_task_resources(task_id):
  1. Cleanup IPC: remove task from all endpoint sender queues
  2. Cleanup grants: revoke grants where task is owner or peer
  3. Cleanup IRQ: unbind all IRQ bindings for this task
  4. Cleanup watchdog: disable heartbeat monitoring
  5. Cleanup priority: restore borrowed priority

sys_exit(exit_code):
  1. Log: "[AegisOS] task N exited (code=X)"
  2. cleanup_task_resources(current_task_id)
  3. Set state = Exited (NOT Faulted — no auto-restart)
  4. Watchdog + epoch_reset skip Exited tasks
  5. Schedule away → pick next ready task
```

> `fault_current_task()` cũng được refactor để gọi `cleanup_task_resources()` — DRY principle.

#### File cần thay đổi (O3)

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/sched.rs` | Sửa | Thêm `TaskState::Exited`, skip Exited trong scheduler + epoch_reset + watchdog |
| `src/kernel/sched.rs` | Sửa | Extract `cleanup_task_resources(task_id)` helper, refactor `fault_current_task()` |
| `src/arch/aarch64/exception.rs` | Sửa | Thêm SYS_EXIT dispatch (case 13) → `sys_exit()` |
| `src/kernel/ipc.rs` | Sửa | `cleanup_task()` đã có — verify nó handle Exited |
| `src/kernel/grant.rs` | Sửa | Cleanup grants on exit |
| `src/kernel/irq.rs` | Sửa | Cleanup IRQ bindings on exit |
| `src/kernel/cap.rs` | Sửa | Thêm `CAP_EXIT = 1 << 18`, `cap_for_syscall(13)` |
| `src/exception.rs` | Sửa | Host stub: thêm SYS_EXIT constant |
| `user/libsyscall/src/lib.rs` | Sửa | Thêm `syscall_exit()` wrapper |
| `tests/host_tests.rs` | Sửa | Tests cho exit state, cleanup, scheduler skip, no auto-restart |

#### Checkpoint O3

```
[AegisOS] task 2 exited (code=0)
```
> User task gọi `syscall_exit(0)` → kernel log, cleanup, schedule away. Task không auto-restart.

---

### O4 — Kani IPC Proofs (P0) + elf_load_addr (P1)

#### Khái niệm

Phase N đã chứng minh scheduler + capability bằng Kani. Phase O mở rộng sang **IPC data integrity** — chứng minh queue overflow safety, message integrity, và cleanup completeness.

> **Scope:** 3 IPC proofs (P0 — mandatory) + 1 optional `elf_load_addr` proof (P1 — only if >5h buffer remaining). Deadlock-freedom = PhD-level problem, skip cho Phase O.

Đây là yêu cầu DO-333 FM.A-7: "Coverage of formal proofs" — mở rộng từ cap/elf sang module phức tạp hơn.

**Pure function extraction:** Để Kani verify được, extract pure functions từ IPC module:
- `push_pure()` — thêm task vào SenderQueue
- `pop_pure()` — lấy task từ SenderQueue
- `copy_message_pure()` — copy payload x0–x3
- `cleanup_pure()` — xóa task khỏi tất cả endpoints

#### Proof harnesses đề xuất (3 P0 + 1 P1)

**Proof 1: SenderQueue overflow safety** (P0)
```
Với mọi chuỗi push/pop trên SenderQueue (MAX_WAITERS=4):
- push khi full → return false (không corrupt)
- pop khi empty → return None (không panic)
- count luôn ∈ [0, MAX_WAITERS]
```

**Proof 2: IPC message integrity** (P0)
```
Với mọi send+recv trên endpoint:
- Message payload (x0–x3) truyền chính xác từ sender → receiver
- Không bao giờ deliver message sai endpoint
```

**Proof 3: Cleanup completeness** (P0)
```
Với mọi task_id ∈ [0, NUM_TASKS):
- cleanup_task(task_id) removes task from ALL sender queues
- Sau cleanup: không endpoint nào reference task_id
```

**(P1 — Optional) Proof 4: elf_load_addr no overlap**
```
Với mọi slot i, j ∈ [0, MAX_ELF_TASKS) where i ≠ j:
- [addr(i), addr(i)+SIZE) ∩ [addr(j), addr(j)+SIZE) = ∅
- addr(slot) ≥ ELF_LOAD_BASE
- addr(slot) + SIZE ≤ ELF_LOAD_BASE + total_region_size
```
> Chỉ implement nếu còn >5h buffer. Host test đã exhaustive cho N=6 slots.

#### File cần thay đổi (O4)

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/ipc.rs` | Sửa | Extract `push_pure()`, `pop_pure()`, `copy_message_pure()`, `cleanup_pure()` |
| `src/kernel/ipc.rs` | Sửa | Thêm 3 `#[cfg(kani)]` proof harnesses (P0) |
| `src/platform/qemu_virt.rs` | Sửa (P1) | Thêm `#[cfg(kani)]` `verify_elf_load_addr_no_overlap` (optional) |
| `tests/host_tests.rs` | Sửa | Thêm tests matching Kani properties |

#### Checkpoint O4

```
Kani verification: 9 proofs, 0 failures (P0)
                   10 proofs, 0 failures (P0+P1, if buffer allows)
```
> 6 proofs cũ (Phase N) + 3 proofs mới IPC (P0) = 9 tổng. +1 elf_load_addr (P1) = 10.

---

### O5 — Build System: README + build-all.sh

#### Khái niệm

Với 2 build commands riêng biệt (user workspace + kernel), cần documentation rõ ràng và convenience script.

#### Thiết kế

**README build order:**
```bash
# 1. Build user crates
cd user && cargo build --release -Zjson-target-spec --target aarch64-user.json

# 2. Build kernel (embeds user binaries via include_bytes!)
cargo build --release -Zjson-target-spec

# 3. Run on QEMU
qemu-system-aarch64 -machine virt -cpu cortex-a53 -nographic -kernel target/aarch64-aegis/release/aegis_os
```

**`scripts/build-all.sh`** (~10 dòng):
```bash
#!/bin/bash
set -euo pipefail
echo "=== Building user crates ==="
(cd user && cargo build --release -Zjson-target-spec --target aarch64-user.json)
echo "=== Building kernel ==="
cargo build --release -Zjson-target-spec
echo "=== Build complete ==="
```

#### File cần thay đổi (O5)

| File | Thao tác | Chi tiết |
|---|---|---|
| `README.md` | Sửa | Thêm build order (user crates → kernel) |
| `scripts/build-all.sh` | Tạo mới | ~10 dòng bash convenience script |

#### Checkpoint O5

```
scripts/build-all.sh → qemu-system-aarch64 → all checkpoints pass
```

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|---|---|---|
| 1 | **No heap** | Bất biến AegisOS | Multi-ELF dùng `include_bytes!` (compile-time), linker sections static |
| 2 | **No FP/SIMD** | CPACR_EL1.FPEN=0 | User binaries cũng không dùng FP |
| 3 | **TrapFrame = 288B** | ABI-locked | Không thay đổi |
| 4 | **W^X** | Page permissions | Per-task ELF page attributes giữ W^X |
| 5 | **Linker ↔ MMU đồng bộ** | Bất biến | Mở rộng .elf_load → cập nhật cả linker.ld + mmu.rs |
| 6 | **Syscall ABI stable** | x7=nr, x6=ep, x0–x3=payload | SYS_EXIT dùng cùng convention (x7=13, x0=exit_code) |
| 7 | **231 tests + 30 checkpoints = regression** | Safety net | Mỗi sub-phase PHẢI pass full suite |
| 8 | **User binary ≤ 16 KiB** | ELF_LOAD_SIZE_PER_TASK | `const_assert!` compile-time + binary phải opt-level="s" + LTO |
| 9 | **Kani trên host (x86_64)** | Không verify asm | Chỉ verify pure Rust logic (IPC data structures) |
| 10 | **User crate dùng custom target** | `aarch64-user.json` | Shared `user/aarch64-user.json` cho toàn workspace |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|---|
| 1 | Multi-binary link address conflict | Thấp | Tasks overwrite nhau | Manual linker.ld per-binary + `const_assert!` kiểm tra binary size ≤ slot size. Scale limit: 5 binaries max |
| 2 | User binary > 16 KiB | Thấp | Load fail | `opt-level="s"` + LTO + minimal code; nếu cần → tăng per-task region |
| 3 | Cache maintenance không đủ cho 6 regions | Thấp | Stale instruction cache | Cache flush toàn bộ .elf_load region (96 KiB) — vẫn nhanh |
| 4 | `libsyscall` Cargo dependency resolution trên custom target | Trung bình | Build fail | Test build `user/hello` với libsyscall trước khi tạo binary mới |
| 5 | Kani IPC proof timeout (state machine phức tạp) | Trung bình | Proof không hoàn thành | Bound: MAX_WAITERS=4, MAX_ENDPOINTS=4 — small enough for CBMC |
| 6 | SYS_EXIT cleanup race với timer interrupt | Thấp | Inconsistent state | Exit handler chạy với interrupts disabled (trong SVC handler, đã đúng) |
| 7 | Exited task ID reused — stale references | Thấp | IPC message đến task đã exit | Cleanup function đã xóa task từ queues; thêm guard `state != Exited` |

---

## Test Plan

### Host unit tests mới (ước lượng: ~25-35 tests)

| # | Test case | Mô tả |
|---|---|---|
| 1-3 | `test_elf_load_addr_computed` | Verify elf_load_addr(0–5) trả đúng addresses |
| 4-5 | `test_elf_load_addr_bounds` | elf_load_addr(slot) nằm trong .elf_load region |
| 6-7 | `test_elf_load_addr_no_overlap` | Không 2 slots nào overlap |
| 8-10 | `test_multi_elf_parse` | Parse 2+ ELF binaries thành công |
| 11-12 | `test_task_state_exited` | TaskState::Exited value, display |
| 13-15 | `test_exit_cleanup_ipc` | SYS_EXIT removes task from sender queues |
| 16-17 | `test_exit_cleanup_grants` | SYS_EXIT revokes task's grants |
| 18-19 | `test_exit_cleanup_irq` | SYS_EXIT unbinds task's IRQs |
| 20-21 | `test_scheduler_skip_exited` | Scheduler never picks Exited task |
| 22-23 | `test_exited_no_restart` | Exited task NOT auto-restarted (unlike Faulted) |
| 24-25 | `test_cap_exit_bit` | CAP_EXIT = bit 18, cap_for_syscall(13) |
| 26-28 | `test_sender_queue_kani_matching` | Host test mirroring Kani SenderQueue proof |
| 29-30 | `test_ipc_cleanup_completeness` | cleanup_task removes from ALL endpoints |
| 31-33 | `test_libsyscall_constants` | SYS_* constants match kernel |
| 34-35 | `test_elf_load_size_per_task` | Size fits within linker allocation |

### QEMU boot checkpoints mới

| # | Checkpoint UART output | Sub-phase |
|---|---|---|
| 31 | `[AegisOS] task 2 loaded from ELF (entry=0x...)` | O1 |
| 32 | `[AegisOS] task 3 loaded from ELF (entry=0x...)` | O1 |
| 33 | `[AegisOS] 3 ELF user tasks loaded, task 7 = IDLE` | O1 |
| 34 | `[AegisOS] task N exited (code=0)` | O3 |

### Kani proofs mới (3 P0 + 1 P1 optional)

| # | Proof | Module | Priority | Property |
|---|---|---|---|---|
| 7 | `verify_sender_queue_no_overflow` | `ipc.rs` | P0 | push full → false, pop empty → None, count ∈ [0,4] (`#[kani::unwind(5)]`) |
| 8 | `verify_message_integrity` | `ipc.rs` | P0 | Message payload preserved across send+recv |
| 9 | `verify_cleanup_completeness` | `ipc.rs` | P0 | cleanup removes task from ALL endpoints |
| 10 | `verify_elf_load_addr_no_overlap` | `qemu_virt.rs` | P1 | No slot overlap, bounds valid (only if >5h buffer) |

---

## Thứ tự triển khai

| Giai đoạn | Timeline | Hành động | Effort | Ưu tiên |
|-----------|----------|-----------|--------|--------|
| **O5**: Build docs | Week 1 | README build order + `scripts/build-all.sh` | ~1h | P0 |
| **O2**: libsyscall | Week 1-2 | `user/` workspace, libsyscall crate, refactor hello | ~6h | P0 |
| **O1**: Multi-ELF | Week 1-2 | Mở rộng .elf_load, `load_elf_to_task()`, 3 user crates, manual linker.ld + `const_assert!` | ~14h | P0 |
| **O3**: SYS_EXIT | Week 2-3 | SYS_EXIT #13, `TaskState::Exited`, `cleanup_task_resources()` | ~8h | P0 |
| **O4**: Kani IPC | Week 3-4 | 3 Kani proofs P0 (SenderQueue, message, cleanup) | ~10h | P0 |
| (P1): elf_load_addr | Week 4 (conditional) | Kani proof only if >5h buffer remaining | ~3h | P1 |
| **O-final**: Integration | Week 4 | Coverage re-measure + regression | ~3h | P0 |
| | | **Tổng P0** | **~42h** | |
| | | **Tổng P0+P1** | **~45h** | |
| | | **Buffer** | **~15–18h** | |

**Sequencing:**
- O5 (build docs) → khởi đầu nhanh, 1h
- O2 (libsyscall) và O1 (linker) có thể **song song** — không phụ thuộc nhau
- O3 (SYS_EXIT) có thể **song song** với O1 — chỉ cần merge cuối cùng
- O4 (Kani) có thể bắt đầu sớm — IPC proofs không phụ thuộc multi-ELF

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|---|---|---|
| **DO-178C** | §5.5 | Single source of truth — `libsyscall` eliminates duplicated syscall code (O2) |
| **DO-178C** | §5.6 | Graceful shutdown — `SYS_EXIT` cho task lifecycle hoàn chỉnh (O3) |
| **DO-178C** | §6.3.4 | Source code verifiable — cleanup logic reviewed + tested (O3) |
| **DO-333** | FM.A-5 | Model checking — Kani IPC deadlock-freedom proof (O4) |
| **DO-333** | FM.A-7 | Coverage of formal proofs — expand từ 6 → 9 proofs (O4) |
| **IEC 62304** | §5.5.3 | Unit verification — 25-35 new test cases covering multi-ELF + exit (O1, O3) |
| **IEC 62304** | Amendment 1 §4.3 | Software unit isolation — 6 independent ELF user tasks (O1) |
| **ISO 26262** | Part 6 §7 | Software unit design — `load_elf_to_task()` reusable, testable (O1) |
| **ISO 26262** | Part 9 | ASIL Decomposition — 6 isolated user partitions (O1) |

---

## Trade-offs đã chấp nhận

| # | Trade-off | Chấp nhận | Lý do |
|---|---|---|---|
| 1 | Manual linker.ld thay vì auto-generation | 75 dòng duplicate (3 files × 25 dòng) | Tránh build script complexity. `const_assert!` đảm bảo safety tĩnh. Scale limit: 5 binaries max |
| 2 | No SYS_KILL | Thiếu kill mechanism | Tránh security attack surface và premature design. Watchdog + fault recovery cover embedded use cases |
| 3 | elf_load_addr proof = P1 | Host test coverage thay vì Kani symbolic verification | Bounded slots (N=5) → exhaustive enumeration test đủ |
| 4 | Separate user/ workspace | 2 build commands (user + kernel) thay vì 1 | Tránh Cargo target mismatch. `build-all.sh` wraps thành 1 command |
| 5 | 3 Kani IPC proofs, không deadlock-freedom | Không chứng minh deadlock-freedom | PhD-level problem, tập trung vào data integrity proofs thực tế |

---

## Backward Compatibility

| Thay đổi | Break API? | Break ABI? | Migration |
|---|---|---|---|
| `.elf_load` 12→96 KiB | Không | Không | Linker change, transparent |
| `load_elf_to_task()` function | Không — additive | Không | kernel_main refactored internally |
| `TaskState::Exited` | Có — enum variant mới | Không | Scheduler + timer handler cần handle Exited |
| `SYS_EXIT` (#13) | Có — new syscall | Có (ABI extension) | Additive, existing syscalls unchanged |
| `CAP_EXIT` (bit 18) | Có — new cap bit | Không | Assign bit 18, existing 0–17 unchanged |
| `libsyscall` crate | Không — additive | Không | Existing user/hello refactored to use it |
| **Syscall ABI (0–12)** | **Không đổi** | **Không đổi** | — |
| **TrapFrame** | **Không đổi** | **Không đổi** | — |
| **Capability bits (0–17)** | **Không đổi** | **Không đổi** | — |

---

## Bước tiếp theo đề xuất

1. [x] Review kế hoạch Phase O → đồng thuận đạt 2026-02-12
2. [x] Triển khai O5: README build order + `scripts/build-all.sh` + `scripts/build-all.ps1` ✅
3. [x] Triển khai O2: `user/` workspace + `libsyscall` crate (14 syscall wrappers) ✅
4. [x] Triển khai O1: Multi-ELF linker (96 KiB) + `load_elf_to_task()` + 3 user crates (hello/sensor/logger) + `const_assert!` ✅ QEMU verified
5. [x] Triển khai O3: SYS_EXIT (#13) + `TaskState::Exited` + `cleanup_task_resources()` + `CAP_EXIT` (bit 18) + CPACR_EL1.FPEN=0b01 ✅ QEMU verified
6. [x] Triển khai O4: Kani IPC proofs P0 (3 proofs) + P1 elf_load_addr ✅ 10/10 proofs pass (aegis-dev Docker)
7. [x] Viết blog kể chuyện về Phase O đã thực hiện ✅ Bài #15 "Ba Chương Trình, Một Hệ Sinh Thái"
8. [x] Chạy test suite đầy đủ — 241 host tests pass, 32/32 QEMU checkpoints pass, 10/10 Kani proofs pass ✅
9. [x] Verify: `scripts/build-all.sh` → `qemu-system-aarch64` → all 32 checkpoints pass ✅
10. [ ] Chuẩn bị Phase P roadmap
11. [x] **Cập nhật `copilot-instructions.md`** ✅ 2026-02-13
12. [ ] **Cập nhật README.md**
