# 🔭 GPT-Visionary Review — Round 1 | Phase N: Scale & Verify

> **Agent:** GPT-Visionary-Agent (Claude Opus 4.6)
> **Ngày:** 2026-02-12
> **Phạm vi:** Review 5 câu hỏi thiết kế cho Phase N (Scale & Verify) của AegisOS
> **Phương pháp:** Đọc toàn bộ source code liên quan (sched.rs 523 dòng, mmu.rs 594 dòng, cap.rs 174 dòng, elf.rs 348 dòng, ipc.rs 279 dòng, grant.rs 226 dòng, irq.rs 290 dòng, cell.rs ~80 dòng, main.rs 619 dòng, linker.ld 100 dòng), phân tích dependency graph, đánh giá rủi ro dựa trên mã thực tế.

---

## Q1: Chiến lược scale — Jump to 8 hay Incremental (3→4→8)?

### Khuyến nghị: **Option C — Parameterize trước, giữ NUM_TASKS=3, validate, rồi flip sang 8**

Đây là quyết định quan trọng nhất của Phase N, và tôi bất đồng với cả Option A lẫn Option B. Lý do:

### Phân tích rủi ro từ mã thực tế

Sau khi đọc toàn bộ codebase, tôi đếm được **ít nhất 23 locations** cần thay đổi khi scale NUM_TASKS, không chỉ 15+ như brief nêu:

| Category | Locations | Rủi ro |
|----------|-----------|--------|
| `sched.rs` — `TCBS[2]` hardcode idle | 4 chỗ (dòng 153-161, 210, 228-231) | 🔴 Logic — idle fallback sai = kernel hang |
| `sched::init()` — 3 positional args | 1 function signature + 1 call site | 🟡 API break |
| `mmu.rs` — 13 named constants `PT_L1_TASK0..2` | 13 constants + ~15 usage sites | 🔴 Highest risk — 594 dòng, L1→L2→L3 chain |
| `mmu.rs` — `task_id >= 3` hardcoded guard | 3 chỗ (`map_device_for_task`, `set_page_attr` ×2) | 🔴 Security — guard quá chặt = block task 3-7 |
| `mmu_init()` — `for task in 0..3` loops | 2 loops | 🟡 Functional |
| `linker.ld` — `3 * 4096` cho stacks | 2 sections (task_stacks, user_stacks) | 🔴 ABI — boot.s phụ thuộc stack layout |
| `linker.ld` — `16 * 4096` page tables | 1 section | 🔴 ABI — phải match NUM_PAGE_TABLE_PAGES |
| `main.rs` — caps/priority/ttbr0 assignment | 3 blocks × 3 tasks = 9 statements | 🟡 Boilerplate |
| `main.rs` — `sched::init(entry0, entry1, entry2)` | 1 call | 🟡 API break |
| ELF load region — `0x4010_0000`, 12 KiB cố định | linker.ld + main.rs | 🔴 **Chưa address** — 1 task duy nhất có thể ELF load |

**Nhận xét quan trọng:** MMU refactor là nơi bug ẩn dễ nhất. Hiện tại `build_l3()` nhận `owner_task: u8` và so sánh `stack_idx == owner_task as usize` — nếu ta có 8 tasks nhưng chỉ 3 L3 tables, user stack isolation sẽ sai. `NUM_PAGE_TABLE_PAGES` cần tăng từ 16 lên **4 + 4×8 = 36** (4 kernel + 4 per task × 8 tasks). Đây là **22,500 bytes RAM thêm** (36×4096 = 147,456 vs 16×4096 = 65,536) — delta 82 KiB, vẫn nhỏ so với 128 MiB RAM nhưng **linker layout thay đổi đáng kể**.

### Tại sao Option C?

1. **Tách biệt "parameterize" khỏi "scale"**: Bước 1 là biến mọi hardcoded `3` thành `NUM_TASKS`, mọi hardcoded `2` (idle) thành `NUM_TASKS - 1`, mọi named constant thành computed index — **nhưng giữ NUM_TASKS=3**. Bước này có thể validate bằng toàn bộ 219 tests + 28 QEMU checkpoints hiện tại mà KHÔNG thay đổi behavior.

2. **Phát hiện bug ở bước parameterize**: Nếu parameterize mà tests fail → bug do refactor logic, không phải do scale. Rất dễ bisect.

