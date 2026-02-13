# Kế hoạch Phase P — Formal Verification Expansion

> **Trạng thái: ✅ APPROVED** (consensus 100% — 2026-02-13) — Mở rộng chứng minh hình thức (Kani) cho 3 module chưa được verify (grant, irq, watchdog), tạo tài liệu proof coverage mapping (DO-333 FM.A-7), và cập nhật README.md (A+). Pure functions dưới `#[cfg(kani)]` — zero runtime changes. Miri deferred (skeleton shim only). Nâng tổng số Kani proofs từ 10 → 18. Effort: **8–11h**.
>
> Xem thảo luận: [`docs/discussions/phase-p-formal-verification-expansion/final_consensus_2026-02-13.md`](../discussions/phase-p-formal-verification-expansion/final_consensus_2026-02-13.md)

---

## Tại sao Phase P?

### Lỗ hổng hiện tại: "3 module kernel quan trọng không có bằng chứng toán học"

Sau 15 phases (A→O), AegisOS có **10 Kani proofs** — nhưng phân bổ **không đều**:

| Module | Proofs | Tình trạng |
|---|---|---|
| `kernel/cap.rs` | 2 | ✅ Đầy đủ |
| `kernel/sched.rs` | 2 | ⚠️ Cơ bản (chưa có budget/watchdog) |
| `kernel/ipc.rs` | 3 | ✅ Tốt (queue, message, cleanup) |
| `mmu.rs` | 2 | ✅ Đầy đủ |
| `platform/qemu_virt.rs` | 1 | ✅ Đầy đủ |
| **`kernel/grant.rs`** | **0** | 🔴 **Không có proof nào** |
| **`kernel/irq.rs`** | **0** | 🔴 **Không có proof nào** |
| **Watchdog (trong sched.rs)** | **0** | 🔴 **Không có proof nào** |

Trong hệ thống safety-critical:

- **Grant** (shared memory) — lỗi dẫn đến data corruption giữa tasks, vi phạm freedom from interference (ISO 26262 Part 9)
- **IRQ** (interrupt routing) — lỗi dẫn đến missed interrupt hoặc orphaned masked INTID, hệ thống mất phản ứng với phần cứng
- **Watchdog** — lỗi dẫn đến task chết không bị phát hiện, vệ tinh mất điều khiển

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|---|---|
| 1 | Grant module: 0 Kani proofs, 0 pure functions | Không thể chứng minh no-overlap, cleanup completeness, slot exhaustion safety |
| 2 | IRQ module: 0 Kani proofs, 0 pure functions | Không thể chứng minh routing correctness, no orphaned masks, cleanup completeness |
| 3 | Watchdog: 0 Kani proofs, logic sống trong `sched.rs` | Không thể chứng minh violation detection bounded, epoch fairness |
| 4 | Không có Miri trong CI | Abstract interpretation (DO-333) chưa được sử dụng — unsafe code chưa kiểm tra runtime |
| 5 | Không có proof coverage mapping | Không đáp ứng DO-333 FM.A-7 — không biết proof nào cover property nào |
| 6 | `README.md` lỗi thời nghiêm trọng | Scheduler "3 tasks" → thực tế 8; tests "189" → thực tế 241; thiếu user workspace |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|---|---|---|
| Pure function extraction | Tách logic ra khỏi `static mut` globals → hàm thuần (immutable input → output) | 1, 2, 3 (prerequisite) |
| Kani proofs batch | ~8 proofs mới cho grant (3) + irq (3) + watchdog/budget (2) | 1, 2, 3 |
| Miri CI job | Thêm `cargo +nightly miri test` vào CI pipeline | 4 |
| FM.A-7 mapping document | Bảng: Module → Property → Proof → Status | 5 |
| README refresh | Cập nhật toàn bộ section lỗi thời | 6 |

---

## Phân tích hiện trạng

### Grant module — `src/kernel/grant.rs`

```
struct Grant { active: bool, owner: Option<usize>, peer: Option<usize>, phys_addr: u64 }
static GRANTS: KernelCell<[Grant; MAX_GRANTS=2]>

Functions (all touch globals directly):
- grant_create(grant_id, owner, peer) → u64 (error code)
- grant_revoke(grant_id, caller) → u64 (error code)
- cleanup_task(task_idx) — owner: EMPTY_GRANT; peer: active=false, peer=None
```

