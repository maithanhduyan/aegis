# Kế hoạch Phase M — Safety Assurance Foundation

> **Trạng thái: ✅ DONE (2026-02-12)** — Xây dựng nền tảng đảm bảo an toàn (safety assurance) cho AegisOS: enhanced panic handler, code coverage measurement, structured logging, và unsafe audit với progressive encapsulation. Phase này không thêm syscall hay feature mới — tập trung 100% vào chứng minh kernel hiện tại hoạt động đúng và tạo infrastructure cho formal verification tương lai.

---

## Tại sao Phase M?

### Lỗ hổng hiện tại: "Kernel hoạt động, nhưng không có bằng chứng nào chứng minh nó đúng"

Sau 12 phases (A→L), AegisOS có một microkernel hoàn chỉnh: scheduler với priority + watchdog, IPC đồng bộ, capability access control, ELF loader, fault isolation, kiến trúc tách biệt arch/kernel/platform. Đó là phần **"build"** — xây dựng chức năng. Phase M mở ra phần **"assure"** — chứng minh chức năng đó đúng.

**Ví dụ thực tế**: Trong hệ thống tên lửa, controller nhận lệnh từ navigation qua IPC. Nếu IPC có bug deadlock ẩn mà 189 tests không cover — tên lửa mất kiểm soát. Trong thiết bị y tế, nếu capability check bỏ sót một edge case — phần mềm cho phép thao tác trái quyền. Trong xe tự lái, nếu scheduler có off-by-one trong budget accounting — task safety-critical bị starve.

189 host tests và 25 QEMU boot checkpoints là tài sản quý, nhưng **chưa đo coverage** (có thể chỉ 50-65%), **chưa có structured logging** để debug production, **panic handler chỉ in "PANIC" rồi loop** (mù khi failure), và **8 `static mut` globals không có documentation hay encapsulation** (Kani/Miri không thể reason).

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|--------|-----------|
| 1 | Panic handler chỉ in "PANIC" rồi halt — không có file:line, task ID, ESR/FAR | Debug trên QEMU mất 15-30 phút/lần tìm root cause; production failure = mù hoàn toàn |
| 2 | Code coverage = 0% measured — không biết tests cover gì | Có thể có critical paths trong cap.rs, elf.rs chưa bao giờ được test |
| 3 | Logging chỉ có `uart_print!` ad-hoc — không có level, tick, task ID | Không thể trace execution flow, khó tái hiện bug, không có audit trail |
| 4 | 8 `pub static mut` globals không có SAFETY comment hay encapsulation | Formal tools (Kani, Miri) không thể reason; DO-178C auditor sẽ flag; multi-core tương lai = data race |
| 5 | Không có `deny(unsafe_op_in_unsafe_fn)` — unsafe ops ẩn trong unsafe fn | Rust 2024 edition sẽ bắt buộc; hiện tại unsafe block boundaries không rõ ràng |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|--------|-------|---------------------|
| M0: Quick Lints | `deny(unsafe_op_in_unsafe_fn)` + clippy safety lints + `core::fmt` FP check | #5 |
| M3: Enhanced Panic Handler | In file:line, task ID, tick count, ESR/FAR khi panic | #1 |
| M4: Code Coverage | `cargo-llvm-cov` setup, đo baseline, viết targeted tests đạt ≥75% | #2 |
| M2-lite: Structured Logging | Macro `klog!` với compile-time level filtering, tick + task metadata | #3 |
| M1: Unsafe Audit + Progressive Encapsulation | SAFETY comments + `KernelCell<T>` wrapper cho 4 globals | #4 |

### Nguồn gốc quyết định

Kế hoạch này dựa trên **đồng thuận 100% (12/12 điểm)** từ thảo luận đa chiều giữa GPT-Visionary-Agent và Gemini-Pragmatist-Agent qua 2 vòng debate. Xem chi tiết tại `docs/discussions/phase-m-safety-assurance/final_consensus_2026-02-12.md`.

---

## Phân tích hiện trạng

### 8 `static mut` globals trong kernel/

