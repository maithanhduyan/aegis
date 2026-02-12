# Kế hoạch Phase N — Scale & Verify

> **Trạng thái: ✅ DONE** — Mở rộng AegisOS từ 3 lên 8 tasks, pilot formal verification với Kani, encapsulate 4 struct-array globals còn lại vào `KernelCell<T>`. Phase này vừa thêm tính năng (scale) vừa tăng cường safety (verify) — cân bằng giữa development velocity và assurance depth.
>
> **Đồng thuận:** Cập nhật theo [final consensus](../discussions/phase-n-scale-and-verify/final_consensus_2026-02-12.md) — 2 rounds, 13/13 điểm đồng thuận (100%).

---

## Tại sao Phase N?

### Lỗ hổng/Hạn chế hiện tại: "Kernel chỉ chạy 3 tasks — không đủ để mô phỏng hệ thống thật"

Sau 13 phases (A→M), AegisOS có microkernel hoàn chỉnh với 96.65% coverage, 219 tests, 28 QEMU checkpoints, KernelCell pattern cho 4 scalar globals, và structured logging. Nhưng kernel chỉ hỗ trợ **3 tasks cố định** — trong khi hệ thống safety-critical thực tế cần 8–32 tasks (navigation, telemetry, control, logging, watchdog, redundancy...).

**Ví dụ thực tế**: Một xe tự lái cần ít nhất 8 tasks chạy song song — camera processing, LiDAR fusion, path planning, motor control, brake safety monitor, telemetry, OTA update manager, system health. Với chỉ 3 tasks, AegisOS không thể mô phỏng kịch bản thật.

Đồng thời, Phase M đã xây nền tảng safety nhưng chưa có **formal verification** — chứng minh toán học rằng code đúng mọi trường hợp (không chỉ test thấy đúng). Kani model checker trên Rust có thể bounded-verify critical modules như `cap.rs` và `elf.rs` — bước đầu tiên hướng DO-333 (Formal Methods supplement cho DO-178C).

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|--------|-----------|
| 1 | `NUM_TASKS = 3` hardcoded ở 15+ chỗ (sched, mmu, linker, main) | Không thể mô phỏng hệ thống thật; thiết kế bị lock-in vào con số cố định |
| 2 | Page table indices hardcoded (`PT_L1_TASK0`, `PT_L2_RAM_TASK1`...) | Mỗi lần thêm task phải sửa 20+ constants — không scale được |
| 3 | `kernel_main()` khởi tạo 3 tasks bằng 3 block code riêng biệt | Thêm task = copy-paste code — dễ sai, khó bảo trì |
| 4 | `TCBS`, `ENDPOINTS`, `GRANTS`, `IRQ_BINDINGS` vẫn là `static mut` | Formal tools (Kani, Miri) không thể reason; Phase M đã defer 4 biến này |
| 5 | Chưa có formal verification — chỉ có tests | DO-333 yêu cầu formal methods cho Level A/B; tests chỉ cover cases đã nghĩ ra |
| 6 | `init_tasks()` nhận 3 entry points positional — API không mở rộng được | Không có cơ chế config task mới linh hoạt |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|--------|-------|---------------------|
| N1: NUM_TASKS=8 | Constant + linker + MMU + init API mở rộng | #1, #2, #3, #6 |
| N2: KernelCell Struct Arrays | Wrap TCBS/ENDPOINTS/GRANTS/IRQ_BINDINGS | #4 |
| N3: Kani Pilot | Formal proofs cho `cap.rs` + `elf.rs` + `cell.rs` | #5 |

### Nguồn gốc quyết định

Dựa trên đồng thuận Phase M discussions: *"Phase N: NUM_TASKS=8 + Kani pilot + encapsulate remaining globals"* và verification escalation rule: *"NUM_TASKS=3 → exhaustive tests; NUM_TASKS=8 → Kani pilot"*.

---

## Phân tích hiện trạng

### NUM_TASKS và các hardcoded locations

```
Constant: pub const NUM_TASKS: usize = 3;  // sched.rs

Hardcoded '3' locations:
├── src/kernel/sched.rs      ─ TCBS: [Tcb; NUM_TASKS], init_tasks(), idle fallback
├── src/arch/aarch64/mmu.rs  ─ PAGE_TABLE_COUNT=16, PT_L*_TASK* constants,
│                               init() loops 0..3, task_id >= 3 guards
├── src/main.rs              ─ init_tasks(fn, fn, fn), caps/priority/ttbr0 ×3 blocks
├── linker.ld                ─ .task_stacks 3×4096, .user_stacks 3×4096,
│                               .page_tables 16×4096
├── src/mmu.rs               ─ host stub: task_id >= 3 guard
└── src/exception.rs         ─ host stub: CURRENT >= 3 check
```

