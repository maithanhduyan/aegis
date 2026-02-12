# 🎼 Synthesis — Round 1 | 2026-02-12

## Chủ đề: Phase O — Multi-ELF & User Ecosystem

## 📊 Bảng đồng thuận

| # | Điểm thảo luận | GPT (Visionary) | Gemini (Pragmatist) | Đồng thuận? |
|---|----------------|-----------------|---------------------|-------------|
| 1 | Q1: Multi-ELF Architecture | C: Hybrid (fixed + build script auto-gen linker.ld) | A: Per-task fixed (manual linker.ld) | ❌ |
| 2 | Q2: libsyscall design | B: Workspace member | A: Standalone path dependency | ❌ |
| 3 | Q3: Task 7 IDLE separation | B: Tách idle thuần | B: Tách idle thuần | ✅ |
| 4 | Q4: SYS_EXIT scope | C: SYS_EXIT only, defer KILL design | A: SYS_EXIT only, NO KILL design | ⚠️ Gần |
| 5 | Q5: Kani IPC proofs | 4 proofs mới (3 IPC + elf_load_addr) = 10 total | 3 proofs đủ, schedule_idle trivial | ❌ |
| 6 | Q6: Build system | C: Script wrapper | A: Manual per-crate | ❌ |

## ✅ Các điểm đã đồng thuận (1/6)

### 1. **Q3: Tách Task 7 = IDLE thuần**
Cả hai đồng ý 100%: Task 7 giữ `IDLE_TASK_ID = 7`, chạy `idle_entry()` (`wfi` loop), không load ELF. ELF demo (`user/hello`) chuyển sang task slot 2–6. Effort ~2h, gần miễn phí vì O1 refactor đã phải di chuyển ELF loading.

**Lý do chung:**
- GPT: "Dual-role vi phạm separation of concerns, idle task phải deterministic cho scheduler fallback"
- Gemini: "`time_budget: 2` cho idle = workaround không phải design. Nếu tất cả 7 tasks blocked, scheduler cần idle đáng tin cậy"

**Quyết định:** Task 7 = idle thuần. ELF demo di chuyển sang task 2.

---

## ⚠️ Điểm gần đồng thuận (1/6)

### Q4: SYS_EXIT — Cả hai nói "chỉ SYS_EXIT, không SYS_KILL"

- **GPT nói (Option C):** "SYS_EXIT only now. **Defer KILL to Phase P.** Reserve bit position, consider authority-based KILL qua capabilities."
- **Gemini nói (Option A):** "Chỉ SYS_EXIT. Dứt khoát KHÔNG design cho KILL ngay. Đừng reserve bit, đừng placeholder. Khi nào cần thì thêm."
- **Khoảng cách:** Rất hẹp — GPT muốn "think about KILL path" (dù không implement), Gemini muốn "zero planning for KILL". Cả hai đồng ý: (a) SYS_EXIT #13 dùng `fault_current_task` logic, (b) thêm `TaskState::Exited`, (c) không auto-restart Exited.
- **Effort:** GPT nói ~14h, Gemini nói ~8h. Gap do GPT tính thêm "grant + IRQ cleanup paths riêng cho Exit vs Fault", Gemini nói reuse trực tiếp.

**Gợi ý Orchestra:** Đây gần như đồng thuận. Chỉ cần cả hai xác nhận: "Implement SYS_EXIT, reuse fault_current_task logic, KHÔNG reserve bit cho KILL, KHÔNG design cleanup riêng cho Exit". Nếu cả hai agree → ✅.

---

## ❌ Các điểm bất đồng (4/6)

### Bất đồng #1: Q1 — Build script auto-gen linker.ld hay manual copy?

