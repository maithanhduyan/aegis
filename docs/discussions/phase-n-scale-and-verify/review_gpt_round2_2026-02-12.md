# 🔭 GPT-Visionary Review — Round 2 | Phase N: Scale & Verify

> **Agent:** GPT-Visionary-Agent
> **Ngày:** 2026-02-12
> **Round:** 2

---

## Bất đồng #1: Kani proofs — 5 vs 4

### ✅ ACCEPT — Giảm xuống 4 proofs

Evidence rõ ràng: `cap_check()` chỉ là `(caps & required) == required` — pure bitwise trên 2 `u64` params. Không có `task_id`, không indexing, không global state. Proof thứ 5 (`cap_check_no_oob`) sẽ chỉ verify rằng bitwise AND trên 2 số không panic — điều này Rust compiler đã đảm bảo tại compile time.

**4 proofs cuối cùng:**

| # | Proof | Module | Property |
|---|-------|--------|----------|
| 1 | `cap_for_syscall_no_panic` | `cap.rs` | No panic, return ⊆ `0x3FFFF` |
| 2 | `cap_for_syscall_completeness` | `cap.rs` | Mọi syscall 0..=12 có cap bit defined |
| 3 | `parse_elf64_no_panic` | `elf.rs` | No panic/OOB cho mọi input ≤ 128 bytes |
| 4 | `kernel_cell_roundtrip` | `cell.rs` | get/get_mut consistency |

**Lý do chấp nhận:** Không flip-flop — Round 1 tôi assume `cap_check` có thể chứa `task_id` indexing (dựa trên plan mô tả `has_capability(task, bit)`). Evidence cho thấy plan sai — hàm `has_capability` không tồn tại, `cap_check` không có task_id. Với data mới, stance mới.

---

## Bất đồng #2: Sequencing — Semi-parallel vs Sequential

### ✅ ACCEPT compromise — N1→N2→N3, N3a micro-parallel

Chấp nhận Gemini's core argument: single developer, sequential tối ưu. Ngoại lệ hợp lý: N3a (Kani install + CI yaml) chạy trong **dead time** khi QEMU tests đang chạy giữa các N1 sub-steps.

**Sequencing chính thức:**
```
N1a → N1b → N1c → N1d → N2.1 → N2.2 → N2.3 → N2.4 → N3a* → N3b → N3c → N3d
                                                          ↑
                                              *N3a có thể chạy trong QEMU wait time
                                               bất kỳ lúc nào từ N1 trở đi
                                               (infrastructure only, zero proof code)
```

**Lý do chấp nhận:** Context switching concern hợp lý. N3a thực tế chỉ là `cargo install` + copy yaml — không cần deep context.

---

## Bất đồng #3: Effort N1 = 16-18h

### ✅ ACCEPT 16-18h

Round 1 tôi không cho số cụ thể — sai sót. Gemini's MMU debug buffer (+4-6h) hợp lý. Option C (validate at NUM_TASKS=3) giảm ~2h debug, nhưng L1→L2→L3 page table chaining vẫn phức tạp.

**Breakdown 16-18h:**
- N1a (constants + linker): 2-3h
- N1b (MMU computed indexing): 6-7h (bao gồm debug buffer)
- N1c (TaskConfig table + loop init): 3-4h
- N1d (Host stubs + tests): 3-4h
- Buffer: 2h

**18h = hard ceiling.** Nếu vượt → scope issue, cần re-evaluate.

---

## Bất đồng #4: Effort TCBS = 10-12h + `kcell_index!()` macro

### 🔄 COUNTER-PROPOSE: 10-12h với macro built in Phase N

Đồng ý macro nên build **trong N2** (không hoãn Phase O). Pattern:

```rust
macro_rules! kcell_index {
    ($cell:expr, $idx:expr) => {
        &mut (*$cell.get_mut())[$idx]
    };
}
```

Macro xây ~1h, test ~0.5h. Amortize across 150+ TCBS refs → tiết kiệm ~1.5-2h sửa thủ công.

**12h hard ceiling cho TCBS.** Nếu macro đã build từ N2.1 (GRANTS) → khi đến N2.4 (TCBS) team đã thuần thục pattern.

**Timeline:**
- N2.1: Build macro + wrap GRANTS (3-4h tổng, bao gồm macro)
- N2.2: Wrap IRQ (2-3h)
- N2.3: Wrap ENDPOINTS (4-5h)
- N2.4: Wrap TCBS (10-12h, dùng macro)

---

## Bất đồng #5: ELF load + grants — Giữ nguyên Phase N

### ✅ ACCEPT — `.elf_load` và `NUM_GRANTS` không đổi

Evidence xác nhận: chỉ task 2 dùng ELF. Tasks 3-7 = kernel-internal idle (function pointers). Không có nhu cầu mở rộng ELF region hay grants trong Phase N.

**Yêu cầu:** Thêm TODO documentation trong plan Phase N:
```
NOTE: .elf_load (12 KiB) và NUM_GRANTS (2) giữ nguyên Phase N.
Phase O sẽ mở rộng khi thêm real ELF user tasks.
```

---

## Tổng kết vị trí Round 2

| # | Bất đồng | Quyết định | Lý do thay đổi (nếu có) |
|---|----------|-----------|------------------------|
| 1 | Kani 5→4 proofs | ✅ ACCEPT 4 | Evidence: `cap_check` = pure bitwise, proof vô nghĩa |
| 2 | Sequencing | ✅ ACCEPT sequential + N3a micro-parallel | Single developer argument hợp lý |
| 3 | Effort N1 | ✅ ACCEPT 16-18h | MMU debug buffer justified |
| 4 | Effort TCBS | 🔄 COUNTER: 10-12h + macro in N2 | Macro amortizes 150+ refs, 12h hard ceiling |
| 5 | ELF + grants | ✅ ACCEPT defer Phase O | Evidence: only task 2 ELF, tasks 3-7 idle |

**Dự kiến đồng thuận: 12-13/13** — chờ Gemini confirm TCBS 10-12h.
