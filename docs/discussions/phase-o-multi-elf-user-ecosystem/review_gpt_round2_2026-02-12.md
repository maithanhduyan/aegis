# 🔮 GPT-Visionary Review — Phase O Round 2 | 2026-02-12

## Phản hồi Compromise

### Q1: Manual + const_assert + migration path

**ACCEPT.**

Tôi thay đổi stance so với Round 1. Lý do:

1. **Gemini đúng về effort/reward ratio.** 3 files × 25 dòng = 75 dòng — build script cho 3 binaries là over-engineering. NUM_TASKS hard-coded = 8, max 5 ELF slots. Với con số này, manual quản lý được.

2. **`const_assert!` là key insight.** Compile-time kiểm tra binary size ≤ `ELF_SLOT_SIZE` giải quyết được lo ngại lớn nhất: **silent overlap khi binary vượt kích thước slot.** Zero runtime cost, phát hiện lỗi trước deploy:
   ```rust
   const_assert!(USER_ELF.len() <= ELF_SLOT_SIZE);
   ```

3. **Migration path phải DOCUMENTED.** Chấp nhận manual cho Phase O với điều kiện: thêm section trong README:
   > "Khi vượt 5 ELF binaries hoặc chuyển sang dynamic loading, migrate sang template-based generation."

**Final stance:** Manual linker.ld per-binary + `const_assert!` size check + documented migration trigger (>5 binaries).

---

### Q2: Separate user/ workspace

**ACCEPT.**

Orchestra's compromise giải quyết triệt để cả hai concerns:

1. **Target mismatch (Gemini's concern):** User crates dùng chung `aarch64-user.json`, kernel dùng `aarch64-aegis.json` — hoàn toàn tách biệt build pipeline.

2. **Workspace benefits (tôi yêu cầu):** `user/Cargo.toml` workspace chứa `libsyscall`, `hello`, `sensor`, `logger` → unified `user/Cargo.lock`, shared deps resolution, `cargo clippy --workspace` cho tất cả user crates.

3. **Scalability:** Khi thêm user crates mới, chỉ cần thêm member vào `user/Cargo.toml`.

Cấu trúc:
```
user/
├── Cargo.toml          # [workspace] members = ["libsyscall", "hello", "sensor", "logger"]
├── aarch64-user.json   # shared custom target
├── libsyscall/
├── hello/
├── sensor/
└── logger/
```

**Final stance:** Separate user/ workspace. Tốt hơn cả đề xuất ban đầu (merged workspace).

---

### Q4: Reuse fault_current_task, no KILL planning

**ACCEPT.**

Tôi bỏ stance Round 1 về "reserve bit for KILL" và "separate cleanup path". Lý do:

1. **`fault_current_task` đã làm đủ cleanup:** IPC + Grant + IRQ + Priority restore + Schedule away. SYS_EXIT chỉ cần thay `Faulted` → `Exited`, bỏ auto-restart. ~20 dòng code delta.

2. **Reserve bit = premature abstraction.** YAGNI — Gemini đúng. Design KILL bây giờ mà không biết use case = design sai.

3. **Risk nếu KHÔNG plan KILL: Thấp.** AegisOS target 8 tasks cố định. Safety-critical tasks = self-exit hoặc watchdog restart. KILL chỉ cần cho supervisor patterns (Phase P+).

**Lưu ý:** Extract cleanup logic thành helper:
```rust
fn cleanup_task_resources(task_id: usize) {
    ipc::cleanup_task(task_id);
    grant::cleanup_task(task_id);
    irq::irq_cleanup_task(task_id);
}
```
Không phải "separate cleanup path" — chỉ refactor tự nhiên.

**Final stance:** Reuse fault_current_task logic, thêm TaskState::Exited, NO KILL planning. ~8-10h.

---

### Q5: 3 IPC proofs P0 + elf_load_addr P1

**ACCEPT.**

Tôi hạ `elf_load_addr` Kani proof từ P0 → P1. Lý do:

1. **Host test `test_elf_load_addr_no_overlap` cover N=6 slots exhaustively** — enumeration test kiểm tra tất cả 5 slots + tất cả 10 cặp.

2. **3 IPC proofs quan trọng hơn:** SenderQueue overflow, message integrity, cleanup completeness — nơi bugs THẬT SỰ nguy hiểm.

3. **P1 trigger:** Còn >5h buffer sau O1-O4 + 3 IPC proofs → làm. Không → Phase P backlog.

**Final stance:** 3 IPC Kani proofs = P0. `elf_load_addr` proof = P1 (>5h buffer).

---

### Q6: README docs + minimal build-all.sh

**ACCEPT.**

1. **README docs = primary reference** (Gemini wins). Developer cần hiểu từng command.

2. **`scripts/build-all.sh` = convenience, not requirement.** ~10 dòng, <30 phút:
   ```bash
   #!/bin/bash
   set -euo pipefail
   cd user && cargo build --release --target aarch64-user.json -Z build-std=core
   cd .. && cargo build --release -Z build-std=core --target aarch64-aegis.json
   ```

**Final stance:** README build docs (primary) + `scripts/build-all.sh` (convenience).

---

## Tổng kết Round 2

| # | Điểm | Round 1 | Round 2 | Thay đổi? | Lý do |
|---|------|---------|---------|-----------|-------|
| Q1 | Linker management | C: Build script | **Manual + const_assert! + migration doc** | ✅ Đổi | 5 slots max, const_assert! đủ safety |
| Q2 | libsyscall workspace | B: Kernel workspace member | **Separate user/ workspace** | ✅ Đổi | Target mismatch là real |
| Q3 | Task 7 IDLE | B: Tách idle | **B: Tách idle** | — | Đồng thuận Round 1 |
| Q4 | SYS_EXIT scope | C: defer KILL | **SYS_EXIT only, NO KILL** | ✅ Đổi | YAGNI, fault logic đủ |
| Q5 | Kani proofs | 4 proofs | **3 IPC P0 + elf_load_addr P1** | ⚠️ Hạ priority | Host test đã exhaustive |
| Q6 | Build system | C: Script only | **README + build-all.sh** | ✅ Đổi | README primary, script convenience |

**Tỷ lệ đồng thuận dự kiến: 6/6 = 100%**

**Effort estimate sau compromise: ~45h / 60h ceiling** — 15h buffer.