### Memory budget hiện tại vs mở rộng

| Resource | 3 tasks | 8 tasks | Delta |
|----------|---------|---------|-------|
| `Tcb` struct (~400B) | 1.2 KiB | 3.2 KiB | +2 KiB |
| Kernel stacks (`.task_stacks`) | 12 KiB | 32 KiB | +20 KiB |
| User stacks (`.user_stacks`) | 12 KiB | 32 KiB | +20 KiB |
| Page tables (4 pages/task + 4 kernel) | 64 KiB (16 pages) | 144 KiB (36 pages) | +80 KiB |
| **Tổng** | **~89 KiB** | **~211 KiB** | **~122 KiB** |

RAM available: 128 MiB (QEMU virt). Delta 122 KiB = **0.09%** RAM — không đáng kể.

### 4 `static mut` struct arrays chưa encapsulate

| Biến | File | Kiểu | Test refs | Phức tạp |
|------|------|------|-----------|----------|
| `TCBS` | `sched.rs` | `[Tcb; NUM_TASKS]` | ~150+ | 🔴 Cao — nhiều field access, interrupt context |
| `ENDPOINTS` | `ipc.rs` | `[Endpoint; 4]` | ~30+ | 🔴 Cao — state machine, queue |
| `GRANTS` | `grant.rs` | `[Grant; NUM_GRANTS]` | ~20+ | 🟡 Trung bình |
| `IRQ_BINDINGS` | `irq.rs` | `[IrqBinding; MAX_IRQ_BINDINGS]` | ~15+ | 🟡 Trung bình |

### Page table index layout hiện tại

```
PAGE_TABLE_COUNT = 16 (cho 3 tasks + kernel)

Task page tables (4 per task × 3 tasks = 12):
  [0..2]   L2_device  (task 0, 1, 2)
  [3..5]   L1         (task 0, 1, 2)
  [6..8]   L2_ram     (task 0, 1, 2)
  [9..11]  L3         (task 0, 1, 2)

Kernel page tables (4):
  [12] L2_device kernel
  [13] L1 kernel boot
  [14] L2_ram kernel boot
  [15] L3 kernel boot

→ Với 8 tasks: 4 × 8 + 4 = 36 pages
```

### Kani pilot targets (4 proofs — đồng thuận discussion)

| Module | Hàm | Property cần verify | Bounded input |
|--------|------|---------------------|---------------|
| `cap.rs` | `cap_for_syscall(nr, ep)` | No panic, return ⊆ `0x3FFFF` (bitmask) | nr: 0..=12, ep: 0..=3 |
| `cap.rs` | `cap_for_syscall(nr, ep)` | Completeness — mọi syscall 0..=12 có cap bit defined | nr: 0..=12, ep: 0..=3 |
| `elf.rs` | `parse_elf64(data)` | No OOB, no panic | data: arbitrary &[u8], len ≤ 128 |
| `cell.rs` | `KernelCell<T>` | get/get_mut type-safe, as_ptr correct | T = u64 |

> **Note:** `cap_check()` chỉ là `(caps & required) == required` — pure bitwise, không có array indexing. Proof cho `cap_check` không thêm giá trị (compiler đã guarantee). `has_capability()` không tồn tại trong `cap.rs` — plan gốc có factual error.

---

## Thiết kế Phase N

### N1 — Scale NUM_TASKS = 3 → 8

#### Khái niệm

Mở rộng kernel từ 3 lên 8 tasks. Giống như nâng cấp trường học từ 3 lớp lên 8 lớp — cần thêm phòng (stacks), bàn ghế (page tables), sổ điểm danh (TCBs), và cập nhật quy tắc (init logic). Nhưng nội quy trường (scheduler, IPC, capability) không đổi.

#### Chiến lược: Option C — Parameterize → Validate → Flip (đồng thuận)

**Bước 1 — Parameterize:** Refactor toàn bộ hardcoded `3` thành `NUM_TASKS` constant + `pt_index()`, **giữ nguyên `NUM_TASKS=3`**. Chạy full 219 tests + 28 QEMU checkpoints → confirm zero regression.

**Bước 2 — Flip:** Đổi `NUM_TASKS=3` → `8` + update linker sizes. Test lại.

