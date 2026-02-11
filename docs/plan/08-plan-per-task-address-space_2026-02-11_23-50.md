# Kế hoạch Phase H — Per-Task Address Space

> **Trạng thái: ✅ HOÀN THÀNH** — Mỗi task có bảng trang riêng (per-task page table). Task A không thể đọc/ghi bộ nhớ của Task B, kể cả cùng chạy ở EL0. TTBR0_EL1 được swap khi context switch + ASID tag TLB. Đây là lớp cách ly bộ nhớ bắt buộc cho safety-critical (DO-178C §5.3.1 — memory partitioning, ISO 26262 Part 6 §7.4.6 — freedom from interference).

---

## Tại sao Phase H?

### Lỗ hổng hiện tại: "Ai cũng nhìn thấy bộ nhớ của nhau"

Phase G đã kiểm soát **syscall** — mỗi task chỉ gọi được syscall mà nó có quyền. Nhưng vẫn còn một lỗ hổng lớn:

**Tất cả task chia chung một bảng trang (page table).** Ba vùng user stack (`__user_stacks_start` → `__user_stacks_end`, 3×4KB) đều dùng descriptor `AP_RW_EL0` — nghĩa là **bất kỳ task EL0 nào cũng đọc/ghi được stack của task khác**.

Ví dụ thực tế:
- Task A (PING) đang lưu dữ liệu nhạy cảm trên stack → Task B có thể đọc trực tiếp
- Task B bị lỗi ghi tràn → có thể ghi đè stack của Task A → Task A crash theo
- Hacker chiếm Task idle → quét toàn bộ 12KB user stack tìm dữ liệu

Capability **không chặn được** điều này — capability kiểm soát syscall, không kiểm soát truy cập bộ nhớ trực tiếp (load/store instruction).

### Giải pháp: Per-Task Page Table + TTBR0 Swap

Mỗi task có bảng trang riêng. Trong bảng trang của Task A:
- Stack của Task A: `AP_RW_EL0` (đọc/ghi được) ✅
- Stack của Task B: **không có mapping** hoặc `AP_RW_EL1` (EL0 fault) ❌
- Stack của Task C: **không có mapping** hoặc `AP_RW_EL1` (EL0 fault) ❌
- Kernel code/data: giữ nguyên mapping (EL1 only hoặc shared code)

Khi context switch: kernel ghi TTBR0_EL1 = bảng trang của task mới → CPU dùng bảng trang mới → task mới chỉ thấy bộ nhớ của chính nó.

---

## Phân tích hiện trạng

### Bảng trang hiện tại: 1 bộ duy nhất, 4 pages

```
L1 (page 0, 512 entries)
├── [0] → L2_device (page 1) — 0x0000_0000..0x3FFF_FFFF
│         ├── [64..72] → Device MMIO 2MB blocks (GIC, UART)
│         └── rest → invalid
│
└── [1] → L2_ram (page 2) — 0x4000_0000..0x7FFF_FFFF
          ├── [0] → L3_kernel (page 3) — 0x4000_0000..0x401F_FFFF (first 2MB)
          │         ├── text pages:  AP_RO_EL0 (exec by both EL0+EL1)
          │         ├── rodata:     AP_RO_EL0, XN
          │         ├── data/bss:   AP_RW_EL1, XN (EL0 no access)
          │         ├── page_tables: AP_RW_EL1, XN
          │         ├── task_stacks: AP_RW_EL1, XN (kernel stacks)
          │         ├── user_stacks: AP_RW_EL0, XN ← 🔴 TẤT CẢ EL0 ĐỌC/GHI ĐƯỢC
          │         ├── guard page:  invalid
          │         └── boot stack:  AP_RW_EL1, XN
          │
          └── [1..63] → RAM 2MB blocks, AP_RW_EL1 (EL0 no access)
```

**Vấn đề cốt lõi:** Chỉ có 1 bảng L3, user stacks dùng `AP_RW_EL0` chung → mọi EL0 task đều truy cập được.

### TTBR0/TTBR1 hiện tại

