# 🎼 Synthesis — Round 1 | 2026-02-12

## Chủ đề: Phase M — Safety Assurance Foundation

---

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|----------------|-----------------|---------------------|-------------|
| 1 | M3 (Panic Handler) ưu tiên đầu | M3 đầu tiên, ~1-2 ngày | M3 đầu tiên, ~2-4 giờ | ✅ |
| 2 | Scope Phase M: 6 sub-phases hay rút gọn? | Giữ nguyên 6 sub-phases (M1–M6), không bỏ | Rút gọn: M3+M4+M2-lite+M1-partial; **defer M5, M6** | ❌ |
| 3 | Thứ tự sau M3: M2 hay M4? | M2 (logging) → M1 → M4 | M4 (coverage) → M2-lite → M1-partial | ❌ |
| 4 | `static mut`: Encapsulate hay SAFETY comments? | `KernelCell<T>` encapsulation ngay, bắt đầu từ TCBS+CURRENT | SAFETY comments trước, pilot encapsulate TICK_COUNT | ❌ |
| 5 | Incremental (không big-bang refactor) | Progressive wrapping 3 tuần | Incremental pilot → validate → mở rộng | ✅ |
| 6 | Bảo vệ 189 tests khỏi regression | Mỗi tuần refactor → chạy full suite | Mỗi bước nhỏ → verify 189 tests | ✅ |
| 7 | Kani đầu tư bây giờ hay defer? | Đầu tư Kani (M5), ~50-60h, ROI 10 năm | Defer Kani, exhaustive tests thay thế, ~10-12h | ❌ |
| 8 | Coverage target cho `kernel/` | 80% statement, 95% cap.rs, 90% elf.rs | 70% statement, 95% cap.rs, 75% elf.rs | ❌ |
| 9 | `cap.rs` là module ưu tiên cao nhất | 95% — gateway mọi syscall | 95% — effort ~1 giờ | ✅ |
| 10 | `arch/` không đo coverage trên host | Đồng ý, verify bằng QEMU + manual review | Đồng ý, QEMU boot checkpoints thay thế | ✅ |
| 11 | Safety-first vs hybrid features+safety | "Verify small, then grow" — safety 100% trước | 70% features + 30% safety, xen kẽ | ❌ |
| 12 | Quick wins (clippy lints, `deny(unsafe_op_in_unsafe_fn)`) | Không đề cập trực tiếp | Đề xuất mạnh: ~2h effort, zero risk | ✅* |

> \* GPT không phản đối — Gemini đề xuất, GPT không mention → coi là đồng thuận tiềm năng (cần xác nhận Round 2).

---

## ✅ Các điểm đã đồng thuận (6/12)

### 1. M3 (Enhanced Panic Handler) ưu tiên đầu tiên
Cả hai đồng ý: M3 có effort thấp nhất (~2-4 giờ), risk = zero (không thay đổi API), value cực cao cho debug. Thêm file:line, task ID, tick count, ESR/FAR vào panic output. **Quick win rõ ràng nhất.**

### 2. Tiếp cận incremental — không big-bang refactor
Cả hai đều phản đối refactor tất cả 8 `static mut` cùng lúc. GPT đề xuất 3 tuần progressive, Gemini đề xuất pilot → validate → mở rộng. **Khác chiến thuật nhưng cùng nguyên tắc: từng bước nhỏ, verify mỗi bước.**

### 3. Bảo vệ 189 tests là ưu tiên tuyệt đối
Bất kỳ refactor nào cũng phải pass 189 host tests + 25 QEMU checkpoints trước khi commit. Đây là "safety net" quan trọng nhất hiện tại.

### 4. `cap.rs` là module ưu tiên cao nhất cho coverage
Cả hai đặt target 95% cho `cap.rs` — gateway cho mọi syscall, sai ở đây = privilege escalation. Effort ~1 giờ (gần đạt rồi).

### 5. `arch/` code không đo coverage trên host
Cả hai đồng ý: `arch/aarch64/` chỉ verify bằng QEMU boot checkpoints + manual review, không ép coverage number.

### 6. Quick wins (clippy lints) có ROI cao
Gemini đề xuất `#![deny(unsafe_op_in_unsafe_fn)]` + `clippy::undocumented_unsafe_blocks`. GPT không phản đối (chưa đề cập). Zero runtime risk, automated lint.

