# 🎼 Synthesis — Round 2 | 2026-02-12

## Chủ đề: Phase N — Scale & Verify (NUM_TASKS 3→8, `KernelCell` Arrays, Kani Pilot)

---

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|----------------|-----------------|---------------------|-------------|
| 1 | Scale strategy = Option C | ✅ (R1) | ✅ (R1) | ✅ |
| 2 | `IDLE_TASK_ID` constant riêng | ✅ (R1) | ✅ (R1) | ✅ |
| 3 | KernelCell order GRANTS→IRQ→ENDPOINTS→TCBS | ✅ (R1) | ✅ (R1) | ✅ |
| 4 | Bỏ `cap_check_monotone` proof | ✅ (R1) | ✅ (R1) | ✅ |
| 5 | Sửa `has_capability()` → `cap_check()` | ✅ (R1) | ✅ (R1) | ✅ |
| 6 | Sửa Kani property: bitmask subset `0x3FFFF` | ✅ (R1) | ✅ (R1) | ✅ |
| 7 | ELF bound 4096→128 bytes | ✅ (R1) | ✅ (R1) | ✅ |
| 8 | TaskConfig hybrid: const metadata + runtime entry | ✅ (R1) | ✅ (R1) | ✅ |
| 9 | **Kani 4 proofs** (bỏ `cap_check_no_oob`) | ✅ ACCEPT (R2) | ✅ MAINTAIN (R2) | ✅ |
| 10 | **Sequential + N3a micro-parallel** | ✅ ACCEPT (R2) | ✅ ACCEPT (R2) | ✅ |
| 11 | **Effort N1 = 16-18h** | ✅ ACCEPT (R2) | ✅ ACCEPT (R2) | ✅ |
| 12 | **Effort TCBS = 10-12h + `kcell_index!()` macro in N2** | 🔄 COUNTER 10-12h (R2) | ✅ ACCEPT 10-12h (R2) | ✅ |
| 13 | **ELF + grants defer Phase O** | ✅ ACCEPT (R2) | ✅ ACCEPT, withdraw red flag (R2) | ✅ |

---

## ✅ Các điểm đã đồng thuận (13/13)

**Round 1 (8 điểm):**
1. Scale strategy = Option C (parameterize at 3, validate, flip to 8)
2. `IDLE_TASK_ID` = explicit constant, decoupled từ `NUM_TASKS`
3. KernelCell order = GRANTS → IRQ_BINDINGS → ENDPOINTS → TCBS
4. Bỏ `cap_check_monotone` Kani proof (trivially correct)
5. Sửa plan: `has_capability()` → `cap_check()` (factual error)
6. Sửa Kani property: return ⊆ `0x3FFFF` (bitmask, không phải "≤ 17")
7. ELF input bound 4096→128 bytes (tránh CBMC timeout)
8. TaskConfig = hybrid (`const` metadata + runtime entry points)

**Round 2 (5 điểm):**
9. **Kani scope = 4 proofs**: GPT accept bỏ `cap_check_no_oob` sau evidence cho thấy `cap_check()` = pure bitwise `(caps & required) == required`
10. **Sequencing = N1→N2→N3 sequential**, N3a (Kani install + CI yaml) micro-parallel trong QEMU wait time. Điều kiện: infrastructure only, zero proof code.
11. **Effort N1 = 16-18h** (18h hard ceiling). Option C giảm ~2h debug vs jump thẳng.
12. **Effort TCBS = 10-12h**: `kcell_index!()` macro built tại N2.1, dùng cho N2.2-N2.4. 12h hard ceiling.
13. **ELF + grants = defer Phase O**: Tasks 3-7 = kernel-internal idle, `.elf_load` (12 KiB) và `NUM_GRANTS` (2) giữ nguyên. Documentation note trong plan.

---

## ❌ Các điểm bất đồng (0/13)

Không còn.

---

## 📈 Tỷ lệ đồng thuận: 13/13 = **100%** 🎉

---

## Consensus Effort Summary

| Sub-phase | Effort | Hard Ceiling | Notes |
|-----------|--------|-------------|-------|
| N1 (Scale) | 16-18h | 18h | MMU debug buffer included; Option C validate-first |
| N2 (KernelCell) | 18-24h | 24h | Macro at N2.1; TCBS 10-12h |
| N3 (Kani) | 8-10h | 10h | 4 proofs; N3a micro-parallel |
| **Tổng** | **43-50h** | **50h** | Vượt 50h → stop & re-evaluate |
