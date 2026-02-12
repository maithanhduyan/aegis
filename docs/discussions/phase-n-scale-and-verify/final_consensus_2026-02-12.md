# 🤝 Final Consensus | Phase N: Scale & Verify | 2026-02-12

## Tổng quan

- **Chủ đề**: Phase N — Scale NUM_TASKS 3→8, `KernelCell` Struct Arrays, Kani Formal Verification Pilot
- **Số vòng thảo luận**: 2
- **Ngày bắt đầu → Đồng thuận**: 2026-02-12 → 2026-02-12
- **Participants**: GPT-Visionary-Agent (Visionary), Gemini-Pragmatist-Agent (Pragmatist)
- **Tỷ lệ**: Round 1 = 61.5% → Round 2 = 100%

---

## Kết luận đồng thuận

### 1. Scale Strategy = Option C (Parameterize → Validate → Flip)

**Quyết định:** Refactor toàn bộ hardcoded `3` thành `NUM_TASKS` constant + computed `pt_index()`, **giữ `NUM_TASKS=3`**, chạy full 219 tests + 28 QEMU checkpoints. Nếu pass → flip sang `NUM_TASKS=8` + update linker sizes.

**Lý do:**
- *Visionary*: Tách refactor risk khỏi scale risk — debug thất bại biết ngay nguyên nhân. Foundation cho future NUM_TASKS=16/32.
- *Pragmatist*: MMU 594 dòng + 13 constants là rủi ro cao nhất. Validate ở N=3 = safety net miễn phí.

**Hành động tiếp theo:**
1. Thêm `pub const IDLE_TASK_ID: usize` — explicit, decoupled từ `NUM_TASKS`
2. Implement `pt_index(task_id, table_type) → usize` thay toàn bộ `PT_L*_TASK*`
3. Validate pass → đổi `NUM_TASKS=8`, linker sections, test lại

### 2. KernelCell Wrapping Order = GRANTS → IRQ → ENDPOINTS → TCBS

**Quyết định:** Experience-first. Wrap từ đơn giản→phức tạp. Macro `kcell_index!()` built tại N2.1 (GRANTS).

**Lý do:**
- *Visionary*: TCBS 150+ refs across 7 files + interrupt context. Sai pattern = kernel crash. Cần kinh nghiệm từ 3 globals đơn giản trước.
- *Pragmatist*: GRANTS (~20 refs) + IRQ (~15 refs) = bãi tập an toàn. Macro built sớm → amortize across ENDPOINTS + TCBS.

**Hành động tiếp theo:**
1. Build `kcell_index!()` macro tại N2.1
2. Wrap GRANTS → test pass → wrap IRQ → test pass → wrap ENDPOINTS → test pass → wrap TCBS → test pass
3. Sau N2.4: 0 `static mut` globals remaining

### 3. Kani Pilot = 4 Proofs

**Quyết định:** 4 bounded model checking proofs:

| # | Proof | Module | Property |
|---|-------|--------|----------|
| 1 | `cap_for_syscall_no_panic` | `cap.rs` | No panic, return ⊆ `0x3FFFF` |
| 2 | `cap_for_syscall_completeness` | `cap.rs` | Mọi syscall 0..=12 có cap bit defined |
| 3 | `parse_elf64_no_panic` | `elf.rs` | No panic/OOB cho mọi input ≤ 128 bytes |
| 4 | `kernel_cell_roundtrip` | `cell.rs` | get/get_mut consistency |

**Lý do:**
- *Visionary*: 4 proofs cover 3 modules — pilot vừa đủ cho DO-333 evidence. `sched.rs` hoãn Phase O (cần mock globals).
- *Pragmatist*: `cap_check()` = pure bitwise, proof vô nghĩa. `elf.rs` 128B bound tránh CBMC timeout. ROI tối ưu.

**Hành động tiếp theo:**
1. `cargo install kani-verifier && cargo kani setup` (N3a, micro-parallel)
2. Thêm CI job `kani-proofs` vào `.github/workflows/ci.yml`
3. Implement 4 proof harnesses với `#[cfg(kani)]`

### 4. Sequencing = N1→N2→N3, N3a Micro-Parallel

**Quyết định:** Strictly sequential cho code changes. N3a (Kani install + CI yaml) chạy trong QEMU wait time — infrastructure only, zero proof code.

**Lý do:**
- *Visionary*: N3a genuinely independent — just tool installation + yaml authoring.
- *Pragmatist*: Single developer, context switching kills productivity. QEMU wait time (~30s-1m per boot) = dead time tận dụng được.

**Hành động tiếp theo:**
```
Week 1: N1a → N1b → N1c → N1d [16-18h]
Week 2: N2.1 (GRANTS + macro) → N2.2 (IRQ) → N2.3 (ENDPOINTS) [8-11h]
Week 3: N2.4 (TCBS) [10-12h] + N3a (Kani setup, micro-parallel)
Week 4: N3b → N3c → N3d (proofs) + integration test [8-10h]
```