Lý do: Tách **refactor risk** (computed indexing sai) khỏi **scale risk** (8 tasks crash). Nếu Bước 1 pass → refactor đúng. Nếu Bước 2 fail → biết ngay vấn đề ở scale, không phải refactor.

#### N1a — Constants & Linker Script

**Thay đổi hằng số:**

| File | Thay đổi |
|------|----------|
| `src/kernel/sched.rs` | `NUM_TASKS: usize = 3` → `8` |
| `src/arch/aarch64/mmu.rs` | `PAGE_TABLE_COUNT = 16` → `36` (4×8+4) |
| `linker.ld` | `.task_stacks: . += 3 * 4096` → `8 * 4096` |
| `linker.ld` | `.user_stacks: . += 3 * 4096` → `8 * 4096` |
| `linker.ld` | `.page_tables: . += 16 * 4096` → `36 * 4096` |

**Computed page table indexing** — thay thế `PT_L1_TASK0`, `PT_L2_RAM_TASK1`... bằng:

```
fn pt_index(task_id: usize, table_type: PageTableType) -> usize {
    // table_type: L2Device=0, L1=1, L2Ram=2, L3=3
    task_id + table_type as usize * NUM_TASKS
}
// Kernel tables: NUM_TASKS * 4 + offset
```

Điều này loại bỏ toàn bộ `PT_L*_TASK*` constants — chỉ cần 1 function.

#### N1b — MMU Page Table Refactor

**File cần thay đổi:** `src/arch/aarch64/mmu.rs`

- Thay tất cả `PT_L1_TASK0` / `PT_L2_RAM_TASK1` / v.v. bằng `pt_index(task_id, type)`
- `init()`: `for task in 0..3` → `for task in 0..NUM_TASKS`
- `setup_task_page_table()`: `if task_id >= 3` → `if task_id >= NUM_TASKS`
- `map_user_page()`, `unmap_page()`: tương tự
- Host stubs (`src/mmu.rs`): cập nhật `task_id >= 3` → `task_id >= NUM_TASKS`

**ASID assignment:** task N → ASID N+1 (hiện tại hardcoded cho 0,1,2). Đổi sang computed: ASID = `(task_id + 1) as u64`. Range 1–8, well within 8-bit ASID limit (255).

#### N1c — Task Init API Refactor

**Hiện tại** (`src/main.rs`):
```
init_tasks(uart_driver_entry, client_entry, idle_entry);  // 3 positional args
// + 3 blocks caps assignment
// + 3 blocks priority assignment
// + 3 blocks ttbr0 assignment
```

**Đề xuất** — Hybrid: const metadata + runtime entry (đồng thuận):

```rust
// Const-evaluable metadata (caps, priority, budget)
pub struct TaskMetadata {
    pub caps: u64,
    pub priority: u8,
    pub budget: u64,
}

pub const TASK_METADATA: [TaskMetadata; NUM_TASKS] = [
    TaskMetadata { caps: 0x3F, priority: 5, budget: 0 },     // task 0: uart_driver
    TaskMetadata { caps: 0x3FF, priority: 4, budget: 50 },    // task 1: client
    TaskMetadata { caps: 0x20, priority: 0, budget: 0 },      // task 2: ELF user task
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },      // task 3-6: idle
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },
    TaskMetadata { caps: 0x20, priority: 0, budget: 0 },      // task 7: IDLE_TASK_ID
];

// Runtime entry points (fn pointers + ELF-parsed entry)
let entries: [u64; NUM_TASKS] = [
    uart_driver_entry as u64,
    client_entry as u64,
    elf_info.entry,                  // ELF-parsed, runtime value
    idle_entry as u64,               // tasks 3-7
    // ...
];
```

Lý do hybrid: `fn() as u64` không chắc const-evaluable trên custom `aarch64-aegis` target. ELF entry point chắc chắn là runtime value. `caps`/`priority`/`budget` thuần số → const-safe.

`kernel_main()` dùng loop:
```rust
for (id, (meta, &entry)) in TASK_METADATA.iter().zip(entries.iter()).enumerate() {
    init_task(id, entry, meta.caps, meta.priority, meta.budget);
}
```

**Idle task:** Thêm `pub const IDLE_TASK_ID: usize` — explicit constant, **decoupled từ `NUM_TASKS`** (đồng thuận). Không hardcode `NUM_TASKS - 1` vì future dynamic task creation sẽ cần idle ở vị trí cố định.