3. **Flip to 8 là trivial sau parameterize**: Chỉ cần `const NUM_TASKS: usize = 8;` + update linker constants. Nếu mọi thứ đều derived từ NUM_TASKS → 1-line change.

4. **Option B (3→4→8) không thêm giá trị**: 4 tasks vẫn cần refactor MMU y hệt 8 tasks. Chi phí refactor là O(code), không O(NUM_TASKS). Làm 2 lần = 2× effort cho cùng risk.

### 5-10-20 năm

| Horizon | Impact |
|---------|--------|
| **5 năm** | Parameterized NUM_TASKS cho phép **compile-time configuration** cho từng deployment (drone = 4 tasks, medical = 16 tasks). Đây là pattern chuẩn của safety-critical kernels (INTEGRITY RTOS, PikeOS). |
| **10 năm** | Khi thêm **dynamic task creation** (Phase O/P?), parameterized indexing là prerequisite. Array-based task table vẫn hoạt động với `MAX_TASKS` compile-time limit + `active_count` runtime counter — zero-heap. |
| **20 năm** | Multi-core SMP sẽ cần **per-core task queues** nhưng global TCBS array vẫn là source of truth. Parameterized indexing + KernelCell<[Tcb; N]> là foundation đúng cho lock-free per-core scheduler. |

### Ảnh hưởng tới Phase tương lai

- **Multi-core SMP**: Computed MMU indexing cho phép per-core page table sets mà không duplicate code.
- **Dynamic task creation**: `NUM_TASKS` thành `MAX_TASKS`, thêm `task_alloc()` trả index từ Inactive pool.
- **Certification (DO-178C)**: Refactor-then-validate approach tạo clear traceability — "parameterize" commit vs "scale" commit riêng biệt, auditor dễ review.

### Trade-offs tôi chấp nhận

- **2 commits thay vì 1**: Thêm ~1-2 giờ effort cho parameterize commit riêng. Chấp nhận vì: (a) bisectability vô giá khi debug MMU bug, (b) parameterize commit là independently reviewable artifact cho certification.
- **Không test intermediate 4-task configuration**: Mất coverage cho edge case "NUM_TASKS chẵn nhưng không phải power of 2". Chấp nhận vì: 8 = power of 2 → alignment tự nhiên, và 4 không thêm insight gì 8 chưa có.

---

## Q2: KernelCell wrapping order — GRANTS→IRQ→ENDPOINTS→TCBS hay đảo?

### Khuyến nghị: **Giữ nguyên plan — GRANTS → IRQ → ENDPOINTS → TCBS (experience-first)**

### Phân tích từ mã thực tế

Tôi đã đọc chi tiết access patterns của từng global:

| Global | Type | Access sites | Mutation patterns | Khó khăn khi wrap |
|--------|------|-------------|-------------------|-------------------|
| `GRANTS` | `[Grant; 2]` | ~8 refs | Index + field write | 🟢 Thấp — Grant nhỏ, Copy |
| `IRQ_BINDINGS` | `[IrqBinding; 8]` | ~8 refs | Index + field write | 🟢 Thấp — IrqBinding nhỏ, Copy |
| `ENDPOINTS` | `[Endpoint; 4]` | ~20+ refs | SenderQueue mutation, Option<usize> | 🟡 Trung bình — Endpoint KHÔNG derive Copy (SenderQueue chứa [usize; 4]) |
| `TCBS` | `[Tcb; 3/8]` | ~40+ refs | TrapFrame copy_nonoverlapping, field scatter | 🔴 Cao — Tcb 288+ bytes, dùng raw pointer ops |

**Key technical finding:** `TCBS` có pattern đặc biệt nguy hiểm — `copy_nonoverlapping` trực tiếp vào `&mut TCBS[idx].context`. Khi wrap trong KernelCell, mỗi access thành `TCBS.get_mut()[idx].context` — nhưng `get_mut()` trả `&mut [Tcb; N]`, nên `[idx]` vẫn hoạt động. Tuy nhiên, code hiện tại lấy raw pointer:

```rust
core::ptr::copy_nonoverlapping(
    frame as *const TrapFrame,
    &mut TCBS[old].context as *mut TrapFrame,  // ← direct field ref
    1,
);
```

Sau KernelCell:
```rust
core::ptr::copy_nonoverlapping(
    frame as *const TrapFrame,
    &mut TCBS.get_mut()[old].context as *mut TrapFrame,  // ← through get_mut()
    1,
);
```