### 5. TaskConfig = Hybrid (const metadata + runtime entry)

**Quyết định:** `const TASK_METADATA: [TaskMetadata; NUM_TASKS]` cho caps/priority/budget + runtime array cho entry points.

**Lý do:**
- *Visionary*: Const metadata = documentation-as-code, compiler-verified. Future config management migration ready.
- *Pragmatist*: `fn() as u64` unreliable trên custom target. ELF entry = runtime value. Hybrid = pragmatic split.

**Hành động tiếp theo:**
1. Define `TaskMetadata` struct (caps, priority, budget)
2. `const TASK_METADATA` array in `main.rs`
3. Runtime `entries: [u64; NUM_TASKS]` array in `kernel_main()`
4. Loop-based `init_task()` calls

### 6. ELF Load + Grants = Giữ nguyên Phase N, Mở rộng Phase O

**Quyết định:** `.elf_load` (12 KiB), `NUM_GRANTS` (2) giữ nguyên. Tasks 3-7 = kernel-internal idle functions, không ELF.

**Lý do:**
- *Visionary*: Phase N focus = parameterize + scale infrastructure. Real user tasks = Phase O scope.
- *Pragmatist*: Evidence xác nhận chỉ task 2 dùng ELF. Mở rộng khi chưa cần = premature.

**Hành động tiếp theo:**
- Thêm scope note trong plan Phase N
- Phase O: thêm real ELF user tasks + mở rộng `.elf_load` + `NUM_GRANTS`

### 7. Plan Corrections (Factual Errors)

**Quyết định:** Sửa 3 lỗi factual trong plan trước khi implement:

| # | Lỗi | Sửa |
|---|------|-----|
| 1 | `has_capability()` referenced | → `cap_check()` (hàm thực tế) |
| 2 | Kani property "return ≤ 17" | → "return ⊆ `0x3FFFF`" (bitmask, không bit index) |
| 3 | ELF bound 4096 bytes | → 128 bytes (tránh CBMC timeout) |

---

## Lộ trình thực hiện

| Giai đoạn | Timeline | Hành động | Ưu tiên | Hard Ceiling |
|-----------|----------|-----------|---------|-------------|
| **N1** Scale | Week 1 (16-18h) | Constants, MMU `pt_index()`, `TaskConfig`, linker, stubs | P0 | 18h |
| **N2** KernelCell | Week 2-3 (18-24h) | `kcell_index!()` macro + wrap 4 globals: GRANTS→IRQ→ENDPOINTS→TCBS | P0 | 24h |
| **N3** Kani | Week 3-4 (8-10h) | Setup (micro-parallel) + 4 proof harnesses + CI job | P1 | 10h |
| **Integration** | Week 4 (2-3h) | Coverage re-measure ≥95%, 30 QEMU checkpoints | P0 | 3h |
| **Tổng** | **4 tuần part-time** | | | **50h** |

---

## Trade-offs đã chấp nhận

1. **Option C thêm ~2-3h validate** vs giảm debug uncertainty: Cả hai chấp nhận — safety net miễn phí cho MMU refactor rủi ro cao.

2. **4 proofs thay vì 6**: Bỏ 2 trivially-correct proofs (`cap_check_no_oob`, `cap_check_monotone`). Chấp nhận vì bitwise ops đã compiler-guaranteed.

3. **Sequential thay vì parallel**: Mất ~2-3h theoretical parallelism. Chấp nhận vì single developer + context switching cost > parallel benefit.

4. **TCBS wrap cuối (delay safety benefit)**: 150+ refs, interrupt context — risk-of-error quá cao nếu wrap sớm. Experience-first tradeoff chấp nhận.

5. **ELF/Grants defer Phase O**: Không mở rộng khi chưa cần. Chấp nhận vì tasks 3-7 chỉ cần function pointers.

6. **`kcell_index!()` macro thêm complexity**: +1h build, nhưng +1.5-2h saved trên 150+ TCBS refs + consistency guarantee. Net positive ROI.

---

## Appendix: Lịch sử thảo luận

| Round | GPT Review | Gemini Review | Synthesis | Đồng thuận |
|-------|-----------|---------------|-----------|------------|
| 1 | [review_gpt_round1](review_gpt_round1_2026-02-12.md) | [review_gemini_round1](review_gemini_round1_2026-02-12.md) | [synthesis_round1](synthesis_round1_2026-02-12.md) | 61.5% (8/13) |
| 2 | [review_gpt_round2](review_gpt_round2_2026-02-12.md) | [review_gemini_round2](review_gemini_round2_2026-02-12.md) | [synthesis_round2](synthesis_round2_2026-02-12.md) | 100% (13/13) |
