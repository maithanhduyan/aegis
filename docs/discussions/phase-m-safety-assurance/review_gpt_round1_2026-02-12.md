# 🔭 GPT-Visionary Review — Round 1 | 2026-02-12

## Chủ đề: Phase M — Safety Assurance Foundation

## Tổng quan nhận định

Phase M là **bước ngoặt chiến lược quan trọng nhất** kể từ Phase A — thời điểm AegisOS chuyển từ "kernel nghiên cứu hoạt động được" sang "kernel có bằng chứng đáng tin cậy". Nhìn lại hành trình 12 phases (A→L): chúng ta đã xây dựng được một microkernel hoàn chỉnh với scheduler, IPC, capability, fault isolation, ELF loader, và kiến trúc tách biệt arch/kernel/platform. Đó là phần **"build"** — xây dựng chức năng. Phase M mở ra phần **"assure"** — chứng minh chức năng đó đúng. Trong lịch sử ngành safety-critical, đây chính xác là điểm mà nhiều dự án thất bại: họ có kernel tốt nhưng không có evidence base đủ mạnh để vượt qua certification. seL4 mất **11 năm** (2005–2016) để đi từ functional correctness proof đến deployment thực tế trong hệ thống quân sự (DARPA HACMS). Green Hills INTEGRITY mất gần **một thập kỷ** để đạt DO-178B Level A cho kernel RTOS đầu tiên trên thế giới (2003). AegisOS không cần đạt certification ngay, nhưng Phase M đặt nền móng cho hành trình đó — và nền móng này sẽ quyết định tốc độ của 5-10 năm tiếp theo.

Từ góc nhìn 20 năm, AegisOS đang ở vị trí cực kỳ thuận lợi mà ít ai nhận ra: **zero external dependencies** (không SOUP — IEC 62304 §7 sẽ rất hài lòng), **no heap** (deterministic — ISO 26262 Part 6 rất ưa thích), **Rust type system** (memory safety không cần garbage collector — điều mà cả VxWorks 7 lẫn RTEMS đều không có). Nhưng lợi thế này sẽ bị lãng phí nếu không có traceability, coverage measurement, và formal verification để **chuyển hóa thiết kế tốt thành bằng chứng có thể kiểm chứng**. Phase M, nếu thực hiện đúng, sẽ biến AegisOS từ "microkernel Rust thú vị" thành "microkernel Rust có safety evidence đầu tiên trên thế giới cho bare-metal AArch64" — một vị thế mà chưa ai chiếm lĩnh trong năm 2026.

## Trả lời 5 câu hỏi

### Câu 1: Thứ tự ưu tiên sub-phases

**Stance: M3 → M2 → M1 → M4 → M6 → M5 (đảo lại so với đề xuất gốc)**

Lý do chiến lược: nhìn từ quan điểm **risk reduction per unit effort** — nguyên tắc mà mọi chương trình DO-178C Level A đều tuân theo (DO-178C §4.1: "planning should consider the criticality of the software").

**M3 (Enhanced Panic Handler) nên làm đầu tiên** — chỉ mất 1-2 ngày nhưng tác động cực lớn. Trong mọi hệ thống safety-critical, **diagnostic information khi failure** là yêu cầu bắt buộc. ISO 26262 Part 6 §9.4.3 yêu cầu "sufficient information for problem analysis". Hiện tại panic chỉ in "PANIC" rồi loop — đây là technical debt nguy hiểm nhất vì mọi phase sau (M1 refactor, M5 Kani) đều có thể gây regression, và khi regression xảy ra mà không có diagnostic info → debug mù. Green Hills INTEGRITY và Wind River VxWorks đều có elaborate crash dump từ version 1.0 — đó không phải tình cờ.

**M2 (Structured Logging) tiếp theo** — tạo nền tảng observability. Khi có `klog!` macro với tick count + task ID, toàn bộ quá trình M1 (unsafe refactor) sẽ dễ debug hơn nhiều. DO-178C §6.4.3 yêu cầu "traceability of testing activities" — structured log chính là raw material cho traceability.