| Biến | File | Loại | Refs trong `host_tests.rs` | Độ phức tạp |
|------|------|------|---------------------------|-------------|
| `TCBS` | `kernel/sched.rs` | `[Tcb; 3]` | ~40+ (read + write fields) | 🔴 Cao — struct array, interrupt context |
| `CURRENT` | `kernel/sched.rs` | `usize` | ~10+ (qua `read_current()` + direct) | 🟠 Trung bình — index scalar |
| `EPOCH_TICKS` | `kernel/sched.rs` | `u64` | ~2 | 🟢 Thấp — counter scalar |
| `ENDPOINTS` | `kernel/ipc.rs` | `[Endpoint; 4]` | ~20+ (read + write + queue) | 🔴 Cao — struct array, state machine |
| `TICK_COUNT` | `kernel/timer.rs` | `u64` | ~12 | 🟡 Trung bình — counter scalar |
| `TICK_INTERVAL` | `kernel/timer.rs` | `u64` | 0 (arch-only, `#[cfg]` gated) | 🟢 Rất thấp — private |
| `GRANTS` | `kernel/grant.rs` | `[Grant; 2]` | ~8 | 🟡 Trung bình |
| `IRQ_BINDINGS` | `kernel/irq.rs` | `[IrqBinding; 8]` | ~8 | 🟡 Trung bình |

### Crate-level attributes hiện tại (`lib.rs`)

```rust
#![no_std]
// Không có deny(unsafe_op_in_unsafe_fn)
// Không có clippy safety lints
```

### Panic handler hiện tại (`main.rs`)

```rust
#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}  // Không có file:line, task ID, tick count, ESR/FAR
}
```

### Logging hiện tại

Chỉ có `uart_print!` macro — output text thuần, không có level, không có metadata:
```rust
uart_print("[AegisOS] boot\n");
uart_print("[AegisOS] MMU enabled\n");
// Không biết tick nào, task nào đang chạy
```

### Test infrastructure hiện tại

- **189 host unit tests** trên x86_64 (`tests/host_tests.rs`)
- **25 QEMU boot checkpoints** (`tests/qemu_boot_test.sh`)
- `reset_test_state()` trực tiếp ghi vào tất cả 8 `static mut` globals
- Tests chạy `--test-threads=1` để tránh data race
- CI: GitHub Actions — 2 jobs (host-tests + qemu-boot) ✅ GREEN

### Capability bits (không thay đổi trong Phase M)

18/64 bits đã dùng (bits 0–17). Phase M không thêm syscall hay capability mới.

---

## Thiết kế Phase M

### M0 — Quick Lints (Preamble)

#### Khái niệm

Hình ảnh: Trước khi xây nhà, quét dọn nền đất. `M0` thêm automated lint rules để compiler tự phát hiện unsafe blocks thiếu documentation — output là danh sách cần audit cho M1.

#### Thay đổi cụ thể

1. **Thêm `#![deny(unsafe_op_in_unsafe_fn)]`** vào `src/lib.rs` — force mọi unsafe operation trong unsafe fn phải nằm trong explicit `unsafe {}` block. Rust 2024 edition sẽ bắt buộc điều này. Cần sửa compiler errors (thêm `unsafe {}` blocks nơi thiếu).

2. **Chạy `cargo clippy -- -W clippy::undocumented_unsafe_blocks`** — output danh sách mọi unsafe block thiếu `// SAFETY:` comment. Danh sách này = input cho M1 audit.

3. **Verify `core::fmt` không emit FP instructions** — disassemble kernel binary, grep cho `fadd`, `fmul`, `fcvt`, `fmov`. Nếu có → ảnh hưởng M2-lite design (phải dùng manual integer-to-string thay vì `write!` macro). Vì `CPACR_EL1.FPEN = 0`, bất kỳ FP instruction nào sẽ trap.

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/lib.rs` | Sửa | Thêm `#![deny(unsafe_op_in_unsafe_fn)]` |
| `src/kernel/*.rs` | Sửa | Thêm `unsafe {}` blocks trong unsafe fn nơi thiếu |
| `src/arch/aarch64/*.rs` | Sửa | Thêm `unsafe {}` blocks trong unsafe fn nơi thiếu |
| `src/main.rs` | Sửa | Sửa tương tự nếu có unsafe fn |

