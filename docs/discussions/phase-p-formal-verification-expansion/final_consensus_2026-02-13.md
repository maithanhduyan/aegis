# 🤝 Final Consensus | Phase P: Formal Verification Expansion | 2026-02-13

## Tổng quan

- **Chủ đề**: Phase P — Formal Verification Expansion (Kani proofs cho grant/irq/watchdog, FM.A-7 mapping, README refresh)
- **Số vòng thảo luận**: 2
- **Ngày bắt đầu → Đồng thuận**: 2026-02-13 → 2026-02-13
- **Participants**: GPT-Visionary-Agent (Visionary), Gemini-Pragmatist-Agent (Pragmatist)

---

## Kết luận đồng thuận

### 1. Pure Function Extraction — `#[cfg(kani)]` + Formal Backlog

**Quyết định**: Pure functions dưới `#[cfg(kani)]` (Kani-only), không refactor production code path.

**Lý do**:
- *Pragmatist*: Zero regression risk — 241 tests + 32 QEMU checkpoints không bị ảnh hưởng. Code stable qua 6 phases, drift risk theoretical.
- *Visionary*: Chấp nhận vì code stability + motivation drain risk. Backlog item đảm bảo migration không bị quên.

**Hành động tiếp theo**:
- Viết 8 pure functions: grant (3) + irq (3) + watchdog/budget (2) dưới `#[cfg(kani)]`
- TODO comment trong mỗi function: `// TODO(Phase-Q+): migrate to always-available when module count > 6 or pre-cert`
- Backlog item trong Phase P plan checklist với trigger condition

---

### 2. Kani Proof Granularity — Tiered per Module

**Quyết định**: Full symbolic cho grant, constrained cho irq và watchdog.

**Lý do**:
- *Cả hai*: MAX_GRANTS=2 → ~10 biến → full symbolic trivially tractable. MAX_IRQ_BINDINGS=8 + u32 INTID → intractable full symbolic. Constrain: `intid 32–127`, `task_id < NUM_TASKS`.
- Timeout budget ≤5 phút/proof là hard constraint.

**Hành động tiếp theo**:
- 8 Kani proofs: grant (3: no_overlap, cleanup_completeness, slot_exhaustion), irq (3: route_correctness, no_orphaned_binding, no_duplicate_intid), watchdog (1: violation_detection), budget (1: epoch_reset_fairness)
- Document constraint strength (Full/Constrained) trong FM.A-7
- `#[kani::unwind(9)]` cho IRQ/watchdog proofs

---

### 3. Miri — Defer CI, Skeleton Shim Only

**Quyết định**: Không tích hợp Miri vào CI trong Phase P. Chỉ viết KernelCell shim skeleton (~15 dòng).

**Lý do**:
- *Pragmatist*: Pure functions không có unsafe → Miri tìm nothing. RefCell shim verify semantics khác production UnsafeCell. ROI âm (4–5h cho verification theater).
- *Visionary*: Chấp nhận — RefCell ≠ UnsafeCell argument decisive. Skeleton shim plant infrastructure seed cho SMP future.
- *Cả hai*: DO-333 §6.3 không bắt buộc abstract interpretation khi đã có model checking (Kani).

**Hành động tiếp theo**:
- Viết `#[cfg(miri)]` KernelCell alternative impl (~15 dòng) — no CI job, no test annotations
- Ghi vào backlog: "Miri CI integration — cần khi AegisOS có SMP hoặc preemptive kernel"

---

### 4. Grant Cleanup Asymmetry — Document as Intentional

**Quyết định**: Không sửa code. Thêm 4 dòng comment giải thích + document trong FM.A-7.

**Lý do**:
- *Cả hai*: Asymmetry có lý do kỹ thuật — peer fault giữ owner field vì owner vẫn sống + MMU mapping active. Full zero sẽ unmap owner's page → gây crash. `active=false` đã gate mọi access path.
- *Pragmatist*: Phase P constraint "zero runtime changes" — ngay cả cosmetic fix vi phạm principle.
- *Visionary*: Chấp nhận drop `phys_addr` zeroing — `grant_create` overwrite toàn bộ slot khi reuse.

**Hành động tiếp theo**:
- 4 dòng comment trong `grant.rs` cleanup peer fault path
- "Design Decisions" section trong FM.A-7 document
- Kani proof `grant_cleanup_completeness` verify behavior AS-IS

---

### 5. FM.A-7 Document — Comprehensive + 1-line CI Check

**Quyết định**: Comprehensive document (bảng mapping + uncovered properties + limitations + design decisions). Không automation script. 1-line CI check.

**Lý do**:
- *Pragmatist*: 18 proofs không cần script. Automation ROI âm dưới 50 proofs. `grep -c` mất 0.1s.
- *Visionary*: 1-line CI check captures 90% automation value. Chấp nhận.
- *Cả hai agree*: WARN bây giờ, upgrade FAIL khi proof count > 25.