---

## ❌ Các điểm bất đồng (6/12)

### Bất đồng #1: Scope Phase M — 6 sub-phases hay rút gọn?

- **GPT nói**: Giữ nguyên 6 sub-phases (M1–M6), mỗi sub-phase phục vụ một objective riêng trong DO-178C verification framework. "Gộp M2+M3 có vẻ hấp dẫn nhưng sẽ tạo PR quá lớn." Tổng scope: ~80-100 giờ.
- **Gemini nói**: Rút gọn còn M3+M4+M2-lite+M1-partial. Defer M5 (Kani) và M6 (Traceability) hoàn toàn. "6 sub-phases thuần safety mà không có feature mới = recipe cho burnout." Tổng scope: ~20-30 giờ.
- **Khoảng cách**: GPT muốn đầu tư ~80-100h cho safety foundation đầy đủ; Gemini muốn ~20-30h rồi chuyển sang features. Chênh lệch **~4x effort**.
- **Gợi ý compromise**: **Phase M gồm 4 sub-phases bắt buộc (M1-M4) + M5/M6 chuyển thành "Phase M-extended" hoặc gắn vào Phase N.** Tức là: làm nền tảng safety cốt lõi (audit, logging, panic, coverage) ngay, defer formal verification và traceability cho khi kernel ổn định hơn. Scope ước tính: ~40-50h — middle ground giữa 20h và 100h.

---

### Bất đồng #2: Thứ tự sau M3 — M2 (Logging) hay M4 (Coverage)?

- **GPT nói**: M2 trước M4 — "Khi có `klog!` macro với tick count + task ID, toàn bộ quá trình M1 (unsafe refactor) sẽ dễ debug hơn nhiều." Logging hỗ trợ refactor.
- **Gemini nói**: M4 trước M2 — "`cargo-llvm-cov` setup ~1-2 giờ, output là lcov report. Bạn sẽ biết ngay `cap.rs` covered 90% hay 40%." Data guides decisions.
- **Khoảng cách**: Cả hai có lý. M2 hỗ trợ debug, M4 cung cấp data. Câu hỏi thực sự: liệu M1 (refactor) xảy ra ngay sau M3 hay sau khi đo coverage?
- **Gợi ý compromise**: **M3 → M4 → M2-lite → M1.** Lý do: M4 (coverage) chỉ mất 1-2h setup, cho data ngay, không thay đổi code. M2-lite (logging) mất 4-6h. Cả hai đều "non-destructive" — làm trước M1 (refactor) để có cả data lẫn debug tools khi bắt đầu refactor.

---

### Bất đồng #3: `static mut` — Encapsulate ngay (KernelCell) hay SAFETY comments trước?

- **GPT nói**: "Mọi OS safety-critical nghiêm túc đều encapsulate kernel state. Không có ngoại lệ." `KernelCell<T>` zero-cost abstraction. Bắt đầu từ TCBS+CURRENT (critical nhất). SAFETY comments không đủ cho formal tools và certification auditors. Trích seL4, Tock OS, INTEGRITY RTOS làm bằng chứng.
- **Gemini nói**: "15-25 giờ effort" cho full encapsulation. ~60+ direct access trong `host_tests.rs` phải sửa. Risk regression trong interrupt context. Bắt đầu từ TICK_COUNT (đơn giản nhất, ~5 references). SAFETY comments trước (3-4h, zero risk) → pilot → validate.
- **Khoảng cách**: GPT bắt đầu từ biến **phức tạp nhất** (TCBS), Gemini bắt đầu từ biến **đơn giản nhất** (TICK_COUNT). GPT coi SAFETY comments là không đủ, Gemini coi chúng là bước đầu cần thiết.
- **Gợi ý compromise**: **SAFETY comments cho tất cả 8 globals (bước 0, ~3-4h) → Pilot encapsulate TICK_COUNT + TICK_INTERVAL (2 biến timer, bước 1) → Nếu pilot OK, encapsulate CURRENT + EPOCH_TICKS (bước 2) → Cuối cùng TCBS + ENDPOINTS (bước 3 — có thể defer sang Phase N nếu effort quá lớn).** Điểm mấu chốt: SAFETY comments là "documentation debt reduction" ngay lập tức, encapsulation là "technical debt reduction" theo lộ trình. Cả hai cần thiết, thứ tự Gemini hợp lý hơn cho risk management.