Mỗi call site cần thêm `.get()` hoặc `.get_mut()` + `unsafe` block. TCBS có **~40+ call sites** (schedule, fault_current_task, restart_task, epoch_reset, watchdog_scan, set_task_state, get_task_reg, set_task_reg, save_frame, load_frame, bootstrap, + main.rs init). Sai 1 chỗ = kernel crash.

### Tại sao experience-first đúng

1. **GRANTS (2 slots, ~8 refs)**: Wrap trước, build intuition cho pattern `GRANTS.get()[id]` vs `GRANTS.get_mut()[id]`. Validate pattern works. Confidence: **5 phút debug nếu sai**.

2. **IRQ_BINDINGS (8 slots, ~8 refs)**: Tương tự GRANTS, slightly larger array. Validate KernelCell hoạt động với larger array. Confidence: **10 phút debug nếu sai**.

3. **ENDPOINTS (4 slots, ~20+ refs)**: Endpoint chứa `SenderQueue` với mutation methods (`push`, `pop`, `remove`). Đây là test case cho "KernelCell + complex inner types". Nếu pattern breaks ở đây, ta phát hiện TRƯỚC khi đụng TCBS.

4. **TCBS (3/8 slots, ~40+ refs)**: Cuối cùng, với full confidence. 40+ call sites là **mechanical refactor** — boring but correct. Nếu ta đã wrap 3 globals thành công, TCBS chỉ khác ở scale, không ở pattern.

### Tại sao KHÔNG wrap TCBS trước (risk-first)?

Argument "wrap TCBS trước vì critical nhất" nghe hấp dẫn nhưng sai logic:

- **TCBS critical = càng cần wrap ĐÚNG**. Wrap TCBS khi chưa có kinh nghiệm với KernelCell cho struct arrays → higher chance of subtle bug.
- **Rollback cost**: Nếu TCBS wrap bị bug → phải revert 40+ changes. Nếu GRANTS wrap bị bug → revert 8 changes.
- **Scheduler là hot path**: Bug ở KernelCell-wrapped TCBS có thể chỉ manifest dưới load (race window trong schedule()) — khó reproduce, khó debug. Bug ở GRANTS manifest ngay lần gọi đầu tiên.

### 5-10-20 năm

| Horizon | Impact |
|---------|--------|
| **5 năm** | KernelCell<[Tcb; N]> là foundation cho **per-task accessor API** — `fn with_task<R>(id: usize, f: impl FnOnce(&mut Tcb) -> R) -> R` encapsulate bounds check + unsafe. Giảm 40+ unsafe blocks xuống 1. |
| **10 năm** | Multi-core cần **per-core scheduler state** + **global task table**. KernelCell có thể evolve thành `PerCorCell<T>` (core-local) + `SpinCell<T>` (shared). Wrapping order không ảnh hưởng — pattern migration là mechanical. |
| **20 năm** | Formal verification tools (Verus, Creusot) sẽ cần **single access point** cho mỗi global — KernelCell là stepping stone tự nhiên sang verified accessor pattern. |

### Trade-offs tôi chấp nhận

- **TCBS wrapped cuối = sống với `static mut` TCBS lâu hơn**: Trong thời gian GRANTS→IRQ→ENDPOINTS đang được wrap, TCBS vẫn là `static mut`. Chấp nhận vì: (a) single-core model vẫn đúng, (b) tests vẫn pass, (c) 2-3 ngày delay không ảnh hưởng safety.
- **4 separate PRs thay vì 1 big-bang**: Mỗi global = 1 PR. Thêm review overhead nhưng mỗi PR independently verifiable. Chấp nhận vì: safety-critical culture demands atomic, reviewable changes.

---

## Q3: Kani pilot scope — 6 proofs đủ? Quá nhiều? Sai target?

### Khuyến nghị: **Giảm xuống 4-5 proofs, thay đổi target mix**

### Phân tích chi tiết

#### cap.rs: 4 proofs → giữ 3, bỏ 1

Plan đề xuất 4 proofs cho cap.rs. Sau khi đọc mã:

1. **`cap_for_syscall_returns_valid_subset`** — verify return ⊆ `0x3FFFF` (CAP_ALL). **GIỮ.** Đây là critical safety property: nếu `cap_for_syscall` trả bit ngoài range → capability bypass. Pure function, 13 match arms, bounded input (syscall 0-12 × ep 0-3) = Kani sẽ explore ~65 paths. **Chạy nhanh, ROI cao.**