#### Checkpoint M0

```
[AegisOS] boot
```
> Kernel boot bình thường — M0 không thay đổi runtime behavior, chỉ thêm compile-time checks. Tất cả 189 host tests + 25 QEMU checkpoints phải pass.

---

### M3 — Enhanced Panic Handler

#### Khái niệm

Hình ảnh: Khi xe hỏng, đèn cảnh báo phải cho biết **gì** hỏng, **ở đâu**, **khi nào** — không chỉ nhấp nháy đỏ. M3 biến panic handler từ "PANIC rồi loop" thành diagnostic report đầy đủ.

Trong mọi hệ thống safety-critical, diagnostic information khi failure là yêu cầu bắt buộc. ISO 26262 Part 6 §9.4.3 yêu cầu "sufficient information for problem analysis".

#### Thiết kế panic handler mới

Panic handler sẽ in ra (sử dụng `uart_print` + `uart_print_hex`, không dùng `core::fmt` để tránh FP):

```
=== KERNEL PANIC ===
Tick: 0x000004A2
Task: 0x01
Message: <panic message nếu có>
Location: <file>:<line> nếu có
ESR_EL1: 0x00000000
FAR_EL1: 0x00000000
===================
```

Với:
- **Tick**: Giá trị `TICK_COUNT` hiện tại — cho biết panic xảy ra lúc nào
- **Task**: Giá trị `CURRENT` — cho biết task nào đang chạy
- **Message**: `PanicInfo::message()` nếu available (cần check FP constraint)
- **Location**: `PanicInfo::location()` → file:line
- **ESR_EL1**: Exception Syndrome Register — loại exception gây panic
- **FAR_EL1**: Fault Address Register — địa chỉ gây fault

**Đọc ESR/FAR**: Sử dụng inline asm `mrs x0, ESR_EL1` / `mrs x0, FAR_EL1` — chỉ trên AArch64, host stub trả 0.

