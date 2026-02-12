# 🔧 Gemini-Pragmatist Review — Phase O Round 1 | 2026-02-12

## Q1: Multi-ELF Architecture — Per-task fixed region hay Shared pool?

**Lập trường:** Option A — Per-task fixed region (16 KiB mỗi slot)

**Effort estimate:** ~14–18h tổng cho O1 (linker + `load_elf_to_task()` + 2 user crates + integration). Kế hoạch nói 26h — tôi nghĩ cao quá nếu thực sự chỉ refactor, không thêm feature mới.

**Risk assessment:** LOW. Đây là thay đổi predictable nhất trong toàn bộ Phase O.

**Lý do:**

1. **YAGNI — Shared pool và PIC là overkill.** Hiện tại user binary `user/hello` chỉ có **62 dòng Rust**, compile ra vài KB. Mỗi slot 16 KiB = 4 pages, tổng 96 KiB = 0.07% RAM. Bạn KHÔNG cần PIC, KHÔNG cần relocation, KHÔNG cần dynamic allocator cho 96 KiB. Option B (shared pool) giải quyết bài toán mà bạn **chưa có** — binary lớn hơn 16 KiB. Khi nào có bài toán đó, mở rộng slot size đơn giản hơn nhiều so với viết allocator.

2. **Option C (Hybrid + build script auto-gen linker.ld) là scope creep.** Hiện tại `user/hello/linker.ld` chỉ có **25 dòng**, hardcode `0x40100000`. Với 3 binaries (hello, sensor, logger), bạn cần 3 linker scripts khác nhau chỉ ở dòng `. = 0x401X_0000`. Copy-paste 3 file × 25 dòng = 75 dòng — nhanh hơn viết build script. Build script chỉ đáng khi có >6 binaries, mà bạn có tối đa 6 slots.

3. **Bằng chứng từ code:**
   - `linker.ld`: Chỉ cần sửa `. += 3 * 4096` → `. += 6 * 16 * 1024` — **1 dòng thay đổi**.
   - `parse_elf64()` đã là pure function nhận `&[u8]` — xem `src/kernel/elf.rs`. Chỉ cần wrapper `load_elf_to_task()` gọi nó với đúng address.
   - Cache maintenance loop trong `kernel_main()` đã generic — chỉ cần parameterize start/end.

4. **Effort thực tế breakdown:**
   - Sửa `linker.ld`: 1 dòng — 10 phút
   - Thêm constants `platform/qemu_virt.rs`: `elf_load_addr()` — 30 phút
   - `load_elf_to_task()` wrapper trong `elf.rs`: extract ~40 dòng từ `kernel_main` — 2h
   - User linker scripts: copy+modify `user/hello/linker.ld` × 2 — 30 phút
   - Tạo `user/sensor` + `user/logger` crates: clone user/hello cấu trúc — 3–4h
   - Integration trong `main.rs`: thêm `include_bytes!` + loop — 2h
   - MMU mapping cho 6 regions: mở rộng existing loop — 1h
   - Tests: 8–10 test cases — 3h
   - **Total: ~14h** (không phải 26h)

**Pitfall thực tế:**
- **Mỗi user crate phải build riêng trước kernel build.** Hiện `include_bytes!` hardcode path. Nếu chưa build user crate → kernel compile fail. Cần document build order rõ ràng.
- **user/hello load address sẽ đổi** nếu chuyển slot. `user/hello/linker.ld` hardcode `0x40100000` (slot 0 = task 2). Nếu giữ hello ở task 7, address phải là `0x40114000`. Cần quyết định dứt khoát mapping task↔slot.

---

## Q2: libsyscall — Standalone crate hay Workspace member?

**Lập trường:** Option A — Standalone path dependency

**Effort estimate:** 4h max. Plan nói 6h — hợp lý nếu tính cả refactor `user/hello`.

**Risk assessment:** LOW, nhưng có 1 gotcha quan trọng.

**Lý do:**

1. **Option B (Workspace member) = rắc rối target mismatch.** Kernel build target là `aarch64-aegis.json`, user build target là `aarch64-user.json`. Cargo workspace muốn build tất cả members cùng target — sẽ phải dùng `--target` khác nhau hoặc loại trừ. Standalone path dep đơn giản hơn: mỗi user crate chỉ `libsyscall = { path = "../libsyscall" }` trong Cargo.toml riêng.