**M1 (Unsafe Audit) là trung tâm** — nhưng cần M2/M3 hỗ trợ trước. seL4 team từng nói: "The hardest part of verification wasn't the proof — it was getting the code into a shape where proofs were tractable." M1 biến 8 `static mut` globals thành safe API → Kani (M5) mới verify được. Đây là prerequisite không thể bỏ qua.

**M4 (Coverage) sau M1** — vì coverage numbers sẽ thay đổi đáng kể sau M1 refactor. Đo coverage trước M1 chỉ tạo ra noise. Ngoài ra, sau khi M1 tạo safe wrappers, coverage tool sẽ chính xác hơn vì không bị `unsafe` blocks che khuất branch information.

**M6 (Traceability Matrix) trước M5** — traceability matrix sẽ reveal gaps trong test suite, giúp focus Kani proofs vào đúng chỗ cần nhất. DO-178C §5.5 rõ ràng: traceability phải tồn tại TRƯỚC verification activities.

**M5 (Kani) cuối cùng** — formal verification cần code sạch (M1), test gaps identified (M6), và debugging tools (M2/M3). Đặt M5 cuối cũng cho phép Kani toolchain (cần Linux/WSL) được setup song song trong khi các sub-phase khác tiến hành trên Windows.

**Không nên bỏ hay gộp sub-phase nào** — mỗi sub-phase phục vụ một objective riêng trong DO-178C verification framework. Gộp M2+M3 có vẻ hấp dẫn nhưng sẽ tạo PR quá lớn, khó review.

### Câu 2: `static mut` — Encapsulate hay SAFETY comments?

**Stance: Encapsulate — không thỏa hiệp, nhưng theo chiến lược "progressive wrapping"**

Đây là câu hỏi có đáp án rõ ràng nếu nhìn từ tầm nhìn 10 năm: **mọi OS safety-critical nghiêm túc đều encapsulate kernel state**. Không có ngoại lệ.

**Bằng chứng từ industry leaders:**

- **seL4**: Kernel state nằm trong abstract data types, mọi mutation đi qua verified functions. Đây là lý do seL4 có thể prove functional correctness — nếu state là `static mut` tự do, proof sẽ impossible.
- **Tock OS** (Rust embedded OS): Dùng `Cell`/`RefCell` trong kernel, **không bao giờ** dùng `static mut` cho shared state. Tock team viết rõ: "We use Rust's type system to enforce access control at compile time."
- **Redox OS** (Rust general-purpose): Dùng `Mutex<>` wrapper cho global state — dù là single-threaded context.
- **INTEGRITY RTOS** (Green Hills, DO-178B Level A certified): Kernel state nằm trong protected structs, access qua accessor functions — tương tự pattern mà M1 đề xuất.

**Tại sao SAFETY comments không đủ:**

1. **DO-178C §6.3.4** yêu cầu "the source code is verifiable" — SAFETY comments là documentation, không phải verification evidence. Auditor sẽ hỏi: "Comment nói single-core, nhưng code ở đâu enforce điều đó?"
2. **Kani/Miri không đọc comments** — formal tools cần API boundary để reason. `static mut` + SAFETY comment = opaque blob cho Kani. `UnsafeCell` wrapper + safe API = verifiable interface.
3. **Future-proofing**: Khi AegisOS chuyển sang multi-core (inevitable trong 5-10 năm, mọi safety-critical system hiện đại đều multi-core — xem ISO 26262:2018 Part 11 cho multi-core), `static mut` + SAFETY comment "single-core" sẽ trở thành bom hẹn giờ. Wrapper + explicit `SingleCoreGuard` type sẽ bắt lỗi tại compile-time.

**Chiến lược "progressive wrapping":**

Không refactor tất cả 8 globals cùng lúc. Thay vào đó:

1. **Tuần 1**: Wrap `TCBS` + `CURRENT` (scheduler — critical nhất, dùng nhiều nhất)
2. **Tuần 2**: Wrap `ENDPOINTS` + `TICK_COUNT` (IPC + timer — coupling cao)
3. **Tuần 3**: Wrap phần còn lại (`GRANTS`, `IRQ_BINDINGS`, `EPOCH_TICKS`, `TICK_INTERVAL`)

