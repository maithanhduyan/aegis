# 🔧 Gemini-Pragmatist Review — Phase O Round 2 | 2026-02-12

## Phản hồi Compromise

### Q1: `const_assert!` size check

**ACCEPT.**

`const_assert!` kiểm tra binary size ≤ 16 KiB tại compile-time = **1 dòng code, zero runtime cost, zero build complexity.** Đây không phải build script — nó chỉ là static assertion trong Rust, giống `static_assert` trong C. Nếu ai đó vô tình viết binary quá lớn, compiler sẽ **fail ngay** thay vì runtime load corruption. Đây là loại safety net mà tôi hoàn toàn ủng hộ: effort gần zero, value rõ ràng.

Cụ thể, implementation sẽ kiểu:
```rust
const _: () = assert!(USER_HELLO_ELF.len() <= ELF_LOAD_SIZE_PER_TASK);
```
Đây là Rust `const` evaluation — không cần crate ngoài, không cần macro phức tạp.

**Stance cuối cùng Q1:** Manual linker.ld cho Phase O + `const_assert!` size check + document migration path. Đồng ý 100% với compromise của Orchestra.

---

### Q2: Separate user/ workspace

**ACCEPT.**

Tôi thay đổi từ "standalone path dep" (Round 1 Option A) sang "user/ workspace riêng" vì Orchestra's compromise **giải quyết đúng concern của cả hai phía:**

1. **Concern của tôi (Round 1):** Target mismatch — kernel dùng `aarch64-aegis.json`, user dùng `aarch64-user.json`. Nếu chung 1 workspace, `cargo build --workspace` sẽ cố build tất cả cùng target → fail. **Giải quyết:** `user/Cargo.toml` là workspace riêng, hoàn toàn tách biệt khỏi root `Cargo.toml` (kernel). Không bao giờ target conflict.

2. **Concern của GPT (Round 1):** Unified Cargo.lock cho user crates, `cargo build --workspace` catch ABI incompatibility. **Giải quyết:** Trong user/ workspace, tất cả members (libsyscall, hello, sensor, logger) share 1 `user/Cargo.lock`. `cargo build --workspace` trong `user/` build tất cả user crates cùng `aarch64-user.json` target. Single lockfile = reproducible.

**Effort thêm so với Round 1:** Gần zero. Thay vì mỗi crate có Cargo.toml riêng rẽ, thêm 1 file `user/Cargo.toml` workspace root (~5 dòng). Mỗi member vẫn có Cargo.toml riêng nhưng giờ thuộc workspace. Path dependency tự nhiên: `libsyscall = { path = "../libsyscall" }` — y hệt approach cũ, chỉ thêm workspace wrapper.

**Lý do thay đổi stance:** Compromise này cho tôi mọi thứ tôi muốn (tách target, đơn giản) + thêm lợi ích GPT muốn (unified lock, workspace build) với effort gần như bằng nhau. Không có lý do gì để từ chối.

Layout cụ thể:
```
user/
├── Cargo.toml          # [workspace] members = ["libsyscall", "hello", "sensor", "logger"]
├── aarch64-user.json   # Shared target spec
├── libsyscall/
│   ├── Cargo.toml
│   └── src/lib.rs
├── hello/
│   ├── Cargo.toml      # depends on libsyscall = { path = "../libsyscall" }
│   ├── linker.ld
│   └── src/main.rs
├── sensor/
│   └── ...
└── logger/
    └── ...
```

---

### Q4: Confirm SYS_EXIT stance

**ACCEPT — xác nhận final stance.**

