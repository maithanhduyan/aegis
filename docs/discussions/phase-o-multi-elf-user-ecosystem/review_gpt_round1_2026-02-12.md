# 🔮 GPT-Visionary Review — Phase O Round 1 | 2026-02-12

## Q1: Multi-ELF Architecture

**Lập trường:** Option C — Hybrid: Per-task fixed region + build script auto-generate linker.ld từ template.

**Lý do:**

Từ góc nhìn 5-10-20 năm, đây là quyết định kiến trúc **nền tảng** — ảnh hưởng đến mọi user binary mãi mãi. Phân tích từng option:

- **Option A (Per-task fixed region, manual linker.ld):** Đúng về isolation — mỗi task có vùng nhớ riêng, deterministic, dễ verify. Nhưng thủ công: 6 tasks = 6 linker.ld files phải đồng bộ bằng tay. Khi scale lên 16 hay 32 tasks (RISC-V port, multi-core future), quản lý thủ công trở thành nightmare. Violates DO-178C §5.5 "configuration management" — thay đổi layout → sửa N files.

- **Option B (Shared pool + PIC/relocation):** Lý tưởng cho long-term (giống Linux dynamic loader), nhưng **phá vỡ constraint hiện tại**: PIC cần `-fPIC` hoặc GOT/PLT, tăng complexity khổng lồ, cần dynamic linker — đi ngược triết lý "zero heap, static allocation". Runtime relocation thêm attack surface cho safety-critical system. ISO 26262 Part 6 §7.4.3 yêu cầu "avoid dynamic behavior" ở ASIL-D.

- **Option C (Hybrid):** Giữ lợi thế deterministic của fixed addresses (mỗi task slot → địa chỉ tĩnh, verifiable, no runtime allocation), nhưng **auto-generate** linker.ld từ template eliminates human error. Khi hiện tại plan đã cho mỗi task 16 KiB tại offset cố định theo `elf_load_addr()`, 1 build script + 1 template linker.ld + `elf_load_addr` constant = single source of truth.

**Viện dẫn safety standards:**
- **DO-178C §5.5:** Traceability — mỗi task address phải traceable đến 1 nguồn duy nhất (template + slot index), không phải N files riêng lẻ.
- **ISO 26262 Part 6 §7.4.3:** "Avoid dynamic memory allocation at ASIL C/D" — fixed regions đáp ứng, PIC không.
- **DO-333 FM.A-5:** Fixed addresses cho phép Kani verify bounds tĩnh (elf_load_addr invariant) — không thể verify runtime pool allocation bằng bounded model checking.

**Đề xuất bổ sung:**

1. **Template linker.ld:**
   ```
   /* user/linker.ld.template */
   ENTRY(_start)
   SECTIONS { . = ${LOAD_ADDRESS}; ... }
   ```
   Build script: `sed "s/\${LOAD_ADDRESS}/$(elf_load_addr $SLOT)/" < template > user/$CRATE/linker.ld`

2. **Kani proof cho `elf_load_addr()`:** Chứng minh ∀ slot ∈ [0,5]: addresses don't overlap ∧ within .elf_load region. Plan đã list test `test_elf_load_addr_no_overlap` nhưng nên **nâng thành Kani proof** — 1 proof thay 3 test cases, coverage mạnh hơn.

3. **Future-proofing cho larger binaries:** 16 KiB/task có thể chật khi sensor task cần lookup tables. Đề xuất thêm compile-time assert: `const_assert!(size_of_binary <= ELF_LOAD_SIZE_PER_TASK)` trong build script. Fail sớm tốt hơn runtime load failure.

4. **20-năm vision — gradual migration path:** Khi port sang real hardware (STM32MP1, i.MX8), MPU/MMU sẽ hỗ trợ per-task page tables thật (không còn shared identity map). Lúc đó mỗi task có virtual address space riêng → tất cả đều link tại `0x0000_1000` giống nhau. Fixed-region design hiện tại là stepping stone đúng hướng — dễ migrate sang per-task VAS hơn PIC/runtime relocation.

