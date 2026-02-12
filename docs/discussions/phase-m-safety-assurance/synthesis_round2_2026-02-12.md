# 🎼 Synthesis — Round 2 | 2026-02-12

## Chủ đề: Phase M — Safety Assurance Foundation

---

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) R2 | Gemini (Pragmatist) R2 | Đồng thuận? |
|---|----------------|---------------------|------------------------|-------------|
| 1 | M3 (Panic Handler) ưu tiên đầu | ✅ Giữ nguyên | ✅ Giữ nguyên | ✅ |
| 2 | Scope: 4 sub-phases (M3+M4+M2-lite+M1) | ADJUST: chấp nhận 4, M5/M6 **scheduled** Phase N | ADJUST: chấp nhận 4 progressive, ~30-40h | ✅ |
| 3 | Thứ tự: M3→M4→M2-lite→M1 | ADJUST: chấp nhận M4 trước M2 | MAINTAIN: M4 trước M2 (giữ R1) | ✅ |
| 4 | `static mut`: SAFETY comments → pilot → progressive | ADJUST: SAFETY bước 0 → pilot EPOCH_TICKS → TICK_COUNT → CURRENT → defer TCBS | ADJUST: SAFETY bước 0 → pilot TICK_COUNT → CURRENT+EPOCH_TICKS → defer TCBS | ✅ |
| 5 | Incremental, không big-bang | ✅ Giữ nguyên | ✅ Giữ nguyên | ✅ |
| 6 | Bảo vệ 189 tests | ✅ Giữ nguyên | ✅ Giữ nguyên | ✅ |
| 7 | Kani: exhaustive tests M → pilot Phase N | ADJUST: chấp nhận exhaustive M + Kani pilot N | MAINTAIN timing + CONCEDE giá trị dài hạn Kani | ✅ |
| 8 | Coverage: 75% overall + module-specific | ADJUST: 75% + 95/85/80 cap/elf/ipc | ADJUST: 75% + 95/85/80 cap/elf/ipc | ✅ |
| 9 | `cap.rs` ưu tiên cao nhất | ✅ Giữ nguyên | ✅ Giữ nguyên | ✅ |
| 10 | `arch/` không đo coverage | ✅ Giữ nguyên | ✅ Giữ nguyên | ✅ |
| 11 | Phase M safety → Phase N features + extend | ADJUST: chấp nhận với 3 guardrails | ADJUST: chấp nhận Phase M safety, Phase N features lead | ✅ |
| 12 | Quick wins (M0: clippy lints) | ADJUST: đồng ý, thêm `core::fmt` FP check | ✅ Đề xuất từ R1, giữ nguyên | ✅ |

---

## ✅ Các điểm đã đồng thuận: 12/12 (100%)

### Đồng thuận mới (6 điểm giải quyết từ Round 1 → Round 2):

1. **Scope Phase M = 4 sub-phases**: M3 + M4 + M2-lite + M1-progressive. M5 (Kani) và M6 (Traceability) scheduled cho Phase N. Effort: ~30-40h. GPT ADJUST từ 6 sub-phases, Gemini ADJUST từ "partial" lên "progressive".

2. **Thứ tự: M3→M4→M2-lite→M1**: GPT ADJUST chấp nhận "data first" (M4 trước M2). Gemini MAINTAIN stance R1. Cả hai align.

3. **`static mut` lộ trình 4 bước**: SAFETY comments (bước 0) → pilot encapsulate vars đơn giản (bước 1-2) → defer TCBS+ENDPOINTS Phase N (bước 3). GPT ADJUST từ "TCBS first" sang "simplest first". Gemini ADJUST từ "SAFETY comments only" sang "progressive encapsulation is the goal".

4. **Kani: exhaustive tests Phase M + Kani pilot Phase N**: GPT ADJUST từ "Kani now" sang "Kani Phase N". Gemini CONCEDE giá trị dài hạn Kani, MAINTAIN timing = Phase N. Cả hai đồng ý Kani pilot cho `cap.rs` Phase N.

5. **Coverage: 75% overall + module-specific targets**: GPT ADJUST từ 80% → 75%. Gemini ADJUST từ 70% → 75%. Module targets đồng nhất: 95% cap, 85% elf, 80% ipc, 75% sched, 70% grant/irq.

6. **Safety → Features flow**: GPT ADJUST từ "safety 100% trước mọi feature" sang "Phase M safety → Phase N features + verify". Gemini ADJUST từ "70/30 hybrid" sang "Phase M safety focused → Phase N features lead". Cả hai chấp nhận "verify small then grow" dạng adaptive.

### Điểm nhỏ còn khác biệt (không ảnh hưởng đồng thuận):

- **Pilot encapsulation order**: GPT muốn EPOCH_TICKS (2 refs) → TICK_COUNT (12 refs) → CURRENT. Gemini muốn TICK_COUNT → CURRENT+EPOCH_TICKS. Cả hai đều đúng (đều đi từ đơn giản → phức tạp). **Quyết định khi implementation — không cần đồng thuận trước.**

- **GPT thêm "Safety Readiness Checkpoint" document cuối Phase M** (~2-3h). Gemini không phản đối nhưng không mention. **Có thể include nếu budget cho phép.**

- **GPT thêm 3 guardrails cho Phase N** (coverage parity, core tests, Kani escalation). **Gemini thêm 4 conditions** (Phase N feature commitment, baseline measurement, exit criteria, 5-week timebox). **Tất cả compatible — include hết.**

---

## 📈 Tỷ lệ đồng thuận: 12/12 = **100%** ✅

---

## 🎯 Kết luận

**Đạt đồng thuận hoàn toàn sau 2 rounds.** Chuyển sang tạo Final Consensus document.