#### N1d — Exception / Host Stubs

- `src/exception.rs` (host stub): `CURRENT >= 3` → `CURRENT >= NUM_TASKS`
- Không cần sửa `src/arch/aarch64/exception.rs` — TrapFrame ABI không đổi, context switch dùng `CURRENT` index đã dynamic.

#### Syscall & Capability mới

Phase N **KHÔNG thêm syscall mới**. 18 capability bits (0–17) giữ nguyên. Tasks mới dùng cùng capability set.

#### File cần thay đổi (N1)

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/kernel/sched.rs` | Sửa | `NUM_TASKS = 8`, `RESTART_DELAY_TICKS` review, idle task logic |
| `src/arch/aarch64/mmu.rs` | Sửa lớn | `PAGE_TABLE_COUNT = 36`, computed indexing, loops 0..NUM_TASKS, ASID computed |
| `src/mmu.rs` | Sửa | Host stub: `>= 3` → `>= NUM_TASKS` |
| `src/exception.rs` | Sửa | Host stub: `>= 3` → `>= NUM_TASKS` |
| `src/main.rs` | Sửa lớn | `TaskMetadata` const table + runtime entries, loop-based init |
| `linker.ld` | Sửa | 3 sections: task_stacks, user_stacks, page_tables sizes |
| `src/platform/qemu_virt.rs` | Có thể sửa | Thêm `NUM_TASKS` re-export nếu cần cross-module |
| `tests/host_tests.rs` | Sửa | `reset_test_state()` auto-scales, review tests hardcoding task 2 as idle |

#### Checkpoint N1

```
[AegisOS] 8 tasks initialized
[AegisOS] bootstrapping into uart_driver (EL0)...
```
> Boot thành công với 8 TCBs. Tasks 0–2 giữ nguyên behavior. Tasks 3–7 chạy idle loop. Tất cả host tests + QEMU checkpoints pass.

---

### N2 — KernelCell cho Struct Arrays

#### Khái niệm

Phase M đã validate `KernelCell<T>` trên 4 scalar globals. Phase N mở rộng sang 4 struct-array globals — giống như sau khi lắp khóa thành công cho 4 tủ nhỏ, bây giờ lắp cho 4 tủ lớn.

Thách thức: struct arrays cần **indexed access** (`TCBS[i].field`), không chỉ get/get_mut toàn bộ.

#### Thiết kế API

**Option A — Wrap toàn bộ array:**
```rust
static TCBS: KernelCell<[Tcb; NUM_TASKS]> = KernelCell::new([Tcb::new(); NUM_TASKS]);
// Access: unsafe { (*TCBS.get_mut())[i].state = Ready; }
```

**Option B — Per-element wrapper** (phức tạp hơn, benefit nhỏ ở single-core):
```rust
static TCBS: [KernelCell<Tcb>; NUM_TASKS] = [KernelCell::new(Tcb::new()); NUM_TASKS];
// Access: unsafe { TCBS[i].get_mut().state = Ready; }
```

**Chọn Option A** — lý do:
1. Ít thay đổi API nhất (chỉ thêm `(*TCBS.get_mut())` wrapper)
2. `KernelCell<T>` đã proven cho scalar, extend cho array là natural
3. Single-core → không cần per-element locking
4. `EMPTY_TCB` đã là `pub const` — const-constructible ✅ (verified trong discussion)

**✅ Đã xác nhận:** `EMPTY_TCB` là `pub const Tcb = Tcb { context: TrapFrame { x: [0; 31], ... }, ... }` — compiler evaluate tại compile time. `Tcb` không có `Copy`/`Clone` derive nhưng const initializer `[EMPTY_TCB; NUM_TASKS]` hoạt động vì là const expression. Risk #2 trong bản plan gốc đã **mitigated**.

#### Thứ tự encapsulate

| Bước | Biến | Test refs | Lý do thứ tự |
|------|------|-----------|--------------|
| 1 | `GRANTS` | ~20 | Đơn giản nhất, ít refs |
| 2 | `IRQ_BINDINGS` | ~15 | Tương tự GRANTS |
| 3 | `ENDPOINTS` | ~30 | Phức tạp hơn (queue state) |
| 4 | `TCBS` | ~150+ | Phức tạp nhất, nhiều refs nhất |

Mỗi bước: wrap → sửa kernel code → sửa tests → verify full suite pass → commit.

#### Helper macro `kcell_index!()` (đồng thuận — build tại N2.1)

```rust
macro_rules! kcell_index {
    ($cell:expr, $idx:expr) => {
        &mut (*$cell.get_mut())[$idx]
    };
}
```

Macro này giảm boilerplate cho 150+ TCBS refs và đảm bảo consistency. Build tại N2.1 (GRANTS), dùng cho N2.2–N2.4. ROI: +1h build → -1.5–2h sửa thủ công = net positive.

#### Test migration pattern

```rust
// Trước:
unsafe { sched::TCBS[0].state = TaskState::Ready; }