> SYS_EXIT (#13) reuse `fault_current_task` logic, thêm `TaskState::Exited`, KHÔNG reserve bit/placeholder cho KILL, KHÔNG design cleanup riêng cho Exit vs Fault.

Đây là stance tôi đã giữ từ Round 1 và không có gì thay đổi. Cụ thể:

- `sys_exit()` ≈ `fault_current_task()` nhưng set `TaskState::Exited` thay vì `Faulted`
- `tick_handler()` skip `Exited` tasks (không auto-restart — khác Faulted)
- Watchdog skip `Exited`
- IPC cleanup: gọi `cleanup_task()` giống hệt fault path
- Grant/IRQ cleanup: reuse existing cleanup, KHÔNG viết path riêng
- Exit code `x0` → log ra UART. **Không store vào TCB** — khi nào cần `SYS_WAIT_EXIT` (Phase P+) thì thêm field. YAGNI.
- **KHÔNG reserve syscall number cho KILL**, KHÔNG thêm `CAP_KILL`, KHÔNG document "future KILL design" trong code. Khi nào cần, thêm.

**Effort: ~8h** — đúng estimate Round 1. Không phải 14h vì reuse trực tiếp, không design cleanup paths mới.

Lưu ý nhỏ: tôi ghi nhận GPT's point về "document Exited slots NOT reusable". Đồng ý thêm 1 comment trong code: `// Exited slots are not reused in Phase O. SYS_SPAWN deferred.` — 1 dòng, zero effort. Nhưng đây là comment, không phải code/design.

---

### Q5: `elf_load_addr` proof as P1

**ACCEPT.**

Tôi thay đổi từ "chỉ 3 proofs, không thêm" (Round 1) sang "3 IPC proofs P0 + `elf_load_addr` proof P1 nếu còn >5h buffer" vì:

1. **Risk = zero.** P1 nghĩa là: nếu hết thời gian, bỏ qua. Tôi không mất gì khi đồng ý. Nếu sprint đúng tiến độ và còn buffer, thêm 1 proof mạnh hơn 1 test — tôi không phản đối.

2. **Effort bounded.** `elf_load_addr()` là pure function nhận `slot: usize` → trả `u64`. Kani proof = verify ∀ slot ∈ [0,5]: no overlap ∧ within bounds. State space cực nhỏ — Kani xong trong giây. Estimate ~2h max (viết harness + test).

3. **Đã có host test `test_elf_load_addr_no_overlap`.** Proof thêm coverage cho ALL inputs thay vì chỉ tested values. Giá trị thêm là có, nhưng nhỏ — nên đúng mức P1.

**Điều kiện rõ ràng:** P1 = nice-to-have. Nếu sprint chạm 55h/60h mà chưa xong 3 IPC proofs → **bỏ `elf_load_addr` proof, không thương lượng.** 3 IPC proofs quan trọng hơn vì IPC là attack surface lớn nhất.

---

### Q6: README + `build-all.sh`

**ACCEPT.**

Tôi thay đổi từ "chỉ README manual" (Round 1) sang "README + `scripts/build-all.sh`" vì:

1. **Effort thực tế:** Script 10 dòng bash, <30 phút. Tôi đã nói Round 1 rằng "script chỉ đáng khi CI cần" — nhưng thực ra, **chính mình cũng sẽ dùng script** khi dev. Gõ 4 lệnh `cd` mỗi lần rebuild = annoying sau lần thứ 5. Thành thật mà nói, tôi sẽ tự viết script này cho bản thân anyway — vậy thì commit luôn.

2. **README vẫn là primary documentation.** Script không thay thế docs. README document build order + từng command + giải thích. Script = automation. Cả hai cùng tồn tại, không conflict.

3. **Không phải "build system".** 10 dòng bash với `set -e` + 4 `cargo build` commands = convenience wrapper. Không có logic phức tạp, không dependency management, không auto-detection. Ai đọc cũng hiểu trong 30 giây.

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../user"
cargo build --release --workspace --target aarch64-user.json -Zbuild-std=core
cd ..
cargo build --release --target aarch64-aegis.json -Zbuild-std=core
echo "Build complete."
```

Thực tế còn ngắn hơn 10 dòng. Xong.

---

## Tổng kết Round 2

| # | Câu hỏi | Round 1 stance | Round 2 stance | Thay đổi? | Lý do |
|---|---------|---------------|---------------|-----------|-------|
| Q1 | Multi-ELF Architecture | A: Manual linker.ld | A + `const_assert!` + document migration | ✅ Mở rộng nhẹ | `const_assert!` = 1 dòng, zero cost, safety net hợp lý |
| Q2 | libsyscall design | A: Standalone path dep | **User/ workspace riêng** | ✅ Đổi | Compromise giải quyết cả 2 concerns, effort tương đương |
| Q3 | Task 7 IDLE | B: Tách idle thuần | B: Tách idle thuần | ❌ Giữ nguyên | Đã đồng thuận Round 1 |
| Q4 | SYS_EXIT scope | A: Chỉ SYS_EXIT, no KILL | A: Chỉ SYS_EXIT, no KILL | ❌ Giữ nguyên | Xác nhận final stance |
| Q5 | Kani proofs | 3 proofs only | 3 IPC (P0) + elf_load_addr (P1) | ✅ Mở rộng nhẹ | P1 = zero risk, có thể bỏ nếu hết thời gian |
| Q6 | Build system | A: Manual only | README + `build-all.sh` | ✅ Đổi | 30 phút effort, bản thân cũng sẽ dùng |

### Effort tổng hợp sau Round 2

| Component | Effort (h) |
|-----------|-----------|
| O1: Multi-ELF + linker.ld manual + `const_assert!` | ~14h |
| O2: libsyscall (user/ workspace) | ~4.5h |
| O3: SYS_EXIT (reuse fault logic) | ~8h |
| O4: Kani 3 IPC proofs (P0) | ~10h |
| O4b: Kani `elf_load_addr` proof (P1) | ~2h (conditional) |
| Q3: Tách idle | ~2h (trong O1) |
| Q6: README + build-all.sh | ~1h |
| **Tổng P0** | **~39.5h** |
| **Tổng P0 + P1** | **~41.5h** |

**Buffer: 18–20h** trong 60h ceiling. Đủ rộng cho unexpected issues (Kani timeout, build coordination bugs, QEMU debugging). Thoải mái.

### Điều tôi sẽ CẮT nếu hết thời gian (priority order)

1. ~~`elf_load_addr` Kani proof~~ (P1, -2h)
2. ~~`user/logger` binary~~ — 2 binaries (hello + sensor) đủ prove multi-ELF (-3h)
3. ~~Kani proof #3 cleanup completeness~~ — 2 IPC proofs đã cover core (-3h)

**Tín hiệu đồng thuận:** 5/5 compromises ACCEPTED. Tôi sẵn sàng để Orchestra finalize plan.
