# Kế hoạch Phase F — Testing Infrastructure & CI

> **Trạng thái: ✅ HOÀN THÀNH** Xây dựng hạ tầng kiểm thử tự động cho AegisOS: unit test chạy trên host cho logic thuần, integration test chạy kernel thật trên QEMU + kiểm tra UART output, và CI pipeline trên GitHub Actions. Đây là nền tảng cho mọi verification trong tương lai (DO-178C §6.4, IEC 62304 §5.7).

---

## Tại sao Phase F?

- **Hiện tại:** Kiểm thử hoàn toàn thủ công — build, chạy QEMU, nhìn UART output bằng mắt. Không có bất kỳ `#[test]` nào. Không có CI. Một thay đổi nhỏ có thể break kernel mà không ai biết cho đến khi chạy QEMU thủ công.
- **Yêu cầu safety:** DO-178C §6.4 (Verification Process) yêu cầu **structural coverage** và **repeatable test evidence**. IEC 62304 §5.7 yêu cầu **software verification** với test plans có thể tái tạo. ISO 26262 Part 6 §9 yêu cầu **unit testing** và **integration testing**. Tất cả đều bắt buộc **automated, reproducible testing**.
- **Tác động lớn, scope nhỏ:** Không sửa kernel logic. Chỉ thêm test harness + CI config. Mọi phase sau (Capability, Memory Isolation, FV) đều hưởng lợi từ test infrastructure có sẵn.
- **Phát hiện regression:** Sau 5 phase, codebase đã ~1.930 dòng. Mỗi phase sau sẽ thay đổi nhiều module — cần safety net để đảm bảo không break những gì đã hoạt động.

---

## Phân tích hiện trạng

### Khả năng kiểm thử từng module

| Module | Hàm testable trên host | Hàm cần QEMU/HW | Ghi chú |
|---|---|---|---|
| `sched.rs` | `schedule()` logic, `current()`/`set_current()`, `get_task()`/`get_task_mut()`, `save_context()`/`restore_context()`, state transitions | `init()` (linker symbols), `bootstrap()` (asm) | Round-robin + restart delay = logic thuần, testable nếu inject TCB state |
| `ipc.rs` | `sys_send()`, `sys_recv()`, `sys_call()`, `transfer_message()`, `cleanup_task()` | Không (chỉ UART print trên error path) | **Module testable nhất** — toàn bộ là state machine thuần trên `ENDPOINTS[]` + `TCBS[]` |
| `exception.rs` | `TrapFrame` layout/size, `handle_svc()` dispatch logic, `handle_write()` validation (range check), `describe_dfsc()` decode | `read_esr()` (mrs), handler entry (asm), `install_vector_table()` (msr) | Pointer validation logic (`0x4000_0000–0x4800_0000`, max 256 bytes) = pure |
| `mmu.rs` | Descriptor constants (bit composition), page classification logic | `init()`, `set_ttbr0()`, tất cả linker symbol refs | Bit-field constants như `VALID`, `TABLE`, `AF`, `AP_RW_EL1` có thể verify |
| `timer.rs` | `tick_count()`, `print_dec()` logic | `init()` (msr), `handle_tick()` (msr + schedule) | Counter read là pure |
| `gic.rs` | Không | Tất cả (MMIO) | Hoàn toàn hardware-dependent |
| `main.rs` | Không | Tất cả (UART, syscall wrappers, task entries) | Entry points + init sequence |

### Các ràng buộc kỹ thuật

| Ràng buộc | Ảnh hưởng |
|---|---|
| `#![no_std]` | Không có standard test harness. Cần `custom_test_frameworks` hoặc host-side test |
| No heap | Tất cả state là `static mut`. Tests phải reset state giữa các lần chạy |
| Custom target `aarch64-aegis.json` | `cargo test` mặc định sẽ build cho AArch64. Host tests cần override target |
| `static mut` tràn lan | `TCBS`, `ENDPOINTS`, `CURRENT`, `TICK_COUNT` — tests không thể chạy song song |
| Inline assembly | Nhiều hàm chứa `asm!()` chỉ compile cho AArch64. Host tests phải `#[cfg]` gate |
| Linker symbols | `__text_start`, `__stack_end`, v.v. không tồn tại khi compile cho host |
| TrapFrame ABI lock | 288 bytes, chia sẻ với assembly. Test verify size/layout rất có giá trị |
| QEMU blocking | `-nographic` chạy vĩnh viễn. Tests cần `timeout` hoặc PSCI shutdown |
| LTO + `opt-level = "z"` | Build release chậm. CI nên cache toolchain |

### Chiến lược testing phù hợp

Sau khi phân tích, chiến lược **hybrid** là tốt nhất cho AegisOS:

1. **Host-side unit tests** — Cho logic thuần (IPC state machine, scheduler round-robin, descriptor bits, TrapFrame layout). Compile cho host target (`x86_64`), dùng standard `#[test]`.
2. **QEMU integration tests** — Cho full boot validation. Build kernel, chạy trên QEMU, capture UART output, assert expected strings.
3. **GitHub Actions CI** — Chạy cả hai loại test trên mỗi push/PR.

---

## Các bước thực hiện

### F1 — Host-side Unit Tests: Tách logic thuần, test trên host

**Mục tiêu:** Viết unit tests cho logic không phụ thuộc hardware, chạy bằng `cargo test` trên máy host (x86_64).

**Thay đổi:**

1. **Tạo thư mục `tests/` và file `tests/host_tests.rs`:**
   - File test chính chạy trên host target.
   - Dùng standard `#[test]` attribute (không cần `custom_test_frameworks`).

2. **Tách logic thuần từ `ipc.rs` vào testable functions:**
   - Thêm `#[cfg(test)]` module trong `ipc.rs` hoặc tạo test riêng.
   - Tuy nhiên vì `static mut` globals, cách tiếp cận tốt hơn là: tạo các hàm logic thuần nhận tham chiếu mutable thay vì truy cập global trực tiếp.
   - **Hoặc đơn giản hơn:** Viết tests trong `tests/` folder, import symbols cần thiết, setup/teardown globals trước mỗi test.
   - **Quyết định: Dùng cách đơn giản** — giữ code kernel không đổi, viết integration-style unit tests set up `static mut` rồi gọi hàm.

3. **Giải quyết vấn đề compilation cho host:**
   - Các file kernel dùng `asm!()`, linker symbols, `#![no_std]` — không compile trên host.
   - **Giải pháp: Tạo module `kernel_logic` (lib)** chứa extracted pure logic, compilable cho cả AArch64 và host.
   - Hoặc **dùng `#[cfg(not(test))]`** trên hardware-dependent code.
   - **Quyết định:** Tạo file `src/testable.rs` chứa các hàm pure logic được extract từ các module khác, dùng `#[cfg(test)] mod tests` bên trong. Kernel code gọi các hàm này, tests cũng gọi chúng.

4. **Các test cases cần viết:**

   **a) TrapFrame layout verification:**
   ```
   - size_of::<TrapFrame>() == 288
   - offset_of x[0] == 0
   - offset_of x[30] == 240
   - offset_of elr_el1 == 248
   - offset_of spsr_el1 == 256
   - offset_of sp_el0 == 264
   - offset_of tpidr_el1 == 272
   - offset_of padding == 280
   ```

   **b) MMU descriptor constant verification:**
   ```
   - VALID bit (bit 0) = 1
   - TABLE bit (bit 1) = 1
   - AF bit (bit 10) = 1
   - AP_RW_EL1 = bits[7:6] = 0b00
   - AP_RW_EL0 = bits[7:6] = 0b01
   - UXN bit (bit 54) = 1
   - PXN bit (bit 53) = 1
   - WXN is SCTLR flag, not descriptor
   - KERNEL_CODE_FLAGS has VALID + AF + RO + UXN (no PXN for kernel exec)
   - USER_STACK_FLAGS has AP_RW_EL0 + UXN + PXN (no exec)
   ```

   **c) SYS_WRITE pointer validation:**
   ```
   - ptr in [0x4000_0000, 0x4800_0000) → valid
   - ptr = 0x0 → invalid
   - ptr = 0x0900_0000 → invalid (UART MMIO!)
   - ptr = 0x4800_0000 → invalid (boundary)
   - len > 256 → clamped to 256
   - len = 0 → valid (no-op)
   ```

   **d) Scheduler round-robin logic:**
   ```
   - 3 tasks Ready: round-robin cycles 0→1→2→0
   - 1 task Faulted: skipped in round-robin
   - All tasks Faulted except idle: idle selected
   - Faulted task past RESTART_DELAY_TICKS: gets restarted
   - Faulted task before RESTART_DELAY_TICKS: stays Faulted
   ```

   **e) IPC state machine:**
   ```
   - send to endpoint with receiver waiting → message transferred, receiver unblocked
   - send to endpoint with no receiver → sender blocked
   - recv from endpoint with sender waiting → message transferred, sender unblocked
   - recv from endpoint with no sender → receiver blocked
   - cleanup_task: clears sender/receiver slots
   - cleanup_task: doesn't affect other tasks' slots
   ```

5. **Cấu hình Cargo cho host tests:**
   - Thêm section trong `Cargo.toml` hoặc dùng workspace setup.
   - Vấn đề: `.cargo/config.toml` set `build.target = aarch64-aegis.json` — `cargo test` sẽ fail.
   - **Giải pháp:** Chạy `cargo test --target x86_64-unknown-linux-gnu` (trên CI Linux) hoặc `cargo test --target x86_64-pc-windows-msvc` (trên local Windows).
   - Hoặc tạo **workspace** riêng cho tests — nhưng phức tạp hơn, để sau.