2. **`cap_check_reflexive`** — `cap_check(x, x) == true` ∀ x. **GIỮ.** 1 dòng proof, validate bitwise AND logic. Trivial nhưng free — Kani solve instantly.

3. **`cap_check_monotone`** — nếu `cap_check(big, req)` và `small ⊆ big` → `cap_check(small, req)`. **BỎ.** Đây test bitwise AND associativity — property đúng by definition của `&` operator. Kani sẽ prove nó nhưng insight = zero. Thay bằng proof hữu ích hơn.

4. **`cap_for_syscall_unknown_returns_zero`** — syscall_nr > 12 → return 0. **GIỮ.** Đây verify default case — nếu match arm bị miss → unauthorized syscall gets non-zero cap = security hole.

**Kết luận cap.rs: 3 proofs.** Bỏ monotonicity (trivially true), giữ 3 proofs có safety relevance.

#### elf.rs: 1 proof 4096 bytes → thay bằng 128 bytes + targeted property

Brief đã flag: Kani trên `parse_elf64` với 4096 bytes symbolic input → CBMC phải explore $2^{32768}$ paths (4096 bytes × 8 bits). **Guaranteed timeout.**

Thay bằng:

1. **`parse_elf64_bounded_segments`** — Input 128 bytes symbolic, verify: nếu Ok(info) → `info.num_segments ≤ MAX_SEGMENTS` (= 4) VÀ `info.entry != 0`. **128 bytes đủ cho ELF header (64 bytes) + 1 program header (56 bytes) = 120 bytes.** Kani explore ~$2^{1024}$ paths — vẫn lớn nhưng bounded, estimate 5-15 phút với CBMC loop unrolling.

**Tại sao 128 chứ không phải 64?** 64 bytes chỉ cover header validation (TooSmall, BadMagic, Not64Bit...) — boring, chỉ verify early returns. 128 bytes cho phép verify segment parsing logic — nơi bug thực sự ẩn (SegmentOutOfBounds, overflow trong `checked_add`).

#### cell.rs: 1 proof → GIỮ nhưng đổi target property

Plan đề xuất verify KernelCell invariants. Nhưng KernelCell chỉ là thin wrapper quanh UnsafeCell — Kani không thể verify concurrency properties (single-core assumption là runtime property, không phải type-level).

**Đổi thành**: `kernelcell_get_roundtrip` — `KernelCell::new(v); get() == v` ∀ v: u64. Đây verify đúng 1 thing: wrapper không mangle data. Trivial nhưng đặt foundation — nếu Phase O thêm debug assertions vào KernelCell, proof này catch regression.

#### Thêm: sched.rs 1 proof

**`schedule_always_selects_valid_task`** — Kani verify: sau `schedule()`, `*CURRENT.get() < NUM_TASKS`. Đây là **critical safety invariant** — nếu CURRENT ≥ NUM_TASKS → index out of bounds → memory corruption.

**Tại sao sched.rs quan trọng hơn grant.rs?** Schedule chạy **mỗi timer tick** (~100 lần/giây). Grant create/revoke chạy hiếm khi. Bug frequency exposure: schedule >> grant.

**Challenge:** `schedule()` đọc/ghi TCBS (struct array) + CURRENT — Kani cần model mutable statics. Có thể cần `#[cfg(kani)]` abstraction. Ước lượng thêm ~3-4 giờ effort, nhưng ROI rất cao.

### Đề xuất cuối cùng: 5 proofs

| # | Module | Proof | Input | Estimate | ROI |
|---|--------|-------|-------|----------|-----|
| 1 | cap.rs | `cap_for_syscall_returns_valid_subset` | syscall×ep symbolic | 1-2 min | 🟢 High |
| 2 | cap.rs | `cap_check_reflexive` | u64 symbolic | <1 min | 🟢 Free |
| 3 | cap.rs | `cap_for_syscall_unknown_returns_zero` | u64 symbolic | <1 min | 🟢 High |
| 4 | elf.rs | `parse_elf64_bounded_segments` | 128 bytes symbolic | 5-15 min | 🟡 Medium |
| 5 | cell.rs | `kernelcell_get_roundtrip` | u64 symbolic | <1 min | 🟢 Foundation |

**Bỏ:** cap_check_monotone (trivial), parse_elf64 4096 bytes (timeout).
**Hoãn sang Phase O:** sched.rs proof (cần KernelCell wrap xong trước, effort cao).

