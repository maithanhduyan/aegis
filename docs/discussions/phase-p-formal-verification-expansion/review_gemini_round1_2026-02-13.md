# ⚡ Gemini-Pragmatist — Review Round 1 | Phase P

## Tổng quan nhận định

Phase P có mục tiêu đúng: 3 module kernel quan trọng (grant, irq, watchdog) đang có **0 Kani proofs** — đó là lỗ hổng thật. Tuy nhiên, plan hiện tại có **4 sub-phases** kéo dài scope đến mức nguy hiểm cho một phase "chỉ verification, không feature". Cụ thể: P1 (pure function extraction) + P2 (Kani) là **core value**. P3 (Miri) và phần README rewrite của P4 là **nice-to-have** dễ trở thành scope creep.

Tôi đã đọc kỹ code thực tế trong `kernel/grant.rs` (196 dòng), `kernel/irq.rs` (261 dòng), và watchdog logic trong `kernel/sched.rs` — code không phức tạp, pure function extraction cho 3 module này là **tractable trong 1-2 ngày**, không cần biến nó thành đại dự án.

Pragmatic Phase P nên ship trong **7–10 giờ**: P1 (2–3h) → P2 (3–4h) → P4-lite (2–3h). Miri (P3) nên **defer** hoặc là optional stretch goal. README nên fix numbers, không rewrite. Mỗi giờ phải tạo ra artifact có thể đo lường: một pure function, một Kani proof pass, một dòng trong FM.A-7 table.

Rủi ro lớn nhất không phải kỹ thuật — mà là **motivation drain**: Phase P không có QEMU checkpoint mới, không có "wow" moment. Nếu kéo dài quá lâu, risk bỏ dở giữa chừng rất cao. Giữ scope nhỏ, ship nhanh, celebrate 18/18 Kani proofs, rồi move on.

---

## Câu hỏi 1: Pure function extraction scope

### Lựa chọn: Option A — `#[cfg(kani)]` only (theo pattern Phase O)

### Lý do thực tế:

**1. Zero regression risk — đây là ưu tiên số 1.**

Ràng buộc cứng: 241 host tests + 32 QEMU checkpoints PHẢI pass. Option B (always-available refactor) thay đổi **call path của production code** — mỗi hàm gốc (`grant_create`, `irq_bind`, `watchdog_scan`) sẽ gọi pure function rồi apply kết quả. Bất kỳ sai sót nào trong refactor (thứ tự apply, error path khác nhau, UART print bị di chuyển) đều có thể break QEMU checkpoints. Option A **không đụng production code** — zero risk.

**2. Lo "logic drift" — nhưng thực tế risk rất thấp.**

"Khi grant logic thay đổi, developer phải nhớ update cả gốc lẫn pure". Thực tế nhìn vào `kernel/grant.rs` — module này **196 dòng**, 3 functions, và đã ổn định qua 6 phases (J→O) mà không thay đổi logic. IRQ tương tự — 261 dòng, ổn định. Watchdog scan — 30 dòng logic. Đây không phải code đang churn. Risk drift là **theoretical**, không practical.

**3. "Auditor DAL A sẽ không chấp nhận" — nhưng chúng ta chưa ở DAL A.**

AegisOS chạy trên QEMU, chưa có phần cứng thật, chưa nộp đơn certification. Optimizing cho DAL A auditor bây giờ là premature. Khi đến lúc (3–5 năm), refactor sang Option B sẽ là phần **nhỏ nhất** của certification effort. Làm Option A bây giờ, ship 8 proofs, rồi refactor khi thật sự cần.

**4. Effort estimate: Option A = 2–3 giờ, Option B = 5–8 giờ.**

Option A: viết 8 pure functions dưới `#[cfg(kani)]`, chỉ compile khi chạy Kani. Không cần sửa test nào. Không cần verify regression.

Option B: viết 8 pure functions + refactor 3 module (thay đổi call path) + verify 241 tests + verify 32 QEMU checkpoints + xử lý edge cases (UART print trong pure function? No — phải tách. `#[cfg(target_arch)]` blocks? Phải để trong wrapper. Watchdog `tick_count()` dependency? Phải inject parameter). Effort gấp 2–3x, benefit chỉ hiện thực hóa khi codebase scale lên 20+ modules — chưa phải bây giờ.

**5. IPC backport — KHÔNG. Scope creep.**

IPC đã có 3 Kani proofs đang pass. Sửa code đang work = risk regression cho zero immediate benefit.

### Rủi ro triển khai:

| Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| Logic drift giữa pure function và hàm gốc | Thấp (code stable) | Trung bình | Comment `// mirrors grant_create()` trong pure function |
| Pure function chỉ có trong Kani build, host tests không dùng được | — | Thấp | Host tests đã cover 14 test cases cho grant, 14 cho IRQ — đủ |
| Future phase thay đổi grant logic, quên update pure function | Thấp | — | Kani proof sẽ fail → caught in CI → safety net tự nhiên |

---

## Câu hỏi 2: Kani proof granularity

### Lựa chọn: Option C — Tiered per module

### Lý do thực tế:

**1. Grant: full symbolic — vì nó trivially cheap.**

`MAX_GRANTS = 2`. Mỗi Grant có 5 fields (`owner: Option<usize>`, `peer: Option<usize>`, `phys_addr: u64`, `active: bool`). Full symbolic = ~10 variables. CBMC solve trong **giây**, không phải phút. Không có lý do constrain.

**2. IRQ: constrained — vì 8 bindings × full symbolic sẽ blow up.**

`MAX_IRQ_BINDINGS = 8`. Full symbolic = 40+ variables. Đặc biệt `intid: u32` nếu fully symbolic = $2^{32}$ cases cho CBMC. Phải constrain: `kani::assume(intid >= 32 && intid <= 127)` (cover 96 SPIs, quá đủ cho QEMU virt). `kani::assume(task_id < NUM_TASKS)`. Document assumptions trong proof comment + FM.A-7.

**3. Watchdog: full symbolic cho `watchdog_should_fault` (4 scalar inputs), constrained cho `budget_epoch_check_pure` (8 tasks).**

`watchdog_should_fault(enabled, interval, ticks_since)` — 3 scalars, Kani handle trivially. `budget_epoch_check_pure` với 8 tasks × 2 fields = 16 variables — tractable nhưng cần `#[kani::unwind(9)]` và có thể cần constrain budget range.

**4. Kani timeout budget ≤5 phút — đây là hard constraint.**

Không thể negotiate. Nếu IRQ proof timeout, giảm scope: (a) reduce `MAX_IRQ_BINDINGS` trong proof tới 4, hoặc (b) strengthen constraints. Đã có precedent: Phase N dùng `#[kani::unwind(5)]` cho IPC.

**5. Escalation plan — một dòng comment đủ.**

Đừng over-engineer. Một dòng comment trong proof: `// Constrained: intid 32–127, task_id 0–7. Full symbolic deferred.`. FM.A-7 table có cột "Strength" (Full/Constrained). Done.

### Rủi ro triển khai:

| Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| IRQ proof timeout dù đã constrain | Trung bình | Trung bình | Giảm bindings tới 4 trong proof, tăng `--object-bits` |
| Constrained proof miss real bug | Thấp | Trung bình | 14 existing host tests + runtime QEMU checkpoints bổ sung |
| `budget_epoch_check_pure` quá phức tạp | Thấp | Thấp | Simplify: chỉ prove "mọi Ready task được reset", không prove full state machine |

---

## Câu hỏi 3: Miri scope và KernelCell compatibility

### Lựa chọn: Option D — Defer Miri, tập trung Kani

### Lý do thực tế:

**1. ROI analysis: Miri effort cao, value không chắc chắn.**

Nhìn thẳng vào thực tế: pure functions mới (Phase P) **không có `unsafe`**. Miri verify chúng sẽ tìm thấy **nothing**. Code `unsafe` hiện tại nằm trong wrapper functions (dùng `KernelCell`) — Miri với `RefCell` shim sẽ hoặc (a) false positive hàng loạt, hoặc (b) cần KernelCell shim mà shim **không verify production code** anyway.

Shim dùng `RefCell` có semantics **khác hẳn** production `UnsafeCell`. Miri + shim verify rằng tests không trigger re-entrant borrow — nhưng production code **dựa vào single-core guarantee**, không dựa vào borrow checking. Đây là verification **theater** — có vẻ verify nhưng thực tế verify thứ khác.

**2. Effort estimate thực tế cho Miri integration:**

- Viết `#[cfg(miri)]` shim: ~1h
- Annotate `#[cfg(not(miri))]` cho 50+ tests dùng inline asm hoặc asm-dependent paths: ~2h
- Debug Miri false positives: ~1–2h (unpredictable)
- CI job setup: ~0.5h
- Tổng: **4–5.5 giờ** — chiếm **30–40% tổng effort Phase P** cho tool verify **shim, không production code**

**3. DO-333 §6.3 không bắt buộc abstract interpretation nếu có model checking.**

