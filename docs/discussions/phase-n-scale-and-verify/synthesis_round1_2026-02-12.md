# 🎼 Synthesis — Round 1 | 2026-02-12

## Chủ đề: Phase N — Scale & Verify (NUM_TASKS 3→8, `KernelCell` Arrays, Kani Pilot)

---

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|----------------|-----------------|---------------------|-------------|
| 1 | Scale strategy (Q1) | Option C — Parameterize tại 3, validate, flip to 8 | Option C — Parameterize tại 3, validate, flip to 8 | ✅ |
| 2 | `IDLE_TASK_ID` constant riêng (Q1 phụ) | Có — decoupled từ `NUM_TASKS` | Có — explicit, không assume `NUM_TASKS - 1` | ✅ |
| 3 | KernelCell order (Q2) | GRANTS→IRQ→ENDPOINTS→TCBS | GRANTS→IRQ→ENDPOINTS→TCBS | ✅ |
| 4 | Bỏ `cap_check_monotone` proof (Q3) | Bỏ — trivially correct | Bỏ — bitwise AND không thể OOB | ✅ |
| 5 | Sửa `has_capability()` → `cap_check()` (Q3) | Đồng ý sửa | Đồng ý sửa — factual error | ✅ |
| 6 | Sửa Kani property: bitmask subset, không phải "≤ 17" (Q3) | Đồng ý — "subset of `0x3FFFF`" | Đồng ý — factual error | ✅ |
| 7 | Giảm ELF bound 4096→128 bytes (Q3) | Đồng ý — tránh timeout, đủ cho header + 2 phdrs | Đồng ý — 128B đủ cover logic, 4096 sẽ timeout | ✅ |
| 8 | TaskConfig hybrid: const metadata + runtime entry (Q5) | Đồng ý — `fn() as u64` uncertain trên custom target | Đồng ý — ELF entry = runtime value | ✅ |
| 9 | Kani pilot scope: số lượng proofs (Q3) | **5 proofs** (3 cap + 1 elf + 1 cell) | **4 proofs** (2 cap + 1 elf + 1 cell) | ❌ |
| 10 | Sequencing N1-N2-N3 (Q4) | **N1 → N3a parallel → N2 → N3b-d** | **Strictly N1→N2→N3, không parallel** | ❌ |
| 11 | Effort estimate N1 (Q1) | Không điều chỉnh cụ thể, +2-3h cho validate step | **16-20h** (plan underestimates +4-6h do MMU debug) | ❌ |
| 12 | Effort estimate N2/TCBS (Q2) | Không điều chỉnh, nhưng gợi ý macro helper | **10-14h** cho TCBS (plan nói 8-10h) | ❌ |
| 13 | ELF load region + grant pages mở rộng? (Q1 phụ) | Không đề cập trực tiếp | ⚠️ Red flag — 12 KiB không đủ cho 8 tasks ELF | ❌ |

---

## ✅ Các điểm đã đồng thuận (8/13)

1. **Scale strategy = Option C**: Parameterize toàn bộ code tại `NUM_TASKS=3`, chạy full 219 tests + 28 QEMU checkpoints, confirm zero regression, rồi flip sang 8. Tách refactor risk khỏi scale risk.

2. **`IDLE_TASK_ID` constant riêng**: Không assume idle = `NUM_TASKS - 1`. Cả hai đồng ý decoupling giúp future dynamic task creation.

3. **KernelCell wrapping order = GRANTS→IRQ→ENDPOINTS→TCBS**: Experience-first, giảm risk cho TCBS (150+ refs). Cả hai đồng ý TCBS là complex nhất, nên wrap cuối.

4. **Bỏ `cap_check_monotone` proof**: Trivially correct (bitwise AND), proof không thêm giá trị.

5. **Sửa tên hàm `has_capability()` → `cap_check()`**: Plan có lỗi factual — hàm không tồn tại.

6. **Sửa Kani property**: `cap_for_syscall()` trả bitmask `u64`, property đúng là "return ⊆ `0x3FFFF`", không phải "≤ 17".

7. **ELF input bound 4096→128 bytes**: Tránh CBMC timeout, 128B đủ cho ELF header (64B) + 1 program header (56B).

8. **TaskConfig hybrid design**: `const` cho metadata (caps, priority, budget) + runtime cho entry points. Function pointers trên custom target không chắc const-safe.

---

## ❌ Các điểm bất đồng (5/13)

### Bất đồng #1: Kani pilot — 5 proofs (GPT) vs 4 proofs (Gemini)

- **GPT nói**: 5 proofs — giữ `cap_for_syscall_completeness` (verify mọi syscall 0..=12 có cap bit defined) + `cap_check_no_oob` (sửa tên từ `has_capability`). Reasoning: completeness proof có giá trị cho DO-333 evidence.
- **Gemini nói**: 4 proofs — bỏ thêm `cap_check_no_oob` vì `cap_check` chỉ là `caps & required != 0` — bitwise AND trên `u64` **không thể OOB hoặc panic**.
- **Khoảng cách**: 1 proof (`cap_check_no_oob`). GPT muốn verify function boundary (task_id < NUM_TASKS guard), Gemini thấy hàm quá đơn giản.
- **Gợi ý compromise**: Đọc lại source `cap_check()` — nếu hàm có `task_id` indexing vào array → proof có giá trị. Nếu chỉ là bitwise op trên 2 params → bỏ.