**Checkpoint:** `cargo test --target x86_64-unknown-linux-gnu` (hoặc Windows equiv) pass toàn bộ unit tests.

---

### F2 — QEMU Integration Tests: Boot kernel, kiểm tra UART output

**Mục tiêu:** Script tự động build kernel, chạy trên QEMU với timeout, capture UART output, assert expected strings xuất hiện theo thứ tự.

**Thay đổi:**

1. **Tạo file `tests/qemu_boot_test.sh`** (cho Linux CI):
   - Build: `cargo build --release`
   - Run QEMU với timeout 10 giây, redirect stdout/stderr vào file
   - Kiểm tra các chuỗi expected output theo thứ tự:
     ```
     [AegisOS] boot
     [AegisOS] MMU enabled
     [AegisOS] W^X enforced
     [AegisOS] exceptions ready
     [AegisOS] scheduler ready
     [AegisOS] timer started
     [AegisOS] bootstrapping into task_a
     A:PING
     B:PONG
     ```
   - Exit code 0 nếu tất cả strings found, 1 nếu thiếu.

2. **Tạo file `tests/qemu_boot_test.ps1`** (cho Windows local dev):
   - Tương tự nhưng bằng PowerShell.
   - Dùng `Start-Process` + redirect output (theo convention trong copilot-instructions.md).

3. **Thêm test cho fault isolation (advanced):**
   - Tạo feature flag `test-fault` trong `Cargo.toml`.
   - Khi `#[cfg(feature = "test-fault")]`, task_b cố đọc `0x0900_0000` (UART MMIO) → trigger Permission Fault.
   - Expected output:
     ```
     [AegisOS] TASK 1 FAULTED
     A:PING
     [AegisOS] TASK 1 RESTARTED
     B:PONG
     ```
   - Script chạy thêm một lần build+QEMU với `--features test-fault` và verify output này.

4. **Giải quyết QEMU exit:**
   - **Phase F1:** Dùng `timeout` (Linux) / `Start-Process` + `WaitForExit(10000)` (Windows) để kill QEMU sau N giây. Đơn giản, đủ dùng.
   - **Phase F nâng cao (tùy chọn):** Thêm PSCI `SYSTEM_OFF` call vào kernel sau khi in sentinel string `[AegisOS] TEST COMPLETE`. Dùng `hvc #0` với `x0 = 0x84000008`. QEMU sẽ tự exit. Cần thêm:
     - Hằng số `TEST_RUN_TICKS: u64` — sau bao nhiêu ticks thì tự shutdown.
     - Hàm `psci_system_off()` với inline asm `hvc`.
     - Gọi trong `timer::handle_tick()` khi `TICK_COUNT >= TEST_RUN_TICKS`.
     - Gate toàn bộ bằng `#[cfg(feature = "test-exit")]`.
   - **Đề xuất:** Dùng `timeout` cho F2, PSCI cho khi cần deterministic exit ở phase sau.

**Checkpoint:** `./tests/qemu_boot_test.sh` pass — in `PASS: All boot strings found` hoặc `FAIL: Missing: ...`.

---

### F3 — GitHub Actions CI: Tự động chạy tests trên mỗi push/PR

**Mục tiêu:** Tạo GitHub Actions workflow chạy unit tests + integration tests trên mỗi push và pull request.

**Thay đổi:**

1. **Tạo file `.github/workflows/ci.yml`:**
   - Trigger: `push` (tất cả branches) + `pull_request` (main/develop).
   - Runner: `ubuntu-latest` (QEMU dễ cài trên Linux, cross-compile từ x86_64 → AArch64 không vấn đề).
   - Steps:
     ```
     a) Checkout repo
     b) Install Rust nightly + rust-src component
     c) Install qemu-system-arm (apt-get)
     d) cargo build --release (verify compilation)
     e) cargo test --target x86_64-unknown-linux-gnu (host unit tests)
     f) chmod +x tests/qemu_boot_test.sh && ./tests/qemu_boot_test.sh (integration)
     ```

2. **Cache Rust toolchain + build artifacts:**
   - Dùng `actions/cache` cho `~/.cargo` và `target/`.
   - Key dựa trên `Cargo.lock` + `rust-toolchain.toml`.

3. **Thêm file `rust-toolchain.toml`** (nếu chưa có):
   - Đảm bảo CI dùng đúng nightly version:
     ```toml
     [toolchain]
     channel = "nightly"
     components = ["rust-src"]
     ```