- **TTBR0_EL1** = `__page_tables_start` (L1 base) — duy nhất, không đổi
- **TTBR1_EL1** = **disabled** (`EPD1=1` trong TCR_EL1)
- **Không dùng ASID** — TTBR0 bits [63:48] = 0

### TCB hiện tại (sau Phase G)

```
Tcb {
    context:        TrapFrame,  // 288B — ABI-locked, offset 0
    state:          TaskState,  // 1B
    id:             u16,        // 2B
    stack_top:      u64,        // 8B — SP_EL1 (kernel stack per task)
    entry_point:    u64,        // 8B — restart point
    user_stack_top: u64,        // 8B — SP_EL0
    fault_tick:     u64,        // 8B — tick khi Faulted
    caps:           CapBits,    // 8B — capability bitmask
}
```

### Context switch hiện tại

```
Timer IRQ (EL0 → EL1):
1. SAVE_CONTEXT_LOWER macro:
   - Stash x9 → TPIDR_EL1
   - Load SP = __stack_end (shared kernel boot stack, 16KB)
   - Save x0–x30, SP_EL0, ELR_EL1, SPSR_EL1 vào TrapFrame trên stack
   - x0 = &TrapFrame

2. handle_timer_irq(frame) → schedule():
   - save_context(): copy TrapFrame vào TCBS[CURRENT].context
   - Chọn next Ready task (round-robin)
   - restore_context(): copy TCBS[next].context ra TrapFrame

3. RESTORE_CONTEXT_LOWER macro:
   - Load SP_EL0, ELR_EL1, SPSR_EL1 từ TrapFrame
   - Load x0–x30
   - eret → EL0

⚠️ KHÔNG swap TTBR0 ở bất kỳ bước nào.
```

### User stack addresses

| Task | Stack Base | Stack Top (SP_EL0) |
|---|---|---|
| 0 | `__user_stacks_start` | `__user_stacks_start + 0x1000` |
| 1 | `__user_stacks_start + 0x1000` | `__user_stacks_start + 0x2000` |
| 2 | `__user_stacks_start + 0x2000` | `__user_stacks_start + 0x3000` |

Tất cả nằm trong 3 page liên tiếp trong vùng L3_kernel.

---

## Thiết kế Phase H

### Chiến lược: TTBR0-only, swap per-task L3

Giữ kiến trúc TTBR0-only (không bật TTBR1), nhưng mỗi task có **bộ page table riêng** với mapping user stack khác nhau. Cụ thể:

| Table | Chia sẻ? | Lý do |
|---|---|---|
| L1 | **Per-task** (3 bản) | TTBR0 trỏ vào đây, cần riêng |
| L2_device | **Chia sẻ** (1 bản) | Device MMIO giống nhau cho mọi task |
| L2_ram | **Per-task** (3 bản) | Entry [0] trỏ vào L3 riêng |
| L3 | **Per-task** (3 bản) | User stack mapping khác nhau |

Thay vì clone toàn bộ L3 512 entries, chỉ cần **thay đổi 3 entry** (3 user stack pages):
- Task 0 L3: user stack page 0 = `AP_RW_EL0`, page 1,2 = `AP_RW_EL1` (hoặc invalid)
- Task 1 L3: user stack page 1 = `AP_RW_EL0`, page 0,2 = `AP_RW_EL1`
- Task 2 L3: user stack page 2 = `AP_RW_EL0`, page 0,1 = `AP_RW_EL1`

Mọi entry khác (text, rodata, data, bss, kernel stacks) **giống hệt nhau**.

### Bảng trang mới: 13 pages

| Page | Mục đích | Nội dung |
|---|---|---|
| 0 | L2_device (chia sẻ) | Giống cũ |
| 1 | L1 cho Task 0 | [0] → L2_device, [1] → L2_ram_task0 |
| 2 | L1 cho Task 1 | [0] → L2_device, [1] → L2_ram_task1 |
| 3 | L1 cho Task 2 | [0] → L2_device, [1] → L2_ram_task2 |
| 4 | L2_ram cho Task 0 | [0] → L3_task0, [1..63] → RAM blocks |
| 5 | L2_ram cho Task 1 | [0] → L3_task1, [1..63] → RAM blocks |
| 6 | L2_ram cho Task 2 | [0] → L3_task2, [1..63] → RAM blocks |
| 7 | L3 cho Task 0 | user_stack_0 = RW_EL0, stack 1,2 = RW_EL1 |
| 8 | L3 cho Task 1 | user_stack_1 = RW_EL0, stack 0,2 = RW_EL1 |
| 9 | L3 cho Task 2 | user_stack_2 = RW_EL0, stack 0,1 = RW_EL1 |
| 10 | L1 kernel (boot) | Dùng cho boot + exception handler trước khi task chạy |
| 11 | L2_ram kernel | Giống cũ, tất cả user stacks = AP_RW_EL1 |
| 12 | L3 kernel | Giống cũ nhưng user stacks = AP_RW_EL1 (kernel access only) |