**Hành động tiếp theo**:
- Tạo `docs/standard/05-proof-coverage-mapping.md` với 18-row mapping table
- Sections: Bảng mapping, Uncovered Properties, Proof Limitations & Assumptions, Design Decisions
- 1-line CI: `test $(grep -rc 'kani::proof' src/) -eq 18 || echo "WARN: proof count mismatch"`
- Backlog: "Upgrade WARN→FAIL at proof count > 25; automate FM.A-7 at proof count > 50"

---

### 6. README Refresh — Option A+ (Numbers + Layout + Links)

**Quyết định**: Fix numbers + source layout tree + "Formal Verification" paragraph + links + memory map fix. ~45–60 phút.

**Lý do**:
- *Pragmatist*: Full rewrite = 2–3h, chiếm 40% effort cho docs. Fix numbers = 80% value.
- *Visionary*: Chấp nhận A+ vì covers critical gaps: `user/` workspace, formal verification mention, accurate numbers. Memory map fix thêm ~5 phút.
- *Cả hai*: `.github/copilot-instructions.md` đã là detailed source of truth. README = elevator pitch + accurate numbers + links.

**Hành động tiếp theo**:
- Fix: 3→8 tasks, 18→19 bits, 13→14 syscalls, 189→~249 tests, 25→32 checkpoints, 10→18 proofs
- Thêm: `user/` source layout, "Formal Verification" paragraph + link FM.A-7
- Fix: memory map table (3×4KB → 8×4KB stacks)
- Thêm: link "Full architecture → `.github/copilot-instructions.md`"

---

## Lộ trình thực hiện

| Giai đoạn | Sub-phase | Hành động | Effort | Ưu tiên |
|---|---|---|---|---|
| **Bước 1** | P1 | Pure function extraction `#[cfg(kani)]` (8 functions) + host tests | 2–3h | P0 |
| **Bước 2** | P2 | 8 Kani proofs (tiered) — verify trong aegis-dev Docker | 3–4h | P0 |
| **Bước 3** | P4 | FM.A-7 doc + README A+ + Miri shim skeleton + comments | 3–4h | P0 |
| **Tổng** | | | **8–11h** | |

**Ghi chú**: P2 và P4 có thể overlap nếu FM.A-7 drafted song song với Kani debug.

---

## Trade-offs đã chấp nhận

### 1. `#[cfg(kani)]` pure functions = logic duplication
- **Chấp nhận vì**: Zero regression risk, code stable 6 phases, backlog item ensures future migration
- **Cả hai sides accept**: Visionary accept timing (not never, just not now). Pragmatist accept commitment (backlog with trigger, not defer indefinitely).

### 2. Miri deferred = no abstract interpretation coverage
- **Chấp nhận vì**: Kani (model checking) đủ cho DO-333 khi không có multi-core. Miri verify wrong thing (shim, not production UnsafeCell).
- **Cả hai sides accept**: Visionary accept ROI argument. Pragmatist accept skeleton seed.

### 3. README partial update = not comprehensive
- **Chấp nhận vì**: Effort budget constraint (10–12h total). Numbers + layout + links = 80% value. Full rewrite deferred to future phase.
- **Cả hai sides accept**: Visionary accept effort target. Pragmatist accept scope expansion beyond numbers-only.

### 4. Grant cleanup asymmetry retained
- **Chấp nhận vì**: Technical justification (owner alive). Documentation + Kani proof verify AS-IS behavior.
- **Cả hai sides accept**: Asymmetry is design decision, not bug.

### 5. Constrained IRQ proofs = not exhaustive
- **Chấp nhận vì**: Timeout budget ≤5min. Full symbolic intractable for 8 bindings. Constraints documented in FM.A-7 with upgrade path.
- **Cả hai sides accept**: Constrained proof > no proof. Escalation path exists.

---

## Appendix: Lịch sử thảo luận

| Round | GPT Review | Gemini Review | Synthesis | Đồng thuận |
|---|---|---|---|---|
| 1 | [review_gpt_round1](review_gpt_round1_2026-02-13.md) | [review_gemini_round1](review_gemini_round1_2026-02-13.md) | [synthesis_round1](synthesis_round1_2026-02-13.md) | 17% (1/6) |
| 2 | [review_gpt_round2](review_gpt_round2_2026-02-13.md) | [review_gemini_round2](review_gemini_round2_2026-02-13.md) | [synthesis_round2](synthesis_round2_2026-02-13.md) | **100% (6/6)** |

## Appendix: Files trong thư mục thảo luận

```
docs/discussions/phase-p-formal-verification-expansion/
├── 00_brief_2026-02-13.md
├── review_gpt_round1_2026-02-13.md
├── review_gemini_round1_2026-02-13.md
├── synthesis_round1_2026-02-13.md
├── review_gpt_round2_2026-02-13.md
├── review_gemini_round2_2026-02-13.md
├── synthesis_round2_2026-02-13.md
└── final_consensus_2026-02-13.md
```
