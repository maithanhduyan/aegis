# 🔭 GPT-Visionary Review — Round 2 | 2026-02-12

## Chủ đề: Phase M — Safety Assurance Foundation

## Lời mở đầu

Round 1, tôi đặt ra tầm nhìn 10-20 năm với full 6 sub-phases, Kani đầu tư ngay, KernelCell từ TCBS, 80% coverage, và "verify small then grow" tuyệt đối. Sau khi đọc kỹ phản biện của Gemini và bản synthesis của Orchestra, tôi nhận ra: **tầm nhìn đúng nhưng lộ trình cần điều chỉnh**. Một chiến lược 10 năm tốt không có nghĩa là phải frontload toàn bộ effort vào Phase M. seL4 cũng mất 4 năm (2005→2009) để hoàn thành proof đầu tiên — họ không làm tất cả trong 1 phase.

Dưới đây tôi trả lời từng điểm bất đồng và 4 câu hỏi cụ thể từ Orchestra.

---

## ❌ Bất đồng #1: Scope Phase M — 6 sub-phases hay rút gọn?

**Stance: ADJUST — Chấp nhận 4 sub-phases core, nhưng với điều kiện rõ ràng cho M5/M6.**

### Ghi nhận lập luận Gemini

Gemini đúng ở hai điểm cốt lõi:
1. **80-100h cho solo developer là quá lớn** cho một phase thuần safety — đặc biệt khi chưa có deadline certification thực sự.
2. **Burnout risk là thực** — 12 phases intense coding rồi chuyển sang 6 phases pure audit sẽ giết momentum.

### Điều chỉnh của tôi

Tôi chấp nhận **Phase M = M3 + M4 + M2-lite + M1 (progressive)** như Orchestra đề xuất. Tuy nhiên, tôi yêu cầu:

1. **M5 (Kani) và M6 (Traceability) phải nằm trong Phase N roadmap** — không phải "defer vô thời hạn" mà là "scheduled cho Phase N, bắt đầu bằng pilot scope." Sự khác biệt: defer = có thể quên; scheduled = có commitment.
2. **Phase M kết thúc bằng một "Safety Readiness Checkpoint"** — document ngắn (~1 trang) liệt kê: (a) coverage baseline đã đo, (b) unsafe audit status, (c) gaps còn lại, (d) Phase N safety tasks. Đây là seed cho Safety Case document tương lai.

### Tại sao ADJUST chứ không MAINTAIN?

Trong Round 1, tôi trích dẫn seL4 và INTEGRITY RTOS để argue cho full 6 sub-phases. Nhưng tôi bỏ qua một sự thật quan trọng: **cả seL4 lẫn INTEGRITY đều có team 5-15 người và funding nhiều năm**. Solo developer với ~40-50h budget cần chiến lược khác — không phải giảm ambition, mà là **phân kỳ ambition**. 4 sub-phases trong Phase M + 2 sub-phases trong Phase N = vẫn đạt 6/6 mục tiêu, chỉ spread ra hợp lý hơn.

### Ước tính giờ (trả lời câu hỏi Orchestra #1)

| Sub-phase | Effort ước tính | Ghi chú |
|-----------|----------------|---------|
| M3 (Panic Handler) | 3-4h | Quick win, không đổi API |
| M4 (Coverage Setup + Baseline) | 2-3h setup + 8-10h viết tests mới | Để đạt 75% target |
| M2-lite (klog! macro) | 4-6h | Compile-time filtering, không buffer |
| M1 (Unsafe Audit — progressive) | 8-12h | SAFETY comments + pilot encapsulation 2-3 globals |
| Safety Readiness Checkpoint | 2-3h | Document status + Phase N plan |
| **Tổng** | **~27-38h** | Nằm trong budget 40-50h |

Nếu budget 40-50h, tôi **không cắt** sub-phase nào — 4 sub-phases đã là compromise. Buffer 10-15h còn lại dùng cho: unexpected issues, QEMU regression debugging, hoặc bắt đầu sớm M5 pilot nếu momentum tốt.