2. **Option C (inline `include!()`) = worse than status quo.** Vẫn duplicate code vào mỗi binary — chỉ hide nó. Không có type checking cross-crate, không có version gì cả.

3. **ROI rõ ràng:** Hiện `user/hello/src/main.rs` có **18 dòng syscall wrappers** (`syscall_write` + `syscall_yield`) trên tổng 62 dòng. Với 3 binaries = 54 dòng duplicate. libsyscall loại bỏ hoàn toàn.

4. **Effort breakdown:**
   - Tạo `user/libsyscall/Cargo.toml` + `src/lib.rs`: copy wrappers từ `user/hello` — 1.5h
   - Refactor `user/hello`: xóa 18 dòng, thêm `use libsyscall::*` — 30 phút
   - Tạo `aarch64-user.json` cho libsyscall (hoặc share từ hello) — 30 phút
   - Test: build hello, verify QEMU output unchanged — 1h
   - **Total: ~4h**

**Pitfall thực tế:**
- **libsyscall PHẢI dùng cùng `aarch64-user.json` target spec.** Giải pháp đơn giản: đặt `aarch64-user.json` ở `user/` root, tất cả crates reference cùng file.
- **Đừng over-scope libsyscall.** Kế hoạch liệt kê 14 syscall wrappers + convenience macros. Thực tế `user/hello` chỉ dùng 2 (write + yield), sensor dùng ~4, logger ~4. Bắt đầu với 5–6 wrappers thực sự cần, thêm dần khi có user. **Ship minimal, expand later.**

---

## Q3: Task 7 — Tách IDLE khỏi ELF demo hay giữ dual-role?

**Lập trường:** Option B — Tách. Task 7 = idle thuần, ELF demo chuyển sang task slot khác.

**Effort estimate:** 2h refactor. Gần như miễn phí vì O1 đã phải sửa `kernel_main()` anyway.

**Risk assessment:** NEGLIGIBLE.

**Lý do:**

1. **Dual-role task 7 = nguồn confusion.** Hiện tại `sched::init()` set task 7 entry = `idle_entry`, nhưng sau đó `kernel_main()` ghi đè thành ELF entry. Task 7 metadata nói `priority: 5, budget: 2` — đây là config cho "demo task chạy 2 ticks rồi dừng", **không phải idle task behavior**. Idle task cần `priority: 0, budget: 0` (luôn sẵn sàng, không hết budget).

2. **`schedule()` fallback = IDLE_TASK_ID = 7.** Nếu không có ready task, scheduler force task 7 ready. Nếu task 7 đang chạy ELF demo code thay vì `wfi` loop, fallback sẽ execute user code thay vì idle — semantic sai.

3. **Phase O cần task 7 là idle thuần** vì bạn sẽ có 5 active ELF tasks (2–6) + 2 kernel tasks (0, 1). Nếu tất cả 7 tasks blocked/exhausted budget, scheduler cần 1 slot idle đáng tin cậy.

4. **Effort: gần zero** vì trong O1 refactor, bạn đã phải di chuyển ELF loading ra khỏi hardcode task 7. Chỉ cần: (a) giữ task 7 entry = `idle_entry`, (b) load `user/hello` vào task 4 hoặc 5 thay vì 7.

**Pitfall:**
- Đảm bảo `IDLE_TASK_ID` constant giữ nguyên = 7. Không đổi const, chỉ đổi behavior.
- `idle_entry()` (`main.rs`) = `wfi` loop. Perfect — giữ nguyên.

---

## Q4: SYS_EXIT scope — Chỉ self-exit hay thêm SYS_KILL?

**Lập trường:** Option A — Chỉ SYS_EXIT (self-exit). Dứt khoát KHÔNG làm SYS_KILL trong Phase O.

**Effort estimate:** 8–10h (plan nói 14h — quá cao).

**Risk assessment:** MEDIUM cho SYS_EXIT (cleanup logic phức tạp), HIGH cho SYS_KILL (attack surface).

**Lý do:**