**Tổng: 13 pages = 52 KiB** (thêm 36 KiB so với hiện tại).

> **Lưu ý:** Kernel boot page table (pages 10-12) dùng trong exception handler context — khi handler chạy, TTBR0 trỏ tới page table của task hiện tại, nhưng kernel code có AP_RW_EL1 nên vẫn truy cập được mọi thứ. **Không cần** swap TTBR0 khi vào exception handler vì kernel chạy ở EL1 — AP_RW_EL1 entries trong per-task page table đã cấp quyền cho kernel.

### ASID (Address Space Identifier)

- TTBR0_EL1 bits [63:48] = **ASID** — tag cho TLB entries
- Mỗi task có ASID riêng: Task 0 = ASID 1, Task 1 = ASID 2, Task 2 = ASID 3
- Khi swap TTBR0: TLB entries tagged ASID cũ **không cần flush** — CPU tự bỏ qua
- Chỉ cần `isb` sau `msr ttbr0_el1` để đảm bảo pipeline consistency
- Giảm **chi phí TLB miss** đáng kể so với `tlbi vmalle1` mỗi lần switch

**TCR_EL1 cần bật A1=0 (ASID từ TTBR0)** — hiện tại A1 bit chưa set → mặc định = 0 → OK.

### TCB thêm field `ttbr0`

```
Tcb {
    ...existing fields...
    caps:  CapBits,    // 8B
    ttbr0: u64,        // 8B — TTBR0 value = (ASID << 48) | page_table_phys_base
}
```

### Context switch mới

```
schedule():
    save_context()          // như cũ
    pick next task          // như cũ
    restore_context()       // như cũ

    // ─── MỚI: swap address space ───
    let new_ttbr0 = TCBS[next].ttbr0;
    msr ttbr0_el1, new_ttbr0
    isb
```

Đặt sau `restore_context()`, trước khi `eret`. Hoặc có thể đặt trong `restore_context()` luôn.

### Hành vi khi restart

`restart_task()` reset context nhưng **giữ nguyên `ttbr0`** — page table là chính sách tĩnh, giống như `caps`. Task restart vẫn dùng cùng address space.

### Ảnh hưởng đến `validate_write_pointer`

Hàm `validate_write_pointer` hiện kiểm tra range `[0x4000_0000, 0x4800_0000)`. Với per-task page table:
- Range vẫn đúng (identity mapping giữ nguyên cho kernel side)
- Nhưng kernel ghi vào user pointer qua **kernel's EL1 access** → luôn có quyền ghi (AP_RW_EL1 entries trong per-task table vẫn accessible)
- **Không cần thay đổi** `validate_write_pointer` — nó chạy ở EL1 context

### Ảnh hưởng đến IPC

- IPC dùng **register copy** (x0–x3 trong TrapFrame) → không truy cập user memory
- `copy_message()` copy giữa `TCBS[].context.x[]` → kernel BSS → OK
- **Không ảnh hưởng**

### Ảnh hưởng đến Shared Code

- Tất cả `.text` là `SHARED_CODE_PAGE` (AP_RO_EL0) → giống nhau trong mọi per-task L3
- Task vẫn execute code ở EL0 bình thường
- **Không ảnh hưởng**

---

## Các bước thực hiện

### H1 — Mở rộng page table storage + refactor `mmu::init()`

**Mục tiêu:** Cấp phát đủ 13 pages cho page tables. Refactor `init()` để build per-task tables.

**Thay đổi:**