- **GPT nói (Option C):** "Auto-generate linker.ld từ template. Single source of truth. DO-178C §5.5 traceability — N files riêng = risk mismatch. Template + `elf_load_addr` = 1 nguồn duy nhất. Khi scale 16–32 tasks, manual không khả thi. Thêm Kani proof cho `elf_load_addr()` invariant."
- **Gemini nói (Option A):** "3 files × 25 dòng = 75 dòng copy. Nhanh hơn viết build script. Build script chỉ đáng khi >6 binaries. Bạn có TỐI ĐA 5 ELF slots (tasks 2–6). YAGNI."
- **Khoảng cách:** Cả hai đồng ý fixed addresses (không PIC, không shared pool). Bất đồng NHỎ: quản lý linker.ld — script hay manual. GPT nhìn 20 năm (scale lên), Gemini nhìn hiện tại (3 binaries).
- **Gợi ý compromise:** **Manual cho Phase O (3 binaries), thêm `const_assert!` kiểm tra address overlap (GPT's idea), document migration path sang template khi >5 binaries.** Cả hai cùng agree fixed addresses = nền tảng không đổi.

### Bất đồng #2: Q2 — Workspace member hay standalone path dep?

- **GPT nói (Option B):** "Workspace member. `cargo build --workspace` catch ABI incompatibility. Unified Cargo.lock. Long-term sẽ có nhiều user crates, workspace scales tốt hơn."
- **Gemini nói (Option A):** "Target mismatch kernel (`aarch64-aegis.json`) vs user (`aarch64-user.json`). Cargo workspace muốn build ALL members cùng target. Standalone path dep = `libsyscall = { path = \"../libsyscall\" }` đơn giản hơn. Ship minimal: 5–6 wrappers, không phải 14."
- **Khoảng cách:** Bất đồng kỹ thuật cụ thể — Cargo workspace có thực sự support multiple custom targets không? GPT assume có thể exclude kernel từ user workspace. Gemini nói target conflict là real problem.
- **Gợi ý compromise:** **Tạo `user/` workspace riêng (user/Cargo.toml = workspace) chứa `libsyscall` + `hello` + `sensor` + `logger`, tách biệt khỏi kernel workspace.** Giữ lợi thế workspace (unified Cargo.lock cho user crates) mà tránh target mismatch với kernel. Gemini's concern giải quyết (user crates cùng target), GPT's concern cũng giải quyết (workspace cho user ecosystem).

### Bất đồng #3: Q5 — 3 hay 4+ Kani proofs?

- **GPT nói:** "4 proofs mới: 3 IPC (SenderQueue overflow, message integrity, cleanup completeness) + 1 `elf_load_addr` invariant. Tổng 10 proofs. Kani proof mạnh hơn test — 1 proof thay 3 test cases."
- **Gemini nói:** "3 proofs đủ. `elf_load_addr` proof = nice to have nhưng test đã cover. Deadlock-freedom = PhD-level, skip. `schedule_idle` update cho Exited = trivial, không cần sửa proof."
- **Khoảng cách:** Cả hai đồng ý 3 IPC proofs. Bất đồng: GPT muốn thêm `elf_load_addr` Kani proof, Gemini nghĩ test đủ. Effort delta: ~2–3h cho 1 proof thêm.
- **Gợi ý compromise:** **3 IPC proofs = P0 (bắt buộc). `elf_load_addr` proof = P1 (nếu còn thời gian trong 60h budget).** Gemini đúng rằng test đã cover, nhưng GPT đúng rằng Kani proof mạnh hơn. Đánh giá cuối sprint — nếu còn >5h buffer, thêm.

### Bất đồng #4: Q6 — Script wrapper hay manual?

- **GPT nói (Option C):** "Script wrapper `scripts/build-all.sh` ~10 dòng. CI cần reproducible single command. Future-proofing."
- **Gemini nói (Option A):** "4 commands copy-paste-able. Debug bằng mắt. Script chỉ đáng khi CI cần. Effort = 30 phút docs vs 2–4h script."
- **Khoảng cách:** Rất nhỏ. GPT muốn convenience script, Gemini muốn docs. Effort delta: ~1–2h.
- **Gợi ý compromise:** **Manual build + README docs (Gemini) + 1 minimal `scripts/build-all.sh` wrapper (GPT) = cả hai. Script = 10 dòng bash, effort <30 phút. Không phải "build system", chỉ là convenience. Document build order trong README là bắt buộc dù có script hay không.**

---

## 📈 Tỷ lệ đồng thuận: 1/6 = 17% (+ 1 gần đồng thuận = ~33% effective)

---

## 🎯 Hướng dẫn cho Round 2

### Câu hỏi cụ thể cho GPT-Visionary:

1. **Q1:** Gemini nói 3 files × 25 dòng = 75 dòng, nhanh hơn viết build script. Với NUM_TASKS max = 8 (5 ELF slots), bạn có chấp nhận **manual cho Phase O** + document migration path sang template cho future phases? Hay bạn khẳng định build script BẮT BUỘC ngay Phase O?

2. **Q2:** Gemini chỉ ra target mismatch giữa kernel workspace và user workspace. Bạn có đồng ý tạo **user/ workspace riêng** (user/Cargo.toml workspace chứa libsyscall + hello + sensor + logger) tách biệt khỏi kernel? Đây vẫn là workspace, nhưng scoped cho user crates only.

3. **Q4:** Bạn có thể bỏ "reserve bit for KILL" và "separate cleanup path for Exit"? Gemini đề xuất reuse `fault_current_task` logic trực tiếp — effort ~8h thay vì ~14h. Bạn thấy risk gì nếu KHÔNG plan cho KILL ngay?

4. **Q5:** `elf_load_addr` proof — nếu host tests `test_elf_load_addr_no_overlap` đã cover N=6 slots, Kani proof thêm value gì cụ thể? Bạn có chấp nhận đưa nó thành P1 (nice-to-have)?

5. **Q6:** Bạn có chấp nhận cả hai: README docs (manual) + minimal `scripts/build-all.sh` (~10 dòng, <30 phút)?

### Câu hỏi cụ thể cho Gemini-Pragmatist:

1. **Q1:** GPT đề xuất `const_assert!` kiểm tra binary size ≤ 16 KiB tại compile-time. Bạn có đồng ý thêm cái này vào manual approach? Nó không phải build script, chỉ là 1 dòng compile-time check.

2. **Q2:** Orchestra đề xuất **user/ workspace riêng**: `user/Cargo.toml` = workspace member `["libsyscall", "hello", "sensor", "logger"]`, tất cả dùng `aarch64-user.json`. Giải quyết target mismatch (tách khỏi kernel) mà vẫn có unified Cargo.lock cho user crates. Bạn có chấp nhận?

3. **Q4:** Bạn và GPT gần đồng thuận. Confirm: "SYS_EXIT reuse fault_current_task logic, TaskState::Exited, KHÔNG reserve bit/placeholder cho KILL" — đây là final stance?

4. **Q5:** GPT muốn `elf_load_addr` Kani proof. Nếu đặt nó là P1 (nice-to-have, chỉ làm nếu còn >5h buffer), bạn có okay?

5. **Q6:** GPT đề xuất thêm `scripts/build-all.sh` (10 dòng bash, <30 phút). Manual build vẫn là primary path, script chỉ convenience. Bạn có chấp nhận bổ sung?

### Đề xuất compromise cần cả hai phản hồi:
- **Q1 compromise:** Manual linker.ld cho Phase O + `const_assert!` size check + document migration path
- **Q2 compromise:** user/ workspace riêng (user/Cargo.toml)
- **Q4 compromise:** Reuse fault_current_task, no KILL planning, ~8–10h effort
- **Q5 compromise:** 3 IPC proofs P0 + elf_load_addr proof P1
- **Q6 compromise:** README docs + minimal build-all.sh script

### Data/evidence cần bổ sung:
- Q2: Ai có thể verify Cargo workspace target handling? (Thử `cargo build -p libsyscall --target aarch64-user.json` trong workspace có kernel member)
- Q5: Kani timeout estimate cho SenderQueue proof (MAX_WAITERS=4, unwind(5))
