# 🎼 Synthesis — Round 1 | 2026-02-13

## Chủ đề: Phase P — Formal Verification Expansion

---

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|---|---|---|---|
| 1 | Pure function extraction scope | **Option B** — always-available, single source of truth | **Option A** — `#[cfg(kani)]` only, zero regression risk | ❌ |
| 2 | Kani proof granularity | **Option C** — tiered + escalation plan | **Option C** — tiered per module | ✅ |
| 3 | Miri scope | **Option C** — KernelCell shim, complement Kani | **Option D** — defer, ROI âm | ❌ |
| 4 | Grant cleanup asymmetry | **Option A** + minor fix (zero phys_addr) | **Option A** strict (no fix at all) | ⚠️ Gần đồng thuận |
| 5 | FM.A-7 document depth | **Option C** — living doc + automation script | **Option B** — comprehensive, no automation | ❌ |
| 6 | README refresh scope | **Option B** — full rewrite | **Option A** — fix numbers only | ❌ |

---

## ✅ Các điểm đã đồng thuận (1/6)

### 1. **Kani proof granularity — Option C (Tiered per module)**

Cả hai agent đồng ý hoàn toàn:
- **Grant:** Full symbolic (MAX_GRANTS=2 → ~10 biến, trivially tractable)
- **IRQ:** Constrained (`intid 32–127`, `task_id < NUM_TASKS`) vì 8 bindings × full symbolic quá lớn
- **Watchdog:** `watchdog_should_fault` full symbolic (3 scalars); `budget_epoch_check_pure` constrained
- **Timeout budget:** ≤5 phút/proof là hard constraint
- **Document assumptions** trong FM.A-7

**Điểm khác biệt nhỏ:** GPT muốn "explicit escalation plan per proof documented in FM.A-7 with strength levels (Full/Bounded/Partial)". Gemini nói "một dòng comment trong proof đủ". Đây là chi tiết triển khai, không ảnh hưởng kỹ thuật → **đồng thuận đạt**.

---

## ❌ Các điểm bất đồng (5/6)

### Bất đồng #1: Pure function extraction scope (Q1)

- **GPT nói:** Option B (always-available). *"Single source of truth — nguyên tắc nền tảng cho safety-critical code. DO-178C §5.2 yêu cầu source code reflect requirements. Hai bản logic = drift risk exponential. Auditor DAL A sẽ hỏi: code verify có phải code production không?"* Estimate: 5–8h nhưng trả lại giá trị 10+ phases.
- **Gemini nói:** Option A (`#[cfg(kani)]` only). *"Zero regression risk — ưu tiên số 1. Code stable qua 6 phases, drift risk theoretical. Chưa ở DAL A, optimizing cho auditor bây giờ là premature."* Estimate: 2–3h. *"Option B effort gấp 2–3x cho benefit chỉ hiện thực hóa khi scale 20+ modules."*
- **Khoảng cách:** GPT ưu tiên correctness-by-design (verify production code path). Gemini ưu tiên shipping speed + zero risk. Cả hai đều acknowledge rằng refactor sang Option B *sẽ cần* — chỉ khác nhau về timing (bây giờ vs. khi cần).
- **Gợi ý compromise:** **Option A bây giờ + formal backlog item cho Option B refactor.** Pure functions dưới `#[cfg(kani)]` cho Phase P (ship nhanh, zero risk). Ghi vào Phase Q backlog: "Refactor pure functions sang always-available khi module count > 6 hoặc trước certification prep". Cả hai agent có thể chấp nhận nếu: (1) backlog item có trigger condition rõ ràng, (2) comment trong code nói `// TODO: migrate to always-available per Phase Q backlog`.

---

### Bất đồng #2: Miri scope (Q3)

- **GPT nói:** Option C (KernelCell shim). *"Kani và Miri complement nhau — logic vs. memory safety. RefCell shim reusable cho mọi phase sau. DO-333 §6.3 compliance differentiate AegisOS."* Effort: ~4h nhưng one-time investment.
- **Gemini nói:** Option D (defer). *"Pure functions không có unsafe → Miri tìm nothing. Shim verify RefCell semantics, không production UnsafeCell → verification theater. 4–5h chiếm 30–40% effort Phase P cho tool verify shim."* Ngoài ra: *"DO-333 §6.3 không bắt buộc nếu đã có model checking."*
- **Khoảng cách:** Fundamental disagreement — GPT coi Miri là strategic investment (defense in depth, SMP prep). Gemini coi nó là waste (ROI âm, verify wrong thing). Gemini đặc biệt mạnh ở argument "shim verify different semantics than production".
- **Gợi ý compromise:** **Defer Miri (Option D) cho Phase P, nhưng tạo `#[cfg(miri)]` shim skeleton (không CI job) như prototype.** Effort: ~30 phút (chỉ viết struct, không annotate tests). Lợi ích: khi Phase Q/R cần Miri, shim đã sẵn sàng. Gemini chấp nhận vì effort minimal. GPT chấp nhận vì infrastructure planted.

---

### Bất đồng #3: Grant cleanup — minor fix hay không (Q4)