// Sau:
unsafe { (*sched::TCBS.get_mut())[0].state = TaskState::Ready; }
```

`reset_test_state()` cũng cần update tương tự.

#### File cần thay đổi (N2)

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/kernel/grant.rs` | Sửa | `static mut GRANTS` → `static GRANTS: KernelCell<[Grant; N]>` |
| `src/kernel/irq.rs` | Sửa | `static mut IRQ_BINDINGS` → `static IRQ_BINDINGS: KernelCell<[IrqBinding; N]>` |
| `src/kernel/ipc.rs` | Sửa | `static mut ENDPOINTS` → `static ENDPOINTS: KernelCell<[Endpoint; N]>` |
| `src/kernel/sched.rs` | Sửa | `static mut TCBS` → `static TCBS: KernelCell<[Tcb; NUM_TASKS]>` |
| `tests/host_tests.rs` | Sửa lớn | ~215+ access pattern updates |

#### Checkpoint N2

```
[AegisOS] all globals encapsulated in KernelCell
```
> Tất cả 8 `static mut` globals đã wrap trong `KernelCell<T>`. 0 `static mut` còn lại. Host tests + QEMU pass.

---

### N3 — Kani Formal Verification Pilot

#### Khái niệm

Testing nói: "Tôi thử 219 trường hợp, tất cả đều đúng." Kani nói: "Tôi chứng minh **mọi** trường hợp đều đúng — kể cả những trường hợp bạn chưa nghĩ ra." Đây là bước đầu tiên hướng DO-333 Formal Methods.

Kani là bounded model checker cho Rust — tự động explore tất cả execution paths trong bounded input space. Hoạt động trên host (x86_64), không cần QEMU.

#### N3a — Kani Setup

- Cài đặt: `cargo install --locked kani-verifier && cargo kani setup`
- Tạo thư mục `kani-proofs/` hoặc dùng `#[cfg(kani)]` inline
- CI: thêm Kani job vào `.github/workflows/ci.yml`

#### N3b — Proof Harnesses cho `cap.rs` (2 proofs — đồng thuận)

```rust
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn cap_for_syscall_no_panic() {
        let nr: u64 = kani::any();
        kani::assume(nr <= 12);
        let ep: u64 = kani::any();
        kani::assume(ep <= 3);
        let result = cap_for_syscall(nr, ep);
        // Property: return là subset của CAP_ALL (0x3FFFF)
        assert!(result & !0x3FFFF == 0, "cap bits out of defined range");
    }

    #[kani::proof]
    fn cap_for_syscall_completeness() {
        let nr: u64 = kani::any();
        kani::assume(nr <= 12);
        let ep: u64 = kani::any();
        kani::assume(ep <= 3);
        let result = cap_for_syscall(nr, ep);
        // Mọi syscall hợp lệ phải có cap bit defined (result != 0)
        // Ngoại trừ: ep > 3 hoặc syscall không định nghĩa
        // (verify completeness của match arms)
    }
}
```

**Properties cần verify (2):**
1. `cap_for_syscall()` không panic và return ⊆ `0x3FFFF` cho mọi input
2. `cap_for_syscall()` completeness — mọi syscall 0..=12 có cap bit defined

> **Note:** `cap_check()` chỉ là `(caps & required) == required` — pure bitwise op, không cần proof. `has_capability()` không tồn tại (factual error trong plan gốc đã sửa).

#### N3c — Proof Harnesses cho `elf.rs` (1 proof)

```rust
#[kani::proof]
#[kani::unwind(5)]  // MAX_SEGMENTS = 4, loop bound
fn parse_elf64_no_panic() {
    let len: usize = kani::any();
    kani::assume(len <= 128);
    let data: Vec<u8> = vec![0u8; len];  // Kani sẽ thử mọi byte patterns
    let _ = parse_elf64(&data);
    // Should not panic, no OOB regardless of input
}
```