**Lưu ý**: `PanicInfo::message()` trả `fmt::Arguments` — cần verify M0 check rằng formatting không emit FP. Nếu emit FP → chỉ in "[message unavailable]" và in location.

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/main.rs` | Sửa | Thay thế panic handler cũ bằng enhanced version |
| `src/uart.rs` | Có thể sửa | Thêm `uart_print_dec(val: u64)` nếu cần in line number dạng decimal |

#### Checkpoint M3

```
[AegisOS] enhanced panic handler ready
```
> UART output mới khi boot. Ngoài ra, có thể trigger test panic (gọi `panic!("test")` tạm thời) để verify output format. Sau verify, remove test panic.

---

### M4 — Code Coverage Baseline & Targeted Tests

#### Khái niệm

Hình ảnh: Bác sĩ khám sức khỏe tổng quát trước khi kê đơn. M4 **đo** coverage hiện tại (baseline), phân tích gap, rồi viết targeted tests để đạt ≥75% kernel/ modules.

DO-178C Level C yêu cầu Statement Coverage — mỗi câu lệnh thực thi ít nhất 1 lần. M4 thiết lập baseline và bắt đầu hành trình này.

#### Quy trình

1. **Setup `cargo-llvm-cov`** (~1-2h):
   - Cài đặt: `cargo install cargo-llvm-cov`
   - Chạy: `cargo llvm-cov --lib --test host_tests -- --test-threads=1`
   - Output: HTML report + lcov file

2. **Đo baseline** (~1h):
   - Ghi nhận coverage % cho mỗi module trong `kernel/`
   - So sánh với target → gap analysis

3. **Viết targeted tests** (~12-15h):
   - Ưu tiên modules theo criticality (cap → elf → ipc → sched → grant → irq)
   - Bao gồm exhaustive tests cho bounded inputs

#### Coverage targets

| Module | Target | Approach | Effort ước tính |
|--------|--------|----------|----------------|
| `kernel/cap.rs` | **95%** | Exhaustive: 18 bits × 13 syscalls = 234 cases | ~2h |
| `kernel/elf.rs` | **85%** | Fuzz-like: malformed headers, overflow offsets, segment overlap | ~3-4h |
| `kernel/ipc.rs` | **80%** | Property-based: send→recv correctness, double-recv rejection, queue full | ~4-5h |
| `kernel/sched.rs` | **75%** | Edge cases: watchdog expire, epoch reset, 3 tasks cùng priority | ~3-4h |
| `kernel/grant.rs` | **70%** | Revoke edge cases, double create, invalid peer | ~2h |
| `kernel/irq.rs` | **70%** | Unbind, double bind, route to faulted task | ~2h |
| `kernel/timer.rs` | **65%** | `tick_count()` logic (phần lớn arch-specific, ít portable) | ~1h |
| **Overall kernel/** | **≥75%** | Weighted average từ module targets | — |

**Điều kiện**: Nếu baseline < 40% → re-evaluate targets. Nếu baseline > 70% → push to 80%.

#### Exhaustive test cho `cap.rs` (ví dụ)

```
// Pseudo-code: 234 exhaustive cases
for syscall in 0..=12 {
    let required_bit = cap_for_syscall(syscall, endpoint);
    for task_caps in [0, required_bit, ALL_CAPS, !required_bit] {
        let result = has_capability(task_id, required_bit);
        assert!(result == (task_caps & required_bit != 0));
    }
}
```

#### Miri integration

Chạy `cargo +nightly miri test --lib --test host_tests -- --test-threads=1` để detect undefined behavior. Setup ~1h, zero ongoing cost. Output: report UB nếu có.

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `tests/host_tests.rs` | Sửa | Thêm ~30-40 targeted tests mới |
| `.github/workflows/ci.yml` | Sửa | Thêm coverage job (optional — có thể defer CI integration) |
| `docs/safety/coverage-baseline.md` | Tạo mới | Document baseline numbers + gap analysis |

#### Checkpoint M4

```
[AegisOS] boot
```
> M4 không thay đổi kernel code — chỉ thêm tests. Kernel boot bình thường. Checkpoint thực tế: coverage report HTML cho thấy ≥75% kernel/.

---

### M2-lite — Structured Kernel Logging

#### Khái niệm

Hình ảnh: Camera an ninh ghi lại **ai** làm **gì**, **lúc nào**. `klog!` macro thay `uart_print!` với metadata tự động: level, tick count, task ID (khi available).

DO-178C §6.4.3 yêu cầu "traceability of testing activities" — structured log là raw material cho traceability.

#### Thiết kế macro `klog!`

```rust
// Signature (pseudo-code):
klog!(Level, "message");
klog!(Level, "format with {}", value);  // CHỈ nếu core::fmt không emit FP

// Output format:
// [TICK:00001A2F] [T1] [INFO] message
// [TICK:00001A30] [T2] [WARN] something happened

// Levels:
// ERROR = 0 — luôn in
// WARN  = 1
// INFO  = 2
// DEBUG = 3 — chỉ in khi compile với feature flag
```

#### Compile-time level filtering

```rust
// Trong build — không dùng feature flag runtime:
const LOG_LEVEL: u8 = 2;  // INFO — compile-time constant

macro_rules! klog {
    ($level:expr, $($arg:tt)*) => {
        if ($level as u8) <= LOG_LEVEL {
            // In metadata + message
        }
    };
}
```

- **Không cần buffering** — output trực tiếp qua UART
- **Không auto-inject task ID** trong v1 — lý do: cần access `CURRENT` trong macro context, phức tạp. Thay vào đó, caller truyền task ID khi cần: `klog!(INFO, "[T{}] msg", task_id)`
- **Tick count**: Đọc `timer::tick_count()` (safe function, đã có)

#### Constraint: `core::fmt` và FP

Nếu M0 verify rằng `core::fmt` emit FP instructions → `klog!` phải dùng manual string output (tương tự `uart_print` + `uart_print_hex` hiện tại). Không dùng `write!` macro hay `format_args!`.

Nếu `core::fmt` KHÔNG emit FP → `klog!` có thể dùng `core::fmt::Write` trait cho formatting linh hoạt hơn.

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/kernel/log.rs` | Tạo mới | Module chứa `klog!` macro, `LogLevel` enum, output function |
| `src/kernel/mod.rs` | Sửa | Thêm `pub mod log;` |
| `src/lib.rs` | Sửa | Re-export `pub use kernel::log;` |
| `src/main.rs` | Sửa | Thay một số `uart_print!` checkpoint bằng `klog!` (optional, incremental) |

