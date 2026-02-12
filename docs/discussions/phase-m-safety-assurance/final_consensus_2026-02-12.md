# 🤝 Final Consensus | Phase M — Safety Assurance Foundation | 2026-02-12

## Tổng quan

- **Chủ đề**: Phase M — Safety Assurance Foundation cho AegisOS
- **Số vòng thảo luận**: 2
- **Ngày bắt đầu → Đồng thuận**: 2026-02-12 → 2026-02-12
- **Participants**: GPT-Visionary-Agent (Visionary), Gemini-Pragmatist-Agent (Pragmatist)
- **Điều phối**: 🎼 Orchestra Agent

---

## Kết luận đồng thuận

### 1. Phase M Scope = 4 Sub-phases + Quick Lints Preamble

**Quyết định**: Phase M gồm **M0 (Quick Lints) + M3 + M4 + M2-lite + M1-progressive**. M5 (Kani) và M6 (Traceability) **scheduled cho Phase N** — không defer vô thời hạn, có commitment cụ thể.

**Lý do**:
- *Visionary*: 6 sub-phases đầy đủ (~90-110h) vượt capacity solo developer. 4 sub-phases tạo safety foundation minimum viable. M5/M6 cần prerequisites (Linux CI stable, API stable) mà Phase M chưa đáp ứng.
- *Pragmatist*: ~30-40h scope hợp lý cho solo developer. Đủ ngắn để không burnout, đủ dài để tạo foundation có ý nghĩa. 4 sub-phases = natural stopping point.

**Effort ước tính**: ~30-40h (4-5 ngày full-time hoặc ~3-4 tuần part-time)

**Hành động tiếp theo**:
- [ ] Tạo plan document `docs/plan/13-plan-phase-m-safety-assurance.md`
- [ ] Setup M0: `#![deny(unsafe_op_in_unsafe_fn)]` + `clippy::undocumented_unsafe_blocks` (~2h)
- [ ] Verify `core::fmt` không emit FP instructions (disassemble check, ~30 phút)

---

### 2. Thứ tự triển khai: M0 → M3 → M4 → M2-lite → M1

**Quyết định**: Data first, tools second, refactor last.

| Bước | Sub-phase | Effort | Mô tả | Risk |
|------|-----------|--------|--------|------|
| 1 | **M0: Quick Lints** | ~2-3h | `deny(unsafe_op_in_unsafe_fn)` + clippy lints + `core::fmt` FP check | Zero |
| 2 | **M3: Enhanced Panic** | ~3-4h | File:line, task ID, tick count, ESR/FAR trong panic output | Zero |
| 3 | **M4: Coverage Baseline** | ~2-4h setup + ~12-15h tests | `cargo-llvm-cov` setup, đo baseline, gap analysis, viết targeted tests | Zero (setup), Low (tests) |
| 4 | **M2-lite: Structured Log** | ~4-6h | `klog!` macro, compile-time level filtering, tick+task metadata | Low |
| 5 | **M1: Unsafe Audit** | ~12-16h | SAFETY comments + progressive encapsulation (3 bước) | Medium |

**Lý do**:
- M0 automated lints feed trực tiếp vào M1 audit list — effort ~2h, output = danh sách unsafe blocks cần document
- M3 (panic) cải thiện debug cho mọi sub-phase sau — nếu regression xảy ra, diagnostic info đầy đủ
- M4 (coverage) cho data thực để guide M2 (log ở đâu?) và M1 (unsafe nào chưa test?)
- M2-lite (logging) cho debug tools cho M1 refactor
- M1 (refactor) cuối cùng — khi đã có data, tools, và diagnostic info

---

### 3. `static mut` — Lộ trình Progressive Encapsulation

**Quyết định**: SAFETY comments trước (documentation debt), encapsulation progressive (technical debt). Bắt đầu từ **đơn giản nhất**, không phải critical nhất.

