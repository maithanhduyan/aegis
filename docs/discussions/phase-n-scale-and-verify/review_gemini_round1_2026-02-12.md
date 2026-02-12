# 🔧 Gemini-Pragmatist Review — Round 1 | Phase N: Scale & Verify

> **Agent:** Gemini-Pragmatist-Agent
> **Ngày:** 2026-02-12
> **Round:** 1

---

## Q1: Chiến lược Scale — Jump to 8 hay Incremental?

### Khuyến nghị: **Option C — Parameterize tại `NUM_TASKS=3`, validate, rồi flip**

**Lý do thực tế:**

MMU refactor (`mmu.rs`, 594 dòng, 13 named constants) là **rủi ro cao nhất** trong toàn bộ Phase N. Một off-by-one trong page table index = kernel crash khi boot, và debug MMU trên QEMU rất khó (không có debugger attach dễ dàng, chỉ có UART output).

Option C cho phép:
1. Refactor tất cả hardcoded `3`, `PT_L1_TASK0`... thành computed expressions dùng `NUM_TASKS`
2. Giữ `NUM_TASKS=3` → chạy **toàn bộ 219 tests + 28 QEMU checkpoints**
3. Nếu pass → refactor đúng, zero regression
4. Đổi `NUM_TASKS=3` → `8` + linker sizes → test lại

**Estimate điều chỉnh:**

Plan ước lượng N1 = 12-14h. **Thực tế: 16-20h.** Lý do:
- MMU debug time bị underestimate. Mỗi lần sai page table index → boot fail → phải đọc UART log + reason about L1→L2→L3 chain. Budget thêm **+4-6h** cho debug.
- Linker `.page_tables` section phải align đúng — thay đổi size có thể shift các symbol khác, gây overlap risk.
- `setup_task_page_table()` gọi trong interrupt context (timer handler → schedule → context switch) — bug ở đây = intermittent crash, khó reproduce.

**Risk mitigation:**
- Commit sau mỗi sub-step (N1a, N1b, N1c, N1d) — nếu fail, rollback chính xác
- Thêm `assert!(task_id < NUM_TASKS)` tại mọi computed index entry point
- QEMU boot test **ngay sau N1a** (chỉ đổi constants, chưa refactor) để catch linker issues sớm

**Vấn đề plan chưa address:**
- **ELF load region** tại `0x4010_0000` chỉ có 3×4096 = 12 KiB. Nếu 8 tasks cần ELF loading → cần mở rộng region hoặc chỉ 1 task dùng ELF. Plan nên clarify: tasks 3-7 sẽ là kernel-internal (function entry) hay ELF-loaded?
- **Grant pages**: `.grant_pages` hiện 2×4096 = 8 KiB. `NUM_GRANTS=2` — có cần tăng lên khi có 8 tasks?

---

## Q2: Thứ tự KernelCell Wrapping

### Khuyến nghị: **Giữ nguyên GRANTS → IRQ → ENDPOINTS → TCBS**

**Phân tích effort thực tế:**

| Global | Test refs | Kernel code refs | Estimated wrap time |
|--------|-----------|-----------------|-------------------|
| `GRANTS` | ~20 | ~10 (grant.rs only) | 2-3h ✅ plan accurate |
| `IRQ_BINDINGS` | ~15 | ~8 (irq.rs + timer.rs) | 2-3h ✅ plan accurate |
| `ENDPOINTS` | ~30 | ~20 (ipc.rs + sched.rs) | 4-5h ✅ plan accurate |
| `TCBS` | ~150+ | ~40+ (sched, ipc, grant, irq, exception, main) | **10-14h** ❌ plan underestimates (8-10h) |

**Tại sao TCBS cuối:**
1. 150+ refs = **1-2 ngày chỉ sửa compile errors**. Nếu sai pattern → 150+ lần sửa lại.
2. TCBS truy cập trong **interrupt context** (SVC handler → `handle_svc()` → `sched::schedule()`). Bug ở đây = kernel hang, không có stack trace.
3. Sau GRANTS, IRQ, ENDPOINTS — team đã có **migration script pattern** rõ ràng: `static mut X` → `static X: KernelCell<T>`, tất cả `unsafe { X[i] }` → `unsafe { (*X.get_mut())[i] }`.

**TCBS migration cụ thể cần cẩn thận:**
- `sched::schedule()` đọc + ghi `TCBS[CURRENT]` và `TCBS[next]` — cần 2 `get_mut()` calls cùng scope
- `exception.rs` SVC dispatch gọi `TCBS[current].context` — phải đảm bảo không double-borrow (single-core nên OK nhưng phải verify)
- `host_tests.rs` có `reset_test_state()` zero tất cả TCBS — access pattern thay đổi

**Adjustment:** Tăng estimate TCBS từ 8-10h → **10-14h**. Tổng N2: 18-25h (plan nói 16-21h).