1. **Sửa `linker.ld`:**
   - Thay `.page_tables` size: `4 * 4096` → `13 * 4096` (52 KiB)
   - Giữ 4KB alignment

2. **Sửa `src/mmu.rs`:**
   - Thêm hằng số `NUM_PAGE_TABLE_PAGES = 13`
   - Thêm hằng số cho page index: `PT_L2_DEVICE = 0`, `PT_L1_TASK0 = 1`, ..., `PT_L3_KERNEL = 12`
   - Refactor `init()`:
     a. Build L2_device (page 0) — chia sẻ, giống cũ
     b. Build L3 cho mỗi task (pages 7,8,9): clone từ template, chỉ thay user stack AP bits
     c. Build L2_ram cho mỗi task (pages 4,5,6): entry[0] → L3 riêng, entries[1..63] → RAM blocks
     d. Build L1 cho mỗi task (pages 1,2,3): [0] → L2_device, [1] → L2_ram riêng
     e. Build kernel boot tables (pages 10,11,12): user stacks = AP_RW_EL1
   - Thêm hàm `pub fn page_table_base(task_id: usize) -> u64` — trả physical address của L1 cho task đó
   - Thêm hàm `pub fn ttbr0_for_task(task_id: usize, asid: u16) -> u64` — trả `(asid << 48) | base`

3. **Sửa `src/boot.s`:**
   - TTBR0_EL1 ban đầu trỏ vào **kernel boot L1** (page 10) — không phải per-task table
   - Thay `__page_tables_start` → `__page_tables_start + 10 * 4096` (hoặc dùng symbol mới)

**Checkpoint:** Build thành công. QEMU boot bình thường — kernel vẫn dùng kernel boot page table, chưa swap. Tất cả test cũ pass.

---

### H2 — Thêm `ttbr0` vào TCB + gán trong `kernel_main`

**Mục tiêu:** Mỗi task biết page table của mình. Chưa swap.

**Thay đổi:**

1. **Sửa `src/sched.rs`:**
   - Thêm `pub ttbr0: u64` vào `Tcb` (cuối struct, sau `caps`)
   - Cập nhật `EMPTY_TCB`: `ttbr0: 0`

2. **Sửa `src/main.rs` — `kernel_main()`:**
   - Sau capability assignment, gán `ttbr0` cho mỗi task:
     ```
     TCBS[0].ttbr0 = mmu::ttbr0_for_task(0, 1);  // ASID=1
     TCBS[1].ttbr0 = mmu::ttbr0_for_task(1, 2);  // ASID=2
     TCBS[2].ttbr0 = mmu::ttbr0_for_task(2, 3);  // ASID=3
     ```
   - Thêm UART: `"[AegisOS] per-task address spaces assigned\n"`

3. **Sửa `src/sched.rs` — `restart_task()`:**
   - Xác nhận `ttbr0` không bị reset (giống `caps`)

**Checkpoint:** Build pass. QEMU boot hiển thị message mới. Chưa swap → hành vi y hệt cũ.

---

### H3 — Swap TTBR0 trong context switch

**Mục tiêu:** Mỗi khi chuyển task, kernel load TTBR0 mới. **Đây là bước critical.**

**Thay đổi:**

1. **Sửa `src/sched.rs` — `schedule()`:**
   - Sau `restore_context()`, trước return:
     ```
     let new_ttbr0 = TCBS[CURRENT].ttbr0;
     unsafe {
         core::arch::asm!(
             "msr ttbr0_el1, {val}",
             "isb",
             val = in(reg) new_ttbr0,
             options(nomem, nostack)
         );
     }
     ```

2. **Sửa `src/sched.rs` — `bootstrap()`:**
   - Trước `eret`, load TTBR0 cho task 0:
     ```
     msr ttbr0_el1, {ttbr0}
     isb
     ```

3. **Edge case: exception handler chạy với per-task TTBR0**
   - Khi timer IRQ xảy ra ở EL0, CPU chuyển sang EL1 nhưng **TTBR0 vẫn là của task cũ**
   - **Không sao** vì: kernel code/data trong per-task table đều có AP_RW_EL1 → kernel đọc/ghi được
   - `SAVE_CONTEXT_LOWER` load SP = `__stack_end` → vùng kernel stack, AP_RW_EL1 → OK
   - `TCBS[]` nằm trong `.bss` → AP_RW_EL1 → OK
   - Chỉ user stacks khác → nhưng handler không truy cập user stack trực tiếp