**Vấn đề phát hiện:**
- Cleanup asymmetry — owner fault zeros grant, peer fault only unmaps. Owner không được thông báo khi peer bị cleanup.
- Không có proof no-overlap giữa active grants.

### IRQ module — `src/kernel/irq.rs`

```
struct IrqBinding { intid: u32, task_id: usize, notify_bit: u64, active: bool, pending_ack: bool }
static IRQ_BINDINGS: KernelCell<[IrqBinding; MAX_IRQ_BINDINGS=8]>

Functions (all touch globals directly):
- irq_bind(intid, task_id, notify_bit) → u64 (error code)
- irq_ack(intid, task_id) → u64 (error code)
- irq_route(intid, frame) — sets notification, unblocks task (aarch64 only)
- irq_cleanup_task(task_id) — unbinds all, unmasks pending, disables
```

**Vấn đề phát hiện:**
- Không detect notify_bit collision (2 INTID cùng task cùng bit → merge, mất identity).
- Cleanup window: unmask → disable có thể fire lại trên HW thật (dù kernel mask DAIF).

### Watchdog — trong `src/kernel/sched.rs`

```
TCB fields: heartbeat_interval: u64 (0=disabled), last_heartbeat: u64
Functions:
- record_heartbeat(task_idx, interval) — sets interval + last_heartbeat = now
- watchdog_scan() — in tick_handler, checks elapsed > interval → Faulted
- epoch_reset() — resets ticks_used for all non-Inactive/Exited tasks
```

**Vấn đề phát hiện:**
- Không có proof rằng scan interval + heartbeat interval → violation detection bounded.
- Không proof epoch reset + budget interaction.

### Host test coverage hiện tại (liên quan)

| Section | Tests | Kani-style pure function tests |
|---|---|---|
| Grant | 14 tests | ❌ Không có — tất cả dùng globals trực tiếp |
| IRQ | 14 tests | ❌ Không có — tất cả dùng globals trực tiếp |
| Watchdog | 6 tests | ❌ Không có — gọi hàm dùng globals |
| Budget/Epoch | 4 tests | ❌ Không có |

---

## Thiết kế Phase P

### P1 — Pure Function Extraction (`#[cfg(kani)]`)

#### Khái niệm

Giống pattern đã dùng thành công trong Phase O (`ipc.rs`: `copy_message_pure`, `cleanup_pure`), tách logic thành hàm thuần **dưới `#[cfg(kani)]`** — chỉ compile cho Kani runner, **không refactor production code path**.

> **Consensus decision**: `#[cfg(kani)]` only. Hàm gốc giữ nguyên — zero regression risk. Mỗi pure function có TODO comment để migration khi cần.

- **Input:** immutable snapshot của array + parameters
- **Output:** new state hoặc `Result`
- **Hàm gốc KHÔNG thay đổi** — pure functions tồn tại song song, chỉ dùng trong Kani harnesses

Pattern (theo IPC precedent):

```rust
// Production code: KHÔNG đổi
pub fn grant_create(grant_id: usize, owner: usize, peer: usize) -> u64 {
    // ... giữ nguyên logic hiện tại ...
}

// Kani-only pure function: song song với production code
// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert
#[cfg(kani)]
fn grant_create_pure(
    grants: &[Grant; MAX_GRANTS],
    grant_id: usize, owner: usize, peer: usize,
) -> Result<Grant, u64> {
    // Logic thuần — không touch globals
}
```

#### Thiết kế dữ liệu

**Grant** — extract 3 pure functions (`#[cfg(kani)]`):

| # | Signature | Mô tả |
|---|---|---|
| 1 | `grant_create_pure(grants: &[Grant; 2], grant_id, owner, peer) → Result<Grant, u64>` | Validate + trả Grant mới (full symbolic, MAX_GRANTS=2) |
| 2 | `grant_revoke_pure(grants: &[Grant; 2], grant_id, caller) → Result<Grant, u64>` | Validate ownership, trả Grant state sau revoke |
| 3 | `grant_cleanup_pure(grants: &[Grant; 2], task_idx) → [Grant; 2]` | Trả array state mới sau cleanup |

**IRQ** — extract 3 pure functions (`#[cfg(kani)]`, constrained: intid 32–127, task_id < 8):

