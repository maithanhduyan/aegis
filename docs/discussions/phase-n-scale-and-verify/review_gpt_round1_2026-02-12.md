# 🔭 GPT-Visionary Review — Round 1 | Phase N: Scale & Verify

> **Agent:** GPT-Visionary-Agent
> **Ngày:** 2026-02-12
> **Round:** 1

---

## Q1: Chiến lược Scale — Jump to 8 hay Incremental?

### Khuyến nghị: **Option C — Parameterize trước, giữ `NUM_TASKS=3`, validate, rồi flip sang 8**

**Lý do:**

Bài toán thực chất có 2 rủi ro **tách biệt nhau**:
1. **Refactor risk**: chuyển từ hardcoded constants sang computed indexing (MMU, linker, init)
2. **Scale risk**: tăng số lượng task thực tế từ 3 lên 8

Option C tách 2 rủi ro này: bước 1 refactor toàn bộ code để dùng `NUM_TASKS` constant thay vì literal `3`, nhưng **giữ nguyên `NUM_TASKS=3`**. Chạy full 219 tests + 28 checkpoints. Nếu pass → refactor đúng. Bước 2 chỉ cần đổi 1 constant sang 8 + update linker sizes.

Option A (jump thẳng 3→8) trộn cả 2 rủi ro — nếu boot fail, không biết lỗi ở refactor hay ở scale. Option B (3→4→8) tốn effort không cần thiết cho bước 4→8.

**Tầm nhìn 5-10-20 năm:**

- **5 năm**: `NUM_TASKS` sẽ không dừng ở 8. Hệ thống thật cần 16-32 tasks. Nền tảng computed indexing hôm nay sẽ scale mượt.
- **10 năm**: Dynamic task creation sẽ cần `MAX_TASKS` compile-time + runtime `active_count`. Parameterize hôm nay = foundation cho runtime flexibility.
- **20 năm**: Multi-core SMP cần per-core task pools. Computed indexing pattern mở đường cho `per_core_tasks[core_id][local_task_id]`.

**Trade-off chấp nhận:** Thêm ~2-3h cho bước validate ở `NUM_TASKS=3` trước khi flip, nhưng giảm debug time khi có lỗi.

**Điểm bổ sung quan trọng:** `IDLE_TASK_ID` nên là constant riêng, **không phải** `NUM_TASKS - 1`. Lý do: future dynamic task creation sẽ cần idle task ở vị trí cố định, không phải "luôn là task cuối cùng". Decoupling idle từ `NUM_TASKS` hôm nay giúp tránh refactor lớn sau này.

---

## Q2: Thứ tự KernelCell Wrapping

### Khuyến nghị: **Giữ nguyên GRANTS → IRQ → ENDPOINTS → TCBS**

**Lý do:**

Experience-first strategy là đúng cho context này. Phase M đã validate `KernelCell<T>` cho **scalar** globals — đây là bước đầu tiên nhảy sang **struct array** globals. GRANTS (~20 refs) là bãi tập an toàn nhất.

TCBS với 150+ refs across 7 files, bao gồm **interrupt context** (exception handler truy cập TCBS trong SVC dispatch), là candidate phức tạp nhất. Sai ở TCBS = kernel crash không debug được. Sau 3 lần thành công (GRANTS, IRQ, ENDPOINTS), team sẽ có pattern migration rõ ràng.

**Tầm nhìn dài hạn:**

- **5 năm**: Pattern `unsafe { (*GLOBAL.get_mut())[i].field }` sẽ lặp lại 100+ lần. Nên cân nhắc helper macro `kcell_index!(TCBS, i).field` — nhưng đây là Phase O concern.
- **10 năm**: Khi chuyển multi-core, `KernelCell<T>` sẽ cần upgrade thành spinlock-guarded hoặc per-core. Wrap array nguyên khối (Option A) dễ upgrade hơn per-element (Option B).

**Trade-off:** TCBS cuối cùng = delay formal verification benefit cho critical struct. Nhưng TCBS sai = 150+ compile errors + potential runtime crash. Safety > speed.

---

## Q3: Kani Pilot Scope

### Khuyến nghị: **5 proofs (giảm từ 6), điều chỉnh targets**

**Thay đổi so với plan:**

| # | Proof | Module | Thay đổi |
|---|-------|--------|----------|
| 1 | `cap_for_syscall_no_panic` | `cap.rs` | ✅ Giữ — nhưng sửa property: "return is subset of `0x3FFFF`" (không phải "≤ 17") |
| 2 | `cap_for_syscall_completeness` | `cap.rs` | ✅ Giữ — verify mọi syscall hợp lệ (0..=12) có cap bit defined |
| 3 | `cap_check_no_oob` | `cap.rs` | ✅ Giữ — sửa tên: `cap_check` không phải `has_capability` |
| 4 | ~~`cap_check_monotone`~~ | `cap.rs` | ❌ Bỏ — bitwise AND check là trivially correct, proof không thêm giá trị |
| 5 | `parse_elf64_no_panic` | `elf.rs` | ✅ Giữ — nhưng **giảm bound từ 4096→128 bytes** (đủ cho header + 2 program headers) |
| 6 | `kernel_cell_roundtrip` | `cell.rs` | ✅ Giữ |

**Về việc thêm `sched.rs`:** Hoãn sang Phase O. Scheduler có side effects (truy cập TCBS, modify CURRENT) — Kani cần mock hoặc isolate, phức tạp hơn pure functions đáng kể. Pilot nên tập trung vào pure functions trước.

**Tầm nhìn dài hạn:**