4. **Edge case: `handle_write` truy cập user pointer**
   - `SYS_WRITE` đọc byte từ user pointer → dùng EL1 access → AP_RW_EL1 cho vùng text/rodata
   - User truyền pointer vào `.text` (shared code, AP_RO_EL0) hoặc `.rodata` → cả hai đều mapped
   - **Nếu user truyền pointer vào stack task khác** → trong per-task table, stack khác = AP_RW_EL1 → EL1 vẫn đọc được → `validate_write_pointer` chỉ check range, không check ownership
   - **Cần xem xét:** có nên thêm check ownership vào `validate_write_pointer`? → Phase H3 chỉ swap TTBR0, ownership check để H4 hoặc tương lai.

**Checkpoint:** QEMU boot, PING/PONG hoạt động bình thường. Log thêm "per-task address spaces active" sau bootstrap. Mỗi task chạy trong address space riêng.

---

### H4 — Kiểm chứng cách ly trên QEMU

**Mục tiêu:** Chứng minh task không đọc được stack của task khác.

**Thay đổi:**

1. **Thêm test scenario (gated by `#[cfg(feature = "test-isolation")]`):**
   - Task A cố đọc địa chỉ user stack của Task B → **Data Abort** → fault → UART: `"data abort at EL0"` + `"faulting task 0"`
   - Task A restart → tiếp tục PING/PONG bình thường

2. **Sửa `Cargo.toml`:** thêm feature `test-isolation`

3. **Sửa `idle_entry` hoặc thêm hàm test riêng (gated):**
   - Trong lần chạy đầu tiên: cố đọc `__user_stacks_start + 0x1000` (stack task 1)
   - Expected: Data Abort → fault → restart

**Checkpoint:** QEMU output hiện "data abort at EL0" cho task vi phạm. Task khác không ảnh hưởng. Sau restart, hệ thống tiếp tục PING/PONG.

---

### H5 — Viết unit tests cho per-task address space

**Mục tiêu:** ~10 test mới trong `tests/host_tests.rs`, nhóm **AddressSpace**.

**Test cases:**

| # | Test name | Mô tả |
|---|---|---|
| 1 | `addr_page_table_base_per_task` | `page_table_base(0) != page_table_base(1) != page_table_base(2)` |
| 2 | `addr_ttbr0_includes_asid` | `ttbr0_for_task(0, 1)` có ASID=1 trong bits [63:48] |
| 3 | `addr_ttbr0_base_aligned_4k` | Base address 4KB-aligned (bits [11:0] = 0) |
| 4 | `addr_per_task_l3_user_stack_own` | Task 0 L3: own stack page = AP_RW_EL0 |
| 5 | `addr_per_task_l3_user_stack_other` | Task 0 L3: other stack pages = AP_RW_EL1 (not EL0) |
| 6 | `addr_per_task_l3_kernel_data_el1_only` | Kernel data pages vẫn AP_RW_EL1 trong per-task table |
| 7 | `addr_per_task_l3_shared_code` | `.text` pages vẫn AP_RO_EL0 (shared, executable) |
| 8 | `addr_kernel_table_user_stacks_el1` | Kernel boot table: user stacks = AP_RW_EL1 |
| 9 | `addr_tcb_ttbr0_survives_restart` | `restart_task()` không xóa `ttbr0` |
| 10 | `addr_asid_unique_per_task` | ASID 1, 2, 3 cho 3 tasks |

**Cập nhật `reset_test_state()`:** Reset `ttbr0` trong mỗi TCB về 0.

**Checkpoint:** `cargo test` — tất cả test cũ + mới pass (~79 tests: 69 cũ + 10 mới).

---

### H6 — Cập nhật QEMU boot test + CI

**Mục tiêu:** Thêm checkpoint mới vào boot test scripts.

**Thay đổi:**