#### Checkpoint M2-lite

```
[AegisOS] klog ready
```
> Hoặc dùng chính `klog!` macro:
```
[TICK:00000000] [K] [INFO] klog ready
```

---

### M1 — Unsafe Audit & Progressive Encapsulation

#### Khái niệm

Hình ảnh: Kiểm kê kho hàng — ghi chép mọi thùng hàng (SAFETY comments), rồi dần dần đặt khóa riêng cho từng thùng (`KernelCell<T>`). Bắt đầu từ thùng nhỏ nhất, không phải thùng quan trọng nhất.

seL4, Tock OS, INTEGRITY RTOS — tất cả đều encapsulate kernel state. SAFETY comments là bước đầu (documentation debt), encapsulation là đích đến (technical debt). Cả hai cần thiết, theo thứ tự này.

#### Bước 0: SAFETY comments cho tất cả unsafe blocks (~3-4h)

Thêm `// SAFETY: <lý do>` ở **mọi** `unsafe {}` block trong `kernel/`. Sử dụng output từ `cargo clippy -- -W clippy::undocumented_unsafe_blocks` (M0) làm checklist.

Template SAFETY comment:
```rust
// SAFETY: Single-core execution (QEMU virt, Cortex-A53 uniprocessor config).
// Interrupts masked via DAIF during kernel execution.
// No concurrent access to this global from another core or preempted context.
unsafe { TICK_COUNT += 1 }
```

**Target**: 100% unsafe blocks có SAFETY comment.

#### Bước 1: Pilot encapsulate `EPOCH_TICKS` + `TICK_INTERVAL` (~2-3h)

Chọn hai biến đơn giản nhất (tổng 2 test references) để validate `KernelCell<T>` pattern:

```rust
// Pattern KernelCell<T> (pseudo-code):
pub struct KernelCell<T>(UnsafeCell<T>);

// SAFETY: KernelCell chỉ dùng trong single-core kernel context.
// Mọi access xảy ra khi interrupts disabled (DAIF mask).
unsafe impl<T> Sync for KernelCell<T> {}

impl<T> KernelCell<T> {
    pub const fn new(val: T) -> Self { Self(UnsafeCell::new(val)) }

    /// # Safety
    /// Caller phải đảm bảo single-core + no concurrent access.
    pub unsafe fn get(&self) -> &T { &*self.0.get() }

    /// # Safety
    /// Caller phải đảm bảo single-core + no concurrent access.
    pub unsafe fn get_mut(&self) -> &mut T { &mut *self.0.get() }
}

// Sử dụng:
static EPOCH_TICKS: KernelCell<u64> = KernelCell::new(0);
// Access: unsafe { *EPOCH_TICKS.get_mut() += 1 }
```

**Test helpers** (chỉ compile khi test):
```rust
#[cfg(test)]
pub fn test_set_epoch_ticks(v: u64) {
    unsafe { *EPOCH_TICKS.get_mut() = v; }
}
```

**Verify**: 189 host tests + 25 QEMU checkpoints phải pass.

#### Bước 2: Encapsulate `TICK_COUNT` + `CURRENT` (~5-7h)

Sau khi pilot thành công, mở rộng sang:
- `TICK_COUNT` (12 test references) — counter scalar, tương tự EPOCH_TICKS
- `CURRENT` (10+ test references) — index scalar, có helper `read_current()` trong tests

`read_current()` sẽ chuyển từ trực tiếp đọc `static mut CURRENT` sang gọi safe accessor (hoặc unsafe accessor với documented SAFETY reason).

**Verify**: 189 host tests + 25 QEMU checkpoints phải pass.

#### Bước 3: Defer sang Phase N

