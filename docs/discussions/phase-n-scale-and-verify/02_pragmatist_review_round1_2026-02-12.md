# 🔧 Gemini-Pragmatist Review — Round 1 | Phase N: Scale & Verify

> **Ngày:** 2026-02-12
> **Agent:** Gemini-Pragmatist-Agent
> **Phương châm:** *Nếu nó không compile, nó không tồn tại. Nếu không có test fail, nó không broken. Nếu mất quá 2 ngày debug, thiết kế sai.*

---

## Q1: Scale strategy — Jump to 8 hay Incremental (3→4→8)?

### Khuyến nghị: **Option C modified — Parameterize với NUM_TASKS=3, validate, rồi flip to 8. Nhưng ĐỪNG tách thành 2 phase.**

Tôi đã đọc từng dòng code liên quan. Đây là reality check:

**Vấn đề thực sự KHÔNG phải là 3→8 hay 3→4→8.** Vấn đề là **13 named constants trong MMU** (`PT_L1_TASK0`, `PT_L1_TASK1`, `PT_L1_TASK2`... đến `PT_L3_TASK2`) phải trở thành computed indexing. Đây là refactor **structural**, không phụ thuộc vào giá trị NUM_TASKS.

Cụ thể, tôi đếm trong `arch/aarch64/mmu.rs`:

| Location | Current | Cần thay đổi |
|----------|---------|--------------|
| `PT_L2_DEVICE_0..2` (dòng 163-165) | 3 named constants | `task_id + 0 * NUM_TASKS` |
| `PT_L1_TASK0..2` (dòng 166-168) | 3 named constants | `task_id + 1 * NUM_TASKS` |
| `PT_L2_RAM_TASK0..2` (dòng 169-171) | 3 named constants | `task_id + 2 * NUM_TASKS` |
| `PT_L3_TASK0..2` (dòng 172-174) | 3 named constants | `task_id + 3 * NUM_TASKS` |
| Kernel tables (dòng 175-178) | 4 named constants | `4 * NUM_TASKS + offset` |
| `NUM_PAGE_TABLE_PAGES` | `= 16` (4×3 + 4) | `= 4 * NUM_TASKS + 4` |
| `mmu_init()` loops (dòng 379-395) | `for task in 0..3` | `for task in 0..NUM_TASKS` |
| `map_device_for_task()` (dòng 428) | `task_id >= 3` | `task_id >= NUM_TASKS` |
| `map_grant_for_task()` (dòng 490) | `PT_L3_TASK0 + task_id` | Computed |
| `set_page_attr()` (dòng 558) | `task_id >= 3` | `task_id >= NUM_TASKS` |
| Host stubs `src/mmu.rs` (dòng 151, 166) | `task_id >= 3` | `task_id >= NUM_TASKS` |

**Thêm 3 vị trí ngoài MMU:**
- `sched.rs` dòng 224: `next = 2; // default to idle` → `next = NUM_TASKS - 1;`
- `sched.rs` dòng 230: `next = 2;` (fallback idle) → `next = NUM_TASKS - 1;`
- `linker.ld`: `.task_stacks += 3 * 4096`, `.user_stacks += 3 * 4096`, `.page_tables += 16 * 4096`

**Tại sao Option C là đúng:**

1. **Parameterize trước, validate bằng test suite hiện tại (NUM_TASKS vẫn = 3).** Nếu 219 tests + 28 QEMU checkpoints vẫn pass → refactor đúng. Đây là safety net miễn phí.

2. **Flip to 8 là 4 thay đổi sau parameterize:**
   - `const NUM_TASKS: usize = 8;` (sched.rs)
   - `linker.ld`: `8 * 4096` cho stacks, `36 * 4096` cho page tables
   - `main.rs`: `sched::init()` chấp nhận TaskConfig array thay vì 3 positional args
   - Tests: update `assert_eq!(NUM_TASKS, 3)` → `assert_eq!(NUM_TASKS, 8)` + thêm tests cho task 3-7

3. **Option B (3→4→8) lãng phí:** MMU refactor effort là O(code structure), KHÔNG phải O(NUM_TASKS). Computed indexing `task_id + table_type * NUM_TASKS` hoạt động giống hệt cho 4 hay 8. Không có edge case nào ở 4 mà không xuất hiện ở 8.