---

## Q2: libsyscall Design

**Lập trường:** Option B — Workspace member (Cargo workspace).

**Lý do:**

Nhìn codebase hiện tại: `user/hello/src/main.rs` duplicate 2 syscall wrappers (~18 lines). Plan O2 thêm `user/sensor` + `user/logger` = **3x duplication**. Đến Phase P-Q với 6+ binaries = disaster.

Phân tích:

- **Option A (Standalone crate, path dependency):** Hoạt động, nhưng phải quản lý Cargo.lock riêng cho mỗi user crate. Không có unified dependency resolution. Khi `libsyscall` thay đổi ABI, phải manually rebuild từng crate — không có `cargo build --workspace` để catch incompatibility.

- **Option B (Workspace member):** `cargo build --workspace` ensures mọi crate dùng **cùng version** libsyscall. Cargo workspace = single Cargo.lock = reproducible builds. Đây là critical cho DO-178C §12.1.3 "software configuration management" — 1 lockfile = 1 truth. Khi thêm binary mới, chỉ thêm entry vào `[workspace.members]`.

- **Option C (include!() pattern):** Anti-pattern. No type checking across crate boundary. No separate compilation unit. Không thể test libsyscall independently. Violates DO-178C §5.3 "software architecture" — module boundary không tồn tại.

**Viện dẫn safety standards:**
- **DO-178C §12.1.3:** Configuration management — Cargo workspace + single Cargo.lock = bit-exact reproducible builds.
- **DO-178C §5.5:** Single source of truth — 1 crate `libsyscall` = 1 module = 1 test set = 1 review unit.
- **IEC 62304 §5.5.3:** Unit verification — libsyscall có thể test riêng trên host (x86_64 stubs cho asm), independent of kernel.

**Đề xuất bổ sung:**

1. **Workspace layout cụ thể:** Tạo `user/Cargo.toml` là workspace root:
   ```toml
   [workspace]
   members = ["libsyscall", "hello", "sensor", "logger"]
   resolver = "2"
   ```
   Giữ kernel Cargo.toml ở root, user workspace riêng — tránh kernel depend on user code.

2. **Host-testable libsyscall:** Thêm `#[cfg(not(target_arch = "aarch64"))]` stubs cho syscall wrappers — return dummy values. Cho phép host tests verify constant values, type signatures.

3. **Syscall ABI versioning:** Thêm `pub const SYSCALL_ABI_VERSION: u32 = 1;` vào libsyscall. Kernel check version tại boot (hoặc compile-time). 10-năm vision: khi ABI thay đổi (Phase R+), version mismatch = compile error thay vì runtime mystery fault.

4. **`#[inline(always)]` cho mọi wrapper:** libsyscall phải giữ nguyên — cross-crate inline cần `#[inline(always)]` (không phải `#[inline]`), vì user binaries link statically.

---

## Q3: Task 7 — IDLE separation

**Lập trường:** Option B — Tách: task 7 = idle thuần, ELF demo → task 2–6.

**Lý do:**

Hiện tại `idle_entry()` chỉ là `wfi` loop, nhưng `kernel_main()` override task 7 entry point bằng ELF binary. Task 7 đang "dual-role": IDLE_TASK_ID trong scheduler logic **và** ELF demo. Đây là architectural smell:

1. **Scheduler assumption bị phá:** `schedule()` có fallback `IDLE_TASK_ID` khi không có task Ready. Nếu IDLE_TASK_ID running user code (print "L5:ELF") thay vì `wfi`, nó **tiêu tốn CPU thay vì idle**. Hiện tại task 7 có `time_budget: 2` nên budget hết nhanh, nhưng đây là workaround, không phải design đúng.