DO-333 §6.3 khuyến nghị abstract interpretation nhưng **không bắt buộc** — nếu đã có model checking (Kani) ở mức đủ mạnh. 18 Kani proofs cover properties quan trọng nhất. Miri là bổ sung **nice-to-have**, không required.

**4. Thời gian tiết kiệm → đầu tư vào Kani proof quality.**

4–5 giờ tiết kiệm từ Miri có thể dùng để: (a) thêm 1–2 Kani proofs bổ sung, (b) tăng proof strength cho IRQ (wider constrained range), (c) viết FM.A-7 kỹ hơn. Tất cả đều có **measurable, permanent value**.

**5. Defer ≠ cancel.**

Ghi vào Phase Q/R backlog: "Miri integration — cần khi AegisOS có > 1 core hoặc preemptive kernel". Single-core + interrupts masked = Miri không add value ngay. Khi thêm SMP, Miri trở nên critical.

### Rủi ro triển khai:

| Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| Unsafe code có UB mà chỉ Miri bắt được | Rất thấp | Cao nếu xảy ra | 241 host tests + QEMU runtime + Kani + single-core → defense in depth đủ tạm |
| Auditor hỏi "tại sao không abstract interpretation?" | Trung bình (tương lai) | Trung bình | Document: "Kani (model checking) selected per DO-333. Miri deferred pending SMP." |

---

## Câu hỏi 4: Grant cleanup asymmetry

### Lựa chọn: Option A — Document as intentional (không sửa code)

### Lý do thực tế:

**1. Phase P constraint #1: "Zero runtime changes."**

Plan nói rõ: Phase P **không thay đổi behavior của kernel**. Option B (fix peer fault → full zero) **thay đổi runtime behavior**. Dù chỉ 2 dòng code, nó thay đổi observable state: sau peer fault, `GRANTS[i]` chuyển từ `Grant { owner: Some(x), peer: None, active: false, ... }` → `EMPTY_GRANT`. Bất kỳ test nào assert on grant state sau cleanup sẽ break.

**2. Asymmetry có lý do kỹ thuật thật.**

Phân tích code cho thấy: peer fault giữ owner field vì **owner vẫn sống, MMU mapping vẫn active**. Full zero sẽ unmap owner's page → gây data loss cho task đang chạy. Asymmetry **bảo vệ** owner, không phải thiếu sót.

**3. Không cần "minor fix" zero `phys_addr`.**

Nhìn code: peer fault đã set `active = false`. Mọi code path khác check `active` trước khi đọc `phys_addr`. Stale `phys_addr` trong inactive grant **không gây bug** — `grant_create` overwrite toàn bộ slot khi reuse. Sửa `phys_addr = 0` là cosmetic, thêm 1 dòng diff không cần thiết, tăng risk (dù nhỏ) cho zero benefit.

**4. Document trong FM.A-7 = 5 phút effort.**

Thêm vào section "Design Decisions":