1. **SYS_KILL = security nightmare cho microkernel.** Cho phép task A kill task B = bypass fault isolation. Trong safety-critical context, task lỗi kill task đúng = catastrophic. seL4 cũng không có raw kill — chỉ có authority-based revocation qua capabilities. **YAGNI cực mạnh ở đây.**

2. **SYS_EXIT cleanup đã gần như sẵn.** `fault_current_task()` trong `sched.rs` đã làm:
   - `cleanup_task()` ✅ (IPC)
   - Set state ✅
   - Schedule away ✅

   SYS_EXIT handler = gần như copy `fault_current_task` nhưng set `Exited` thay vì `Faulted`. **~20 dòng code mới trong kernel.**

3. **Khác biệt Faulted vs Exited duy nhất = restart policy.** `tick_handler()` check `state == Faulted`. `Exited` sẽ không match → không auto-restart. **Zero logic change trong scheduler.**

4. **Effort breakdown:**
   - `TaskState::Exited` variant — 1 dòng — 10 phút
   - `CAP_EXIT = 1 << 18` + `cap_for_syscall(13)` — 3 dòng — 20 phút
   - `handle_svc()` thêm case 13 — 5 dòng — 30 phút
   - `sys_exit()` function: clone `fault_current_task` logic — ~30 dòng — 1.5h
   - Watchdog skip Exited — 1 dòng — 5 phút
   - `libsyscall` thêm `syscall_exit()` wrapper — 10 dòng — 20 phút
   - Host tests: 8–10 cases — 3h
   - QEMU checkpoint: 1 task gọi exit → verify log — 1h
   - **Total: ~8h** (không phải 14h)

**Pitfall:**
- **Option C nói "SYS_KILL defer" — đồng ý defer, nhưng đừng design cho nó ngay.** Không cần reserved bit, không cần placeholder. Khi nào cần thì thêm.
- `Exited` state phải được handle ở **MỌI nơi** check TaskState: scheduler, watchdog, epoch_reset, IPC. Grep `TaskState` để tìm tất cả call sites — hiện có ~8 chỗ. Thêm `Exited` match arm ở mỗi chỗ = safe.

---

## Q5: Kani IPC proofs — 3 proofs đủ hay mở rộng?

**Lập trường:** 3 proofs là đủ cho Phase O. KHÔNG thêm deadlock-freedom hay priority inversion. Update `schedule_idle_guarantee` cho Exited = CÓ, nhưng trivial.

**Effort estimate:** 8–10h cho 3 proofs. Plan nói 12h — hợp lý nếu tính debugging Kani.

**Risk assessment:** MEDIUM — Kani có thể timeout trên IPC proofs phức tạp.

**Lý do:**

1. **3 proofs được chọn đúng target.** `SenderQueue` là circular buffer with 4 slots — state space nhỏ (MAX_WAITERS=4), Kani handle dễ. Message integrity = copy 4 × u64, verifiable. Cleanup completeness = iterate 4 endpoints × 4 waiters — bounded.

2. **Deadlock-freedom proof = QUÁI VẬT.** Để chứng minh "không có state nào tất cả tasks đều Blocked", bạn cần model:
   - 8 tasks × 5 states = state space cực lớn
   - 4 endpoints × (sender queue + receiver) = combinatorial explosion
   - Priority inheritance interaction

   Kani với CBMC backend sẽ **timeout** trên state space này nếu không có abstraction thông minh. Đây là bài toán PhD, không phải 12h sprint task.

3. **Priority inversion proof cũng quá scope.** `priority` và `base_priority` đã có trong `sched.rs` — nhưng chứng minh nó correct cần model toàn bộ scheduler + IPC interaction. Defer sang Phase P.

4. **Update `schedule_idle_guarantee` cho Exited = 5 phút.** `pick_next_task_pure()` nhận `is_ready: [bool; N]` — Exited task sẽ có `is_ready = false`. Proof đã cover case "tất cả ineligible → return IDLE". **Không cần sửa proof, chỉ cần thêm comment.**

5. **Effort breakdown cho 3 proofs:**
   - SenderQueue overflow: extract pure functions `push_pure()`/`pop_pure()` → Kani harness — 3h
   - Message integrity: extract `copy_message_pure()` — 2h
   - Cleanup completeness: extract `cleanup_pure()` — 3h
   - Debug + timeout tuning (`#[kani::unwind(5)]`) — 2h
   - **Total: ~10h**