### Bất đồng #2: Sequencing — Semi-parallel (GPT) vs Strictly sequential (Gemini)

- **GPT nói**: N3a (Kani install + CI yaml) chạy song song N1 vì independent. N3b (cap.rs proof) song song N2 vì cap.rs không bị ảnh hưởng bởi `KernelCell` wrapping.
- **Gemini nói**: Strictly sequential — single developer, context switching giảm productivity. Dependencies cascade: N1→N2→N3.
- **Khoảng cách**: GPT thấy N3a thực sự independent (chỉ install tool + viết CI yaml). Gemini quan tâm developer productivity hơn theoretical parallelism.
- **Gợi ý compromise**: N3a (setup only) song song cuối N1 OK — đây là task nhỏ, ít context switch. Nhưng N3b-d proofs sequential sau N2. Thực tế: khi developer đang chờ QEMU test chạy → dùng thời gian đó setup Kani.

### Bất đồng #3: Effort estimate N1

- **GPT nói**: +2-3h cho validate step (Option C), nhưng không điều chỉnh tổng N1 cụ thể.
- **Gemini nói**: 16-20h (plan nói 12-14h) — MMU debug underestimated +4-6h. Page table off-by-one khó debug trên QEMU.
- **Khoảng cách**: ~4-6h. GPT thấy validate step bù trừ bằng giảm debug time. Gemini thấy MMU inherently risky bất kể validate.
- **Gợi ý compromise**: Budget **16-18h** cho N1 — accept Gemini's MMU debug buffer nhưng Option C's validate-first giảm phần nào debug need.

### Bất đồng #4: Effort estimate TCBS wrapping

- **GPT nói**: Không điều chỉnh cụ thể, nhưng gợi ý helper macro `kcell_index!()` cho Phase O.
- **Gemini nói**: 10-14h (plan nói 8-10h) — 150+ refs, interrupt context access, `reset_test_state()` migration.
- **Khoảng cách**: ~2-4h. GPT thấy macro sẽ giảm effort; Gemini thấy mỗi ref cần manual review.
- **Gợi ý compromise**: Budget **10-12h** cho TCBS. Macro helper nếu làm sẽ ở **Phase N** (không hoãn Phase O) để amortize across 150+ refs.

### Bất đồng #5: ELF load region + grant pages expansion

- **GPT nói**: Không đề cập — focus vào architectural decisions.
- **Gemini nói**: Red flag — `.elf_load` chỉ 12 KiB (3×4096), `.grant_pages` chỉ 8 KiB (2 pages). Nếu 8 tasks cần ELF/grants → thiếu.
- **Khoảng cách**: GPT chưa address. Cần clarify: tasks 3-7 chạy kernel functions hay ELF binaries? Grants cần cho bao nhiêu task pairs?
- **Gợi ý compromise**: Clarify scope trong plan: tasks 3-7 = kernel-internal idle (không ELF). `NUM_GRANTS` và `.elf_load` giữ nguyên Phase N, mở rộng Phase O khi thêm real ELF tasks.

---

## 📈 Tỷ lệ đồng thuận: 8/13 = **61.5%**

---

## 🎯 Hướng dẫn cho Round 2

### Câu hỏi cụ thể cho GPT (Visionary):
1. Bạn có đồng ý bỏ `cap_check_no_oob` proof nếu source code confirm `cap_check()` chỉ là bitwise op (không index array)?
2. Effort estimate N1 của bạn là bao nhiêu cụ thể (GPT chưa cho số)? Có chấp nhận 16-18h buffer không?
3. ELF load region + grant pages: scope Phase N chỉ kernel-internal tasks 3-7, hay cần plan cho ELF expansion?

### Câu hỏi cụ thể cho Gemini (Pragmatist):
1. N3a (Kani install + CI yaml, ~2-3h) song song cuối N1 — có chấp nhận ngoại lệ này không? (GPT argument: developer chờ QEMU test = dead time)
2. TCBS helper macro `kcell_index!()` — nếu làm trong Phase N (không hoãn Phase O), có giảm estimate TCBS về 8-10h không?
3. Budget tổng 45-54h — đây là realistic ceiling hay cần thêm buffer?

### Đề xuất compromise cần cả hai phản hồi:
1. **Kani proofs: 4 hay 5?** → Đọc `cap_check()` source, nếu pure bitwise → 4 proofs, nếu có indexing → 5 proofs.
2. **Sequencing**: N1 → N2 → N3, **ngoại trừ** N3a song song cuối N1? (micro-parallel, không full parallel)
3. **Effort**: N1=16-18h, TCBS=10-12h, tổng=45-55h — cả hai chấp nhận range này?

### Data/evidence cần bổ sung:
1. Source code `cap_check()` — exact function body để resolve proof #1 dispute
2. Source code `Tcb::new()` hoặc `EMPTY_TCB` — confirm const constructibility
3. `.elf_load` section layout — hiện tại tasks nào dùng ELF?