`TCBS`, `ENDPOINTS`, `GRANTS`, `IRQ_BINDINGS` (~60+ test references, struct arrays, interrupt context concerns) — defer sang Phase N khi:
- Pattern `KernelCell<T>` đã validated trên 4 scalar globals
- API design cho struct array access đã rõ ràng
- Test helpers pattern đã stable

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|------|----------|---------|
| `src/kernel/cell.rs` | Tạo mới | `KernelCell<T>` definition |
| `src/kernel/mod.rs` | Sửa | Thêm `pub mod cell;` |
| `src/lib.rs` | Sửa | Re-export `pub use kernel::cell;` |
| `src/kernel/sched.rs` | Sửa | Bước 0: SAFETY comments. Bước 1: wrap `EPOCH_TICKS`. Bước 2: wrap `CURRENT` |
| `src/kernel/timer.rs` | Sửa | Bước 0: SAFETY comments. Bước 1: wrap `TICK_INTERVAL`. Bước 2: wrap `TICK_COUNT` |
| `src/kernel/ipc.rs` | Sửa | Bước 0: SAFETY comments only (defer encapsulate) |
| `src/kernel/grant.rs` | Sửa | Bước 0: SAFETY comments only |
| `src/kernel/irq.rs` | Sửa | Bước 0: SAFETY comments only |
| `tests/host_tests.rs` | Sửa | Cập nhật access cho EPOCH_TICKS, TICK_INTERVAL, TICK_COUNT, CURRENT |

#### Checkpoint M1

```
[AegisOS] safety audit complete
```
> Hoặc qua klog (nếu M2-lite đã xong):
```
[TICK:00000000] [K] [INFO] safety audit complete — 4 globals encapsulated
```

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|-----------|-------|---------------|
| 1 | **No heap** — tất cả static | Bất biến AegisOS | `KernelCell<T>` dùng `UnsafeCell`, zero allocation |
| 2 | **No FP/SIMD** — CPACR_EL1.FPEN=0 | Constraint phần cứng | M0 verify `core::fmt` không emit FP; `klog!` dùng manual string nếu cần |
| 3 | **TrapFrame = 288 bytes** | ABI-locked | Phase M KHÔNG thay đổi TrapFrame |
| 4 | **Linker script ↔ MMU** | Đồng bộ bắt buộc | Phase M KHÔNG thêm section mới |
| 5 | **W^X** | No page vừa write vừa exec | Phase M KHÔNG thay đổi memory map |
| 6 | **Kernel EL1, Task EL0** | Isolation model | Phase M KHÔNG thay đổi privilege levels |
| 7 | **Syscall ABI** | x7=syscall#, x6=endpoint, x0-x3=payload | Phase M KHÔNG thêm syscall mới |
| 8 | **189 tests + 25 checkpoints = regression gate** | Safety net | Mỗi sub-phase PHẢI pass full suite trước khi tiến tiếp |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|--------|----------|-----------|------------|
| 1 | `core::fmt` emit FP instructions → `klog!` format bị trap | Trung bình | M2-lite phải redesign (manual string only) | M0 verify trước; fallback = `uart_print` + `uart_print_hex` |
| 2 | `deny(unsafe_op_in_unsafe_fn)` gây cascade compiler errors | Cao | ~2-3h sửa thay vì ~30min | Dự trù effort, sửa incremental per-module |
| 3 | `KernelCell<T>` encapsulation break host tests | Trung bình | 10-15 tests cần update access pattern | Pilot với EPOCH_TICKS (2 refs) trước; rollback nếu fail |
| 4 | Coverage baseline thấp hơn ước tính (< 40%) | Thấp | Target 75% cần nhiều tests hơn dự kiến | Re-evaluate targets; ưu tiên critical modules (cap, elf) |
| 5 | `cargo-llvm-cov` không tương thích Windows dev environment | Thấp | Phải chạy qua WSL hoặc CI only | Test trên local trước; fallback = CI-only coverage |
| 6 | Solo developer burnout từ 100% safety work | Trung bình | Abandon Phase M | Timebox 5 tuần max; Phase N bắt đầu bằng features |

---

## Test Plan

### Host unit tests mới (ước lượng: ~30-40 tests)