Nếu bắt buộc phải cắt thêm: **M2-lite** là thứ hy sinh được — `uart_print!` hiện tại vẫn hoạt động, chỉ thiếu metadata. M3+M4+M1 là bộ ba không thể tách rời.

---

## ❌ Bất đồng #2: Thứ tự sau M3 — M2 hay M4?

**Stance: ADJUST — Chấp nhận M3 → M4 → M2-lite → M1.**

### Ghi nhận lập luận Gemini

Gemini có argument thuyết phục: *"cargo-llvm-cov setup ~1-2h, output là lcov report. Bạn sẽ biết ngay cap.rs covered 90% hay 40%. Data guides decisions."* Đây là nguyên tắc **evidence-based decision making** — chính xác là thứ mà DO-178C §5.1 yêu cầu: "planning activities should be based on available data."

### Tại sao tôi thay đổi

Round 1, tôi đặt M2 trước M4 vì cho rằng logging hỗ trợ debug khi refactor M1. Nhưng suy nghĩ lại:

1. **M4 là non-destructive** — chạy coverage tool KHÔNG thay đổi code, không ảnh hưởng 189 tests. Zero risk.
2. **M2-lite cũng non-destructive** — thêm macro mới, không thay API cũ. Low risk.
3. **Cả M4 lẫn M2-lite đều nên xảy ra TRƯỚC M1** (refactor) — M4 cho data, M2 cho debug tools. Thứ tự M4 vs M2 ít quan trọng hơn việc cả hai phải trước M1.
4. **M4 nhanh hơn M2** (2-3h vs 4-6h) → làm trước cho quick feedback loop.

Thứ tự final: **M3 (3-4h) → M4 (2-3h setup) → M2-lite (4-6h) → M1 (8-12h progressive).**

---

## ❌ Bất đồng #3: `static mut` — KernelCell ngay hay SAFETY comments trước?

**Stance: ADJUST — Chấp nhận SAFETY comments là bước 0, pilot TICK_COUNT trước TCBS.**

### Ghi nhận lập luận Gemini

Tôi đã kiểm tra thực tế codebase và Gemini đúng về effort:
- `TCBS` có **~40+ references** trong `host_tests.rs` (sched::TCBS[i].state, .context.x[n], .fault_tick, .entry_point, .caps, ...)
- `ENDPOINTS` có **~20+ references**
- `TICK_COUNT` có **12 references** trong tests (không phải 5 như Gemini ước tính, nhưng vẫn ít hơn TCBS 3x)
- `EPOCH_TICKS` chỉ có **2 references** — đây mới là biến đơn giản nhất

Bắt đầu encapsulate từ TCBS (40+ test references, interrupt context) thay vì TICK_COUNT (12 references, đơn giản) giống như **học lái xe bằng Formula 1** — đúng mục tiêu nhưng sai chiến thuật.

### Điều chỉnh cụ thể

Tôi chấp nhận lộ trình Orchestra đề xuất, với bổ sung:

| Bước | Biến target | Effort | Phase |
|------|------------|--------|-------|
| Bước 0 | SAFETY comments cho **tất cả** 8 globals | 3-4h | M (M1) |
| Bước 1 | Pilot encapsulate `EPOCH_TICKS` (2 refs) + `TICK_INTERVAL` (private, 0 test refs) | 2-3h | M (M1) |
| Bước 2 | Encapsulate `TICK_COUNT` (12 refs) + `CURRENT` (10+ refs) | 4-6h | M (M1) |
| Bước 3 | `TCBS` + `ENDPOINTS` + `GRANTS` + `IRQ_BINDINGS` | 10-15h | **Phase N** |

**Tại sao EPOCH_TICKS trước TICK_COUNT?** Vì EPOCH_TICKS chỉ có **2 references** trong tests — nó là pilot scope nhỏ nhất có thể. Nếu pattern hoạt động (189 tests pass, QEMU OK), mở rộng sang TICK_COUNT (12 refs) với confidence.

