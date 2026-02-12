# 🔧 Gemini-Pragmatist Review — Round 2 | Phase N: Scale & Verify

> **Agent:** Gemini-Pragmatist-Agent
> **Ngày:** 2026-02-12
> **Round:** 2

---

## Bất đồng #1: Kani proofs — Maintain 4 proofs

### MAINTAIN 4 proofs ✅

Evidence xác nhận chính xác analysis Round 1:
- `cap_check(caps, required) → (caps & required) == required` — bitwise AND trên 2 `u64`. Rust compiler đảm bảo không panic, không OOB. Proof sẽ chỉ re-state compiler guarantee.
- `cap_for_syscall()` — pure nested `match`, mọi branch trả constant. Đây MỚI là target Kani có giá trị: verify completeness + range.

**4 proofs cuối cùng:**
1. `cap_for_syscall_no_panic` — cap.rs — return ⊆ `0x3FFFF`
2. `cap_for_syscall_completeness` — cap.rs — mọi syscall 0..=12 mapped
3. `parse_elf64_no_panic` — elf.rs — no panic ≤ 128 bytes
4. `kernel_cell_roundtrip` — cell.rs — get/get_mut roundtrip

GPT đã accept ở Round 2 → **đồng thuận đạt**.

---

## Bất đồng #2: Sequencing — ACCEPT compromise

### ✅ ACCEPT: N1→N2→N3, N3a micro-parallel trong QEMU wait time

**Điều kiện chấp nhận:**
1. N3a **chỉ** infrastructure: `cargo install kani-verifier`, `cargo kani setup`, viết CI yaml
2. **Zero proof code** trong N3a — proof harnesses viết ở N3b-d
3. N3a tối đa 2-3h, xen kẽ vào QEMU wait time (mỗi lần boot test ~30s-1m dead time)

**Lý do chấp nhận:** GPT đúng rằng QEMU wait time là dead time thực tế. `cargo install` + viết yaml không cần context switch nặng — khác hẳn viết proof harnesses. Đây không phải full parallel, chỉ là **tận dụng idle time**.

**Sequencing chính thức:**
```
N1a → N1b → N1c → N1d → N2.1 → N2.2 → N2.3 → N2.4 → N3b → N3c → N3d
          ↑                                        ↑
     N3a có thể xen kẽ vào bất kỳ
     QEMU wait time nào (infrastructure only)
```

---

## Bất đồng #3: Effort N1 = ACCEPT 16-18h

### ✅ ACCEPT 16-18h

Option C (validate at NUM_TASKS=3 trước) giảm ~2h debug so với jump thẳng. Cắt upper bound Round 1 từ 20h → 18h.

**Breakdown chấp nhận:**
- N1a: 2-3h (constants + linker — nhanh nhất)
- N1b: 6-7h (MMU — **rủi ro cao nhất**, budget debug time)
- N1c: 3-4h (TaskConfig + loop)
- N1d: 3-4h (stubs + tests + Option C validation)
- Buffer: ~1-2h

**18h = hard ceiling.** Vượt 18h → stop, tạo checkpoint, review approach.

---

## Bất đồng #4: Effort TCBS — ACCEPT 10-12h + macro in N2

### ✅ ACCEPT 10-12h với `kcell_index!()` macro

GPT's counter-propose hợp lý:

1. **Macro definition ~1h** — simple pattern:
```rust
macro_rules! kcell_index {
    ($cell:expr, $idx:expr) => {
        &mut (*$cell.get_mut())[$idx]
    };
}
```

2. **Build macro tại N2.1** (GRANTS) — validate pattern sớm
3. **Dùng macro cho N2.2, N2.3, N2.4** — consistency + speed
4. **TCBS 10-12h** bao gồm:
   - Wrap static: 0.5h
   - Sửa kernel code (~40 refs): 3-4h
   - Sửa host_tests.rs (~100+ refs): 4-5h
   - `reset_test_state()` migration: 1h
   - Test + debug: 2-3h

**12h hard ceiling cho TCBS, 50h hard ceiling tổng Phase N.**

**Lý do chấp nhận:** Macro saves ~1.5-2h thủ công trên 150+ refs. Investment 1h → return 1.5-2h = positive ROI. Và quan trọng hơn: **consistency** — mọi access qua cùng pattern, giảm risk sai.

---

## Bất đồng #5: ELF + grants — ACCEPT defer, withdraw red flag

### ✅ ACCEPT: `.elf_load` và `NUM_GRANTS` giữ nguyên Phase N

Evidence rõ ràng:
- Chỉ task 2 dùng ELF loading
- Tasks 3-7 = kernel-internal idle functions (giống task 0, task 1 — function pointers)
- Không có nhu cầu mở rộng ELF region hay grant pages

**Withdraw red flag.** Round 1 tôi flag vì chưa có evidence — bây giờ data xác nhận scope đủ.

**Yêu cầu documentation:** Thêm vào plan Phase N:
```markdown
> **Scope note:** `.elf_load` (12 KiB, 3 pages) và `NUM_GRANTS` (2) giữ nguyên.
> Tasks 3-7 là kernel-internal idle, không dùng ELF loading.
> Mở rộng các resource này sang Phase O khi thêm real ELF user tasks.
```

---

## Tổng kết vị trí Round 2

| # | Bất đồng | Quyết định | Convergence |
|---|----------|-----------|-------------|
| 1 | Kani proofs | MAINTAIN 4 ✅ | GPT accepted → **đồng thuận** |
| 2 | Sequencing | ACCEPT sequential + N3a micro-parallel ✅ | Cả hai đồng ý → **đồng thuận** |
| 3 | Effort N1 | ACCEPT 16-18h ✅ | Cả hai đồng ý → **đồng thuận** |
| 4 | Effort TCBS | ACCEPT 10-12h + macro ✅ | Cả hai đồng ý → **đồng thuận** |
| 5 | ELF + grants | ACCEPT defer Phase O ✅ | Cả hai đồng ý → **đồng thuận** |

**Tỷ lệ đồng thuận: 13/13 = 100%** 🎉

### Effort consensus tổng hợp

| Sub-phase | Effort | Notes |
|-----------|--------|-------|
| N1 | 16-18h | 18h hard ceiling |
| N2 | 18-24h | Macro built tại N2.1; TCBS 10-12h, 12h ceiling |
| N3 | 8-10h | 4 proofs; N3a micro-parallel |
| **Tổng** | **43-50h** | **50h hard ceiling** |