1. **Sửa `tests/qemu_boot_test.sh`:**
   - Thêm checkpoint: `"[AegisOS] per-task address spaces assigned"`

2. **Sửa `tests/qemu_boot_test.ps1`:**
   - Tương tự

3. **CI tự động pass** — `.github/workflows/ci.yml` không cần sửa (dùng script hiện có).

**Checkpoint:** CI green: host tests + QEMU boot all pass.

---

## Tóm tắt thay đổi theo file

| File | Thay đổi | Sub-phase |
|---|---|---|
| `linker.ld` | `.page_tables` 4×4096 → 13×4096 | H1 |
| `src/mmu.rs` | Refactor `init()` build 13 tables, thêm `page_table_base()`, `ttbr0_for_task()` | H1 |
| `src/boot.s` | TTBR0 trỏ kernel boot L1 (page 10) thay vì page 0 | H1 |
| `src/sched.rs` | Thêm `ttbr0: u64` vào Tcb, TTBR0 swap trong `schedule()` + `bootstrap()` | H2, H3 |
| `src/main.rs` | Gán `ttbr0` cho 3 tasks, thêm UART message | H2 |
| `Cargo.toml` | Thêm feature `test-isolation` | H4 |
| `src/main.rs` | Test isolation scenario (gated) | H4 |
| `tests/host_tests.rs` | ~10 test mới nhóm AddressSpace | H5 |
| `tests/qemu_boot_test.sh` | Thêm checkpoint | H6 |
| `tests/qemu_boot_test.ps1` | Thêm checkpoint | H6 |

### Không thay đổi:
- `src/ipc.rs` — IPC dùng register copy, không truy cập user memory
- `src/cap.rs` — capability system giữ nguyên
- `src/gic.rs`, `src/timer.rs`, `src/uart.rs` — không liên quan
- `src/exception.rs` — handler chạy ở EL1 với per-task TTBR0, vẫn OK (tất cả kernel regions có AP_RW_EL1)

---

## Sơ đồ page table sau Phase H

```
                    ┌─── L1_task0 (page 1) ──┬── [0] → L2_device (page 0, SHARED)
                    │                        └── [1] → L2_ram_task0 (page 4)
                    │                                    ├── [0] → L3_task0 (page 7)
                    │                                    │          ├── .text: AP_RO_EL0 ✅
                    │                                    │          ├── .data: AP_RW_EL1
                    │                                    │          ├── user_stack_0: AP_RW_EL0 ✅
Task 0 (TTBR0) ────┘                                    │          ├── user_stack_1: AP_RW_EL1 🔒
                                                         │          └── user_stack_2: AP_RW_EL1 🔒
                                                         └── [1..63] → RAM 2MB blocks

                    ┌─── L1_task1 (page 2) ──┬── [0] → L2_device (page 0, SHARED)
                    │                        └── [1] → L2_ram_task1 (page 5)
                    │                                    ├── [0] → L3_task1 (page 8)
                    │                                    │          ├── .text: AP_RO_EL0 ✅
                    │                                    │          ├── .data: AP_RW_EL1
                    │                                    │          ├── user_stack_0: AP_RW_EL1 🔒
Task 1 (TTBR0) ────┘                                    │          ├── user_stack_1: AP_RW_EL0 ✅
                                                         │          └── user_stack_2: AP_RW_EL1 🔒
                                                         └── [1..63] → RAM 2MB blocks

                    ┌─── L1_task2 (page 3) ──┬── [0] → L2_device (page 0, SHARED)
                    │                        └── [1] → L2_ram_task2 (page 6)
                    │                                    ├── [0] → L3_task2 (page 9)
                    │                                    │          ├── .text: AP_RO_EL0 ✅
                    │                                    │          ├── .data: AP_RW_EL1
                    │                                    │          ├── user_stack_0: AP_RW_EL1 🔒
Task 2 (TTBR0) ────┘                                    │          ├── user_stack_1: AP_RW_EL1 🔒
                                                         │          └── user_stack_2: AP_RW_EL0 ✅
                                                         └── [1..63] → RAM 2MB blocks

Kernel boot (pages 10-12): tất cả user stacks = AP_RW_EL1 (không EL0 nào truy cập)
```

---