| Bước | Biến target | Refs trong tests | Effort | Phase |
|------|------------|-----------------|--------|-------|
| **Bước 0** | SAFETY comments cho **tất cả** 8 globals + 44 unsafe blocks | N/A | ~3-4h | M (M1) |
| **Bước 1** | Pilot: `EPOCH_TICKS` (2 refs) + `TICK_INTERVAL` (0 test refs) | 2 | ~2-3h | M (M1) |
| **Bước 2** | `TICK_COUNT` (12 refs) + `CURRENT` (10+ refs) | ~22 | ~5-7h | M (M1) |
| **Bước 3** | `TCBS` + `ENDPOINTS` + `GRANTS` + `IRQ_BINDINGS` | ~60+ | ~15-20h | **Phase N** |

**Pattern**: `KernelCell<T>` — zero-cost abstraction trên `UnsafeCell<T>`. Access qua function boundary để formal tools (Kani/Miri) có thể reason. Test helpers via `#[cfg(test)] pub fn test_set_*()`.

**Nguyên tắc**: Mỗi bước PHẢI pass 189 host tests + 25 QEMU checkpoints trước khi tiến bước tiếp.

**Lý do**:
- *Visionary*: Encapsulation là mục tiêu cuối — SAFETY comments là documentation, không phải enforcement. Formal tools và certification auditors cần API boundary, không phải comments.
- *Pragmatist*: SAFETY comments trước vì: (a) forced review of assumptions, (b) zero risk, (c) encapsulation bên trong KernelCell vẫn cần SAFETY comment. Start simple (EPOCH_TICKS, 2 refs) → validate pattern → scale.

---

### 4. Formal Verification — Exhaustive Tests Now, Kani Later

**Quyết định**: Phase M = exhaustive tests + property-based tests + Miri (~10-12h). Phase N = Kani pilot cho `cap.rs` (~8-10h).

**Phase M (immediate value)**:

| Target | Approach | Effort | Value |
|--------|----------|--------|-------|
| `cap.rs` | Exhaustive: 18 bits × 13 syscalls = 234 cases | ~2h | 100% input space covered cho current config |
| `elf.rs` | Fuzz-like: malformed headers, overflow offsets, segment overlap | ~3-4h | Catch panic paths, OOB access |
| `ipc.rs` | Property-based: send→recv correctness, double-recv rejection | ~4-5h | State machine consistency |
| All | Miri run trên host_tests | ~1h setup | Detect undefined behavior |

**Phase N (long-term value)**:

| Target | Approach | Effort | Value |
|--------|----------|--------|-------|
| `cap.rs` | Kani proof: `has_capability()` + `cap_for_syscall()` no-panic, no-OOB | ~4-5h | Proves absence of bugs cho **arbitrary** inputs |
| `elf.rs` | Kani proof: `parse_elf64()` no-OOB | ~4-5h | Proves parser safety |

**Lý do**:
- *Visionary*: Kani proves **absence of bugs** (exhaustive tests chỉ checks expected behavior). DO-333 formal methods = competitive advantage cho certification. Kani KHÔNG phải optional long-term.
- *Pragmatist*: 50-60h Kani cho Phase M = over budget. Exhaustive tests 234 cases cho `cap.rs` ~2h = immediate value. Prerequisites Kani (Linux CI, API stable) chưa đáp ứng. Same destination, smoother journey.
- *Cả hai đồng ý*: Exhaustive tests bây giờ → foundation cho Kani proofs sau. Complementary, không thay thế nhau.

**Verification escalation rule**: NUM_TASKS=3 → exhaustive tests. NUM_TASKS=8 → Kani pilot. NUM_TASKS=16+ → full formal proofs.

---

### 5. Code Coverage: 75% Overall + Module-Specific Targets

**Quyết định**: Statement coverage ≥75% cho `kernel/` modules, driven by module-specific targets.