Mỗi tuần: refactor → chạy 189 tests → chạy 25 QEMU checkpoints → commit. **Giảm risk regression từ big-bang refactor.**

Pattern wrapper đề xuất: `KernelCell<T>` — zero-cost abstraction trên `UnsafeCell<T>` với `get()` method chỉ callable trong `unsafe` block nhưng với `// SAFETY` lý do cụ thể tại call site. Đây là middle ground: vẫn `unsafe` tại access point nhưng state KHÔNG phải `pub static mut` — encapsulation ở module level.

### Câu 3: Kani Formal Verification — khả thi không?

**Stance: Đầu tư vào Kani — đây là ROI cao nhất trong 10 năm tới, nhưng cần complementary tools**

**Kani là khả thi và đáng đầu tư.** Phân tích chi tiết:

**ROI Analysis (5-10-20 năm):**

- **Năm 1-2**: Investment cao (setup WSL/Docker, viết proof harnesses, learn bounded model checking). Output: ~15-20 proofs cho `cap.rs`, `elf.rs`, `ipc.rs` — chứng minh no-panic, no-OOB, capability soundness.
- **Năm 3-5**: Proofs trở thành **regression guard** — mỗi commit chạy Kani CI, bắt bugs mà unit tests miss. seL4 team report rằng formal proofs caught ~150 bugs mà testing missed trong 5 năm đầu.
- **Năm 10+**: AegisOS có **verified kernel core** (portable logic) — đây là moat cạnh tranh mà VxWorks/RTEMS không có. DO-333 (Formal Methods supplement to DO-178C) cho phép dùng formal verification **thay thế một phần testing** — giảm 30-50% verification cost cho certification.

**Limitations và mitigations:**

1. **Kani không verify inline asm** → Nhưng Phase L đã tách arch/kernel! `kernel/` module (ipc.rs, cap.rs, sched.rs logic, elf.rs, grant.rs) là 100% portable Rust — Kani verify được toàn bộ. Inline asm nằm trong `arch/aarch64/` → verify bằng manual review + QEMU testing (DO-178C §6.3.2 cho phép combination of methods).
2. **Bounded model checking cần `kani::unwind()` limits** → Cho AegisOS với NUM_TASKS=3, NUM_ENDPOINTS=4, NUM_GRANTS=2 — bounds rất nhỏ! Kani sẽ **exhaustive** với bounds thực tế. Đây là lợi thế của static allocation — IPC state machine với 4 endpoints × 3 tasks = 12 states, hoàn toàn tractable.
3. **Windows dev → cần WSL/Docker** → Docker đã có trong `ci/Dockerfile`. Thêm Kani vào Docker image là straightforward. CI chạy trên Linux rồi.

**So sánh alternatives:**

| Tool | Pros | Cons | Verdict |
|---|---|---|---|
| **Kani** | Rust-native, CBMC backend, exhaustive bounded | Không verify asm, cần Linux | ✅ Primary choice |
| **Prusti** | Rust verification, annotation-based | Immature, less industry adoption | ⚠️ Watch, don't adopt yet |
| **CBMC trực tiếp** | Mature, C/C++ proven | Không hiểu Rust semantics | ❌ Wrong tool |
| **Proptest** | Property-based testing, easy | Not exhaustive — random | ✅ Complement Kani |
| **Miri** | Detects UB in Rust | Interpreter, not prover | ✅ Complement Kani |

**Đề xuất: Kani (primary) + Proptest (complement) + Miri (UB detection).** Ba tools này cover 3 layers: exhaustive proof (Kani), randomized exploration (Proptest), UB runtime detection (Miri). Đây là defense-in-depth cho verification — DO-178C §6.4 khuyến khích "complementary verification methods".

**Target cụ thể cho Phase M5:**
- `cap.rs`: Prove `has_capability()` never panics, `cap_for_syscall()` returns correct bit for all 13 syscalls
- `elf.rs`: Prove `parse_elf64()` never OOB on any input ≤ segment limit, no-panic
- `ipc.rs`: Prove endpoint state machine consistency — no orphaned blocked tasks
- `sched.rs` (portable logic): Prove priority ordering, budget accounting correctness

