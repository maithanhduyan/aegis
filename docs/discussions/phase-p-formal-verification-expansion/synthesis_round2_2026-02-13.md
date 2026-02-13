# 🎼 Synthesis — Round 2 | 2026-02-13

## Chủ đề: Phase P — Formal Verification Expansion

---

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|---|---|---|---|
| 1 | Pure function extraction scope | Option A + backlog + TODO | Option A + backlog + TODO | ✅ |
| 2 | Kani proof granularity | Option C (tiered) | Option C (tiered) | ✅ (R1) |
| 3 | Miri scope | Skeleton shim 30min, no CI | Skeleton shim 30min, no CI | ✅ |
| 4 | Grant cleanup asymmetry | Option A strict + comment | Option A strict + comment | ✅ |
| 5 | FM.A-7 document depth | Option B + 1-line CI (WARN→FAIL at >25) | Option B + 1-line CI (WARN→FAIL at >25) | ✅ |
| 6 | README refresh scope | Option A+ (~45–60 min) | Option A+ (~45–60 min) | ✅ |

---

## ✅ Các điểm đã đồng thuận (6/6)

### 1. Pure function extraction — Option A + backlog
Cả hai chấp nhận `#[cfg(kani)]` pure functions bây giờ, với formal backlog item (trigger: module count > 6 hoặc pre-certification).

### 2. Kani proof granularity — Option C (tiered)
Full symbolic cho grant (MAX_GRANTS=2), constrained cho irq/watchdog. Đã đồng thuận từ Round 1.

### 3. Miri — Skeleton shim only
Defer Miri CI. Viết KernelCell shim skeleton (~15 dòng, 30 phút). Không CI job, không test annotations.

### 4. Grant cleanup — Document as intentional
Option A strict (no code changes ngoài comment). 4 dòng comment giải thích asymmetry. Không zero `phys_addr`.

### 5. FM.A-7 — Comprehensive + 1-line CI check
Comprehensive document (bảng + uncovered + limitations). 1-line CI: WARN bây giờ, FAIL khi proof count > 25.

### 6. README — Option A+
Fix numbers + source layout tree + "Formal Verification" paragraph + links + memory map fix. ~45–60 phút.

---

## 📈 Tỷ lệ đồng thuận: 6/6 = 100% ✅

## Bonus agreements:
- **IPC backport:** OUT — cả hai đồng ý
- **Effort target:** 10–12h — GPT nói 9–12h, Gemini nói 8–11h, overlap tại 10–11h
- **Phase P scope:** P1 + P2 + P4 (merged, no standalone P3)

→ **Chuyển sang Phase kết thúc: Final Consensus**