2. **Safety invariant:** IDLE task **phải always schedulable** — nó là "last resort". Nếu ELF demo fault → task 7 Faulted → auto-restart sau 100 ticks. Trong 100 ticks đó, nếu mọi task khác cũng blocked/faulted, scheduler **không có ai để chạy** → force idle nhưng idle đang Faulted → phải restart ngay (đã handle trong code), nhưng phức tạp và fragile.

3. **DO-178C §5.3 (Architecture):** Separation of concerns — idle task có semantic riêng (power management, watchdog feed), không nên mix với demo logic. ISO 26262 Part 6 §7.4.1 cũng yêu cầu "single responsibility" cho mỗi software unit.

**Viện dẫn safety standards:**
- **ISO 26262 Part 6 §7.4.1:** "Each software unit shall implement a single functionality" — IDLE ≠ ELF demo.
- **DO-178C §5.3:** Software architecture phải clearly separate concerns. IDLE là kernel-level safety mechanism.
- **ARINC 653 (aerospace partitioning):** Idle partition là system partition, không phải application partition.

**Đề xuất bổ sung:**

1. **Task 7 (IDLE) = kernel-linked, always `wfi`:** Không bao giờ load ELF vào task 7. IDLE không cần caps (CAP_NONE), priority 0, budget 0 (unlimited).

2. **ELF demo → task 2:** `user/hello` load vào task 2 slot (`elf_load_addr(0)`). Demo output "L5:ELF" vẫn giữ, nhưng ở task 2 thay vì task 7.

3. **Option C (Double-idle) là overkill hiện tại** nhưng ghi nhận cho 20-năm vision: multi-core cần per-core idle task.

4. **Idle hardening:** `idle_entry` nằm trong `.text` kernel (không phải `.elf_load`). IDLE không bao giờ fault vì chỉ chạy `wfi` — 1 instruction, no memory access, no syscall.

---

## Q4: SYS_EXIT scope

**Lập trường:** Option C — SYS_EXIT only, SYS_KILL defer Phase P.

**Lý do:**

- **Option A (Chỉ SYS_EXIT):** An toàn nhất cho Phase O. Self-exit = task quyết định khi nào nó xong. Nhưng **thiếu** cho production: supervisor task cần kill runaway task.

- **Option B (SYS_EXIT + SYS_KILL cùng lúc):** Scope creep nguy hiểm. SYS_KILL cần:
  - Capability `CAP_KILL_TASK_X` (per-target? per-task?) — design space lớn
  - Race condition: task đang trong IPC + bị kill → cleanup phức tạp hơn self-exit
  - DO-178C §5.6 yêu cầu "analysis of failure modes" — SYS_KILL failure modes nhiều hơn SYS_EXIT gấp bội

- **Option C (SYS_EXIT now, SYS_KILL defer):** Phase O focus self-exit + cleanup. Phase P thêm SYS_KILL khi SYS_EXIT đã proven correct (Kani proof cho exit cleanup), cleanup logic đã battle-tested qua QEMU checkpoints.

**Viện dẫn safety standards:**
- **DO-178C §5.6:** Handle normal trước, abnormal sau.
- **ISO 26262 Part 6 §9.4.2:** Mỗi mechanism test riêng. Mix SYS_EXIT + SYS_KILL = test matrix explodes.

**Đề xuất bổ sung:**

1. **Exit code logging + store:** `exit_code` trong x0 nên log nhưng **cũng store vào TCB** (`pub exit_code: u64`). Future SYS_WAIT_EXIT (Phase P/Q) cần return exit code cho parent.

2. **Exited task slot reuse:** Phase O nên **explicitly** document: "Exited slots NOT reusable in Phase O. Reuse deferred to Phase P (SYS_SPAWN)." Tech debt có chủ đích — document nó.

3. **Kani proof cho SYS_EXIT:** Thêm proof thứ 4 cho exit cleanup completeness — chứng minh cleanup xóa task khỏi tất cả endpoints, grants, IRQ bindings.

---

## Q5: Kani IPC proofs

