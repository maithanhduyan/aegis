# 🎼 Synthesis — Round 2 | 2026-02-12

## Chủ đề: Phase O — Multi-ELF & User Ecosystem

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|----------------|-----------------|---------------------|-------------|
| 1 | Q1: Linker management | Manual + const_assert! + migration doc | Manual + const_assert! + migration doc | ✅ |
| 2 | Q2: libsyscall workspace | Separate user/ workspace | Separate user/ workspace | ✅ |
| 3 | Q3: Task 7 IDLE | Tách idle thuần | Tách idle thuần | ✅ |
| 4 | Q4: SYS_EXIT scope | SYS_EXIT only, reuse fault logic, NO KILL, extract helper | SYS_EXIT only, reuse fault logic, NO KILL, extract helper | ✅ |
| 5 | Q5: Kani proofs | 3 IPC P0 + elf_load_addr P1 | 3 IPC P0 + elf_load_addr P1 | ✅ |
| 6 | Q6: Build system | README docs + build-all.sh | README docs + build-all.sh | ✅ |

## ✅ Các điểm đã đồng thuận (6/6)

### 1. **Q1: Manual linker.ld + `const_assert!` + migration doc**
- Manual linker.ld per-binary cho Phase O (3 binaries, 5 ELF slots max)
- `const_assert!` kiểm tra binary size ≤ `ELF_SLOT_SIZE` tại compile-time
- Document migration trigger trong README: "khi >5 binaries → chuyển sang template-based generation"

### 2. **Q2: Separate user/ workspace**
- `user/Cargo.toml` = workspace chứa `["libsyscall", "hello", "sensor", "logger"]`
- `user/aarch64-user.json` = shared custom target cho tất cả user crates
- Tách biệt hoàn toàn khỏi kernel workspace → no target mismatch
- Unified `user/Cargo.lock` → ABI consistency

### 3. **Q3: Tách Task 7 = IDLE thuần**
- Task 7 giữ `IDLE_TASK_ID = 7`, chạy `idle_entry()` (`wfi` loop)
- ELF demo `user/hello` di chuyển sang task slot 2
- Effort ~2h, gần miễn phí (trong O1 refactor)

### 4. **Q4: SYS_EXIT only, NO KILL planning**
- SYS_EXIT (#13) reuse `fault_current_task` cleanup logic
- Thêm `TaskState::Exited` — không auto-restart
- Extract `cleanup_task_resources()` helper (IPC + Grant + IRQ cleanup)
- `CAP_EXIT = 1 << 18`
- KHÔNG reserve bit/placeholder cho SYS_KILL
- Effort ~8–10h

### 5. **Q5: 3 IPC Kani proofs P0 + elf_load_addr P1**
- P0 (mandatory): SenderQueue overflow, message integrity, cleanup completeness (~10h)
- P1 (nice-to-have): `elf_load_addr` invariant proof, chỉ khi >5h buffer remaining (~3h)
- Host tests đã cover exhaustively cho P1

### 6. **Q6: README docs (primary) + scripts/build-all.sh (convenience)**
- README liệt kê từng build command rõ ràng
- `scripts/build-all.sh` ~10 dòng bash, <30 phút effort
- Script = convenience, KHÔNG phải "build system"

## 📈 Tỷ lệ đồng thuận: 6/6 = 100% ✅

## Effort tổng hợp (consensus)

| Sub-phase | GPT estimate | Gemini estimate | Consensus |
|-----------|-------------|-----------------|-----------|
| O1: Multi-ELF + linker | ~14h | ~14h | **~14h** |
| O2: libsyscall + user workspace | ~8h | ~5h | **~6h** |
| O3: SYS_EXIT + Exited state | ~8h | ~8h | **~8h** |
| O4: 3 IPC Kani proofs | ~10h | ~10h | **~10h** |
| O5: build docs + script | ~2h | ~0.5h | **~1h** |
| P1: elf_load_addr proof | ~3h | ~2h | **~3h** |
| **Total P0** | **~42h** | **~37.5h** | **~39h** |
| **Total P0+P1** | **~45h** | **~39.5h** | **~42h** |
| **Buffer (60h ceiling)** | **15h** | **20h** | **~18h** |