| # | Signature | Mô tả |
|---|---|---|
| 4 | `irq_bind_pure(table: &[IrqBinding; 8], intid, task_id, notify_bit) → Result<usize, u64>` | Validate + trả slot index |
| 5 | `irq_route_pure(table: &[IrqBinding; 8], intid) → Option<(usize, u64)>` | Tìm binding, trả (task_id, notify_bit) |
| 6 | `irq_cleanup_pure(table: &[IrqBinding; 8], task_id) → [IrqBinding; 8]` | Trả array sau cleanup |

**Watchdog/Budget** — extract 2 pure functions (`#[cfg(kani)]`):

| # | Signature | Mô tả |
|---|---|---|
| 7 | `watchdog_should_fault(interval: u64, elapsed: u64) → bool` | Kiểm tra vi phạm (interval > 0 && elapsed > interval) |
| 8 | `epoch_reset_pure(states: &[TaskState; NUM_TASKS], ticks_used: &[u64; NUM_TASKS]) → [u64; NUM_TASKS]` | Reset ticks_used cho non-Inactive/Exited tasks |

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/grant.rs` | Sửa | Thêm 3 `#[cfg(kani)]` pure functions (production code KHÔNG đổi) |
| `src/kernel/irq.rs` | Sửa | Thêm 3 `#[cfg(kani)]` pure functions (production code KHÔNG đổi) |
| `src/kernel/sched.rs` | Sửa | Thêm 2 `#[cfg(kani)]` pure functions cho watchdog/budget |
| `tests/host_tests.rs` | Sửa | Thêm ~8 unit tests cho pure functions (test cùng logic dùng direct struct construction) |

#### QEMU Checkpoint

Không thay đổi runtime behavior → **32/32 checkpoints hiện tại PHẢI vẫn pass** (regression test).

#### Backlog Item

- [ ] **Phase-Q+ migration trigger**: Khi module count > 6 hoặc pre-certification, migrate pure functions sang `always-available` (remove `#[cfg(kani)]`, refactor production code gọi pure functions)

---

### P2 — Kani Proofs Batch

#### Khái niệm

Viết Kani verification harnesses cho pure functions vừa extract ở P1. Target: **~8 proofs mới**, nâng tổng từ 10 → 18.

#### Grant proofs (3)

| # | Harness | Property được chứng minh |
|---|---|---|
| 1 | `grant_no_overlap` | Hai active grants không thể map cùng `peer_page` cho cùng peer |
| 2 | `grant_cleanup_completeness` | Sau cleanup, task không còn trong bất kỳ active grant nào (owner hoặc peer) |
| 3 | `grant_slot_exhaustion_safe` | Khi tất cả slots đầy, create trả lỗi — không corrupt state hiện có |

#### IRQ proofs (3)

| # | Harness | Property được chứng minh |
|---|---|---|
| 4 | `irq_route_correctness` | Route luôn deliver đúng `(task_id, notify_bit)` cho INTID đã bind |
| 5 | `irq_no_orphaned_binding` | Sau cleanup, không còn active binding nào cho `task_id` |
| 6 | `irq_bind_no_duplicate_intid` | Không thể bind cùng INTID hai lần |

#### Watchdog/Budget proofs (2)

| # | Harness | Property được chứng minh |
|---|---|---|
| 7 | `watchdog_violation_detection` | Nếu task không heartbeat trong `interval` ticks, `watchdog_should_fault` trả `true` |
| 8 | `budget_epoch_reset_fairness` | Mọi Ready/Running task đều được reset `ticks_used` khi epoch kết thúc |

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/grant.rs` | Sửa | Thêm 3 `#[cfg(kani)] #[kani::proof]` harnesses |
| `src/kernel/irq.rs` | Sửa | Thêm 3 `#[cfg(kani)] #[kani::proof]` harnesses |
| `src/kernel/sched.rs` | Sửa | Thêm 2 `#[cfg(kani)] #[kani::proof]` harnesses |

#### Xác nhận

```bash
# Trong aegis-dev Docker container
docker exec -w /workspaces/aegis aegis-dev cargo kani --tests
# Expected: 18/18 proofs pass
```

---

### P3 — Miri Skeleton (Deferred)