### Câu 4: Code coverage target

**Stance: Target 80% statement coverage cho `kernel/`, 60% overall — với lộ trình rõ ràng lên 90%+ trong 2 năm**

**Tham chiếu tiêu chuẩn:**

- **DO-178C Level C**: Statement Coverage (SC) — mỗi câu lệnh thực thi ít nhất 1 lần. Đây là mức tối thiểu cho AegisOS ở giai đoạn hiện tại.
- **DO-178C Level B**: Decision Coverage (DC) — mỗi branch TRUE/FALSE đều covered. Target cho năm 2-3.
- **DO-178C Level A**: MC/DC — mỗi condition trong mỗi decision independently affects outcome. Target cho năm 5+ (cần tool support tốt hơn cho Rust).
- **IEC 62304 Class C**: Yêu cầu unit verification — coverage là evidence cho unit verification.
- **ISO 26262 ASIL D**: Yêu cầu MC/DC + branch coverage — nhưng tool chain cho Rust MC/DC chưa mature.

**Tại sao 80% cho `kernel/`, không phải 100%?**

1. **100% statement coverage là illusion** — code paths trong error handling (ví dụ: `_ => return Err(...)` trong match arms) có thể untestable trên host mà không trigger trên QEMU. DO-178C cho phép "dead code analysis" thay vì force 100%.
2. **80% là "sweet spot" cho effort/value** — nghiên cứu của NASA JPL (JPL Rule of Ten) cho thấy bugs-found-per-coverage-percent giảm mạnh sau 80%. Từ 80→90% tốn gấp 3 lần effort so với 60→80%.
3. **`arch/aarch64/` không thể measure trên host** — coverage chỉ đo được cho `kernel/` modules chạy trên x86_64. Đây là ~60% codebase. Vì vậy "80% kernel/" ≈ "48% overall", cộng thêm QEMU boot checkpoints cover arch code → ~60% overall estimate.

**Module priority cho coverage:**

| Module | Tầm quan trọng | Target | Lý do |
|---|---|---|---|
| `kernel/cap.rs` | 🔴 Critical | 95% | Gateway cho mọi syscall — sai ở đây = privilege escalation |
| `kernel/elf.rs` | 🔴 Critical | 90% | Parse untrusted input — sai ở đây = code execution |
| `kernel/ipc.rs` | 🔴 Critical | 85% | Core IPC — sai ở đây = deadlock hoặc data corruption |
| `kernel/sched.rs` | 🟠 High | 80% | Scheduler logic — sai ở đây = missed deadlines |
| `kernel/grant.rs` | 🟡 Medium | 75% | Shared memory — sai ở đây = isolation breach |
| `kernel/irq.rs` | 🟡 Medium | 75% | IRQ routing — sai ở đây = missed interrupts |
| `kernel/timer.rs` | 🟢 Low (mostly arch) | 70% | Chỉ có TICK_COUNT logic portable |

**Lộ trình coverage (5 năm):**

- **Phase M (2026)**: Statement coverage, target 80% kernel/ — thiết lập baseline
- **Phase P-Q (2026-2027)**: Decision coverage, target 85% — thêm branch testing
- **Phase T-U (2027-2028)**: MC/DC cho critical modules (cap.rs, elf.rs) — cần tool investment
- **Year 3-5**: Full MC/DC cho toàn bộ kernel/ — DO-178C Level A readiness

### Câu 5: Safety Foundation vs Feature Development

**Stance: Safety Foundation TRƯỚC — đây là quyết định chiến lược quan trọng nhất, và lịch sử chứng minh đáp án rõ ràng**

Đây là câu hỏi mà tôi, với vai trò Visionary, muốn trả lời mạnh mẽ nhất: **Đầu tư vào Safety Foundation (Phase M) TRƯỚC khi thêm bất kỳ feature nào.**

**Bằng chứng lịch sử — những dự án chọn sai:**