### Điều tôi MAINTAIN

1. **Encapsulation là mục tiêu cuối, không phải SAFETY comments.** SAFETY comments là bước 0 cần thiết (documentation), nhưng mục tiêu là safe API wrapper. Comment nói "single-core" nhưng không có gì trong type system enforce điều đó — DO-178C auditor sẽ flag điểm này.
2. **Pattern phải là `KernelCell<T>` hoặc tương đương** — không phải chỉ move vào module-private. Access phải qua function boundary để Kani/Miri có thể reason về invariants.
3. **Test helpers**: Khi encapsulate, tạo `#[cfg(test)] pub fn test_set_tick_count(v: u64)` thay vì expose `pub static mut`. Điều này giữ API sạch cho production code nhưng vẫn cho phép test setup.

---

## ❌ Bất đồng #4: Kani — đầu tư bây giờ hay defer?

**Stance: ADJUST — Exhaustive tests Phase M, Kani pilot Phase N cho `cap.rs`.**

### Ghi nhận lập luận Gemini

Gemini đúng ở ba điểm thực tế:
1. **50-60h cho Kani là quá lớn** trong budget 40-50h — nó một mình đã vượt toàn bộ Phase M.
2. **Windows → WSL/Docker friction là thật** — mỗi iteration loop chậm hơn 2-3x so với native.
3. **Exhaustive tests cho bounded inputs (234 cap cases, malformed ELF) cho value ngay** — không cần learning curve.

### Tại sao tôi thay đổi

Round 1, tôi argue Kani ROI 10 năm. Tôi vẫn tin điều đó. Nhưng ROI 10 năm không có nghĩa là phải trả cost ngay năm 1. Phân tích lại:

- **Exhaustive tests** cho `cap.rs` (234 cases, ~2h) → **proves absence of bugs** cho bounded input space NGAY, chạy trên Windows native, tích hợp vào 189 tests. Value: **immediate + concrete**.
- **Kani proof** cho `cap.rs` (~8-10h bao gồm setup) → **proves absence of bugs** cho arbitrary input space, nhưng cần WSL, learning curve, Docker. Value: **stronger but delayed**.

Cả hai đều "prove absence of bugs" — chỉ khác scope (bounded vs arbitrary) và timing (now vs later). Vì AegisOS có **static bounds** (NUM_TASKS=3, MAX_ENDPOINTS=4), exhaustive tests với bounded inputs **IS** exhaustive verification cho current configuration. Kani adds value khi bounds tăng (NUM_TASKS=8+) hoặc khi cần certification evidence.

### Lộ trình cụ thể

**Phase M (10-12h):**
- Exhaustive tests cho `cap.rs`: tất cả 18 bits × 13 syscalls = 234 cases (~2h)
- Fuzz-like tests cho `elf.rs`: malformed headers, overflow offsets, segment overlap (~3-4h)
- Property-based tests cho `ipc.rs`: send→recv correctness, double-recv rejection (~4-5h)
- Miri run trên toàn bộ host_tests (~1h setup — zero ongoing cost)

**Phase N (8-10h):**
- Kani pilot: setup Docker + verify `has_capability()` + `cap_for_syscall()` (~4-5h)
- Nếu pilot thành công: thêm Kani proofs cho `parse_elf64()` no-OOB (~4-5h)
- Tích hợp vào CI Docker image

### Điều tôi MAINTAIN

**Kani KHÔNG phải optional cho long-term.** Exhaustive tests prove "these 234 inputs work correctly." Kani proves "ALL possible inputs work correctly." Khi NUM_TASKS tăng lên 8 hoặc 16, exhaustive space tăng exponentially — chỉ Kani (bounded model checking) scale được. DO-333 formal methods supplement sẽ là competitive advantage thực sự khi AegisOS đến gần certification.

---

## ❌ Bất đồng #5: Coverage target — 70% hay 80%?

**Stance: ADJUST — Chấp nhận 75% overall + module-specific targets.**