**Tại sao ĐỪNG tách thành 2 phase:**
Parameterize + flip = cùng 1 PR. Không cần PR riêng cho "parameterize only". Reason: nếu merge parameterize nhưng chưa flip, code có hỗn hợp `NUM_TASKS` references và hardcoded `3` → confusing, không ai biết cái nào đã convert cái nào chưa. Commit history nên là:
- Commit 1: Parameterize all hardcoded 3 → NUM_TASKS (tests pass, NUM_TASKS=3)
- Commit 2: Flip NUM_TASKS=8 + linker update (tests updated + pass)
- Commit 3: New tests cho task 3-7

### Effort estimate & risk

| Task | Estimate plan | Estimate tôi | Risk |
|------|--------------|-------------|------|
| MMU computed indexing | "12-14h" | **6-8h** cho parameterize | 🟡 Medium — phải rất cẩn thận L1→L2→L3 chain |
| Linker.ld update | included | **1h** | 🟢 Low — arithmetic chỉ |
| sched.rs idle fallback | included | **0.5h** | 🟢 Low — 2 dòng |
| main.rs init refactor | "TaskConfig table" | **2-3h** | 🟡 Medium — xem Q5 |
| Host stubs update | included | **1h** | 🟢 Low |
| Flip to 8 | included | **1h** | 🟢 Low — nếu parameterize đúng |
| New tests for tasks 3-7 | included | **3-4h** | 🟡 Medium — phải cover scheduler, IPC, fault |
| QEMU validation | included | **2h** | 🟡 Medium — debug nếu page fault |
| **Tổng N1** | **12-14h** | **16-20h** | |

**Plan underestimates by ~5h.** Lý do:
- Plan không tính time viết tests mới cho 8-task scenarios
- Plan không tính QEMU debug time khi page tables sai (đây là nơi mất thời gian nhất — silent corruption hoặc data abort mà UART output mất)
- Plan không tính linker math verification (36 pages × 4096 = 144KB — phải đảm bảo không overlap với ELF load region at 0x4010_0000)

**Cái gì có thể sai:**
1. **Linker overlap:** Hiện tại kernel image kết thúc trước 0x4010_0000 với 16 pages. Với 36 pages (+80KB), `.page_tables` section lớn hơn đáng kể. Cần verify `.page_tables_end < __elf_load_start`. **Mitigation:** Thêm `ASSERT(. <= 0x40100000, "kernel too large")` vào linker.ld.
2. **ELF load region quá nhỏ:** 3×4096 = 12KB cho 1 ELF binary. Với 8 tasks, nếu muốn load nhiều ELF → cần mở rộng. **Nhưng:** hiện tại chỉ 1 ELF binary (user/hello) được load cho task 2. Các task 3-6 sẽ là kernel functions (như task 0, 1 hiện tại). Nên 12KB vẫn đủ cho phase N.
3. **ASID overflow:** Task ID + 1 = ASID. Với 8 tasks, ASID max = 9. AArch64 supports 8-bit ASID (0-255) hoặc 16-bit. Không vấn đề.

---

## Q2: KernelCell wrapping order — GRANTS→IRQ→ENDPOINTS→TCBS hay khác?

### Khuyến nghị: **GRANTS → IRQ_BINDINGS → ENDPOINTS → TCBS. Giữ nguyên plan. Đây là thứ tự đúng.**

Tôi đã đếm references thực tế (grep `unsafe` + tên biến):

| Global | File(s) | Unsafe refs | Complexity | Fields accessed |
|--------|---------|-------------|------------|-----------------|
| `GRANTS` | grant.rs, host_tests.rs | ~20 | Simple struct, 2 slots | owner, peer, phys_addr, active |
| `IRQ_BINDINGS` | irq.rs, host_tests.rs | ~25 | Simple struct, 8 slots | intid, task_id, active, pending_ack |
| `ENDPOINTS` | ipc.rs, host_tests.rs | ~30 | SenderQueue has methods | sender_queue, receiver |
| `TCBS` | sched.rs, ipc.rs, grant.rs, irq.rs, exception.rs, main.rs, host_tests.rs | **150+** | TrapFrame copy, scheduler | Mọi field |

**Lý do giữ nguyên thứ tự:**