1. **Therac-25 (1985-1987)**: Thêm features (dual-energy mode) trước khi verify core safety → 6 tai nạn, 3 tử vong. Race condition trong software đã tồn tại từ Therac-6 nhưng hardware interlock che giấu. Khi thêm feature mới (loại bỏ hardware interlock), lỗi cũ bộc lộ. **Bài học: feature trên nền không verified = bom hẹn giờ.**

2. **Boeing 737 MAX MCAS (2018-2019)**: Feature mới (MCAS) được thêm vào mà không đánh giá đầy đủ tác động lên toàn hệ thống. Lack of traceability giữa requirement → code → test. 346 người thiệt mạng. **Bài học: DO-178C traceability không phải bureaucracy — nó cứu mạng người.**

3. **Mars Climate Orbiter (1999)**: Feature integration (navigation system) thiếu verification interface giữa modules. Unit mismatch (pound-force vs Newton) không bị caught. $327.6 million mất. **Bài học: interface verification (chính xác là thứ Kani + capability system prove) phải có TRƯỚC integration.**

**Bằng chứng từ industry leaders chọn đúng:**

1. **seL4 (2009)**: NICTA team quyết định prove functional correctness cho microkernel **8,700 dòng C** TRƯỚC khi thêm multicore, device driver framework, hay filesystem. Kết quả: proof hoàn thành 2009, deployment trong DARPA HACMS 2016, Boeing autonomous helicopter 2017. Nếu họ thêm features trước → proof sẽ intractable.

2. **Green Hills INTEGRITY (1997-2003)**: Hoàn thiện kernel core, xây dựng safety evidence, rồi mới mở rộng. Kết quả: DO-178B Level A certification đầu tiên cho RTOS (2003). Mọi competitor (VxWorks, LynxOS, RTEMS) đều có nhiều features hơn nhưng certification đến sau nhiều năm.

3. **RTEMS**: Ngược lại — thêm features liên tục (POSIX, SMP, nhiều BSPs) mà không invest vào formal verification. Kết quả: 30+ năm tuổi, dùng rộng rãi trong space (ESA, NASA), nhưng **chưa bao giờ đạt DO-178C certification** cho kernel. RTEMS đang phải retrofit safety evidence — tốn gấp 10 lần so với build-in-from-start.

**Phân tích chiến lược cho AegisOS:**

NUM_TASKS = 3 **không phải limitation — đó là simplification có chủ đích** cho verification. seL4 proof ban đầu cũng giới hạn số lượng capabilities và endpoints. Khi proof framework established, mở rộng sang dynamic tasks sẽ dễ hơn nhiều vì:

1. Proof framework đã verify core invariants (capability soundness, IPC correctness)
2. Dynamic task chỉ là generalization — thêm `NUM_TASKS = N` thay vì `3`
3. Coverage + traceability đã sẵn sàng catch regression khi thêm features

Nếu thêm features trước:
1. Mỗi feature thêm → codebase lớn hơn → coverage measurement sau này cover ít hơn → mất baseline
2. Dynamic tasks thay đổi scheduler, IPC, capability → traceability matrix phải viết lại
3. Kani proofs phức tạp hơn exponentially với state space lớn hơn

**Kết luận: "Verify small, then grow" — không phải "Grow big, then verify."**

AegisOS ở trạng thái lý tưởng để verify: ~3,500 dòng Rust portable, 3 tasks, 4 endpoints, 2 grants. Đây là **cửa sổ vàng** — nếu bỏ qua, mỗi feature thêm vào sẽ đóng cửa sổ này thêm một chút.

## Đề xuất bổ sung

### 1. Safety Case Document (5 năm)

Ngoài 6 sub-phases đề xuất, AegisOS cần bắt đầu **Safety Case** — tài liệu chính cho certification. DO-178C gọi đó là PSAC (Plan for Software Aspects of Certification), ISO 26262 gọi là Safety Case, IEC 62304 gọi là Software Development Plan. Bắt đầu bằng một file `docs/safety/safety-case-v0.md` liệt kê:
- Scope of certification target (kernel only, không bao gồm user tasks)
- Applicable standards (DO-178C Level C initially, upgrade path to Level B/A)
- Evidence inventory (tests, proofs, coverage, traceability)