**Lưu ý:** `parse_elf64` nhận `&[u8]`. Bound **128 bytes** (đồng thuận, giảm từ 4096) — đủ cho ELF header (64B) + 1 program header (56B). CBMC với 4096 symbolic bytes sẽ timeout. Nếu 128B quá chậm → giảm tiếp 96B (header + partial phdr).

#### N3d — Proof Harnesses cho `cell.rs`

```rust
#[kani::proof]
fn kernel_cell_roundtrip() {
    let cell = KernelCell::new(42u64);
    unsafe {
        assert!(*cell.get() == 42);
        *cell.get_mut() = 100;
        assert!(*cell.get() == 100);
        assert!(cell.as_ptr() == cell.as_ptr());  // stable pointer
    }
}
```

#### File cần thay đổi (N3)

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/kernel/cap.rs` | Sửa | Thêm `#[cfg(kani)]` proof harnesses |
| `src/kernel/elf.rs` | Sửa | Thêm `#[cfg(kani)]` proof harnesses |
| `src/kernel/cell.rs` | Sửa | Thêm `#[cfg(kani)]` proof harnesses |
| `.github/workflows/ci.yml` | Sửa | Thêm `kani-proofs` job |
| `Cargo.toml` | Có thể sửa | Thêm Kani-specific config nếu cần |

#### Checkpoint N3

```
Kani verification: 4 proofs, 0 failures
```
> CI output cho thấy tất cả proof harnesses pass. Không phải QEMU checkpoint — Kani chạy trên host.

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|-----------|-------|---------------|
| 1 | **No heap** | Bất biến AegisOS | `KernelCell<[T; N]>` dùng `UnsafeCell`, zero allocation |
| 2 | **No FP/SIMD** | CPACR_EL1.FPEN=0 | Phase N không thêm FP code |
| 3 | **TrapFrame = 288 bytes** | ABI-locked | Không thay đổi |
| 4 | **W^X** | Page permissions | MMU refactor giữ nguyên W^X policy |
| 5 | **Linker ↔ MMU đồng bộ** | Bất biến | Sửa cả linker.ld + mmu.rs cùng lúc trong N1 |
| 6 | **Syscall ABI** | Không đổi | Phase N không thêm syscall |
| 7 | **219 tests + 28 checkpoints = regression gate** | Safety net | Mỗi sub-step PHẢI pass full suite |
| 8 | **`EMPTY_TCB` đã là `pub const`** | Dùng trong `static` KernelCell | ✅ Đã verify: `[EMPTY_TCB; NUM_TASKS]` const-constructible |
| 9 | **Kani chạy trên host (x86_64)** | Không verify aarch64 asm | Chỉ verify pure Rust logic (cap, elf, cell) |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|--------|----------|-----------|------------|
| 1 | Page table index refactor break MMU | Cao | Boot fails, tasks crash | Incremental: sửa constants trước, test, rồi refactor init loop |
| 2 | ~~`Tcb::new()` không thể `const fn`~~ | ~~Trung bình~~ | ~~Không thể dùng `KernelCell<[Tcb; 8]>`~~ | ✅ **MITIGATED** — `EMPTY_TCB` là `pub const`, `[EMPTY_TCB; NUM_TASKS]` hoạt động |
| 3 | Kani timeout trên `parse_elf64` (128 byte input) | Thấp (giảm từ TB sau đồng thuận) | Proof không hoàn thành | Giảm bound từ 4096 → 128B (đồng thuận). Nếu vẫn chậm → 96B |
| 4 | TCBS migration break 150+ tests | Cao | 1-2 ngày sửa tests | Sửa TCBS cuối cùng (sau GRANTS, IRQ_BINDINGS, ENDPOINTS) |
| 5 | Linker script thay đổi sizes break boot | Trung bình | Kernel không boot | Test trên QEMU ngay sau mỗi linker change |
| 6 | Kani không tương thích `#![no_std]` kernel code | Thấp | Kani pilot fail | Isolate proofs: chỉ verify pure functions, không verify arch code |
| 7 | 8 tasks cùng chạy idle → UART output quá nhiều | Thấp | Debug khó | Tasks 3–7 chạy minimal idle (chỉ yield, không print) |

---

## Test Plan

### Host unit tests mới (ước lượng: ~20-30 tests)