---

## Q3: Kani Pilot Scope

### Khuyến nghị: **4 proofs (giảm từ 6), sửa 2 lỗi factual trong plan**

**Lỗi factual trong plan:**
1. **`has_capability()` KHÔNG TỒN TẠI** trong `cap.rs`. Chỉ có `cap_check(task_id, required_caps)`. Plan cần sửa tên hàm.
2. **`cap_for_syscall()` trả `u64` bitmask**, không phải bit index. Property "return ≤ 17" là **SAI**. Đúng: "return is subset of `CAP_ALL` (`0x3FFFF`)".

**Proofs đề xuất (4):**

| # | Proof | Module | Property | Estimate |
|---|-------|--------|----------|----------|
| 1 | `cap_for_syscall_no_panic` | `cap.rs` | Không panic cho mọi input, return ⊆ `0x3FFFF` | 1-2h |
| 2 | `cap_for_syscall_completeness` | `cap.rs` | Mọi syscall 0..=12 có cap bit defined | 1h |
| 3 | `parse_elf64_no_panic` | `elf.rs` | Không panic/OOB cho mọi input ≤ **128 bytes** | 3-4h |
| 4 | `kernel_cell_roundtrip` | `cell.rs` | get/get_mut consistency | 1h |

**Tại sao bỏ 2 proofs:**
- `cap_check_no_oob` → `cap_check` chỉ là `caps & required != 0` — bitwise AND trên `u64` **không thể OOB**. Proof trivial, không thêm giá trị.
- `cap_check_monotone` / `has_capability_correctness` → hàm không tồn tại hoặc trivially correct.

**Tại sao giảm ELF bound 4096→128:**
- `parse_elf64` dùng `read_u16`, `read_u32`, `read_u64` helpers — mỗi helper index trực tiếp vào `data[offset..offset+N]`
- CBMC symbolic execution trên 4096 symbolic bytes = **4096 × 8 = 32768 symbolic bits** — mỗi branch tạo 2^N paths
- ELF header = 64 bytes, mỗi program header = 56 bytes, MAX_SEGMENTS=4 → max meaningful input = 64 + 4×56 = 288 bytes
- 128 bytes đủ cover header + 1 program header — Kani sẽ verify bounds checking logic
- Nếu 128 bytes quá chậm → giảm tiếp xuống 96 (header + partial phdr)

**Kani setup estimate:** Plan nói 2-3h. **Thực tế: 3-5h.** Lý do:
- Kani cần CBMC backend — trên Ubuntu CI runner phải install `cbmc` package
- `#[cfg(kani)]` module phải compile clean trên cả host tests lẫn Kani run
- CI job yaml cần test: `cargo kani --tests` hoặc `cargo kani --harness <name>`
- First-time debug Kani config luôn tốn thời gian

**Tổng N3 điều chỉnh:** 9-12h (plan nói 9-13h — tương đối chính xác nếu scope giảm).

---

## Q4: Sequencing N1-N2-N3

### Khuyến nghị: **Strictly N1 → N2 → N3. Không parallel.**

**Lý do:**

1. **Single developer** — parallel chỉ có ý nghĩa khi có 2+ người. Với 1 developer, context switching giữa MMU refactor (N1) và Kani setup (N3a) sẽ **giảm productivity**, không tăng.

2. **Cascading dependencies:**
   - N1 đổi `NUM_TASKS=8` → TCBS type đổi thành `[Tcb; 8]` → N2 wrap `KernelCell<[Tcb; 8]>` (phụ thuộc N1)
   - N2 wrap `TCBS` → Kani `cell.rs` proof nên test pattern thật, không phải toy example (N3 hưởng lợi từ N2)
   - N1 sửa `cap_check()` nếu có `task_id >= NUM_TASKS` guard → Kani `cap.rs` proof cần stable API (N3 phụ thuộc N1)

3. **Risk management:** Nếu N1 fail (MMU crash) → phải debug trước khi bất cứ gì khác. Parallel work trên N3 sẽ bị block anyway.

**Sequencing cụ thể:**

```
Week 1: N1a → N1b → N1c → N1d (constants, MMU, TaskConfig, tests)
Week 2: N2.1 → N2.2 → N2.3 (GRANTS, IRQ, ENDPOINTS)
Week 3: N2.4 (TCBS — dài nhất) + N3a (Kani setup, lúc này OK parallel)
Week 4: N3b → N3c → N3d (proofs) + integration test
```

**Lưu ý:** N3a (Kani setup) có thể chạy song song N2.4 vì Kani setup chỉ là install + CI yaml — không phụ thuộc code changes. Nhưng N3b-d proofs nên **sau** N2 done.

**Tổng timeline realistic:** 4 tuần part-time hoặc 2 tuần full-time.

---