1. **Experience-first > Risk-first.** Phase M đã wrap 4 scalars — team biết pattern. Nhưng wrapping **arrays** khác hoàn toàn: `GRANTS[i].field` trở thành `unsafe { (*GRANTS.get())[i].field }` hoặc `unsafe { GRANTS.get()[i].field }`. Syntax mới, cần muscle memory. GRANTS (20 refs) là nơi rẻ nhất để sai.

2. **TCBS cuối là ĐÚNG vì cascading dependencies.** TCBS được truy cập trong:
   - `sched.rs` (context switch — performance critical)
   - `ipc.rs` (qua `sched::get_task_reg`, `sched::set_task_reg`, `sched::TCBS[tid]`)
   - `irq.rs` (`sched::TCBS[tid].notify_pending`, `.notify_waiting`, `.state`, `.context.x[0]`)
   - `grant.rs` (qua `sched::NUM_TASKS`)
   - `main.rs` (caps, priority, ttbr0 assignment)
   - `host_tests.rs` (50+ direct `TCBS[i]` accesses)

   Nếu wrap TCBS trước và sai, **toàn bộ kernel break** — không chạy được cả scheduler. Không có fallback test nào work.

3. **Rollback cost analysis:**
   - GRANTS sai → chỉ grant tests fail (5 tests). Rollback: revert 1 file.
   - IRQ sai → IRQ tests fail (8 tests). Rollback: revert 1 file.
   - ENDPOINTS sai → IPC tests fail (15 tests). Rollback: revert 1 file.
   - TCBS sai → **mọi test fail** (150+ tests). Rollback: revert 6 files.

**Nhưng tôi có 1 correction cho plan:**

Plan nói mỗi sub-step là commit riêng. Tôi đề xuất **mỗi global wrap phải kèm theo:**
1. Sửa source file (e.g., `grant.rs`)
2. Sửa mọi caller (e.g., `host_tests.rs`)
3. Chạy full test suite
4. Commit

**KHÔNG commit source mà chưa sửa tests.** Partial state = broken build = team confusion.

### Effort estimate

| Step | Plan | Tôi | Notes |
|------|------|-----|-------|
| GRANTS wrap | 2-3h | **2h** | 20 refs, simple |
| IRQ wrap | 2-3h | **2h** | 25 refs, similar |
| ENDPOINTS wrap | 3-5h | **4h** | SenderQueue has internal state |
| TCBS wrap | 8-10h | **10-12h** | 150+ refs, 7 files, scheduler + IPC |
| **Tổng N2** | **16-21h** | **18-20h** | Plan is accurate here |

**Plan estimate cho N2 là tương đối chính xác.** Tôi đồng ý range 16-21h.

**Cái gì có thể sai:**
1. **TCBS wrap breaks `irq_route()`:** `irq.rs` trực tiếp access `sched::TCBS[tid].notify_pending |= bit` (dòng ~228). Sau wrap, cần `unsafe { (*sched::TCBS.get_mut())[tid].notify_pending |= bit }`. Dễ quên `get_mut()` ↔ `get()` distinction khi chỉ cần `|=`.
2. **`copy_nonoverlapping` trong scheduler:** `schedule()` dùng `core::ptr::copy_nonoverlapping` với raw pointer vào TCBS. Sau KernelCell wrap, pointer arithmetic thay đổi: `&mut TCBS[old].context` → `&mut (*TCBS.get_mut())[old].context`. Phải verify alignment vẫn đúng.
3. **`host_tests.rs` là nơi mất thời gian nhất.** 50+ direct TCBS accesses, mỗi cái phải thêm `unsafe { ... .get() ... }` hoặc `unsafe { ... .get_mut() ... }`. Monotonous, error-prone. Suggest: viết helper macro `fn tcb(i) -> &Tcb` và `fn tcb_mut(i) -> &mut Tcb` trong test utils.

---

## Q3: Kani pilot scope — 6 proofs đủ? Targets đúng?

### Khuyến nghị: **Thu hẹp xuống 4 proofs. Cut 2 cap.rs proofs, giữ 1 elf.rs với input 128 bytes, thêm 1 cho sched bounds.**

**Phân tích từng proof trong plan:**

#### cap.rs — Plan đề xuất 4 proofs