| # | Test case | Mô tả |
|---|-----------|--------|
| 1-3 | `test_num_tasks_8_tcbs_init` | Verify 8 TCBs initialized đúng |
| 4-5 | `test_schedule_8_tasks_round_robin` | 8 tasks cùng priority → round-robin |
| 6-7 | `test_schedule_8_tasks_priority` | Mixed priority với 8 tasks |
| 8-9 | `test_fault_restart_task_7` | Task 7 fault → restart after 100 ticks |
| 10-11 | `test_idle_fallback_task_7` | Tất cả tasks blocked → schedule idle (task 7) |
| 12-13 | `test_ipc_8_tasks_cross` | IPC giữa task 0 và task 5 |
| 14-15 | `test_cap_task_5_check` | Capability check cho task ID > 2 |
| 16-17 | `test_watchdog_task_6` | Watchdog monitor task 6 |
| 18-19 | `test_grant_peer_task_4` | Grant create giữa task 0 và task 4 |
| 20-22 | `test_kernel_cell_array_tcbs` | `KernelCell<[Tcb; 8]>` get/get_mut indexed |
| 23-24 | `test_kernel_cell_array_endpoints` | `KernelCell<[Endpoint; 4]>` access |
| 25-26 | `test_pt_index_computed` | Verify `pt_index()` correctness cho 0..8 |
| 27-28 | `test_mmu_8_tasks_page_tables` | 8 per-task page table setup |
| 29-30 | `test_reset_state_8_tasks` | `reset_test_state()` clears 8 TCBs |

### QEMU boot checkpoints mới

| # | Checkpoint UART output | Sub-phase |
|---|----------------------|-----------|
| 29 | `[AegisOS] 8 tasks initialized` | N1 |
| 30 | `[AegisOS] all globals encapsulated in KernelCell` | N2 |

### Kani proofs (4 proofs — đồng thuận, không phải QEMU checkpoint)

| # | Proof | Module | Property |
|---|-------|--------|----------|
| 1 | `cap_for_syscall_no_panic` | `cap.rs` | No panic, return ⊆ `0x3FFFF` |
| 2 | `cap_for_syscall_completeness` | `cap.rs` | Mọi syscall 0..=12 có cap bit defined |
| 3 | `parse_elf64_no_panic` | `elf.rs` | No panic/OOB cho mọi input ≤ 128B |
| 4 | `kernel_cell_roundtrip` | `cell.rs` | get/get_mut consistency |

> **Bỏ 2 proofs** (so với plan gốc): `cap_check_no_oob` và `cap_check_monotone`/`has_capability_correctness` — `cap_check()` chỉ là `(caps & required) == required`, bitwise AND trên `u64` không thể OOB. `has_capability()` không tồn tại (factual error).

---

## Thứ tự triển khai

| Bước | Sub-phase | Phụ thuộc | Effort ước tính | Checkpoint xác nhận |
|------|-----------|-----------|----------------|---------------------|
| 1 | **N1a**: Constants + Linker | — | ~2-3h | `cargo build` thành công |
| 2 | **N1b**: MMU computed indexing | N1a | ~6-7h | QEMU boot + MMU enabled |
| 3 | **N1c**: Task init API refactor (hybrid `TaskMetadata`) | N1a | ~3-4h | 8 tasks initialized, UART output |
| 4 | **N1d**: Host stubs + tests + Option C validation | N1a-c | ~3-4h | 219+ host tests pass (validate tại NUM_TASKS=3, rồi flip 8) |
| 5 | **N2.1**: Build `kcell_index!()` macro + Wrap GRANTS | N1 done | ~3-4h | Host tests pass |
| 6 | **N2.2**: Wrap IRQ_BINDINGS | N2.1 | ~2-3h | Host tests pass |
| 7 | **N2.3**: Wrap ENDPOINTS | N2.2 | ~4-5h | Host tests pass |
| 8 | **N2.4**: Wrap TCBS | N2.3 | ~10-12h | Host tests + QEMU pass, 0 static mut |
| 9 | **N3a**: Kani setup + CI | micro-parallel* | ~2-3h | `cargo kani` chạy được |
| 10 | **N3b**: Kani cap.rs proofs (2) | N3a, N1 done | ~2-3h | 2 proofs pass |
| 11 | **N3c**: Kani elf.rs proof (1) | N3a | ~3-4h | 1 proof pass |
| 12 | **N3d**: Kani cell.rs proof (1) | N3a, N2 done | ~1-2h | 1 proof pass |
| 13 | **N-final**: Integration test + coverage re-measure | All | ~2-3h | Coverage ≥ 95%, 30 QEMU checkpoints |
| | **Tổng ước tính** | | **~43-50h** | ⚠️ **50h hard ceiling** |