---

### Bất đồng #4: Kani — Đầu tư bây giờ hay defer?

- **GPT nói**: ROI 10 năm cực cao. Bounds nhỏ (3 tasks, 4 endpoints) = exhaustive tractable. Bổ sung Proptest + Miri. "DO-333 cho phép dùng formal verification thay thế một phần testing — giảm 30-50% verification cost." Target: cap.rs, elf.rs, ipc.rs, sched.rs logic. ~50-60h total.
- **Gemini nói**: "50-60h cho setup + learning + 15 proofs" = 6-8 ngày full-time. Windows dev cần WSL/Docker. Alternative: exhaustive tests 234 cases cho cap.rs (~2h), fuzz-like cho elf.rs (~3-4h), property-based cho ipc.rs (~4-5h). Tổng: ~10-12h cho "value tương đương phần lớn 15 Kani proofs."
- **Khoảng cách**: GPT thấy Kani là **investment cho tương lai** (10 năm ROI), Gemini thấy là **opportunity cost** (50-60h có thể dùng cho features + tests). Cả hai đều đúng ở perspective riêng.
- **Gợi ý compromise**: **Phase M: exhaustive tests + property-based (approach Gemini, ~10-12h). Phase N hoặc O: Kani pilot cho cap.rs duy nhất (~8-10h) khi CI Linux runner đã stable.** Lý do: Gemini đúng rằng exhaustive tests cho cap.rs (234 cases) cho 100% coverage nhanh hơn. Nhưng GPT đúng rằng Kani proves **absence of bugs** (exhaustive tests chỉ checks presence of expected behavior). Compromise: dùng exhaustive tests để tăng confidence ngay, Kani để prove formal correctness sau.

---

### Bất đồng #5: Coverage target — 70% hay 80%?

- **GPT nói**: 80% kernel/ statement coverage. Lộ trình: 80% → Decision Coverage (85%, năm 2-3) → MC/DC (90%+, năm 5). Tham chiếu NASA JPL "Rule of Ten".
- **Gemini nói**: 70% kernel/ statement coverage. "70% cho kernel/ modules portable code là đủ tốt cho giai đoạn prototype." Effort: ~15h cho ~15-20 tests mới.
- **Khoảng cách**: Chỉ 10 percentage points. Câu hỏi thực tế: liệu effort từ 70→80% có xứng đáng ở giai đoạn prototype?
- **Gợi ý compromise**: **Target 75% overall cho `kernel/`, nhưng giữ target module-specific cao cho critical modules: 95% cap.rs, 85% elf.rs, 80% ipc.rs.** Điều này đạt mục tiêu cả hai: coverage trung bình hợp lý (Gemini) nhưng critical modules được bảo vệ mạnh (GPT). Đo baseline trước → gap analysis → viết targeted tests.

---

### Bất đồng #6: Safety-first vs Hybrid (features + safety)

- **GPT nói**: "Verify small, then grow." Trích dẫn Therac-25, Boeing 737 MAX, seL4, INTEGRITY RTOS. "NUM_TASKS = 3 không phải limitation — đó là simplification có chủ đích cho verification." "Cửa sổ vàng" verify kernel nhỏ sẽ đóng lại với mỗi feature.
- **Gemini nói**: "Verify quá sớm = verify hai lần." Khi expand NUM_TASKS → phải verify lại. 6 sub-phases thuần safety = "recipe cho burnout." "Features drive testing naturally." Đề xuất Phase M-hybrid: 4 tuần safety + 4 tuần features xen kẽ.
- **Khoảng cách**: Đây là **bất đồng lớn nhất và quan trọng nhất**. GPT muốn verify nền tảng hiện tại (3 tasks, 4 endpoints) rồi mới mở rộng. Gemini muốn mở rộng kernel (8 tasks) rồi verify cùng lúc. Hai chiến lược đối lập.
- **Gợi ý compromise**: **Phase M (4 sub-phases safety, ~40-50h) → Phase N bắt đầu bằng feature nhẹ (expand NUM_TASKS lên 8 — thay đổi constants + thêm TCB slots, ước tính ~8-10h) → Chạy lại coverage + tests cho 8 tasks.** Lý do: GPT đúng rằng verify nền tảng nhỏ trước dễ hơn. Nhưng Gemini đúng rằng NUM_TASKS=3→8 là thay đổi **incremental** (chỉ constants + array size, không thay đổi algorithm) → phần lớn verification vẫn valid. Đây không phải "verify hai lần" — đây là "verify, rồi extend proof." Phase M tạo framework, Phase N mở rộng scope.