| Proof | Mục tiêu | Verdict tôi |
|-------|----------|-------------|
| `cap_for_syscall_returns_valid` | Return là subset of 0x3FFFF | ✅ GIỮ — nhưng fix property (plan viết "≤ 17" = SAI) |
| `cap_check_soundness` | `cap_check(caps, required)` đúng | ❌ CẮT — `(caps & required) == required` là 1 dòng boolean algebra. CBMC verify = overkill. Unit test đủ. |
| `has_capability_no_oob` | No OOB khi task < NUM_TASKS | ❌ CẮT — `has_capability()` KHÔNG TỒN TẠI trong code. Chỉ có `cap_check(caps, required)` nhận 2 u64, KHÔNG có array access → KHÔNG CÓ OOB risk. Plan có factual error ở đây. |
| `cap_for_syscall_unknown_zero` | Unknown syscall → returns 0 | ✅ GIỮ — hữu ích, verify exhaustive match |

**4 proofs cho cap.rs là overkill.** `cap.rs` chỉ 174 dòng, 2 functions thuần (pure, no side effects). `cap_check` là **1 dòng bitwise AND** — chứng minh formal cho `(a & b) == b` là academic exercise, không có ROI. `has_capability` không tồn tại.

**Giữ 2 proofs cho cap.rs:**
1. `cap_for_syscall_returns_valid_bitmask` — verify mọi (syscall_nr, ep_id) → result ⊆ 0x3FFFF
2. `cap_for_syscall_unknown_returns_zero` — verify syscall_nr ≥ 13 → result == 0

#### elf.rs — Plan đề xuất 1 proof

| Proof | Mục tiêu | Verdict |
|-------|----------|---------|
| `parse_elf64_no_panic` | No panic for any 4096-byte input | ⚠️ SỬA — giảm input xuống **128 bytes** |

**Vấn đề với 4096 bytes:**
- `parse_elf64` đọc header (64 bytes) rồi iterate program headers. CBMC phải symbolic-execute mọi path.
- Với 4096 bytes symbolic, state space ~2^32768. CBMC sẽ **timeout hoặc OOM** trước khi complete.
- Thực tế `e_phnum` tối đa = 4 (MAX_SEGMENTS check), `e_phentsize` thường = 56. Parser chỉ cần ~64 + 4×56 = 288 bytes.
- **128 bytes** đủ cover: 64B header + 1 program header (56B) + 8B padding. Covers mọi error path + happy path với 1 segment.

**Thêm Kani harness thực tế:**
```rust
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(6)] // max 4 segments + 2 loop iterations
fn parse_elf64_no_panic() {
    let data: [u8; 128] = kani::any();
    let _ = parse_elf64(&data); // must not panic
}
```

#### cell.rs — Plan đề xuất 1 proof

| Proof | Mục tiêu | Verdict |
|-------|----------|---------|
| `kernel_cell_no_ub` | get/get_mut không UB | ✅ GIỮ — nhưng clarify scope |

`KernelCell` là `repr(transparent)` wrapper quanh `UnsafeCell`. Kani proof hữu ích: verify rằng `get()` và `get_mut()` return valid references (non-null, aligned). Nhanh, rẻ, dễ viết.

#### Thêm: sched.rs bounds proof

**Tôi đề xuất thêm 1 proof mà plan thiếu:**

```rust
#[cfg(kani)]
#[kani::proof]
fn schedule_selects_valid_task() {
    // After schedule(), CURRENT < NUM_TASKS
    // This is THE critical safety invariant — OOB on TCBS = memory corruption
}
```

Đây là proof có giá trị cao nhất trong toàn bộ kernel. Nếu `*CURRENT.get() >= NUM_TASKS` sau `schedule()`, mọi subsequent TCBS access = undefined behavior. **Nhưng:** viết harness cho `schedule()` phức tạp vì cần setup toàn bộ TCBS state. Suggest dùng bounded model: kani::any() cho mỗi `TCBS[i].state` và `TCBS[i].priority`, verify `*CURRENT.get() < NUM_TASKS` after `schedule()`.

### Scope cuối cùng: 4 proofs

| # | Target | Property | Input bound | Effort |
|---|--------|----------|-------------|--------|
| 1 | `cap_for_syscall` | Return ⊆ 0x3FFFF | syscall: 0..u64, ep_id: 0..u64 | 1h |
| 2 | `cap_for_syscall` | Unknown → 0 | syscall ≥ 13 | 0.5h |
| 3 | `parse_elf64` | No panic | 128 bytes symbolic | 3h (tuning unwind) |
| 4 | `KernelCell` | get/get_mut valid | Scalar T | 1h |