4. **Badge trong README (tùy chọn):**
   - Thêm CI status badge vào `README.md` (nếu có).

**Checkpoint:** Push lên GitHub → Actions tab hiện workflow chạy → ✅ Green = tất cả tests pass.

---

## Tóm tắt thay đổi theo file

| File | Thay đổi | Sub-phase |
|---|---|---|
| `src/exception.rs` | Extract `validate_write_ptr()` thành hàm public riêng (hiện inline trong `handle_write`) | F1 |
| `src/mmu.rs` | Đảm bảo descriptor constants là `pub const` (có thể đã public) | F1 |
| `src/sched.rs` | Đảm bảo `RESTART_DELAY_TICKS`, `TaskState`, `Tcb` là `pub` cho tests | F1 |
| `src/ipc.rs` | Đảm bảo `Endpoint`, `ENDPOINTS`, `NUM_ENDPOINTS` là `pub` cho tests | F1 |
| `tests/host_tests.rs` | **MỚI** — Unit tests cho TrapFrame, descriptors, validation, scheduler, IPC | F1 |
| `tests/qemu_boot_test.sh` | **MỚI** — Bash script: build + QEMU + assert UART output | F2 |
| `tests/qemu_boot_test.ps1` | **MỚI** — PowerShell equiv cho local dev trên Windows | F2 |
| `Cargo.toml` | Thêm `[features]` section: `test-fault`, `test-exit` | F2 |
| `.github/workflows/ci.yml` | **MỚI** — GitHub Actions workflow | F3 |
| `rust-toolchain.toml` | **MỚI** (hoặc cập nhật) — Pin nightly + `rust-src` | F3 |
| `linker.ld` | Không thay đổi | — |
| `src/boot.s` (trong `main.rs`) | Không thay đổi | — |

---

## Điểm cần lưu ý

1. **Host tests phải override target.** `.cargo/config.toml` set `build.target = aarch64-aegis.json`. Chạy `cargo test` phải thêm `--target x86_64-unknown-linux-gnu` (Linux) hoặc `--target x86_64-pc-windows-msvc` (Windows). Nếu không, cargo sẽ compile tests cho AArch64 — thiếu linker symbols, thiếu OS runtime → link error.

2. **`static mut` và test isolation.** Các tests truy cập `TCBS`, `ENDPOINTS`, v.v. phải chạy **tuần tự** (Rust default cho `cargo test` là multi-thread). Dùng `--test-threads=1` hoặc mỗi test tự reset global state trước khi chạy. Cân nhắc wrapper `unsafe fn reset_test_state()` để clear tất cả globals.

3. **`#[cfg]` gating cho inline asm.** Khi compile cho host, mọi `asm!()` sẽ fail (target khác). Cần `#[cfg(target_arch = "aarch64")]` trên các hàm chứa asm, với stub `#[cfg(not(target_arch = "aarch64"))]` cho host tests. Ưu tiên extract logic thuần ra khỏi hàm có asm thay vì stub.

4. **Không sửa kernel logic.** Phase F chỉ thêm tests + CI. Kernel behavior không thay đổi. Nếu phát hiện bug qua tests — ghi nhận, sửa ở phase riêng.

5. **QEMU timeout phải đủ dài.** Fault restart cần 100 ticks × 10ms = 1 giây. Integration test cho fault isolation cần ít nhất 3–5 giây QEMU runtime. Đề xuất timeout 10 giây cho CI (QEMU trên CI runner có thể chậm hơn local).

6. **Feature flags không ảnh hưởng production.** `test-fault` và `test-exit` chỉ active khi build với `--features`. Default build (`cargo build --release`) không bị ảnh hưởng.

7. **DO-178C mapping:**
   - Unit tests = §6.4.2.2 (Low-level Testing)
   - Integration tests = §6.4.3 (Software Integration Testing)
   - CI = §6.4.1 (Test Environment — repeatable, controlled)
   - Test scripts trong repo = §6.4.4 (Test Coverage Analysis — traceability từ requirement → test)

---

## Đề xuất hành động tiếp theo

1. **Bắt đầu F1** — Viết unit tests cho TrapFrame layout + MMU descriptor constants trước (dễ nhất, zero risk). Verify `cargo test --target x86_64-pc-windows-msvc` pass trên Windows local.

2. **Tiếp F1** — Thêm IPC state machine tests + scheduler round-robin tests. Cần `unsafe` setup cho `static mut` globals — viết helper `reset_test_state()`.

3. **F2** — Viết `qemu_boot_test.sh`, test trên WSL hoặc để dành cho CI.

4. **F3** — Push CI workflow lên GitHub, verify green build.

5. **Sau F3** — Bắt đầu lên kế hoạch Phase G (Capability-Based Access Control) với test infrastructure đã sẵn sàng.