**Lập trường:** 3 proofs Phase O + 1 proof mở rộng (schedule_idle update) = **4 proofs mới**, tổng 10. Deadlock-freedom và priority inversion defer Phase P.

**Lý do:**

**Proof 1 (SenderQueue overflow):** ✅ Cần thiết. MAX_WAITERS=4, state space nhỏ, Kani handle dễ. Nên mở rộng thêm FIFO ordering: `push(A); push(B); pop() == A; pop() == B`.

**Proof 2 (Message integrity):** ✅ Cần thiết. Copy x[0..3] — phải prove payload preserved. Cần pure function refactor.

**Proof 3 (Cleanup completeness):** ✅ Critical cho SYS_EXIT correctness.

**Deadlock-freedom:** ⚠️ Defer Phase P. State space 8 tasks × 4 endpoints × 4 waiters = hàng triệu states. Kani CBMC timeout risk cao. Cần abstraction hoặc compositional verification.

**Priority inversion:** ⚠️ Defer Phase P. Cần temporal logic (TLA+ hoặc SPIN).

**schedule_idle update cho Exited:** ✅ Cần trong Phase O — trivial update, Exited task sẽ có `is_ready = false`.

**Viện dẫn safety standards:**
- **DO-333 FM.A-7:** 10 proofs cho 5 modules = good coverage breadth.
- **DO-333 FM.A-2:** Kani/CBMC phù hợp cho bounded data structure verification. Deadlock cần different method.

**Đề xuất bổ sung:**

1. **Phase P Kani roadmap:** Document: P1 deadlock-freedom (SPIN/TLA+), P2 priority inversion, P3 exit cleanup cross-module.
2. **Regression:** CI phải `cargo kani --all` — confirm 10 proofs stay under timeout.

---

## Q6: User binary build system

**Lập trường:** Option C — Makefile/Script wrapper, với **migration path** sang build.rs Phase P.

**Lý do:**

- **Option A (Manual):** 3+ binaries = người quên rebuild → stale binary embedded → runtime failure. DO-178C §12.1.3 requires automated, repeatable build.

- **Option B (build.rs):** Cross-target build script = fragile. Cargo lock contention, nested cargo builds deadlock risk.

- **Option C (Script):** Đơn giản, transparent, reproducible. Makefile IS documentation. DO-178C §12.3.1 "build environment documented".

**Viện dẫn safety standards:**
- **DO-178C §12.1.3:** Script = 1 command = reproducible.
- **ISO 26262 Part 8 §11.4.7:** Script checked into git = versioned.

**Đề xuất bổ sung:**

1. **`scripts/build-user.sh`:** Fail-fast (`set -e`), visible output, binary size gate ≤ 16 KiB.
2. **Hash embedding:** SHA-256 của mỗi binary, kernel log hash at boot → DO-178C §12.3.2 integrity.
3. **Migration path → build.rs Phase P:** Khi Cargo multi-target workspace stabilize.
4. **CI integration:** `scripts/build-user.sh` trước test pipeline.

---

## Tổng kết tầm nhìn dài hạn

| Quyết định | Phase O | 5-10 năm | 20 năm |
|---|---|---|---|
| Q1 ELF arch | Fixed + template | MPU regions on real HW | Full MMU per-task |
| Q2 libsyscall | Workspace member | Stable syscall API | POSIX-subset layer |
| Q3 IDLE | Tách thuần | Per-core idle | DVFS integration |
| Q4 EXIT/KILL | SYS_EXIT only | Full process lifecycle | Hot-reload |
| Q5 Kani | 10 proofs | Deadlock + SPIN/TLA+ | Certified formal proofs |
| Q6 Build | Script wrapper | build.rs migration | Certified toolchain |

**Khuyến nghị cuối:** Đề xuất cắt O1c xuống 2 binaries (sensor + hello, defer logger Phase P) để giữ trong 60h ceiling. Logger chỉ khác sensor ở RECV thay SEND — value-add thấp cho 6-8h effort.