| # | Test case | Module | Mô tả |
|---|-----------|--------|--------|
| 1-10 | `test_cap_exhaustive_syscall_*` | `cap.rs` | Exhaustive 18 bits × 13 syscalls — tất cả combinations |
| 11-12 | `test_cap_no_caps_rejected` | `cap.rs` | Task với caps=0 bị reject mọi syscall |
| 13-14 | `test_cap_all_caps_accepted` | `cap.rs` | Task với ALL_CAPS pass mọi syscall |
| 15-18 | `test_elf_malformed_magic` | `elf.rs` | ELF header với magic sai → parse fail gracefully |
| 19-20 | `test_elf_overflow_offset` | `elf.rs` | Program header offset vượt buffer → no OOB |
| 21-22 | `test_elf_segment_overlap` | `elf.rs` | PT_LOAD segments overlap → handled |
| 23-25 | `test_ipc_send_recv_property` | `ipc.rs` | Send rồi recv phải nhận đúng message |
| 26-27 | `test_ipc_double_recv_rejection` | `ipc.rs` | Double recv trên cùng endpoint phải handle |
| 28-29 | `test_ipc_queue_full` | `ipc.rs` | Tất cả waiters đầy → sender blocked đúng |
| 30-32 | `test_sched_all_same_priority` | `sched.rs` | 3 tasks cùng priority → round-robin đúng |
| 33-34 | `test_sched_watchdog_expire` | `sched.rs` | Task không heartbeat → watchdog triggers |
| 35-36 | `test_grant_double_create` | `grant.rs` | Tạo grant trùng → rejected |
| 37-38 | `test_irq_double_bind` | `irq.rs` | Bind cùng INTID 2 lần → handled |
| 39-40 | `test_kernel_cell_basic` | `cell.rs` | `KernelCell<T>` new/get/get_mut behavior |

### QEMU boot checkpoints mới

| # | Checkpoint UART output | Sub-phase |
|---|----------------------|-----------|
| 26 | `[AegisOS] enhanced panic handler ready` | M3 |
| 27 | `[AegisOS] klog ready` | M2-lite |
| 28 | `[AegisOS] safety audit complete` | M1 |

---

## Thứ tự triển khai

| Bước | Sub-phase | Phụ thuộc | Effort | Kết quả thực tế |
|------|-----------|-----------|--------|---------------------|
| 1 | **M0**: Quick Lints | — | ✅ done | `deny(unsafe_op_in_unsafe_fn)` active. 54 clippy locations flagged. **0 FP instructions** in kernel binary. Commit `75a9593`. |
| 2 | **M3**: Enhanced Panic | M0 (FP check) | ✅ done | tick/task/location/ESR_EL1/FAR_EL1 + wfe halt. `uart_print_dec()` added. Commit `75a9593`. 26 QEMU checkpoints. |
| 3 | **M4**: Coverage Baseline | — | ✅ done | Baseline: overall 80.57% (ipc 43%, sched 79%, cap 88%, elf 96.5%, grant 98.9%, irq 100%, timer 100%). |
| 4 | **M4**: Targeted Tests | M4 baseline | ✅ done | +30 tests → **219 total**. Coverage: **96.65%** (cap 100%, ipc 100%, sched 99.45%, elf 96.5%, grant 98.9%, irq 100%, timer 100%). Commit `3358ff5`. |
| 5 | **M2-lite**: Structured Log | M0 (FP check) | ✅ done | `klog!` macro + `LogLevel` enum + `core::fmt::Write` (FP-safe). Format: `[TICK:XXXXXXXX] [TN] [LEVEL] msg`. Commit `ffde0d2`. 27 checkpoints. |
| 6 | **M1**: SAFETY Comments | M0 (clippy list) | ✅ done | ~92 `// SAFETY:` comments across 10 files. Commit `974af60`. |
| 7 | **M1**: Pilot KernelCell (EPOCH_TICKS + TICK_INTERVAL) | M1 comments | ✅ done | `KernelCell<T>` created in `src/kernel/cell.rs`. 2 globals wrapped. Commit `df9f9fa`. |
| 8 | **M1**: Encapsulate TICK_COUNT + CURRENT | M1 pilot | ✅ done | 15 + 22 test refs updated. 4/4 scalar globals encapsulated. Commit `02afae8`. 28 checkpoints. |
| | **Tổng** | | **✅ ALL DONE** | **219 host tests + 28 QEMU checkpoints. Pushed `origin/main`.** |

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|-------------|------------|-------------------|
| **DO-178C** | §5.5 | Traceability — bidirectional requirement↔code↔test (M6, defer Phase N) |
| **DO-178C** | §6.3.4 | Source code verifiable — unsafe audit + encapsulation (M1) |
| **DO-178C** | §6.4.1 | Statement Coverage (Level C minimum) — coverage measurement (M4) |
| **DO-178C** | §6.4.3 | Traceability of testing activities — structured logging (M2-lite) |
| **DO-333** | §6.1 | Formal Methods supplement — Kani proofs (defer Phase N) |
| **IEC 62304** | §5.5.3 | Software unit verification — coverage + exhaustive tests (M4) |
| **IEC 62304** | §7 | Software maintenance — no SOUP, all static (maintained by M1 audit) |
| **ISO 26262** | Part 6 §9.4.3 | Sufficient diagnostic info — enhanced panic handler (M3) |
| **ISO 26262** | Part 6 §7.4.12 | WCET analysis — defer Phase R+ (requires cycle counting) |
| **ISO 26262** | Part 11 | Multi-core (tương lai) — KernelCell<T> pattern chuẩn bị cho multi-core |