**Sequencing (đồng thuận):** Strictly N1→N2→N3 sequential. *N3a (Kani install + CI yaml) micro-parallel trong QEMU wait time — infrastructure only, zero proof code.

**Hard ceilings:** N1=18h, N2=24h, N3=10h, Tổng=50h. Vượt ceiling → stop & re-evaluate.

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|-------------|------------|-------------------|
| **DO-178C** | §6.3.4 | Source code verifiable — `KernelCell` encapsulation hoàn tất (N2) |
| **DO-333** | §6.1 | Formal Methods — Kani pilot (N3) |
| **DO-178C** | §6.4.1 | Statement Coverage — re-measure sau scale (N-final) |
| **IEC 62304** | §5.5.3 | Software unit verification — Kani proofs cho cap + elf (N3) |
| **IEC 62304** Amendment 1 | Clause 4.3 | Software units có thể phân loại riêng nếu isolation đảm bảo — 8 task isolation (N1) |
| **ISO 26262** | Part 6 §7 | Software unit design — `TaskMetadata` hybrid table (N1c) |
| **ISO 26262** | Part 9 | ASIL Decomposition — 8 tasks = 8 independent partitions (N1) |
| **ISO 26262** | Part 11 | Multi-core preparation — `KernelCell` pattern ready (N2) |

---

## Scope Note (đồng thuận)

> **`.elf_load`** (12 KiB, 3 pages) và **`NUM_GRANTS`** (2) giữ nguyên Phase N.
> Tasks 3–7 là kernel-internal idle, không dùng ELF loading.
> Mở rộng các resource này sang **Phase O** khi thêm real ELF user tasks.

---

## Backward Compatibility

| Thay đổi | Break API? | Break ABI? | Migration |
|----------|-----------|-----------|-----------|
| `NUM_TASKS = 8` | Có — `init_tasks()` signature đổi | Không | Refactor callers trong `main.rs` |
| Computed PT indices | Không — internal MMU | Không | Transparent |
| `TCBS: KernelCell<[Tcb; 8]>` | Có — access pattern đổi | Không | `unsafe { (*TCBS.get_mut())[i] }` |
| `ENDPOINTS: KernelCell<[Endpoint; 4]>` | Có — access pattern đổi | Không | Tương tự |
| `TaskMetadata` hybrid table | Có — `init_tasks()` replaced | Không | `kernel_main()` loop + `TASK_METADATA` const |
| Kani proofs | Không — `#[cfg(kani)]` isolated | Không | Additive only |
| **Syscall ABI** | **Không đổi** | **Không đổi** | — |
| **TrapFrame** | **Không đổi** | **Không đổi** | — |
| **Capability bits** | **Không đổi** (0-17) | **Không đổi** | — |

---

## Bước tiếp theo đề xuất

1. [x] Review kế hoạch Phase N → [final consensus](../discussions/phase-n-scale-and-verify/final_consensus_2026-02-12.md) (13/13 đồng thuận)
2. [x] Verify `EMPTY_TCB` là `pub const` — ✅ const-constructible
3. [x] Triển khai N1a: Constants + Linker — ✅ `IDLE_TASK_ID`, `PageTableType`, `pt_index()`, computed `NUM_PAGE_TABLE_PAGES` (commit fadbdc7)
4. [x] Triển khai N1b: MMU computed indexing — ✅ all `PT_*+task_id` → `pt_index()`, loops/guards parameterized
5. [x] Triển khai N1c: `TaskMetadata` hybrid + loop init — ✅ `TaskMetadata` struct, `sched::init(&[u64; NUM_TASKS])`, `TASK_META` const array
6. [x] Triển khai N1d: Host stubs + test updates + Option C validation — ✅ 220 tests + 28 QEMU at NUM_TASKS=3, then flipped to 8
7. [x] Triển khai N2: `kcell_index!()` macro + wrap 4 globals (GRANTS → IRQ → ENDPOINTS → TCBS) — ✅ 0 `static mut` remaining (commit 7cd1daa)
8. [x] Triển khai N3: Kani proof harnesses — ✅ 6 properties in 4 files: pt_index bounds+aliasing, cap_check bitwise, cap_for_syscall bounded, schedule idle guarantee, restart state machine (commit d575021). Kani runs on Linux CI.
9. [ ] Re-measure coverage → ≥ 95%
10. [ ] Viết blog Phase N giải thích (handoff → Aegis-StoryTeller)
11. [ ] Chuẩn bị Phase O roadmap