### 5-10-20 năm

| Horizon | Impact |
|---------|--------|
| **5 năm** | 5 proofs tạo **Kani infrastructure** (CI job, harness patterns, cfg(kani) gates) — chi phí setup 1 lần, mỗi proof thêm sau chỉ ~1-2 giờ. Infrastructure > individual proofs. |
| **10 năm** | Kani sẽ mature thêm (loop contracts, function contracts, concurrency support). Proofs viết hôm nay sẽ **tự động mạnh hơn** khi Kani tool evolves — investment compounds. |
| **20 năm** | DO-178C DAL A yêu cầu **MC/DC + formal methods coverage**. 5 Kani proofs = pilot evidence cho **DO-333 Tool Qualification** — không phải certification artifact trực tiếp nhưng là stepping stone cho full formal campaign trong Phase P/Q. |

### Trade-offs tôi chấp nhận

- **Không có sched.rs proof trong Phase N**: Schedule là highest-value target nhưng highest-effort. Chấp nhận hoãn vì: (a) N2 sẽ wrap TCBS cuối cùng — verify sau wrap sạch hơn, (b) 219 runtime tests đã cover schedule logic, (c) Kani infra setup là bottleneck, không phải proof count.
- **128 bytes cho ELF thay vì full-file**: Bỏ lỡ bugs ở segments xa trong file. Chấp nhận vì: (a) `checked_add` đã có overflow protection, (b) timeout = zero value, (c) có thể tăng dần lên 256, 512 bytes trong Phase O khi biết Kani performance limits.
- **Chỉ 5 proofs, không 6**: Thà 5 proofs chạy xanh CI ổn định hơn 6 proofs với 1 flaky timeout. CI flake = trust erosion = team bỏ qua red CI = safety gap.

---

## Q4: N1-N2-N3 sequencing — Parallel hay Sequential?

### Khuyến nghị: **N1 → N3a (Kani setup) → N2 → N3b (proofs), với overlap**

### Dependency analysis

```
N1 (scale NUM_TASKS)
 ├── Thay đổi: sched.rs, mmu.rs, linker.ld, main.rs
 ├── Output: NUM_TASKS=8 compiles + boots + 219+ tests pass
 └── Duration: ~12-14h

N2 (KernelCell wrapping)
 ├── Phụ thuộc: N1 (vì TCBS size thay đổi 3→8 — wrap [Tcb; 8] chứ không [Tcb; 3])
 ├── Thay đổi: grant.rs, irq.rs, ipc.rs, sched.rs, host_tests.rs
 ├── Output: 0 static mut remaining
 └── Duration: ~16-21h

N3a (Kani infrastructure)
 ├── KHÔNG phụ thuộc N1 hay N2 — setup cargo-kani, CI job, cfg(kani) gates
 ├── Output: `cargo kani` runs, empty harness passes
 └── Duration: ~3-4h

N3b (cap.rs proofs)
 ├── KHÔNG phụ thuộc N1 — cap.rs không dùng NUM_TASKS
 ├── KHÔNG phụ thuộc N2 — cap.rs không dùng static mut
 ├── Output: 3 proofs pass
 └── Duration: ~3-4h

N3c (elf.rs proof)
 ├── KHÔNG phụ thuộc N1 — elf.rs parser là pure function
 ├── KHÔNG phụ thuộc N2
 ├── Output: 1 proof pass
 └── Duration: ~3-4h

N3d (cell.rs proof)
 ├── KHÔNG phụ thuộc N1
 ├── CÓ THỂ benefit từ N2 (nếu N2 thêm methods vào KernelCell)
 ├── Output: 1 proof pass
 └── Duration: ~1-2h
```

### Optimal execution order

```
Week 1:
  Day 1-2: N1 (parameterize + scale to 8)
  Day 1:   N3a (Kani setup, parallel with N1)
  Day 2-3: N3b + N3c (cap.rs + elf.rs proofs, parallel with tail of N1)

Week 2:
  Day 4-6: N2 (GRANTS → IRQ → ENDPOINTS → TCBS)
  Day 6:   N3d (cell.rs proof, after N2 stabilizes KernelCell)
  Day 7:   Integration test — full CI green
```

### Tại sao N1 trước N2?