> **Consensus decision**: Không tích hợp Miri vào CI trong Phase P. Chỉ viết KernelCell shim skeleton (~15 dòng). Lý do: pure functions không có unsafe → Miri tìm nothing. RefCell shim verify semantics khác production UnsafeCell. DO-333 §6.3 không bắt buộc abstract interpretation khi đã có model checking (Kani).

#### Hành động

- Viết `#[cfg(miri)]` KernelCell alternative impl (~15 dòng) trong `src/kernel/cell.rs`
- Không thêm CI job, không thêm test annotations

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/cell.rs` | Sửa | Thêm `#[cfg(miri)]` shim (~15 dòng) |

#### Backlog

- [ ] "Miri CI integration — cần khi AegisOS có SMP hoặc preemptive kernel"

---

### P4 — Proof Coverage Mapping (FM.A-7) & README Refresh

#### Khái niệm

Tạo tài liệu mapping mỗi Kani proof → module → property → safety standard requirement. Đây là yêu cầu DO-333 objective FM.A-7 ("Verification of Verification Results").

#### Tài liệu FM.A-7

Tạo file `docs/standard/05-proof-coverage-mapping.md`:

```markdown
# Proof Coverage Mapping — DO-333 FM.A-7

## Bảng mapping

| # | Module | Proof Harness | Property | DO-333 | ISO 26262 | Phase |
|---|---|---|---|---|---|---|
| 1 | cap.rs | cap_check_bitwise_correctness | Logic correct | FM.A-5 | — | N |
| 2 | cap.rs | cap_for_syscall_no_panic_and_bounded | No panic | FM.A-5 | — | N |
| 3 | sched.rs | schedule_idle_guarantee | IDLE fallback | FM.A-4 | Part 6 §8 | N |
| 4 | sched.rs | restart_task_state_machine | State transition | FM.A-4 | Part 6 §8 | N |
| 5 | ipc.rs | ipc_queue_no_overflow | Queue bounds | FM.A-5 | Part 9 DFA | O |
| 6 | ipc.rs | ipc_message_integrity | Payload preserved | FM.A-5 | Part 9 DFA | O |
| 7 | ipc.rs | ipc_cleanup_completeness | Cleanup complete | FM.A-5 | Part 9 DFA | O |
| 8 | mmu.rs | pt_index_in_bounds | Index bounded | FM.A-5 | — | N |
| 9 | mmu.rs | pt_index_no_task_aliasing | No aliasing | FM.A-5 | Part 9 FFI | N |
| 10 | qemu_virt.rs | elf_load_addr_no_overlap | No overlap | FM.A-5 | — | O |
| 11–18 | grant/irq/sched | (Phase P proofs) | (See P2) | FM.A-5 | Part 9 | P |

## Uncovered Properties (backlog)
- Scheduler deadlock-freedom
- Priority inversion absence
- IPC timeout correctness (khi có)
- Grant delegation chain (khi có)

## Proof Limitations & Assumptions
- Kani unwinding bounds: MAX_GRANTS=4, MAX_IRQ=8, NUM_TASKS=8
- Single-core assumption (no data races)
- Pure functions only — không verify side effects (MMIO, GIC calls)
```

#### README Refresh (Option A+)

> **Consensus decision**: Fix numbers + source layout tree + "Formal Verification" paragraph + links + memory map fix. ~45–60 phút. Không full rewrite.

Cập nhật `README.md`:

| Section | Hiện tại (sai) | Cần sửa thành |
|---|---|---|
| Scheduler | "3 tasks" | 8 tasks, priority-based, watchdog, 6 states |
| Capabilities | "18 bits" | 19 bits (0–18, bao gồm CAP_EXIT) |
| Syscalls | "13" | 14 (0–13, bao gồm SYS_EXIT) |
| Tests | "189 host tests" | ~249 host tests (241 + ~8 Phase P) |
| Checkpoints | "25" | 32 |
| Memory map | "3×4KB stacks" | 8×4KB task + 8×4KB user stacks |
| Source layout | Missing | user/ workspace (libsyscall, hello, sensor, logger) |
| Thêm mới | — | "Formal Verification" paragraph + link FM.A-7 |
| Thêm mới | — | Link to `.github/copilot-instructions.md` cho full architecture |

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `docs/standard/05-proof-coverage-mapping.md` | **Tạo mới** | FM.A-7 proof coverage mapping |
| `README.md` | Sửa (lớn) | Full refresh tất cả section lỗi thời |
| `docs/.vitepress/config.mts` | Sửa | Thêm link `standard/05-*` + `plan/16-*` |
| `docs/index.md` | Sửa | Cập nhật stats (tests, proofs) |

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|---|---|---|
| 1 | Pure functions KHÔNG modify globals | Kani verify immutable inputs | Hàm nhận `&[T; N]`, trả giá trị mới |
| 2 | Kani harness KHÔNG dùng `static mut` | Kani cần deterministic state | Tạo local arrays trong harness, gọi pure function |
| 3 | No heap trong pure functions | Ràng buộc bất biến AegisOS | Trả fixed-size arrays hoặc `Result` |
| 4 | Kani unwinding bounds phải hữu hạn | `MAX_GRANTS=2`, `MAX_IRQ_BINDINGS=8`, `NUM_TASKS=8` | `#[kani::unwind(N)]` với N = max loop count + 1 |
| 5 | Miri deferred — skeleton shim only | Consensus: RefCell ≠ UnsafeCell | `#[cfg(miri)]` alternative impl, no CI |
| 6 | TrapFrame 288B — không đổi | ABI-locked | Phase P không đụng TrapFrame |
| 7 | Capability bits: 19/64 đã dùng | Phase P không thêm syscall/capability mới | 0 bits mới |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|---|
| 1 | Kani timeout trên IRQ harness (8 bindings × symbolic) | Trung bình | Trung bình | Giảm `MAX_IRQ_BINDINGS` trong harness (ví dụ 4), hoặc tăng `--cbmc-args --unwind 5` |
| 2 | ~~Miri false positives~~ | — | — | **Mitigated**: Miri deferred (skeleton only, no CI) |
| 3 | `#[cfg(kani)]` logic drift từ production code | Thấp | Trung bình | TODO comments + backlog trigger. Code stable 6 phases. |
| 4 | Grant cleanup asymmetry là design decision, không phải bug | Trung bình | Thấp | Document trong FM.A-7 + 4-line code comment, không sửa behavior |
| 5 | ~~Miri CI timeout~~ | — | — | **Mitigated**: Miri deferred |

---

## Test Plan

### Host unit tests mới (ước lượng: ~8 tests)

| # | Test case | Module | Mô tả |
|---|---|---|---|
| 1 | `test_grant_create_pure_basic` | grant | Pure function trả correct Grant |
| 2 | `test_grant_cleanup_pure_completeness` | grant | Cleanup removes task from all slots |
| 3 | `test_irq_bind_pure_basic` | irq | Pure function returns correct slot |
| 4 | `test_irq_route_pure_correctness` | irq | Route returns correct task+bit |
| 5 | `test_irq_cleanup_pure_completeness` | irq | Cleanup unbinds all for task |
| 6 | `test_watchdog_should_fault_basic` | sched | Returns true when exceeded interval |
| 7 | `test_watchdog_should_fault_within_interval` | sched | Returns false when within interval |
| 8 | `test_budget_epoch_pure` | sched | All eligible tasks flagged for reset |

### QEMU boot checkpoints mới

| # | Checkpoint UART output |
|---|---|
| — | Không có checkpoint mới — Phase P không thay đổi runtime behavior |

Verify: **32/32 existing checkpoints vẫn pass** (regression test).

### Kani proofs mới: 8 harnesses

(Chi tiết ở mục P2 phía trên — tổng cộng 18/18 proofs phải pass)

### Miri verification

CI job `miri-check` pass trên host tests (trừ asm-dependent tests).

---

## Thứ tự triển khai

| Bước | Sub-phase | Effort | Phụ thuộc | Checkpoint xác nhận |
|---|---|---|---|---|
| 1 | **P1** — Pure function extraction `#[cfg(kani)]` | 2–3h | — | 241 host tests pass + ~8 new pure function tests |
| 2 | **P2** — Kani proofs batch (tiered) | 3–4h | P1 | `cargo kani --tests` → 18/18 pass (aegis-dev Docker) |
| 3 | **P4** — FM.A-7 doc + README A+ + Miri shim + comments | 3–4h | P2 | Docs complete, 32/32 QEMU regression pass |
| **Tổng** | | **8–11h** | | |