**Pitfall:**
- **Kani proofs phải dùng pure functions, không phải globals.** Pattern đã establish trong `sched.rs` — `pick_next_task_pure()` mirrors `schedule()` nhưng nhận parameters. Phải làm tương tự cho IPC.
- **SenderQueue `remove()` method** trong `ipc.rs` rebuild toàn bộ queue — O(n). Kani sẽ unroll loop — cần `#[kani::unwind(5)]` cho MAX_WAITERS=4.

---

## Q6: Build system — Manual, build.rs, hay script?

**Lập trường:** Option A — Manual per-crate. Thêm README documentation rõ ràng.

**Effort estimate:** 30 phút viết docs. Plan Option B (build.rs) = 4–8h. Option C (script) = 2–4h.

**Risk assessment:** LOW cho manual. MEDIUM cho build.rs (cross-compilation complexity).

**Lý do:**

1. **Bạn có 3 user binaries.** Ba. Không phải 30. Manual build 3 crates:
   ```bash
   cd user/hello  && cargo build --release --target aarch64-user.json -Zbuild-std=core
   cd user/sensor && cargo build --release --target aarch64-user.json -Zbuild-std=core
   cd user/logger && cargo build --release --target aarch64-user.json -Zbuild-std=core
   cd ../.. && cargo build --release -Zbuild-std=core --target aarch64-aegis.json
   ```
   4 commands. Copy-paste-able. Ai cũng hiểu. Debug được bằng mắt.

2. **build.rs cross-compilation = pain.** `build.rs` chạy trên host target. Muốn nó invoke `cargo build --target aarch64-user.json` = spawn child process trong build script. Cargo KHÔNG recommend this — nó break incremental compilation, parallel builds, và `cargo check`.

3. **Script wrapper (Option C) chỉ đáng khi CI cần.** Thêm `scripts/build-all.sh` = 10 dòng bash wrapper — nếu muốn. Nhưng đây không phải build system, chỉ là convenience script.

4. **`include_bytes!` paths là compile-time checked.** Nếu user binary chưa build → kernel compile fail với error rõ ràng ("file not found"). Đây là **feature, không phải bug** — nó force đúng build order.

**Pitfall:**
- **Document build order rõ ràng** trong README hoặc Makefile comment. Hiện README chỉ nói `cargo build --release` cho kernel — cần thêm user build steps.
- Nếu tương lai >6 binaries → revisit. Nhưng NUM_TASKS=8, task 0+1 = kernel entry points, task 7 = idle. Bạn chỉ có **tối đa 5 ELF slots** (tasks 2–6). Manual works fine.

---

## Tổng kết Pragmatist

| Câu hỏi | Chọn | Effort thực tế | Ship trong sprint? |
|---------|------|----------------|-------------------|
| Q1 | A: Per-task fixed | ~14h | ✅ Yes |
| Q2 | A: Standalone path dep | ~4h | ✅ Yes |
| Q3 | B: Tách idle | ~2h (trong O1) | ✅ Yes (free) |
| Q4 | A: Chỉ SYS_EXIT | ~8h | ✅ Yes |
| Q5 | 3 proofs đủ | ~10h | ✅ Yes |
| Q6 | A: Manual | ~0.5h | ✅ Yes |

**Tổng effort ước tính: ~38h** (plan nói 44–62h). Cắt được ~25% bằng cách tránh over-engineering. 60h hard ceiling → có 22h buffer cho unexpected issues.

**Biggest risk overall:** Multi-binary build coordination. 3 user crates × custom target × `include_bytes!` = nhiều moving parts. Giải pháp: build + test **từng crate một**, QEMU checkpoint sau mỗi binary mới. Đừng batch 3 binaries rồi debug cùng lúc.

**Điều tôi sẽ CẮT nếu hết thời gian:**
1. ~~user/logger~~ — 2 binaries (hello + sensor) đủ chứng minh multi-ELF. Logger là "nice to have".
2. ~~Kani proof #3 (cleanup completeness)~~ — 2 proofs (overflow + integrity) đã cover core properties.
3. ~~Grant + IRQ cleanup trong SYS_EXIT~~ — Reuse `fault_current_task` logic trực tiếp, defer separate cleanup paths.
