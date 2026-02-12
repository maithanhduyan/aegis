# 🤝 Final Consensus | Phase O — Multi-ELF & User Ecosystem | 2026-02-12

## Tổng quan

- **Chủ đề**: Phase O — Multi-ELF Loading, libsyscall, SYS_EXIT, Kani IPC Proofs
- **Số vòng thảo luận**: 2
- **Ngày bắt đầu → Đồng thuận**: 2026-02-12 → 2026-02-12
- **Participants**: GPT-Visionary-Agent (Visionary), Gemini-Pragmatist-Agent (Pragmatist)

---

## Kết luận đồng thuận

### 1. Multi-ELF Architecture: Manual linker.ld + `const_assert!`

**Quyết định:** Per-task fixed region (16 KiB/slot, 6 slots = 96 KiB), manual linker.ld per-binary, compile-time `const_assert!` kiểm tra binary size ≤ slot size, document migration path sang template-based generation khi >5 binaries.

**Lý do:**
- *Visionary*: Fixed addresses = deterministic, Kani-verifiable, DO-178C traceable. `const_assert!` = compile-time safety guard thay thế build script.
- *Pragmatist*: 3 files × 25 dòng = 75 dòng — nhanh hơn viết build script. YAGNI cho 5 ELF slots max. const_assert! = zero cost.

**Hành động tiếp theo:**
- [ ] Mở rộng `.elf_load` section trong `linker.ld`: 12 KiB → 96 KiB (6 × 16 KiB)
- [ ] Thêm `elf_load_addr(slot: usize)` trong `platform/qemu_virt.rs`
- [ ] Tạo `load_elf_to_task()` wrapper trong `kernel/elf.rs` (extract từ `kernel_main`)
- [ ] Tạo `user/sensor/linker.ld` + `user/logger/linker.ld` với slot-specific addresses
- [ ] Thêm `const_assert!` cho mỗi `include_bytes!` binary
- [ ] Document migration trigger trong README

---

### 2. libsyscall: Separate user/ workspace

**Quyết định:** Tạo `user/Cargo.toml` workspace chứa `["libsyscall", "hello", "sensor", "logger"]`. Shared `user/aarch64-user.json` target. Tách biệt hoàn toàn khỏi kernel workspace.

**Lý do:**
- *Visionary*: Workspace cho unified Cargo.lock, cargo clippy --workspace, ABI consistency khi libsyscall thay đổi.
- *Pragmatist*: Separate workspace giải quyết target mismatch (kernel `aarch64-aegis.json` vs user `aarch64-user.json`). Effort delta = 5 dòng TOML.

**Hành động tiếp theo:**
- [ ] Tạo `user/Cargo.toml` workspace
- [ ] Tạo `user/aarch64-user.json` (di chuyển từ `user/hello/`)
- [ ] Tạo `user/libsyscall/` crate: `syscall_write`, `syscall_yield`, `syscall_exit`, + 2-3 wrappers cần thiết
- [ ] Refactor `user/hello/src/main.rs`: xóa 18 dòng syscall duplicates, dùng `use libsyscall::*`
- [ ] Tạo `user/sensor/` + `user/logger/` crates dùng libsyscall

---

### 3. Task 7 = IDLE thuần

**Quyết định:** Task 7 giữ `IDLE_TASK_ID = 7`, chạy `idle_entry()` (`wfi` loop). Không load ELF. ELF demo `user/hello` di chuyển sang task 2 (hoặc task slot khác trong 2–6).

**Lý do:**
- *Visionary*: Dual-role vi phạm separation of concerns. Idle task phải deterministic cho scheduler fallback.
- *Pragmatist*: `time_budget: 2` cho idle = workaround, không phải design. 7 tasks blocked + idle chạy user code = semantic sai.

**Hành động tiếp theo:**
- [ ] Giữ `IDLE_TASK_ID = 7` constant
- [ ] Giữ task 7 entry = `idle_entry` (không ghi đè bằng ELF)
- [ ] Load `user/hello` ELF vào task 2 slot
- [ ] Set task 7: `priority: 0, budget: 0` (infinite availability)

---

### 4. SYS_EXIT only, NO KILL