Đây là tài liệu sống, cập nhật mỗi phase. Càng bắt đầu sớm, càng ít technical debt.

### 2. Deterministic Execution Analysis / WCET (10 năm)

ISO 26262 Part 6 §7.4.12 yêu cầu **Worst-Case Execution Time (WCET)** analysis cho ASIL C/D. AegisOS có lợi thế lớn: no heap + no recursion + static allocation = WCET analysis tractable. Nhưng hiện tại không có tool/measurement nào. Đề xuất:
- Phase N hoặc O: Thêm cycle counting instrumentation cho critical paths (syscall dispatch, context switch, IPC)
- Dùng `PMCCNTR_EL0` (ARM Performance Monitor) để đo trên QEMU
- Tạo WCET table trong documentation

Đây là evidence mà cả 3 tiêu chuẩn (DO-178C, IEC 62304, ISO 26262) đều yêu cầu cho các cấp cao.

### 3. Independent Review Process (5-10 năm)

DO-178C §6.3 yêu cầu "independence of verification" — Level A cần 30 objectives verified bởi người không phải tác giả. Solo developer là constraint hiện tại, nhưng có thể mitigate:
- **AI-assisted review**: Dùng LLM (đúng như discussion này) làm "second pair of eyes" — không đủ cho formal independence nhưng tốt hơn self-review
- **Open-source community**: Khi safety evidence đủ mature, publish và invite community review. seL4 open-sourced proofs năm 2014 → community caught several proof gaps
- **Formal tool as independent verifier**: Kani proofs = machine-checked evidence. DO-333 §6.1 cho phép "tool-based verification can satisfy independence objectives" nếu tool đạt qualification level

### 4. RISC-V Readiness Score (20 năm)

Mỗi phase nên đánh giá **RISC-V readiness** — percentage of code portable. Phase L đã tách arch/kernel, nhưng có thể có implicit ARM assumptions trong `kernel/` (ví dụ: page size 4KB hardcode, endianness assumption). Đề xuất thêm một metric:
- `RISC-V Readiness = (portable lines) / (total lines) × 100%`
- Target: ≥70% sau Phase M, ≥85% sau Phase P

Trong 10-20 năm, RISC-V sẽ là ISA dominant cho safety-critical (ESA đã chọn RISC-V cho LEON replacement, DARPA SSITH dùng RISC-V). AegisOS cần sẵn sàng.

### 5. Fault Injection Testing Framework (10 năm)

ISO 26262 Part 6 §9.4.2 và IEC 62304 Amendment 1 đều nhấn mạnh **robustness testing** — inject faults và verify system responds correctly. AegisOS đã có fault isolation (Phase E), nhưng chưa có **systematic fault injection**:
- Inject invalid syscall numbers → verify capability rejection
- Inject corrupted ELF headers → verify parser rejection
- Inject stack overflow in user task → verify fault containment
- Inject timer starvation → verify watchdog triggers

Đây không cần hardware — có thể làm hoàn toàn trên QEMU. 20-30 fault injection tests sẽ là evidence cực mạnh cho certification.

## Tóm tắt stance

| Câu hỏi | Stance tóm tắt (1 dòng) |
|---|---|
| 1 | **M3→M2→M1→M4→M6→M5**: Diagnostic/logging trước → refactor → measure → trace → prove. Risk reduction trước, proof cuối. |
| 2 | **Encapsulate bằng `KernelCell<T>` progressive wrapping** — SAFETY comments không đủ cho formal tools và certification auditors. |
| 3 | **Kani đáng đầu tư** — bounds nhỏ (3 tasks, 4 endpoints) = exhaustive tractable. Bổ sung Proptest + Miri cho defense-in-depth. |
| 4 | **80% statement coverage cho `kernel/`**, 95% cho cap.rs/elf.rs — lộ trình lên MC/DC trong 3-5 năm theo DO-178C levels. |
| 5 | **Safety Foundation TRƯỚC** — "Verify small, then grow." Cửa sổ vàng verify kernel nhỏ sẽ đóng lại với mỗi feature thêm vào. |