- **GPT nói:** Option A + zero `phys_addr` on peer fault. *"Defense-in-depth — giá trị 0 gây fault rõ ràng hơn stale address nếu có bug đọc inactive grant."*
- **Gemini nói:** Option A strict. *"Mọi code path check active trước khi đọc phys_addr. Stale phys_addr không gây bug. grant_create overwrite toàn bộ slot khi reuse. Cosmetic fix thêm 1 dòng diff không cần thiết."*
- **Khoảng cách:** Rất nhỏ — cả hai chọn Option A (document). Khác nhau ở 1 dòng code (`g.phys_addr = 0`). Đây là style preference hơn là architectural disagreement.
- **Gợi ý compromise:** **Option A strict (không zero phys_addr) + code comment.** Gemini đúng rằng `active=false` đã ngăn access, và `grant_create` overwrite toàn bộ. GPT's defense-in-depth concern giải quyết bằng comment: `// Note: stale phys_addr retained in inactive grant — overwritten on reuse. See FM.A-7 Design Decisions.`

---

### Bất đồng #4: FM.A-7 document depth (Q5)

- **GPT nói:** Option C (living doc + automation script). *"Proof count tăng ~8/phase → >50 by Phase S. Script ~15 dòng grep. CI verify mỗi commit = audit-grade evidence."*
- **Gemini nói:** Option B (comprehensive, no automation). *"18 proofs không cần script. Manual table = 15 phút viết. Automation = 1–2h viết + maintain. ROI âm dưới 50 proofs. Script break khi source structure thay đổi."*
- **Khoảng cách:** Đồng ý comprehensive content (bảng + uncovered + limitations). Bất đồng chỉ ở automation script. Gemini có point: 18 proofs thì grep đủ. GPT có point: automation prevents human error ở scale.
- **Gợi ý compromise:** **Option B (comprehensive, no automation) + 1-line CI check.** Thay vì full-blown script, thêm 1 dòng vào CI: `test $(grep -rc 'kani::proof' src/) -eq 18 || echo "WARN: FM.A-7 may be outdated"`. Effort: 2 phút. Bắt being out-of-sync mà không cần maintain complex script. Upgrade to full automation khi proof count > 50.

---

### Bất đồng #5: README refresh scope (Q6)

- **GPT nói:** Option B (full rewrite). *"README là front door — first impression matters. copilot-instructions.md đã là source of truth, just adapt. Safety engineers cần standalone README."*
- **Gemini nói:** Option A (fix numbers only). *"Full rewrite = 2–3h. 40% effort cho docs. copilot-instructions đã là source of truth. Safety engineers đọc FM.A-7, không README."*
- **Khoảng cách:** GPT coi README là "Software Description Document lite" cho external audience. Gemini coi README là "elevator pitch cho GitHub visitors" — numbers accuracy đủ. Core question: audience nào quan trọng hơn?
- **Gợi ý compromise:** **Option A+ (fix numbers + thêm source layout tree + link to docs).** Hơn Option A (Gemini) nhưng ít hơn Option B (GPT). Cụ thể: fix tất cả numbers sai + thêm `user/` source layout tree + thêm section "Formal Verification" (1 paragraph + link to FM.A-7) + link "Full architecture: see `.github/copilot-instructions.md`". Effort: ~45–60 phút (vs. 30 phút Option A, 2–3h Option B). Gemini chấp nhận vì effort reasonable. GPT chấp nhận vì covers critical gaps.

---

## 📈 Tỷ lệ đồng thuận: 1/6 = 17%

(+1 gần đồng thuận ở Q4 = thực tế ~2/6 = 33%)

---

## 🎯 Hướng dẫn cho Round 2

### Câu hỏi cụ thể cho GPT-Visionary:

1. **Q1 compromise:** Bạn có chấp nhận Option A + formal backlog item (trigger: module count > 6 hoặc pre-certification) thay vì Option B ngay? Nếu không, argue tại sao Phase P cụ thể cần Option B mà Phase Q không thể.
2. **Q3 compromise:** Bạn có chấp nhận defer Miri CI nhưng viết KernelCell shim skeleton (30 phút, không CI job) như prep? Nếu không, address Gemini's point rằng shim verify RefCell semantics ≠ production UnsafeCell.
3. **Q5 compromise:** 1-line CI check (`grep -c` so sánh) có đủ thay automation script không?
4. **Q6 compromise:** Option A+ (numbers + source layout + links) có đủ không? Nếu không, phần nào của full rewrite là non-negotiable?

### Câu hỏi cụ thể cho Gemini-Pragmatist:

1. **Q1 compromise:** Bạn có chấp nhận backlog item "migrate pure functions to always-available" với trigger condition rõ ràng? Hoặc bạn muốn NO backlog item (defer indefinitely)?
2. **Q3 compromise:** KernelCell shim skeleton (30 phút, không CI, không annotate tests) — acceptable hay vẫn scope creep?
3. **Q4:** GPT đồng ý Option A. Bạn có chấp nhận thêm 1 comment `// INTENTIONAL asymmetry` trong code (2 dòng) hay muốn zero code changes?
4. **Q6 compromise:** Option A+ (numbers + source layout tree + 1 paragraph formal verification + links) — effort ~45–60 phút — acceptable?

### Đề xuất compromise cần cả hai phản hồi:

1. **Phase P scope reduction:** Gemini đề xuất cắt P3 (Miri) + giảm P4. GPT có chấp nhận Phase P = P1 + P2 + P4-lite (nếu Miri shim skeleton là stretch goal)?
2. **Total effort target:** GPT implicit ~17–23h, Gemini explicit 7–10h. Có thể agree on **10–14h** as target?
3. **IPC backport:** GPT đề xuất, Gemini phản đối. Cần quyết định dứt khoát: IN hoặc OUT của Phase P scope.

### Data/evidence cần bổ sung:
- Không cần data mới — cả hai đã analyze code thực tế đầy đủ. Round 2 tập trung vào compromise.