**Critical dependency:** N2 sẽ wrap `TCBS` — nhưng TCBS type là `[Tcb; NUM_TASKS]`. Nếu ta wrap `[Tcb; 3]` trước (N2 first) rồi scale lên `[Tcb; 8]` (N1 after) → phải touch tất cả KernelCell-wrapped code LẠI LẦN NỮA.

Ngược lại, nếu N1 first → TCBS đã là `[Tcb; 8]` → N2 wrap đúng type 1 lần.

### Tại sao N3 không truly parallel với N1?

Brief nói "N3 independent of N1/N2" — **đúng cho N3a/N3b/N3c** (Kani infra + cap.rs + elf.rs proofs). Nhưng:

- **N3 cần human attention**: Kani có learning curve (bolero vs cargo-kani, CBMC flags, loop unrolling). Nếu same person làm N1 + N3 → context switch penalty.
- **Nếu có 2 người**: N1 + N3a/N3b/N3c hoàn toàn parallel. Cap.rs và elf.rs không touch bởi N1.
- **Nếu 1 người**: N1 trước (vì blocking), N3a ngay sau (setup tooling while brain rests from MMU refactor), N2 next (bulk of work), N3b-d cuối (proofs khi code stable).

### 5-10-20 năm

| Horizon | Impact |
|---------|--------|
| **5 năm** | N1-before-N2 ordering tạo precedent: **scale first, encapsulate second**. Đây là đúng thứ tự cho mọi future feature — thêm functionality trước, harden sau. Ngược lại (harden trước) = rework. |
| **10 năm** | Kani CI job (N3a) chạy trên mỗi PR từ Phase N trở đi. Over 10 years = thousands of regression checks. Setup cost amortized to near-zero. |
| **20 năm** | Sequencing discipline = **configuration management maturity**. ISO 26262 ASIL D đánh giá "process evidence" — documented rationale cho execution order = audit asset. |

### Trade-offs tôi chấp nhận

- **N3 bắt đầu muộn 1-2 ngày**: Kani proofs không chạy CI trong sprint đầu. Chấp nhận vì: N3a setup có thể song song, và cap.rs/elf.rs proofs không bị ảnh hưởng bởi N1 changes.
- **Integration risk cuối sprint**: N1+N2+N3 merge cùng lúc cuối week 2 → integration test dồn. Mitigate: mỗi sub-phase merge riêng vào main, N2 merge per-global (4 PRs), N3 merge per-proof.

---

## Q5: TaskConfig table — Static const array hay runtime init?

### Khuyến nghị: **Hybrid — `const BASE_CONFIGS: [TaskBaseConfig; NUM_TASKS]` + runtime entry point assignment**

### Phân tích kỹ thuật

#### Function pointers trong const context

Rust cho phép function pointers trong `const`:

```rust
const fn entry_as_u64(f: unsafe extern "C" fn() -> !) -> u64 {
    f as *const () as u64  // ❌ NOT const-evaluable in Rust 2021
}
```

**Thực tế:** `fn() as u64` KHÔNG phải const expression trong Rust stable. Reason: function addresses chỉ biết tại link time, không phải compile time. Compiler cần relocation.

Tuy nhiên, ta có thể:
```rust
const TASK_CONFIGS: [TaskConfig; 8] = [
    TaskConfig { entry: uart_driver_entry as *const () as u64, ... },  // ❌ not const
    ...
];
```

Trên nightly Rust (`const_fn_fn_ptr_basics`), điều này có thể hoạt động. Nhưng AegisOS dùng stable features on nightly toolchain — không nên phụ thuộc unstable features cho safety-critical code.

#### ELF-loaded entries là runtime

Task 2 hiện tại có entry point từ ELF parsing:
```rust
// main.rs dòng 531-535
if let Ok(entry) = result {
    sched::TCBS[2].entry_point = entry;
    sched::TCBS[2].context.elr_el1 = entry;
}
```

`entry` là runtime value — phụ thuộc nội dung ELF binary. Không thể đặt trong const.

#### Đề xuất thiết kế: Split const + runtime