## Q5: TaskConfig — Static const hay Runtime?

### Khuyến nghị: **Hybrid — const metadata + runtime entry points**

**Phân tích kỹ thuật cụ thể:**

```rust
// ✅ Const-safe — chỉ chứa số nguyên
pub struct TaskMetadata {
    pub caps: u64,
    pub priority: u8,
    pub budget: u64,
}

pub const TASK_METADATA: [TaskMetadata; NUM_TASKS] = [
    TaskMetadata { caps: 0x3F, priority: 5, budget: 0 },     // task 0: uart_driver
    TaskMetadata { caps: 0x3FF, priority: 4, budget: 50 },    // task 1: client
    TaskMetadata { caps: 0x20, priority: 0, budget: 0 },      // task 2: placeholder
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },      // task 3-6: idle
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },
    TaskMetadata { caps: 0x00, priority: 0, budget: 0 },
    TaskMetadata { caps: 0x20, priority: 0, budget: 0 },      // task 7: idle fallback
];
```

**Tại sao KHÔNG full const:**
- `fn() as u64` **có thể** là const trên native targets, nhưng trên custom `aarch64-aegis.json` target — **không chắc**. Nightly Rust behavior cho custom targets hay thay đổi.
- ELF-loaded task (user/hello) có entry point từ `parse_elf64()` → **chắc chắn** runtime value.
- Trộn const và runtime entry trong cùng 1 array → phải dùng `Option<u64>` hoặc sentinel (0) cho ELF entries → phức tạp hóa.

**Pattern đề xuất:**

```rust
fn init_all_tasks() {
    // Runtime entry points
    let entries: [u64; NUM_TASKS] = [
        uart_driver_entry as u64,        // kernel function
        client_entry as u64,             // kernel function
        elf_info.entry,                  // ELF-parsed, runtime
        idle_entry as u64,               // tasks 3-6
        idle_entry as u64,
        idle_entry as u64,
        idle_entry as u64,
        idle_entry as u64,               // task 7: idle fallback
    ];

    for (id, (meta, &entry)) in TASK_METADATA.iter().zip(entries.iter()).enumerate() {
        sched::init_task(id, entry, meta.caps, meta.priority, meta.budget);
    }
}
```

**Effort estimate:** 3-4h cho refactor này — plan nói 3-4h cho N1c, phù hợp.

**Vấn đề chưa giải quyết:** Hiện tại chỉ task 2 dùng ELF loading (user/hello binary). Nếu tasks 3-7 cũng cần ELF → `.elf_load` region (3×4096) không đủ. Plan cần clarify: tasks 3-7 chạy kernel-internal functions hay ELF binaries?

---

## Tổng kết vị trí

| # | Câu hỏi | Vị trí Gemini-Pragmatist | Lý do cốt lõi |
|---|---------|--------------------------|----------------|
| Q1 | Scale strategy | **Option C** — Parameterize tại `NUM_TASKS=3`, validate, flip | Tách refactor vs scale risk; MMU debug is painful |
| Q2 | KernelCell order | **GRANTS→IRQ→ENDPOINTS→TCBS** (giữ plan) | 150+ refs quá rủi ro cho first migration; build experience first |
| Q3 | Kani scope | **4 proofs** (bỏ 2 trivial, sửa lỗi factual, ELF bound→128B) | `has_capability()` không tồn tại; `cap_check` trivially safe; Kani timeout risk |
| Q4 | Sequencing | **Strictly N1→N2→N3** — không parallel (single developer) | Context switching giảm productivity; cascading dependencies |
| Q5 | TaskConfig | **Hybrid** — const metadata + runtime entry | `fn() as u64` unreliable trên custom target; ELF entry = runtime |

### Estimate tổng hợp điều chỉnh

| Sub-phase | Plan estimate | Gemini estimate | Delta | Lý do |
|-----------|--------------|-----------------|-------|-------|
| N1 | 12-14h | **16-20h** | +4-6h | MMU debug time underestimated |
| N2 | 16-21h | **18-25h** | +2-4h | TCBS 150+ refs → 10-14h (not 8-10h) |
| N3 | 9-13h | **9-12h** | -1h | Scope giảm (4 proofs vs 6) |
| **Total** | **38-50h** | **45-57h** | +7h | ~15% underestimate in plan |

### Red flags trong plan cần fix trước khi implement

1. ❌ `has_capability()` không tồn tại — sửa thành `cap_check()`
2. ❌ `cap_for_syscall()` trả bitmask, không phải bit index — sửa Kani property
3. ⚠️ ELF load region (12 KiB) không đủ cho 8 tasks nếu cần ELF loading
4. ⚠️ `.grant_pages` (8 KiB, 2 pages) có cần mở rộng cho 8 tasks?
5. ⚠️ `IDLE_TASK_ID` nên explicit, không assume `NUM_TASKS - 1`