### Ghi nhận lập luận Gemini

Gemini đúng: sự khác biệt 70% vs 80% overall ít quan trọng hơn module-specific targets. Một kernel với 80% overall nhưng 40% `cap.rs` nguy hiểm hơn kernel với 70% overall nhưng 95% `cap.rs`.

### Điều chỉnh

| Module | Target Phase M | Lý do |
|--------|---------------|-------|
| `kernel/cap.rs` | **95%** | Gateway mọi syscall — consensus cả hai bên |
| `kernel/elf.rs` | **85%** | Parse untrusted input — giảm từ 90% (Round 1) vì parser error paths khó trigger hết trên host |
| `kernel/ipc.rs` | **80%** | Core IPC state machine — consensus gần |
| `kernel/sched.rs` | **75%** | Nhiều nhánh cần QEMU (fault restart, real timer) |
| `kernel/grant.rs` | **70%** | Medium criticality |
| `kernel/irq.rs` | **70%** | Medium criticality |
| `kernel/timer.rs` | **65%** | Chủ yếu arch-specific, chỉ tick_count() portable |
| **Overall kernel/** | **≥75%** | Weighted average tự nhiên từ module targets |

### Tại sao 75% chứ không giữ 80%?

NASA JPL "Rule of Ten" mà tôi trích Round 1 nói rằng cost tăng exponentially sau 80%. Nhưng rule đó áp dụng cho mature codebases — AegisOS là prototype. Ở giai đoạn prototype, **going from 50% → 75% gives the highest bug-find rate** (Capers Jones, "Software Assessments, Benchmarks, and Best Practices", 2000). Pushing to 80% có thể đợi Phase N khi tests ổn định hơn.

### Lộ trình coverage (điều chỉnh từ Round 1)

- **Phase M**: Statement coverage ≥75% kernel/, module-specific targets — baseline + gap analysis + targeted tests
- **Phase O-P (2026-2027)**: Decision coverage ≥80% — thêm branch testing
- **Phase R+ (2027-2028)**: MC/DC cho `cap.rs`, `elf.rs` — tool investment khi Rust MC/DC mature hơn

---

## ❌ Bất đồng #6: Safety-first vs Hybrid

**Stance: ADJUST — Chấp nhận "Phase M safety → Phase N small feature → verify lại" nhưng với guardrails.**

### Ghi nhận lập luận Gemini

Gemini có một insight tôi không thể bỏ qua: *"NUM_TASKS=3→8 là thay đổi constants + array size, không thay đổi algorithm."* Tôi đã verify điều này trong codebase:

- `NUM_TASKS` là constant → thay đổi 1 dòng
- `TCBS: [Tcb; NUM_TASKS]` → array tự mở rộng
- Scheduler algorithm (priority ordering, budget accounting) → **không đổi**
- Capability system → **không đổi** (per-task bitmask, không phụ thuộc NUM_TASKS)
- IPC → **không đổi** (endpoint-based, task ID chỉ là index)

Core invariants (capability soundness, IPC correctness, priority ordering, fault isolation) **thực sự không thay đổi** khi tăng NUM_TASKS. Gemini đúng ở điểm này.

### Điều chỉnh — nhưng với guardrails

Tôi chấp nhận flow:

```
Phase M (safety, ~30-38h)
    → Safety Readiness Checkpoint
    → Phase N bắt đầu bằng NUM_TASKS=8 (~8-10h)
    → Chạy lại full test suite + coverage
    → Nếu regression → fix trước khi feature tiếp
```

**Guardrails tôi yêu cầu:**

1. **"Expand then verify" rule**: Sau mỗi feature expansion (NUM_TASKS=8, thêm syscall, etc.), phải đạt **cùng coverage target** trước khi feature tiếp. Ví dụ: nếu Phase M đạt 75% kernel/ → Phase N expand NUM_TASKS=8 → phải đạt lại 75% cho new test cases trước khi Phase N feature tiếp.

2. **Core invariant tests KHÔNG được skip**: Tests cho capability soundness, IPC state machine, priority ordering phải pass 100% — không phải chỉ "189 tests pass" mà "189 tests + N new tests cho 8 tasks" pass.

3. **Formal verification escalation**: Nếu Phase N expand NUM_TASKS lên 8, thì Kani pilot cho `cap.rs` phải xảy ra trong Phase N (không defer thêm). Lý do: 18 bits × 13 syscalls × 8 tasks = 1,872 cases — vẫn exhaustive-testable, nhưng đang tiến gần ranh giới mà manual test maintenance trở nên fragile. Kani proof sẽ **scale** khi Phase O mở thêm.

### Điều tôi MAINTAIN

**"Verify small then grow" vẫn đúng — chỉ định nghĩa "verify" flexible hơn.** Round 1, tôi định nghĩa "verify" = Kani + traceability + full coverage. Round 2, tôi chấp nhận "verify" = coverage ≥75% + exhaustive tests cho critical modules + SAFETY audit. Đây vẫn là **verification trước expansion** — chỉ là scope verification phù hợp hơn với giai đoạn prototype.

Những thảm họa tôi trích Round 1 (Therac-25, Boeing 737 MAX) vẫn relevant — nhưng context khác. Therac-25 deploy cho bệnh nhân thật mà không verify. AegisOS chạy trên QEMU với 0 users. Risk profile khác → verification depth phải tương xứng.

---

## 🎯 Trả lời 4 câu hỏi cụ thể từ Orchestra

### Câu 1: Ước tính giờ và budget 40-50h

**Ước tính full M1-M6 của tôi: ~90-110h** (gần với Gemini ước tính 80-100h, tôi thêm buffer cho WSL/Docker setup Kani).

**Với budget 40-50h**, tôi chọn M3+M4+M2-lite+M1(progressive) = ~27-38h. Buffer 10-15h cho unexpected issues. **Không cắt sub-phase nào** trong 4 sub-phases core — nếu ép phải cắt, hy sinh M2-lite (logging) vì `uart_print!` vẫn dùng được.

### Câu 2: Kani timing

**ADJUST** — Tôi đồng ý rằng:
- **Exhaustive tests add value NOW** (Phase M) — 234 cap cases, malformed ELF, IPC properties. Value tương đương Kani proofs cho current bounded configuration.
- **Kani adds DIFFERENT value LATER** (Phase N) — proves correctness cho arbitrary inputs, scales khi bounds tăng, provides DO-333 evidence.

Cả hai **complementary, không thay thế nhau**. Phase M: exhaustive tests + Miri. Phase N: Kani pilot cho cap.rs.

### Câu 3: `static mut` order

**ADJUST** — Tôi đồng ý pilot `TICK_COUNT` (thực tế là `EPOCH_TICKS` trước vì chỉ 2 references) thay vì TCBS. Gemini đúng: bắt đầu từ biến đơn giản nhất để validate pattern, rồi mở rộng.

Tuy nhiên, data thực tế cho thấy `TICK_COUNT` có **12 references** trong tests (không phải 5 như Gemini ước tính). Vì vậy tôi đề xuất: `EPOCH_TICKS` (2 refs, bước 1) → `TICK_COUNT` (12 refs, bước 2) → `CURRENT` (10+ refs, bước 3) → `TCBS`+phần còn lại (defer Phase N).

### Câu 4: Hybrid approach

**ADJUST** — Flow Phase M (safety) → Phase N (NUM_TASKS=8) → rerun tests **CÓ thỏa mãn** "verify small then grow," với điều kiện:

1. Phase M đạt coverage ≥75% kernel/ và exhaustive tests cho critical modules trước khi kết thúc
2. Phase N bắt đầu bằng feature nhỏ (NUM_TASKS=8) rồi immediately verify (coverage + new tests)
3. Kani pilot cho `cap.rs` nằm trong Phase N (không defer thêm)

Tôi **KHÔNG yêu cầu** complete verification (Kani + traceability) trước mọi feature. Nhưng tôi yêu cầu **verification level phải escalate cùng với complexity** — NUM_TASKS=3 chỉ cần exhaustive tests, NUM_TASKS=8 cần Kani pilot, NUM_TASKS=16+ cần full formal proofs.

---

## ✅ Xác nhận điểm đồng thuận bổ sung

### Quick wins (clippy lints)

Round 1, tôi không đề cập `#![deny(unsafe_op_in_unsafe_fn)]` và `clippy::undocumented_unsafe_blocks`. Tôi **đồng ý hoàn toàn** — đây là zero-risk, high-value, và nên làm **ngay đầu Phase M** (trước cả M3). Effort ~2h, automated lint output trực tiếp feed vào M1 audit. Coi như **M0: Quick Lints**.

### `core::fmt` FP concern

Gemini flag concern rằng `core::fmt` có thể emit FP instructions. Tôi đồng ý đây phải **verify trước M2** — nếu `core::fmt` emit FP instructions, structured logging phải dùng manual integer-to-string conversion thay vì `write!` macro. 30 phút disassemble check → critical blocker cho M2 design.

---

## 📊 Tóm tắt stance Round 2

| # | Điểm bất đồng | Round 1 | Round 2 | Thay đổi |
|---|---------------|---------|---------|----------|
| 1 | Scope Phase M | 6 sub-phases, ~90-110h | 4 sub-phases, ~27-38h, M5/M6 scheduled Phase N | **ADJUST** |
| 2 | Thứ tự sau M3 | M2 → M1 → M4 | M4 → M2-lite → M1 (data first) | **ADJUST** |
| 3 | `static mut` strategy | KernelCell từ TCBS ngay | SAFETY comments (bước 0) → pilot EPOCH_TICKS → TICK_COUNT → CURRENT → TCBS defer Phase N | **ADJUST** |
| 4 | Kani timing | Đầu tư ngay, 50-60h | Exhaustive tests Phase M + Kani pilot Phase N | **ADJUST** |
| 5 | Coverage target | 80% kernel/ | 75% kernel/ + module-specific (95% cap, 85% elf, 80% ipc) | **ADJUST** |
| 6 | Safety vs Features | Safety 100% trước mọi feature | Phase M safety → Phase N small feature + verify, với guardrails | **ADJUST** |

| # | Câu hỏi Orchestra | Trả lời |
|---|-------------------|---------|
| Q1 | Hour estimate, 40-50h budget | ~90-110h full; 40-50h → M3+M4+M2-lite+M1, hy sinh M2-lite nếu ép |
| Q2 | Kani timing | **ADJUST** — exhaustive tests now (Phase M), Kani pilot later (Phase N) |
| Q3 | static mut order | **ADJUST** — EPOCH_TICKS (2 refs) → TICK_COUNT (12 refs) → CURRENT → defer TCBS |
| Q4 | Hybrid approach | **ADJUST** — chấp nhận với 3 guardrails (coverage parity, core tests, Kani escalation) |

---

## 🔑 Nguyên tắc không thay đổi

Dù điều chỉnh 6/6 điểm, tôi **MAINTAIN** ba nguyên tắc nền tảng:

1. **Encapsulation là mục tiêu cuối cho `static mut`** — SAFETY comments là bước đầu, không phải đích đến. AegisOS hướng tới certification → type system enforcement > documentation.

2. **Kani (formal verification) là required cho long-term** — không phải "nice to have" mà là "must have" khi AegisOS tiến tới deployment thực tế. Phase N pilot, không phải Phase ∞.

3. **Verification level phải escalate cùng complexity** — kernel nhỏ (3 tasks) cần exhaustive tests, kernel vừa (8 tasks) cần Kani pilot, kernel lớn (16+ tasks, multi-core) cần full formal proofs. Đây là "verify small then grow" dạng adaptive — không rigid, nhưng có direction.

*"The best time to plant a tree was 20 years ago. The second best time is now — but you don't need to plant the entire forest today."*