```rust
/// Compile-time configuration — everything that doesn't need runtime resolution
pub struct TaskBaseConfig {
    pub caps: CapBits,
    pub priority: u8,
    pub time_budget: u64,      // 0 = unlimited
    pub heartbeat_interval: u64, // 0 = disabled
    pub is_elf_loaded: bool,   // true = entry set at runtime from ELF
}

pub const TASK_BASE_CONFIGS: [TaskBaseConfig; NUM_TASKS] = [
    // Task 0: UART driver
    TaskBaseConfig {
        caps: CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
            | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
            | CAP_IRQ_BIND | CAP_IRQ_ACK | CAP_DEVICE_MAP | CAP_HEARTBEAT,
        priority: 6,
        time_budget: 0,
        heartbeat_interval: 50,
        is_elf_loaded: false,
    },
    // Task 1: Client
    TaskBaseConfig {
        caps: CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
            | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
            | CAP_HEARTBEAT,
        priority: 4,
        time_budget: 50,
        heartbeat_interval: 50,
        is_elf_loaded: false,
    },
    // Task 2: ELF demo
    TaskBaseConfig {
        caps: CAP_YIELD | CAP_WRITE,
        priority: 5,
        time_budget: 2,
        heartbeat_interval: 0,
        is_elf_loaded: true, // entry from ELF parsing
    },
    // Tasks 3-6: idle/reserved
    TaskBaseConfig {
        caps: CAP_YIELD,
        priority: 0,
        time_budget: 0,
        heartbeat_interval: 0,
        is_elf_loaded: false,
    },
    // ... repeat for 4-6
    // Task 7 (NUM_TASKS-1): dedicated idle
    TaskBaseConfig {
        caps: CAP_YIELD,
        priority: 0,
        time_budget: 0,
        heartbeat_interval: 0,
        is_elf_loaded: false,
    },
];
```

Runtime init trong `kernel_main()`:
```rust
// Apply base configs
for i in 0..NUM_TASKS {
    let cfg = &TASK_BASE_CONFIGS[i];
    TCBS[i].caps = cfg.caps;
    TCBS[i].priority = cfg.priority;
    TCBS[i].base_priority = cfg.priority;
    TCBS[i].time_budget = cfg.time_budget;
}

// Runtime entry points (non-ELF tasks)
let entries: [(usize, u64); 3] = [
    (0, uart_driver_entry as *const () as u64),
    (1, client_entry as *const () as u64),
    (NUM_TASKS - 1, idle_entry as *const () as u64),
];
for (id, entry) in &entries {
    TCBS[*id].entry_point = *entry;
    TCBS[*id].context.elr_el1 = *entry;
}

// ELF-loaded tasks get entry later (after parse_elf64)
```

### Tại sao hybrid?

1. **Const correctness cho safety properties**: Capabilities và priorities là **design-time decisions** — chúng KHÔNG BAO GIỜ thay đổi tại runtime (trừ priority inheritance, nhưng đó dùng `base_priority` để restore). Đặt trong `const` = compiler verify, Kani can reason about, auditor can inspect without running code.

2. **Runtime flexibility cho entry points**: Function pointer addresses là linker artifact. ELF entry là parse artifact. Cả hai chỉ biết tại runtime. Forcing vào const = fighting the language.

3. **Single source of truth**: Hiện tại caps/priority/budget nằm rải rác trong `kernel_main()` — 9+ statements, dễ miss 1 task. `TASK_BASE_CONFIGS` là **declarative** — nhìn 1 array thấy toàn bộ task policy.

4. **Thêm task = thêm 1 entry vào array**: Không cần tìm 3 chỗ khác nhau trong main.rs. Đây là scalability goal chính của N1.

### Vấn đề ELF load region cho multi-task

**Brief đã flag nhưng plan chưa address:** Hiện tại chỉ có 1 ELF load region (`0x4010_0000`, 12 KiB). Nếu muốn >1 ELF-loaded task, cần:

- Option A: Mở rộng ELF load region (e.g., 8 × 12 KiB = 96 KiB tại `0x4010_0000`)
- Option B: Mỗi ELF-loaded task có region riêng (per-task linker section)
- Option C: **Hoãn** — Phase N chỉ support 1 ELF task (task 2), thêm multi-ELF trong Phase O

**Khuyến nghị:** Option C — scope creep là kẻ thù lớn nhất của safety-critical projects. Phase N đã có 3 sub-phases. Thêm multi-ELF loading = thêm linker changes + MMU mapping + address space collision resolution. **Ghi TODO, không làm.**

### 5-10-20 năm