---

## 📈 Tỷ lệ đồng thuận: 6/12 = 50%

---

## 🎯 Hướng dẫn cho Round 2

### Câu hỏi cụ thể cho GPT (Visionary):

1. **Về scope**: Gemini ước tính full Phase M = ~80-100h. GPT ước tính bao nhiêu giờ cho M1–M6? Nếu budget là **40-50h** (compromise), bạn sẽ cắt gì?
2. **Về Kani timing**: Gemini đề xuất exhaustive tests (10-12h) cho value tương đương Kani. GPT có đồng ý rằng exhaustive tests **bổ sung giá trị ngay** (Phase M) còn Kani **bổ sung giá trị lâu dài** (Phase N/O)? Hay Kani phải ở Phase M?
3. **Về `static mut` order**: Gemini cho rằng bắt đầu từ TICK_COUNT (5 references, đơn giản) an toàn hơn TCBS (~20+ references, interrupt context). GPT có đồng ý pilot TICK_COUNT trước? Hay vẫn khẳng định TCBS phải đầu tiên?
4. **Về hybrid approach**: Nếu Phase M (safety) kết thúc → Phase N bắt đầu bằng expand NUM_TASKS = 8 (thay đổi nhỏ, ~8-10h) → chạy lại tests — liệu điều này có thỏa mãn "verify small then grow" không? Hay GPT yêu cầu verify **hoàn toàn** (bao gồm Kani + traceability) trước mọi feature?

### Câu hỏi cụ thể cho Gemini (Pragmatist):

1. **Về SAFETY comments**: GPT trích dẫn rằng DO-178C auditor sẽ flag `static mut` + SAFETY comment vì "comment nói single-core, nhưng code ở đâu enforce điều đó?" Gemini có phản biện cụ thể nào? Hay đồng ý rằng SAFETY comments là bước 0, encapsulation là mục tiêu cuối?
2. **Về Kani ROI dài hạn**: GPT nói Kani proves **absence of bugs** (exhaustive tests chỉ checks expected behavior). Gemini có đồng ý rằng Kani có giá trị dài hạn — chỉ disagree về **timing** (bây giờ vs sau)?
3. **Về coverage gap 70% vs 80%**: Nếu target module-specific (95% cap.rs, 85% elf.rs, 80% ipc.rs) thì weighted average tự nhiên sẽ ~75-80%. Gemini có chấp nhận target **75% overall + module-specific targets** không?
4. **Về "verify hai lần"**: GPT phản biện rằng expand NUM_TASKS = 3→8 là thay đổi **constant, không thay đổi algorithm** → phần lớn verification vẫn valid. Gemini có đồng ý rằng core invariants (capability soundness, IPC correctness, priority ordering) **không thay đổi** khi tăng NUM_TASKS?

### Đề xuất compromise cần cả hai phản hồi:

1. **Phase M scope = M3 + M4 + M2-lite + M1 (progressive)**: 4 sub-phases thực thi, M5/M6 defer sang Phase N. Cả hai có chấp nhận không?
2. **`static mut` lộ trình**: SAFETY comments (bước 0) → pilot TICK_COUNT (bước 1) → CURRENT+EPOCH_TICKS (bước 2) → TCBS+ENDPOINTS defer Phase N. Cả hai có chấp nhận không?
3. **Coverage target**: 75% overall kernel/, với 95% cap.rs, 85% elf.rs, 80% ipc.rs. Cả hai có chấp nhận không?
4. **Kani timeline**: Exhaustive tests Phase M (Gemini approach) + Kani pilot cho cap.rs Phase N (GPT approach). Cả hai có chấp nhận không?

### Data/evidence cần bổ sung:

- Đo **thực tế** baseline coverage bằng `cargo-llvm-cov` để xác nhận estimates (cả hai ước tính 50-65% nhưng chưa đo)
- Đếm **chính xác** số references tới mỗi `static mut` trong `host_tests.rs` để ước tính effort refactor
- Kiểm tra `core::fmt` có emit FP instructions không (Gemini flag concern này — ảnh hưởng M2 logging)