### Effort estimate

| Task | Plan | Tôi |
|------|------|-----|
| Kani setup (Cargo.toml, CI) | 3-4h | **4-5h** (Windows toolchain issues likely) |
| 6 proofs | 6-9h | — |
| 4 proofs (adjusted) | — | **5-6h** |
| Debug/tune unwind bounds | included | **2-3h** (elf.rs sẽ cần tuning) |
| **Tổng N3** | **9-13h** | **11-14h** |

**Plan estimates hơi optimistic cho Kani setup.** Kani trên Windows + nightly 1.95.0 có thể cần workarounds. CBMC installation trên Windows != trivial. Suggest: Kani chỉ chạy trong CI (Docker), không yêu cầu local.

**Cái gì có thể sai:**
1. **Kani version incompatibility với nightly 1.95.0.** Kani thường lag behind nightly. Verify Kani supports exact nightly version trước khi bắt đầu.
2. **`parse_elf64` với 128 bytes vẫn chậm** nếu unwind bound sai. CBMC default unwind = unbounded → exponential. Phải explicit set `#[kani::unwind(6)]`.
3. **False positives từ unsafe trong KernelCell.** Kani sẽ flag `UnsafeCell::get()` dereference. Cần `kani::assume()` để model single-core invariant.

---

## Q4: N1-N2-N3 sequencing

### Khuyến nghị: **N1 → N2 → N3. Strictly sequential. KHÔNG parallel.**

**Dependency analysis thực tế:**

```
N1 (NUM_TASKS=8)
 ├── Changes: sched.rs, mmu.rs, linker.ld, main.rs, host_tests.rs
 ├── Output: NUM_TASKS=8, 36 page tables, 8 stacks, idle=task 7
 └── Tests: 219+ tests updated + new tests pass + QEMU boot

N2 (KernelCell wrap)
 ├── Depends on N1: TCBS type = [Tcb; 8] (không phải [Tcb; 3])
 ├── Changes: grant.rs, irq.rs, ipc.rs, sched.rs, host_tests.rs
 └── Tests: all tests pass with new access pattern

N3 (Kani proofs)
 ├── Depends on N1: cap_for_syscall properties reference NUM_TASKS?
 │   → KHÔNG. cap_for_syscall() không dùng NUM_TASKS. ✅ Independent
 ├── Depends on N2: KernelCell proof cần final KernelCell usage?
 │   → CÓ. Nếu viết proof trước wrap → proof targets sai API.
 └── Tests: kani verify pass in CI
```

**Tại sao KHÔNG parallel:**

1. **N3 setup (Kani install) CÓ THỂ parallel với N1.** Nhưng nếu N3 proofs reference code đang thay đổi bởi N1/N2 → merge conflicts + rework. Not worth it.

2. **N1 phải xong trước N2.** Lý do chết người: nếu wrap `TCBS: KernelCell<[Tcb; 3]>` trước (N2 first) rồi scale lên `KernelCell<[Tcb; 8]>` (N1 after) → phải touch **mọi KernelCell-wrapped access LẠI LẦN NỮA** để verify bounds vẫn đúng. Double effort.

3. **Cap.rs proofs THỰC SỰ independent.** `cap_for_syscall(syscall_nr, ep_id)` là pure function, không dùng NUM_TASKS, không dùng TCBS. Nhưng ROI của starting Kani setup early (khi N1 đang chạy) thấp: Kani setup = 4-5h, N1 = 16-20h. Kani developer sẽ idle waiting cho N1 trong 12-15h. Trong single-developer project → sequential.

**Optimal sequence:**

```
Week 1: N1 (16-20h)
  Day 1-2: Parameterize MMU, sched, stubs (NUM_TASKS=3 validate)
  Day 3: Flip to 8, update linker
  Day 4: New tests, QEMU validation

Week 2: N2 (18-20h)
  Day 5: GRANTS + IRQ wrap
  Day 6: ENDPOINTS wrap
  Day 7-8: TCBS wrap (biggest chunk)
  Day 9: Full test suite pass

Week 2-3: N3 (11-14h)
  Day 10: Kani setup + CI integration
  Day 11-12: Write + debug 4 proofs
  Day 12: Merge
```