**Ghi chú:** P2 và P4 có thể overlap nếu FM.A-7 drafted song song với Kani debug.

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|---|---|---|
| **DO-333** | FM.A-5 | Formal verification of source code — Kani proofs cho grant/irq/watchdog |
| **DO-333** | FM.A-7 | Verification of Verification Results — proof coverage mapping document |
| **DO-333** | §6.3 | Abstract Interpretation — Miri integration |
| **DO-178C** | §6.1 | Robustness — watchdog violation detection proof |
| **DO-178C** | §4 | Truy vết hai chiều — FM.A-7 mapping cung cấp proof↔requirement traceability |
| **ISO 26262** | Part 9 §7 | DFA (Dependent Failure Analysis) — grant cleanup completeness proof (freedom from interference) |
| **ISO 26262** | Part 6 §8 | Software unit verification — mở rộng Kani coverage |
| **IEC 62304** | §5.5.5 | Software unit verification — formal methods bổ sung testing |

---

## Backward Compatibility

| Thay đổi | Break API? | Break ABI? | Ghi chú |
|---|---|---|---|
| Pure function extraction | ❌ | ❌ | `#[cfg(kani)]` — chỉ compile cho Kani runner, production code KHÔNG đổi |
| Kani proofs | ❌ | ❌ | `#[cfg(kani)]` — chỉ compile cho Kani runner |
| Miri shim | ❌ | ❌ | `#[cfg(miri)]` — skeleton only, không ảnh hưởng binary |
| README/docs | ❌ | ❌ | Documentation only |

**Zero runtime changes.** Phase P là pure verification & documentation — không thay đổi behavior của kernel hay user tasks.

---

## Liên kết với phases trước

| Phase | Nền tảng sử dụng |
|---|---|
| **Phase J** | Grant + IRQ modules — code sẽ được refactor thêm pure functions |
| **Phase K** | Watchdog/budget logic — sẽ extract pure functions |
| **Phase M** | `KernelCell`, `klog!`, unsafe audit, coverage baseline 96.65% — infrastructure |
| **Phase N** | Kani pilot (6 proofs) — established pattern + aegis-dev Docker setup |
| **Phase O** | Kani IPC proofs (4 proofs) — pure function extraction pattern (`copy_message_pure`, `cleanup_pure`) |

---

## Checklist triển khai

1. [x] Review kế hoạch Phase P → consensus 100% (2 rounds)
2. [x] Cập nhật plan per consensus decisions
3. [x] **P1**: 8 pure functions `#[cfg(kani)]` (grant ×3, irq ×3, sched ×2) + TODO comments
4. [x] **P1**: 9 host unit tests cho pure functions (250/250 pass)
5. [x] **P1**: Verify 250 tests pass (241 existing + 9 new)
6. [x] **P2**: 3 Kani proofs cho grant (full symbolic, MAX_GRANTS=2)
7. [x] **P2**: 3 Kani proofs cho irq (constrained: intid 32–127, task_id < 8)
8. [x] **P2**: 2 Kani proofs cho watchdog/budget (constrained)
9. [x] **P2**: Verify 18/18 proofs pass trong aegis-dev Docker ✅
10. [x] **P4**: FM.A-7 mapping document (`docs/standard/05-proof-coverage-mapping.md`)
11. [x] **P4**: README refresh (A+: numbers + layout + FV paragraph + links)
12. [x] **P4**: Miri shim skeleton (`#[cfg(miri)]` KernelCell, ~15 lines)
13. [x] **P4**: Grant cleanup asymmetry comment (4 lines)
14. [x] **P4**: 1-line CI proof count check (ci.yml + pre-push.ps1)
15. [x] Regression: 32/32 QEMU checkpoints pass ✅
16. [x] Viết blog #16 kể chuyện Phase P ✅

### Backlog Items (Phase Q+)

- [ ] Migrate `#[cfg(kani)]` pure functions → always-available (trigger: module count > 6 or pre-cert)
- [ ] Miri CI integration (trigger: SMP or preemptive kernel)
- [ ] IPC backport: production code gọi pure functions (OUT of Phase P scope)
- [ ] Upgrade FM.A-7 CI check: WARN→FAIL at proof count > 25; automate at > 50