- **5 năm**: Kani sẽ mature hơn — support symbolic execution cho struct arrays. Lúc đó verify `sched.rs` invariants sẽ feasible.
- **10 năm**: DO-333 compliance sẽ yêu cầu formal proofs cho **tất cả** Level A components. Pilot hôm nay xây dựng institutional knowledge.
- **20 năm**: Model checking + proof assistants (Lean4, Coq) sẽ integrate vào CI standard. Kani pilot = first step trên con đường đó.

**Trade-off:** Bỏ 1 proof giảm coverage nhẹ, nhưng tăng ROI (mỗi proof có giá trị riêng biệt thay vì overlap).

---

## Q4: Sequencing N1-N2-N3

### Khuyến nghị: **N1 → N3a (setup, parallel) → N2 → N3b-d**

**Lý do chi tiết:**

```
Timeline:
├─ N1a-d: Scale constants + MMU + TaskConfig + tests  [~14-16h]
│    └─ N3a: Kani install + CI job (parallel)          [~2-3h]
├─ N2.1-2.4: KernelCell wrapping                       [~16-21h]
└─ N3b-d: Write proof harnesses                        [~7-10h]
```

- **N1 trước N2:** Bắt buộc — `KernelCell<[Tcb; NUM_TASKS]>` cần `NUM_TASKS=8` từ N1. Nếu wrap TCBS trước scale → phải sửa 2 lần (wrap `[Tcb; 3]` rồi đổi thành `[Tcb; 8]`).
- **N3a song song N1:** Kani setup + CI job không phụ thuộc code changes.
- **N3b-d sau N2:** Kani proofs cho `cell.rs` nên verify pattern **sau khi** `KernelCell` đã wrap thực tế — proof có context thực.
- **N3b (cap.rs) có thể chạy song song N2** — `cap.rs` không bị ảnh hưởng bởi `KernelCell` wrapping.

**Tầm nhìn dài hạn:** Thiết lập CI pipeline mà formal verification chạy **sau** unit tests — pattern này sẽ scale khi thêm proofs. Test first → prove second.

---

## Q5: TaskConfig Table — Static const hay Runtime?

### Khuyến nghị: **Hybrid — `const TaskBaseConfig` + runtime entry point assignment**

**Phân tích kỹ thuật:**

```rust
// Const-evaluable metadata
pub struct TaskBaseConfig {
    pub caps: u64,
    pub priority: u8,
    pub budget: u64,
}

pub const TASK_BASE_CONFIGS: [TaskBaseConfig; NUM_TASKS] = [
    TaskBaseConfig { caps: 0x3F, priority: 5, budget: 0 },   // uart_driver
    TaskBaseConfig { caps: 0x3FF, priority: 4, budget: 50 },  // client
    TaskBaseConfig { caps: 0x20, priority: 0, budget: 0 },    // idle
    // ... tasks 3-7
];

// Runtime — vì entry points là function pointers hoặc ELF-parsed
fn init_all_tasks() {
    let entries: [u64; NUM_TASKS] = [
        uart_driver_entry as u64,
        client_entry as u64,
        idle_entry as u64,
        // ...
    ];
    for (id, (base, entry)) in TASK_BASE_CONFIGS.iter().zip(entries.iter()).enumerate() {
        init_task(id, base, *entry);
    }
}
```

**Lý do hybrid:**
- Function pointers (`fn()`) trong Rust có thể **là** const trên native targets, nhưng trên custom `aarch64-aegis` target không chắc compiler cho phép `fn() as u64` trong const context.
- ELF-loaded tasks (user/hello) có entry point **từ ELF parsing** — chắc chắn runtime value.
- `caps`, `priority`, `budget` thuần số → const-safe, compiler verify tại compile time.

**Tầm nhìn dài hạn:**

- **5 năm**: `TaskBaseConfig` sẽ mở rộng thêm fields: `memory_region`, `device_access_list`, `ipc_permissions`. Const table = documentation-as-code.
- **10 năm**: Device tree / config file sẽ replace hardcoded configs. Hybrid pattern dễ migrate: const → parsed from config.
- **20 năm**: Task configs sẽ đến từ certified configuration management system. Const base + runtime overlay = pattern chuẩn trong DO-178C.

---

## Tổng kết vị trí

| # | Câu hỏi | Vị trí GPT-Visionary | Lý do cốt lõi |
|---|---------|---------------------|----------------|
| Q1 | Scale strategy | **Option C** — Parameterize ở `NUM_TASKS=3`, validate, flip to 8 | Tách refactor risk khỏi scale risk; `IDLE_TASK_ID` constant riêng |
| Q2 | KernelCell order | **GRANTS→IRQ→ENDPOINTS→TCBS** (giữ plan) | Experience-first; TCBS quá complex cho first migration |
| Q3 | Kani scope | **5 proofs** (bỏ `cap_check_monotone`, giảm ELF bound 4096→128B) | ROI per proof; tránh Kani timeout; hoãn `sched.rs` sang Phase O |
| Q4 | Sequencing | **N1 → N3a parallel → N2 → N3b-d** | N1 trước N2 (type dependency); N3a independent; N3b-d cần stable code |
| Q5 | TaskConfig | **Hybrid** — const metadata + runtime entry | Function ptr const-safety uncertain; ELF entry = runtime; future config flexibility |

### Cross-cutting concerns cho Phase O+

1. **Helper macro** `kcell_index!(GLOBAL, i)` để giảm boilerplate sau N2
2. **`IDLE_TASK_ID`** constant decoupled từ `NUM_TASKS`
3. **Kani expansion** roadmap: Phase O = `sched.rs` invariants, Phase P = `mmu.rs` address space isolation
4. **Multi-core prep**: `KernelCell` + array wrapping = foundation cho future per-core migration