**Tổng: ~45-54h thực tế.** Plan estimate 38-50h là **optimistic by ~5-10h**.

### Cái gì có thể sai:
1. **N1 MMU debug takes longer than expected.** Page fault trong QEMU virt = no stack trace, chỉ có ESR/FAR output. Debug bằng cách đọc hex = chậm. Tôi add 4h buffer.
2. **N2 TCBS wrap breaks QEMU boot.** Context switch path (`schedule()` → `copy_nonoverlapping`) là hot path. Nếu KernelCell wrapper thêm 1 layer dereference sai → silent corruption → task runs garbage code → random behavior. **Mitigation:** after TCBS wrap, chạy QEMU test 5 lần liên tục, verify output deterministic.

---

## Q5: TaskConfig table — Static const hay runtime?

### Khuyến nghị: **Hybrid nhưng KHÁC plan. Dùng `const` cho metadata, runtime cho entry points. NHƯNG đừng over-engineer.**

**Phân tích kỹ thuật:**

Plan đề xuất:
```rust
const TASK_CONFIGS: [TaskConfig; NUM_TASKS] = [ ... ];
```

Vấn đề #1: **Function pointers trong const.** Rust cho phép `fn_name as *const () as u64` trong const context? **CÓ**, kể từ Rust 1.63+ (const fn pointer casts). Ví dụ:

```rust
const UART_DRIVER_ENTRY_PTR: u64 = uart_driver_entry as *const () as u64;
```

**Compile trên nightly 1.95.0? Phải test.** Const evaluation của function pointer cast có thể khác nhau giữa const context và runtime. Trên `no_std` target, linker resolves address → const eval có thể dùng relocation, which WORKS. Nhưng nếu compiler refuses → fallback to runtime.

Vấn đề #2: **ELF entry point = runtime value.** `parse_elf64()` trả `info.entry` = giá trị đọc từ ELF binary at runtime. Không thể đặt vào const array.

Vấn đề #3: **Không phải tất cả 8 tasks đều active.** Task 3-6 có thể Inactive ban đầu. TaskConfig cần field `active: bool` hoặc `entry: Option<u64>`.

**Thiết kế thực tế:**

```rust
/// Static metadata — compiles to .rodata, zero runtime cost
pub struct TaskBaseConfig {
    pub caps: CapBits,
    pub priority: u8,
    pub time_budget: u64,        // 0 = unlimited
    pub heartbeat_interval: u64, // 0 = disabled
}

pub const TASK_BASE_CONFIGS: [TaskBaseConfig; NUM_TASKS] = [
    // Task 0: UART driver
    TaskBaseConfig { caps: CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | ...,
                     priority: 6, time_budget: 0, heartbeat_interval: 50 },
    // Task 1: client
    TaskBaseConfig { caps: CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | ...,
                     priority: 4, time_budget: 50, heartbeat_interval: 50 },
    // Task 2: ELF demo
    TaskBaseConfig { caps: CAP_YIELD | CAP_WRITE,
                     priority: 5, time_budget: 2, heartbeat_interval: 0 },
    // Tasks 3-6: inactive placeholders
    TaskBaseConfig { caps: CAP_NONE, priority: 0, time_budget: 0, heartbeat_interval: 0 },
    TaskBaseConfig { caps: CAP_NONE, priority: 0, time_budget: 0, heartbeat_interval: 0 },
    TaskBaseConfig { caps: CAP_NONE, priority: 0, time_budget: 0, heartbeat_interval: 0 },
    TaskBaseConfig { caps: CAP_NONE, priority: 0, time_budget: 0, heartbeat_interval: 0 },
    // Task 7: idle
    TaskBaseConfig { caps: CAP_YIELD, priority: 0, time_budget: 0, heartbeat_interval: 0 },
];
```

**Init loop trong kernel_main():**