> **Grant cleanup asymmetry:** Owner fault → EMPTY_GRANT (owner's lifecycle ends). Peer fault → deactivate + clear peer (owner alive, MMU mapping retained). Verified by `grant_cleanup_completeness` proof.

Done. Zero runtime changes. Zero risk. 5 phút.

### Rủi ro triển khai:

| Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| Stale data gây confusion khi debug | Rất thấp | Thấp | Comment in code: `// INTENTIONAL asymmetry — see FM.A-7` |
| Kani proof phải handle asymmetric behavior | — | — | `grant_cleanup_pure` trả kết quả phản ánh current behavior → proof verifies AS-IS |

---

## Câu hỏi 5: FM.A-7 document depth

### Lựa chọn: Option B — Comprehensive document (KHÔNG automation)

### Lý do thực tế:

**1. 18 proofs không cần automation.**

Automation script extract proof list từ source code — cool idea, nhưng cho **18 proofs**? `grep -rn 'kani::proof' src/` mất 0.1 giây. Manual table với 18 dòng mất 15 phút viết, 2 phút update khi thêm proof. Automation script mất 1–2 giờ viết + maintain. ROI **âm** cho ít nhất 5 phases tiếp theo.

**2. Automation script tạo maintenance burden.**

Script phải: parse Rust source → extract harness name → match against FM.A-7 table → CI fail on mismatch. Khi source structure thay đổi (rename module, move function), script break → CI break → developer phải fix script thay vì viết code. Cho 18 proofs, overhead không xứng đáng.

**3. Option B (comprehensive) đủ cho FM.A-7.**

Bảng mapping 18 dòng + "Uncovered Properties" section + "Proof Limitations" section + "Design Decisions" section. Tổng: ~100 dòng markdown. Effort: **1.5–2 giờ**. Deliverable cụ thể, dễ review, đủ cho DO-333 FM.A-7.

**4. Khi nào cần automation?**

Khi proof count > 50. Ghi vào backlog: "Automate FM.A-7 proof extraction when proof count exceeds 50."

### Rủi ro triển khai:

| Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| Document outdated sau 3 phases | Trung bình | Thấp | Phase checklist item: "Update FM.A-7 if proofs changed" |
| Thiếu một proof trong table (human error) | Thấp | Thấp | Review step: `grep -c 'kani::proof' src/` → so sánh với table row count |

---

## Câu hỏi 6: README refresh scope

### Lựa chọn: Option A — Fix numbers only + thêm 1 section source layout

### Lý do thực tế:

**1. README full rewrite = 2–3 giờ effort cho documentation.**

Phase P đã có FM.A-7 document, plan document, blog post sắp tới. Thêm README full rewrite = tổng cộng **~5 giờ documentation** trong 1 phase. Đối với OS project có 1 developer, đó là **40% effort cho docs**. Không sustainable.

**2. Fix numbers = 30 phút, đạt 80% value.**

Sửa: 3→8 tasks, 18→19 bits, 13→14 syscalls, 189→249 tests, 25→32 checkpoints, 10→18 Kani proofs. Thêm: source layout tree (bao gồm `user/`), `KernelCell` mention, Kani mention. 30–45 phút effort, README chính xác ở mọi con số.

**3. `.github/copilot-instructions.md` đã là source of truth chi tiết.**

Ai clone repo sẽ thấy `copilot-instructions.md` — 250+ dòng, đầy đủ memory map, module table, syscall ABI, build instructions. README không cần duplicate toàn bộ nội dung này. README = elevator pitch + accurate numbers + links.

**4. Safety engineers đọc FM.A-7 chứ không README.**

Safety engineer cần proof coverage mapping, traceability matrix, test results. Họ đọc `docs/standard/05-proof-coverage-mapping.md`, không README. README là cho GitHub visitors — fix numbers là đủ cho audience này.

### Rủi ro triển khai:

| Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|
| README vẫn thiếu architecture diagram | — | Thấp | Thêm link "See copilot-instructions.md for detailed architecture" |
| Phải rewrite anyway trong Phase Q/R | Trung bình | — | Tốt hơn rewrite khi scope rõ ràng |

---

## Đề xuất cắt giảm / điều chỉnh

### Cắt bỏ hoàn toàn:
1. **P3 (Miri integration)** — defer sang Phase Q. ROI âm cho Phase P. Tiết kiệm 4–5 giờ.
2. **IPC pure function backport** — code đang work, không đụng. Scope creep.
3. **Notify_bit collision detection** — đây là **feature mới**, không phải verification. Phase Q.

### Giảm scope:
4. **README** — fix numbers only, không full rewrite. Tiết kiệm 1.5–2 giờ.
5. **FM.A-7** — comprehensive table, không automation script. Tiết kiệm 1–2 giờ.

### Phase P tối ưu (Pragmatist version):

| Bước | Nội dung | Effort |
|---|---|---|
| P1 | Pure function extraction (`#[cfg(kani)]`) cho grant (3) + irq (3) + watchdog (2) | 2–3h |
| P2 | 8 Kani proofs (tiered: full symbolic grant, constrained irq/watchdog) | 3–4h |
| P3 | FM.A-7 comprehensive document + README number fixes | 2–3h |
| **Tổng** | | **7–10h** |

---

## Tóm tắt lựa chọn

| Câu hỏi | Lựa chọn | Lý do 1 dòng |
|---|---|---|
| **Q1: Pure function extraction** | **Option A** (`#[cfg(kani)]` only) | Zero regression risk, code stable qua 6 phases, refactor sang B khi thật cần |
| **Q2: Kani granularity** | **Option C** (tiered) | Full symbolic cho grant (2 slots = trivial), constrained cho irq/watchdog (documented) |
| **Q3: Miri scope** | **Option D** (defer) | ROI âm — pure functions không unsafe, shim verify thứ khác production, tiết kiệm 4–5h |
| **Q4: Grant cleanup** | **Option A** (document only) | Zero runtime changes = Phase P constraint #1; asymmetry có lý do kỹ thuật thật |
| **Q5: FM.A-7 depth** | **Option B** (comprehensive, no automation) | 18 proofs không cần script; manual table = 1.5h; automation ROI âm dưới 50 proofs |
| **Q6: README refresh** | **Option A** (fix numbers only) | 30 phút đạt 80% value; full rewrite tốn 2–3h cho docs trong phase không có feature mới |