| Module | Target Phase M | Criticality | Rationale |
|--------|---------------|-------------|-----------|
| `kernel/cap.rs` | **95%** | 🔴 Critical | Gateway mọi syscall — sai = privilege escalation |
| `kernel/elf.rs` | **85%** | 🔴 Critical | Parse untrusted input — sai = code execution |
| `kernel/ipc.rs` | **80%** | 🔴 Critical | Core IPC state machine — sai = deadlock/corruption |
| `kernel/sched.rs` | **75%** | 🟠 High | Scheduler logic — nhiều nhánh cần QEMU |
| `kernel/grant.rs` | **70%** | 🟡 Medium | Shared memory — medium complexity |
| `kernel/irq.rs` | **70%** | 🟡 Medium | IRQ routing — medium complexity |
| `kernel/timer.rs` | **65%** | 🟢 Low | Chủ yếu arch-specific, chỉ `tick_count()` portable |
| **Overall kernel/** | **≥75%** | — | Weighted average từ module targets |

**Quy trình**: Đo baseline (M4) → gap analysis → viết targeted tests → re-measure. Tool: `cargo-llvm-cov`.

**Điều kiện**: Nếu baseline < 40%, re-evaluate targets. Nếu baseline > 70%, celebrate + push to 80%.

**`arch/` code**: Không đo coverage trên host. Verify bằng 25 QEMU boot checkpoints + manual review.

**Lộ trình coverage dài hạn**:

| Giai đoạn | Level | Target | Timeline |
|-----------|-------|--------|----------|
| Phase M | Statement Coverage | ≥75% kernel/ | Now |
| Phase O-P | Decision Coverage | ≥80% kernel/ | 2026-2027 |
| Phase R+ | MC/DC (critical modules) | cap.rs, elf.rs | 2027-2028 |

---

### 6. Safety-First → Features Follow (Adaptive Verification)

**Quyết định**: Phase M = safety focused (4 sub-phases). Phase N = features lead + safety extend. Mỗi phase sau Phase M interleave features + verification.

**Flow**:
```
Phase M (safety, ~30-40h, max 5 tuần)
    → Safety Readiness Checkpoint (document status + Phase N plan)
    → Phase N: NUM_TASKS=8 (~8-10h) + Kani pilot cap.rs (~8-10h)
        + encapsulate TCBS/ENDPOINTS (~15-20h)
    → Rerun full test suite + coverage → đạt coverage parity
    → Phase O onwards: features lead + safety follows
```

**Guardrails (từ GPT)**:
1. **"Expand then verify" rule**: Sau mỗi feature expansion → đạt cùng coverage target trước feature tiếp
2. **Core invariant tests KHÔNG skip**: Capability soundness, IPC state machine, priority ordering = 100% pass
3. **Verification escalation**: Complexity tăng → verification depth tăng tương ứng

**Conditions (từ Gemini)**:
1. **Phase N feature commitment**: Phase N PHẢI bắt đầu bằng features, không thêm pure safety sub-phases
2. **Baseline measurement**: Đo thực tế trước khi commit targets
3. **Exit criteria Phase M**: (a) panic handler enhanced, (b) coverage measured + documented, (c) `klog!` hoạt động, (d) SAFETY comments 100%, (e) ≥4 globals encapsulated, (f) 189 tests + 25 checkpoints pass
4. **Timebox**: Phase M max 5 tuần. Chưa xong → defer, chuyển Phase N

**Lý do**:
- *Visionary*: "Verify small then grow" — AegisOS ở trạng thái lý tưởng (~3,500 LOC, 3 tasks, 4 endpoints). Cửa sổ vàng. Foundation verification không mất giá trị khi expand — cần extend, không redo.
- *Pragmatist*: "Cửa sổ vàng" real. Core invariants survive NUM_TASKS expansion (extend ~6-9h, not redo ~30-40h). 4 sub-phases acceptable scope, Phase N phải có features để duy trì momentum.

---

## Lộ trình thực hiện

| Giai đoạn | Timeline | Hành động | Ưu tiên |
|-----------|----------|-----------|---------|
| **Phase M** | 0-5 tuần | M0 (lints) → M3 (panic) → M4 (coverage) → M2-lite (logging) → M1 (unsafe audit + encapsulate 4 globals) | **P0** |
| **Phase N** | 5-12 tuần | NUM_TASKS=8 + Kani pilot `cap.rs` + encapsulate TCBS/ENDPOINTS + extend tests | **P0** |
| **Phase O-P** | 3-6 tháng | Features (filesystem? RISC-V port?) + Decision Coverage + Kani expand (elf.rs, ipc.rs) | **P1** |
| **Phase R+** | 1-3 năm | MC/DC cho critical modules + Safety Case document + WCET analysis | **P2** |
| **Long-term** | 5-10 năm | Full formal proofs + multi-core support + certification readiness | **P3** |

---

## Trade-offs đã chấp nhận

### 1. M5 (Kani) và M6 (Traceability) defer sang Phase N
**Why both accept**: Kani cần Linux CI stable + API stable + learning curve 15-20h = vượt Phase M budget. Traceability manual = maintenance burden sẽ outdated nhanh. Exhaustive tests Phase M cho value tương đương cho current bounded config. Kani scheduled Phase N, không defer vô thời hạn.

### 2. Coverage target 75% thay vì 80% (GPT) hay 70% (Gemini)
**Why both accept**: Module-specific targets (95% cap, 85% elf, 80% ipc) là driver thực sự — weighted average tự nhiên ~75%. Difference 70→75% chỉ ~2-3h thêm. 75% = "nghiêm túc" trong safety documentation mà không excessive cho prototype.

### 3. SAFETY comments trước encapsulation (thay vì encapsulate ngay)
**Why both accept**: GPT đúng rằng comments không đủ cho certification. Gemini đúng rằng documentation debt phải trả trước technical debt (comments = forced review, zero risk). Encapsulation bên trong `KernelCell<T>` vẫn cần SAFETY comment. Lộ trình progressive = validate pattern trước khi scale.

### 4. Pilot từ EPOCH_TICKS/TICK_COUNT thay vì TCBS (critical nhất)
**Why both accept**: GPT đúng rằng TCBS critical nhất. Gemini đúng rằng TCBS phức tạp nhất (~40+ test refs, interrupt context, struct access). Start simple → validate pattern → scale = engineer best practice. "Critical nhất" ≠ "refactor đầu tiên".

### 5. Phase M = safety only, Phase N = features + safety extend
**Why both accept**: GPT's "verify small then grow" + Gemini's "burnout prevention" → Phase M safety focused (acceptable scope ~30-40h) + Phase N starts with features (maintains momentum). Core invariants survive expansion → verification foundation retains value.

---

## Appendix: Lịch sử thảo luận

| Round | GPT Review | Gemini Review | Synthesis | Đồng thuận |
|-------|-----------|---------------|-----------|------------|
| 1 | [review_gpt_round1](review_gpt_round1_2026-02-12.md) | [review_gemini_round1](review_gemini_round1_2026-02-12.md) | [synthesis_round1](synthesis_round1_2026-02-12.md) | 50% (6/12) |
| 2 | [review_gpt_round2](review_gpt_round2_2026-02-12.md) | [review_gemini_round2](review_gemini_round2_2026-02-12.md) | [synthesis_round2](synthesis_round2_2026-02-12.md) | **100% (12/12)** |

---

## Appendix: Đề xuất bổ sung đã đồng thuận

Cả hai agents đề xuất các items bổ sung tương thích:

| Đề xuất | Agent | Phase | Effort | Status |
|---------|-------|-------|--------|--------|
| `#![deny(unsafe_op_in_unsafe_fn)]` | Gemini R1 + GPT R2 | M (M0) | ~2h | ✅ Include |
| `clippy::undocumented_unsafe_blocks` | Gemini R1 + GPT R2 | M (M0) | ~0h | ✅ Include |
| `core::fmt` FP instruction check | Gemini R1 + GPT R2 | M (M0) | ~30min | ✅ Include |
| Safety Readiness Checkpoint doc | GPT R2 | M (end) | ~2-3h | ✅ Include if budget allows |
| Miri run on host_tests | GPT R2 | M (M4) | ~1h | ✅ Include |
| Safety Case document (PSAC) | GPT R1 | Future | TBD | 📋 Track for Phase O+ |
| WCET analysis | GPT R1 | Future | TBD | 📋 Track for Phase R+ |
| Fault Injection Testing | GPT R1 | Future | TBD | 📋 Track for Phase O+ |
| RISC-V Readiness Score | GPT R1 | Each phase | ~30min | 📋 Track |
| Convention-based traceability (test naming) | Gemini R1 | Phase N (M6) | ~4-6h | 📋 Track for Phase N |

---

*Consensus achieved in 2 rounds. Both agents moved from 50% agreement to 100% through evidence-based argumentation and practical compromise. Key insight: long-term vision (Visionary) and short-term feasibility (Pragmatist) are not opposites — they're different time horizons of the same strategy.*