```rust
// Apply base configs
for i in 0..NUM_TASKS {
    unsafe {
        TCBS[i].caps = TASK_BASE_CONFIGS[i].caps;
        TCBS[i].priority = TASK_BASE_CONFIGS[i].priority;
        TCBS[i].base_priority = TASK_BASE_CONFIGS[i].priority;
        TCBS[i].time_budget = TASK_BASE_CONFIGS[i].time_budget;
        TCBS[i].heartbeat_interval = TASK_BASE_CONFIGS[i].heartbeat_interval;
    }
}

// Runtime entry points — can't be const (ELF loading)
let entry_overrides: [(usize, u64); 3] = [
    (0, uart_driver_entry as *const () as u64),
    (1, client_entry as *const () as u64),
    (NUM_TASKS - 1, idle_entry as *const () as u64),
];
for (task_id, entry) in entry_overrides.iter() {
    // ... set entry point, state=Ready
}

// ELF-loaded task (runtime entry point)
if let Ok(info) = parse_elf64(USER_ELF) {
    // ... load and set TCBS[2].entry_point = info.entry
}
```

**Tại sao KHÔNG đặt function pointers trong const:**
1. Risk: compiler behavior trên custom target (`aarch64-aegis.json`) có thể khác standard targets. Not worth debugging.
2. ELF entry point CẦN runtime anyway → đã cần hybrid approach → tại sao force những cái không cần const vào const?
3. `const` array chỉ tiết kiệm ~256 bytes RAM (8 × 32B config). Kernel có 16KB stack + 12KB task stacks + 12KB user stacks + 64KB page tables. 256 bytes = không đáng kể.

### Effort estimate

| Approach | Effort | Risk |
|----------|--------|------|
| Plan: full const TaskConfig | 3-4h | 🟡 Function pointer const eval risk |
| Tôi: hybrid const metadata + runtime entry | **2-3h** | 🟢 Low — no const fn pointer issues |
| Alternative: just use a loop, no const | 1-2h | 🟢 Lowest — nhưng config scattered |

**Plan's approach is acceptable nhưng unnecessary.** Hybrid is simpler and avoids const fn pointer risk.

---

## Summary Table

| Question | Plan's Position | Tôi (Pragmatist) | Confidence | Delta vs Plan |
|----------|----------------|-------------------|------------|---------------|
| **Q1: Scale** | Jump 3→8 (12-14h) | **Option C**: Parameterize@3 → validate → flip to 8 (**16-20h**) | 🟢 90% | +4-6h, safer |
| **Q2: KernelCell order** | GRANTS→IRQ→EP→TCBS (16-21h) | **Same order** (**18-20h**) | 🟢 95% | Aligned |
| **Q3: Kani scope** | 6 proofs (9-13h) | **4 proofs**, cut 2 cap.rs, fix elf.rs input (**11-14h**) | 🟡 80% | Fewer proofs, similar time |
| **Q4: Sequencing** | N3 parallel with N1-N2 | **Strictly N1→N2→N3** | 🟢 90% | Sequential, no rework |
| **Q5: TaskConfig** | Full const array | **Hybrid**: const metadata + runtime entry (**2-3h**) | 🟢 85% | Simpler, less risk |

### Tổng effort

| | Plan | Pragmatist |
|--|------|-----------|
| N1 | 12-14h | **16-20h** |
| N2 | 16-21h | **18-20h** |
| N3 | 9-13h | **11-14h** |
| **Total** | **38-50h** | **45-54h** |

**Plan underestimates by ~15%.** Chủ yếu vì:
1. MMU debug time bị thiếu (~4h)
2. Test migration cho TCBS wrap bị thiếu (~2h)
3. Kani Windows/nightly setup issues (~2h)

### Critical Bugs Trong Plan Cần Fix Trước Khi Implement

| Bug | Severity | Location |
|-----|----------|----------|
| `has_capability()` không tồn tại — plan references phantom function | 🔴 Factual error | N3 Kani proof #3 |
| `cap_for_syscall` return "≤ 17" — sai, return là bitmask ⊆ 0x3FFFF | 🔴 Wrong property | N3 Kani proof #1 |
| Kani `elf.rs` 4096 bytes symbolic → likely timeout/OOM | 🟡 Performance | N3 Kani proof #5 |
| `next = 2` idle fallback chưa được list trong plan's changeset | 🟡 Missing item | N1 sched.rs changes |
| Linker overlap risk (36 pages near 0x4010_0000) chưa được address | 🟡 Latent risk | N1 linker.ld |

---

*"Hoàn hảo là kẻ thù của đủ tốt. Ship nó, test nó, fix nó."*