---

## Exit Criteria Phase M

Phase M **DONE** khi tất cả điều kiện sau đạt:

- [x] `#![deny(unsafe_op_in_unsafe_fn)]` active, compile thành công — **commit `75a9593`**
- [x] `core::fmt` FP check documented — **0 FP instructions** (`rust-objdump -d` grep fadd/fmul/fcvt/fmov = 0 matches)
- [x] Panic handler in file:line, task ID, tick count, ESR/FAR — **commit `75a9593`**
- [x] Coverage measured bằng `cargo-llvm-cov` — baseline 80.57%, sau targeted tests **96.65%**
- [x] Coverage ≥75% overall `kernel/` — **96.65%** (cap 100%, ipc 100%, sched 99.45%, elf 96.5%, grant 98.9%, irq 100%, timer 100%)
- [x] `klog!` macro hoạt động, compile-time level filtering — `src/kernel/log.rs`, `LOG_LEVEL=2` (INFO), **commit `ffde0d2`**
- [x] SAFETY comments trên 100% unsafe blocks — ~92 comments across 10 files, **commit `974af60`**
- [x] `EPOCH_TICKS` + `TICK_INTERVAL` + `TICK_COUNT` + `CURRENT` encapsulated trong `KernelCell<T>` — **commits `df9f9fa` + `02afae8`**
- [x] 219 host tests pass (+30 tests mới từ M4) — **219/219 ok**
- [x] 28 QEMU boot checkpoints pass (+3 checkpoints: enhanced panic, klog ready, safety audit) — **28/28 ok**
- [ ] Safety Readiness Checkpoint document created — defer sang Phase N prep
- [x] **Timebox**: Hoàn thành trong 1 ngày (2026-02-12). Tất cả exit criteria đạt trừ Safety Readiness doc.

---

## Bước tiếp theo đề xuất

1. [x] Review kế hoạch Phase M → phản hồi/chỉnh sửa
2. [x] Triển khai M0: Quick Lints — `deny(unsafe_op_in_unsafe_fn)` + 0 FP instructions
3. [x] Triển khai M3: Enhanced Panic Handler — tick/task/location/ESR/FAR
4. [x] Triển khai M4: Coverage 80.57% → **96.65%** (+30 targeted tests)
5. [x] Triển khai M2-lite: `klog!` macro với `core::fmt::Write`
6. [x] Triển khai M1: ~92 SAFETY comments + `KernelCell<T>` (4 globals)
7. [x] Chạy test suite đầy đủ — **219 host tests + 28 QEMU checkpoints pass**
8. [x] Push to `origin/main` — commits `75a9593`..`02afae8`
9. [ ] Viết blog Phase M giải thích (handoff → Aegis-StoryTeller)
10. [ ] Chuẩn bị Phase N roadmap: NUM_TASKS=8 + Kani pilot + TCBS/ENDPOINTS/GRANTS/IRQ_BINDINGS encapsulation