## Điểm cần lưu ý

1. **Page tables phải nằm trong `.page_tables` section.** Linker đặt ở vùng data → AP_RW_EL1, XN. `mmu::init()` ghi trực tiếp qua `write_volatile`. Mở rộng section không ảnh hưởng alignment.

2. **TCB size tăng 8 byte** (`ttbr0: u64`). Offset các field cũ không đổi (repr(C), thêm cuối).

3. **ASID 8-bit vs 16-bit.** TCR_EL1 hiện tại AS bit (bit 36) = 0 → 8-bit ASID → hỗ trợ 256 ASID. 3 tasks chỉ cần 3 ASID → dư sức.

4. **`isb` sau `msr ttbr0_el1`** là đủ khi dùng ASID. Không cần `tlbi` vì ASID tag TLB entries — entries cũ tự nhiên không match ASID mới.

5. **Boot sequence:** boot.s init MMU với kernel boot table → kernel_main() build per-task tables → gán ttbr0 cho TCBs → bootstrap() swap sang task 0 TTBR0 → eret.

6. **Exception handler TTBR0:** Khi IRQ ở EL0, TTBR0 vẫn là của task đang chạy. Handler ở EL1 → dùng AP_RW_EL1 entries → truy cập mọi kernel memory OK. Khi schedule() chọn task mới → swap TTBR0 trước eret. **Không cần swap TTBR0 khi vào handler.**

7. **restart_task() ảnh hưởng:** Giữ nguyên `ttbr0` — task restart dùng cùng address space. Hàm `restart_task` zero-out context + set entry/stack → user stack physical address không đổi → mapping vẫn đúng.

8. **Bộ nhớ thêm:** 9 pages × 4KB = **36 KiB BSS**. Tổng page tables: 52 KiB. Vẫn rất nhỏ cho bare-metal.

9. **DO-178C mapping:**
   - Per-task address space = §5.3.1 (Memory Partitioning)
   - TTBR0 swap = §5.3.3 (Detailed Design — address space isolation)
   - Isolation test = §6.4.3 (Integration Testing — interference freedom)
   - ISO 26262 Part 6 §7.4.6 — Freedom from interference between software partitions

---

## Tổng kết chi phí

| Metric | Giá trị |
|--------|---------|
| File mới | 0 |
| File sửa | 7 (`linker.ld`, `mmu.rs`, `boot.s`, `sched.rs`, `main.rs`, `host_tests.rs`, boot test scripts) |
| Dòng code thêm | ~120 dòng kernel + ~100 dòng test |
| Bộ nhớ thêm | 36 KiB BSS (9 × 4KB page tables) |
| Tests mới | ~10 |
| Tổng tests sau Phase H | ~79 (69 cũ + 10 mới) |
| Risk | **Trung bình** — swap TTBR0 là thao tác nhạy cảm, sai = crash. Nhưng logic đơn giản (chỉ thay 3 user stack entries), test coverage cao. |

---

## Đề xuất hành động tiếp theo

1. **Bắt đầu H1** — Mở rộng `.page_tables` trong `linker.ld` lên 13 pages. Refactor `mmu::init()` build per-task tables. Sửa `boot.s` trỏ kernel boot L1. Verify QEMU boot bình thường.

2. **Tiếp H2** — Thêm `ttbr0: u64` vào Tcb. Gán `ttbr0` cho 3 tasks trong `kernel_main()`. Verify build + test cũ pass.

3. **H3 (critical)** — Swap TTBR0 trong `schedule()` + `bootstrap()`. Verify QEMU boot PING/PONG bình thường. **Đây là bước rủi ro cao nhất — test kỹ trên QEMU.**

4. **H4** — Test isolation: task cố đọc stack task khác → Data Abort → chứng minh cách ly hoạt động.

5. **H5** — Viết 10 unit tests. Verify ~79 tests pass.

6. **H6** — Cập nhật boot test scripts + verify CI green.

7. **Sau H6** — Viết blog #08 ("Mỗi nhà có hàng rào riêng — Per-Task Address Space"). Lên kế hoạch Phase I (ví dụ: Dynamic Memory Grant, Capability Delegation, hoặc Kernel Hardening).