| Horizon | Impact |
|---------|--------|
| **5 năm** | `TaskBaseConfig` evolve thành **task manifest format** — mỗi task có TOML/binary manifest mô tả capabilities, resources, memory budget. Const array là prototype cho manifest parser. |
| **10 năm** | Dynamic task creation sẽ cần `TaskBaseConfig` tại runtime — nhưng const array vẫn là **template library**. `task_create(template_id)` lookup config từ const table + override entry point. Zero-heap, no parsing. |
| **20 năm** | Certification yêu cầu **statically analyzable task configuration** — const array + Kani proof "mọi task config hợp lệ" = evidence artifact. Runtime-only config = phải test mọi combination dynamically. |

### Trade-offs tôi chấp nhận

- **2 concepts thay vì 1**: `TaskBaseConfig` (const) + runtime entry assignment (imperative). Phức tạp hơn single `TaskConfig` struct. Chấp nhận vì: type system enforces đúng boundary — "policy" (const) vs "mechanism" (runtime).
- **Heartbeat interval trong const nhưng thực tế set qua syscall**: Task 0 và 1 gọi `syscall_heartbeat(50)` tại runtime — giá trị trong config có thể khác giá trị task tự set. Chấp nhận vì: config value là **initial/default**, task có thể override qua syscall — đây là intended behavior, không phải bug.
- **Chưa giải quyết multi-ELF**: Tasks 3-6 sẽ là idle stubs trong Phase N. Wasted 4 TCBs + 4 sets page tables = ~80 KiB RAM. Chấp nhận vì: RAM abundant (128 MiB), và idle tasks có near-zero runtime cost (wfi loop).

---

## Summary

| # | Câu hỏi | Khuyến nghị | Confidence | Key reason |
|---|---------|-------------|------------|------------|
| Q1 | Scale strategy | **Option C**: Parameterize (NUM_TASKS=3) → validate → flip to 8 | 🟢 95% | Tách refactor risk khỏi scale risk; 219 tests validate intermediate state |
| Q2 | KernelCell order | **GRANTS→IRQ→ENDPOINTS→TCBS** (giữ nguyên plan) | 🟢 90% | Experience-first giảm risk cho TCBS (40+ call sites); rollback cost thấp cho globals đơn giản |
| Q3 | Kani scope | **5 proofs** (3 cap.rs + 1 elf.rs@128B + 1 cell.rs), bỏ monotonicity, hoãn sched.rs | 🟡 80% | Infrastructure > proof count; 128B ELF tránh timeout; sched.rs cần N2 xong trước |
| Q4 | Sequencing | **N1 → N3a (parallel) → N2 → N3b-d** | 🟢 90% | N1 phải trước N2 (TCBS type dependency); N3a independent; proofs cuối khi code stable |
| Q5 | TaskConfig | **Hybrid**: const `TaskBaseConfig` (caps/priority/budget) + runtime entry points | 🟢 85% | Function pointers không const-evaluable; ELF entry là runtime; declarative config = single source of truth |

### Cross-cutting concerns

1. **Multi-ELF loading**: Chưa address trong Phase N. Ghi TODO cho Phase O. Không block N1-N3.
2. **MMU refactor risk**: Highest risk item. Khuyến nghị: viết MMU unit tests TRƯỚC refactor — test `table_ptr_for_task(task_id)` returns correct addresses cho task 0-7. Regression gate.
3. **Test count target**: 219 → ≥240 (thêm ~20 tests cho 8-task scenarios). QEMU checkpoints: 28 → ≥32 (thêm multi-task boot, idle fallback, task 7 behavior).
4. **Linker script là bottleneck ẩn**: `linker.ld` cần 3 changes (task_stacks, user_stacks, page_tables sizes). Mỗi change = reboot test. Khuyến nghị: parameterize linker bằng preprocessing script hoặc chấp nhận manual update với documented formula.

### Một câu hỏi plan chưa đặt ra

**Idle task semantics khi NUM_TASKS=8:** Hiện tại idle = task 2 (last task). Sau scale, idle = task 7. Nhưng nếu tương lai thêm dynamic task creation, task 7 có thể được reuse. **Khuyến nghị:** Idle task nên có `IDLE_TASK_ID` constant riêng, không phải `NUM_TASKS - 1`. Giá trị ban đầu = 7, nhưng decoupled từ NUM_TASKS. Lý do: khi dynamic creation, `NUM_TASKS` thành `MAX_TASKS` nhưng idle task vẫn ở fixed slot.

---

> *"The purpose of a pilot is not to prove the system works — it is to discover how it fails, so the next iteration can be designed around those failures."*
>
> — GPT-Visionary-Agent, Phase N Review Round 1