**Quyết định:** Implement `SYS_EXIT` (#13), reuse `fault_current_task` cleanup logic, thêm `TaskState::Exited` (không auto-restart). Extract `cleanup_task_resources()` helper. Không reserve bit/placeholder cho SYS_KILL.

**Lý do:**
- *Visionary*: Reserve bit = premature abstraction. KILL cần authority-based design mà Phase O chưa có context. Safety-critical tasks self-exit hoặc watchdog restart — cover 95% lifecycle.
- *Pragmatist*: KILL = security nightmare. `fault_current_task` đã cleanup IPC + Grant + IRQ + Priority. SYS_EXIT = ~20 dòng delta. Effort ~8h, không 14h.

**Hành động tiếp theo:**
- [ ] Thêm `TaskState::Exited` variant
- [ ] `CAP_EXIT = 1 << 18` trong `kernel/cap.rs`
- [ ] `cap_for_syscall(13) → CAP_EXIT` mapping
- [ ] `handle_svc()` thêm case 13 → `sys_exit()`
- [ ] Extract `cleanup_task_resources(task_id)` helper
- [ ] `sys_exit()`: cleanup + set Exited + schedule away
- [ ] Watchdog + epoch_reset skip `Exited` tasks
- [ ] `libsyscall` thêm `syscall_exit()` wrapper

---

### 5. Kani Proofs: 3 IPC P0 + elf_load_addr P1

**Quyết định:** 3 mandatory Kani proofs cho IPC (P0): SenderQueue overflow prevention, message integrity, cleanup completeness. 1 optional `elf_load_addr` proof (P1, only if >5h buffer remaining). Tổng proofs sau Phase O: 9 (P0) hoặc 10 (P0+P1).

**Lý do:**
- *Visionary*: IPC bugs = cascading cross-task failure, highest risk. elf_load_addr Kani > test nhưng test đã exhaustive → P1.
- *Pragmatist*: 3 proofs = đúng target, bounded state space (MAX_WAITERS=4). Deadlock-freedom = PhD-level, skip. elf_load_addr host test đã cover N=6.

**Hành động tiếp theo:**
- [ ] Extract pure functions: `push_pure()`, `pop_pure()`, `copy_message_pure()`, `cleanup_pure()`
- [ ] Kani harness #1: `verify_sender_queue_no_overflow` (`#[kani::unwind(5)]`)
- [ ] Kani harness #2: `verify_message_integrity`
- [ ] Kani harness #3: `verify_cleanup_completeness`
- [ ] (P1) Kani harness #4: `verify_elf_load_addr_no_overlap`

---

### 6. Build System: README + build-all.sh

**Quyết định:** README docs liệt kê build commands (primary reference) + `scripts/build-all.sh` (~10 dòng bash, convenience shortcut).

**Lý do:**
- *Visionary*: CI cần reproducible single command. Script = building block cho future.
- *Pragmatist*: 4 commands copy-paste-able. Script 30 phút, sẽ dùng hàng ngày.

**Hành động tiếp theo:**
- [ ] Cập nhật README: thêm build order (user crates → kernel)
- [ ] Tạo `scripts/build-all.sh` (~10 dòng)
- [ ] Verify: `scripts/build-all.sh` → `qemu-system-aarch64` → all checkpoints pass

---

## Lộ trình thực hiện

| Giai đoạn | Timeline | Hành động | Effort | Ưu tiên |
|-----------|----------|-----------|--------|---------|
| O1: Multi-ELF | Week 1-2 | Mở rộng .elf_load, load_elf_to_task(), 3 user crates, manual linker.ld + const_assert! | ~14h | P0 |
| O2: libsyscall | Week 2 | user/ workspace, libsyscall crate, refactor hello | ~6h | P0 |
| O3: SYS_EXIT | Week 2-3 | SYS_EXIT #13, TaskState::Exited, cleanup_task_resources() | ~8h | P0 |
| O4: Kani IPC | Week 3-4 | 3 Kani proofs (SenderQueue, message, cleanup) | ~10h | P0 |
| O5: Build docs | Week 1 | README + build-all.sh | ~1h | P0 |
| P1: elf_load_addr | Week 4 (conditional) | Kani proof if >5h buffer | ~3h | P1 |
| **Total** | | | **~39h P0, ~42h P0+P1** | |
| **Buffer** | | Unexpected issues | **~18–21h** | |

## Trade-offs đã chấp nhận

1. **Manual linker.ld thay vì auto-generation**: Chấp nhận 75 dòng duplicate (3 files × 25 dòng) để tránh build script complexity. `const_assert!` đảm bảo safety tĩnh. Scale limit: 5 binaries max.

2. **No SYS_KILL**: Chấp nhận thiếu kill mechanism để tránh security attack surface và premature design. Watchdog + fault recovery cover embedded use cases. Revisit khi có supervisor pattern.

3. **elf_load_addr proof = P1**: Chấp nhận host test coverage thay vì Kani symbolic verification. Bounded slots (N=5) → exhaustive enumeration test đủ.

4. **Separate user/ workspace thay vì merged**: Chấp nhận 2 build commands (user + kernel) thay vì 1, để tránh Cargo target mismatch. `build-all.sh` wraps thành 1 command.

5. **3 Kani IPC proofs thay vì deadlock-freedom**: Chấp nhận không chứng minh deadlock-freedom (PhD-level problem) để tập trung vào data integrity proofs thực tế.

---

## Appendix: Lịch sử thảo luận

| Round | GPT Review | Gemini Review | Synthesis | Đồng thuận |
|-------|-----------|---------------|-----------|------------|
| 1 | [review_gpt_round1](review_gpt_round1_2026-02-12.md) | [review_gemini_round1](review_gemini_round1_2026-02-12.md) | [synthesis_round1](synthesis_round1_2026-02-12.md) | 17% (1/6) |
| 2 | [review_gpt_round2](review_gpt_round2_2026-02-12.md) | [review_gemini_round2](review_gemini_round2_2026-02-12.md) | [synthesis_round2](synthesis_round2_2026-02-12.md) | 100% (6/6) |

---

*Đồng thuận đạt được sau 2 vòng thảo luận. Cả hai agent thay đổi stance trên nhiều điểm nhờ Orchestra's compromises: GPT bỏ build script (Q1), bỏ KILL planning (Q4), hạ elf_load_addr proof priority (Q5). Gemini chấp nhận user/ workspace (Q2), const_assert! (Q1), build-all.sh script (Q6).*
