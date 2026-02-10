
Summarized conversation historyTôi đã thu thập đầy đủ thông tin. Đây là kế hoạch cho Phase D:

---

## Plan: AegisOS Phase D — User/Kernel Separation (EL0 Isolation)

**Trạng thái: ✅ HOÀN THÀNH**

Nâng cấp AegisOS từ "kernel chạy mọi thứ ở EL1" thành "kernel ở EL1, task ở EL0" — bức tường cách ly đã hứa trong blog #03. Gồm 2 trụ cột: (1) chuyển task xuống EL0 với shared kernel stack, (2) page permission đảm bảo task không thể chạm vào kernel memory/device MMIO. Chia thành 4 sub-phase tăng dần, mỗi sub-phase boot được trên QEMU.

### Steps — Tất cả hoàn thành

1. **D1 — EL0 Task Execution** ✅: Đổi SPSR từ `0x345` (EL1h) thành `0x000` (EL0t) trong `sched::init()`. Lower-EL vectors (`_exc_sync_lower64`, `_exc_irq_lower64`) đã sẵn sàng — SVC và IRQ từ EL0 tự động đi qua group 2. Thêm `SAVE_CONTEXT_LOWER` macro sử dụng `TPIDR_EL1` để stash x9, load SP từ `__stack_end` (shared kernel boot stack), rồi save context. `RESTORE_CONTEXT_LOWER` giống RESTORE_CONTEXT. **Verified:** Permission fault khi EL0 task cố ghi UART trực tiếp → xác nhận task đang chạy ở EL0.

2. **D2 — Per-task User Stack & Kernel Stack Separation** ✅: Thêm `.user_stacks` section (3×4KB) trong linker.ld cho EL0 SP. `.task_stacks` giữ nguyên (kernel stacks). `sched::init()` set `context.sp_el0` = top of user stack, `stack_top` = top of kernel stack. MMU: user stack pages dùng `USER_DATA_PAGE` (`AP_RW_EL0`) để EL0 đọc/ghi được. **Kiến trúc:** Dùng shared 16KB kernel boot stack cho tất cả exception handling (single-core, no nesting) — đơn giản và đúng.

3. **D3 — Kernel Memory Isolation** ✅ (không cần per-task page table): Kernel data/bss/page tables/kernel stack giữ `KERNEL_DATA_PAGE` (`AP_RW_EL1`, EL0 No Access). Device memory (UART, GIC) giữ `DEVICE_BLOCK` (`AP_RW_EL1`, EL0 No Access). Code pages dùng `SHARED_CODE_PAGE` (`AP_RO_EL0`, cả EL1 và EL0 executable). Rodata dùng `AP_RO_EL0` (EL0 đọc string literals). **Quyết định:** Không cần per-task L3 table swap vì task code nằm trong kernel `.text` — isolation bằng AP bits là đủ cho 3 static tasks.

4. **D4 — UART Syscall & Deny Direct Device Access** ✅: Thêm `SYS_WRITE = 4` trong `handle_svc`: nhận `x0`=buf, `x1`=len, validate pointer trong RAM range (0x4000_0000–0x4800_0000), max 256 bytes, rồi copy bytes ra UART. Task dùng `user_print()` → `syscall_write()` thay vì `uart_print()`. **Checkpoint cuối:** A:PING B:PONG chạy ổn định, task ở EL0, kernel memory isolated, UART qua syscall.

### Further Considerations

1. **Shared kernel stack vs per-task kernel stack**: Chọn shared 16KB boot stack cho tất cả exception handling vì: single-core, IRQ masked trong handler (không nesting), chỉ 1 exception frame tồn tại tại bất kỳ thời điểm nào. Per-task kernel stack chỉ cần thiết khi có multi-core hoặc nested interrupts.

2. **Shared code section**: Task code và kernel code nằm chung trong `.text`. Dùng `SHARED_CODE_PAGE` (`AP_RO_EL0`, no PXN, no UXN) để cả EL1 và EL0 execute được. Tách riêng task code vào section riêng là Phase E nếu muốn PXN cho task code.

3. **CNTKCTL_EL1**: Không set — EL0 không truy cập timer counters (mặc định deny). An toàn cho safety-critical.

4. **CPACR_EL1.FPEN**: Giữ 0b00 (trap FP/SIMD ở cả EL0 và EL1). Đúng ý đồ — no floating point.

### Đề xuất Phase E: Fault Isolation & Task Restart

Sau khi EL0 isolation hoàn tất, hướng tiếp theo nên là **Fault Isolation** — đảm bảo rằng một task crash (Data Abort, Instruction Abort, illegal syscall) KHÔNG kéo kernel crash. Thay vào đó, kernel đánh dấu task là Faulted và tiếp tục chạy các task còn lại.

**Tại sao Fault Isolation trước?**
- Safety-critical hệ thống **phải** tiếp tục hoạt động khi một component fail (DO-178C, IEC 62304, ISO 26262 đều yêu cầu fault containment).
- Hiện tại, mọi exception từ EL0 mà kernel không handle → `loop { wfe }` = toàn bộ hệ thống dừng.
- Đây là bước nhỏ (chỉ sửa exception handlers) nhưng tác động lớn về safety.

**Các sub-phase đề xuất:**

1. **E1 — Fault Handler**: Thay `loop { wfe }` trong `handle_data_abort`/`handle_instruction_abort`/`handle_unknown` (khi source = lower EL) bằng: print diagnostic → set `TCBS[current].state = Faulted` → `schedule(frame)` để chuyển sang task khác. Task faulted không bao giờ được schedule lại.

2. **E2 — Task Restart**: Thêm `TaskState::Faulted`. Kernel có thể reset TCB của faulted task (reload entry point, reset stack, clear context) và đặt lại `Ready`. Có thể qua SYS_RESTART syscall từ một supervisor task, hoặc tự động sau N ticks.

3. **E3 — Watchdog Timer**: Task phải gọi `SYS_HEARTBEAT` trong mỗi N ticks. Nếu không → kernel tự động đánh dấu Faulted và restart. Phát hiện infinite loop / deadlock.

**Alternatives cho Phase E nếu không chọn Fault Isolation:**
- **(B)** Per-task page table (true address space isolation) — mỗi task có L3 riêng, kernel swap L2 entry khi context switch. Nặng hơn nhưng mạnh hơn.
- **(C)** Shared memory IPC — vùng nhớ chia sẻ giữa 2 task cho zero-copy messaging. Cần mmap syscall.
- **(D)** Multi-core (PSCI boot secondary cores) — chạy task trên nhiều core. Cần spinlock, per-core GIC, IPI.
- **(E)** VirtIO block device driver — đọc/ghi file system từ disk. Cần VirtIO MMIO driver + simple FS.
