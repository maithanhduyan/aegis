## Plan: AegisOS Phase C — Exception Handling + Timer + Scheduler + IPC

Nâng cấp AegisOS từ "kernel biết bảo vệ bộ nhớ" thành "kernel biết chạy nhiều task, chuyển đổi bằng timer, và giao tiếp qua IPC". Chia thành 5 sub-phase tăng dần — mỗi phase build + boot được trên QEMU, in checkpoint qua UART. Tất cả dùng static allocation (không heap), task chạy ở EL1 trước (EL0 để Phase D).

---

### Sub-phase C1 — Full Exception Frame + ESR Dispatch

Nâng cấp [src/exception.rs](src/exception.rs) từ "print rồi halt" thành full context save/restore có thể return:

1. Định nghĩa `TrapFrame` struct (`#[repr(C)]`, 288 byte = 36 × `u64`): `x[0..31]`, `sp_el0`, `elr_el1`, `spsr_el1`, `_pad` — đây là **ABI cố định** cho toàn bộ Phase C.

2. Viết lại vector table assembly trong [src/exception.rs](src/exception.rs): 16 entry đầy đủ (4 group × 4 loại), mỗi entry `sub sp, sp, #288` → save `x0`–`x30` bằng `stp` pairs → save `SP_EL0`, `ELR_EL1`, `SPSR_EL1` → `mov x0, sp` → `bl exception_dispatch` → restore ngược lại → `add sp, sp, #288` → `eret`.

3. Tạo hàm Rust `exception_dispatch(frame: &mut TrapFrame)`: đọc `ESR_EL1` xác định EC field, dispatch theo bảng:
   - `0x15` → `handle_svc(frame)` (stub, cho C4)
   - `0x20`/`0x21` → `handle_instruction_abort(frame)` (print + halt)
   - `0x24`/`0x25` → `handle_data_abort(frame)` (print + halt)
   - `0x00` → unknown (print + halt)
   - Default → unhandled (print EC + halt)

4. Tách handler IRQ riêng: vector IRQ gọi `irq_dispatch(frame: &mut TrapFrame)` thay vì `exception_dispatch` — chuẩn bị cho C2.

5. Cập nhật [src/main.rs](src/main.rs): sau `exception::init()`, in `[AegisOS] exceptions ready\n`. Thêm test: ghi vào guard page (`unsafe { ptr::write_volatile(__stack_guard as *mut u64, 0) }`) → phải thấy Data Abort decoded trên UART.

**Checkpoint:** boot → 4 dòng cũ + `[AegisOS] exceptions ready` + Data Abort message với ESR/FAR/ELR.

---

### Sub-phase C2 — GICv2 Driver + Timer Interrupt

Tạo [src/gic.rs](src/gic.rs) — driver GICv2 tối thiểu:

1. Định nghĩa base address: `GICD_BASE = 0x0800_0000`, `GICC_BASE = 0x0801_0000`. Các register offset: `GICD_CTLR`(0x000), `GICD_ISENABLER`(0x100), `GICD_IPRIORITYR`(0x400), `GICC_CTLR`(0x000), `GICC_PMR`(0x004), `GICC_IAR`(0x00C), `GICC_EOIR`(0x010).

2. Hàm `gic_init()`: disable GICD → enable INTID 30 (CNTP timer PPI) trong `GICD_ISENABLER[0]` bit 30 → set priority 0x00 tại byte offset `0x41E` → enable GICD → set `GICC_PMR = 0xFF` → enable GICC.

3. Hàm `gic_acknowledge() -> u32`: đọc `GICC_IAR`, return INTID.

4. Hàm `gic_end_interrupt(intid: u32)`: ghi `intid` vào `GICC_EOIR`.

Tạo [src/timer.rs](src/timer.rs) — ARM Generic Timer:

1. Hàm `timer_init(tick_ms: u32)`: đọc `CNTFRQ_EL0` (QEMU = 62.5 MHz) → tính `ticks = freq * tick_ms / 1000` → ghi `CNTP_TVAL_EL0 = ticks` → ghi `CNTP_CTL_EL0 = 1` (enable, unmask).

2. Hàm `timer_rearm()`: ghi lại `CNTP_TVAL_EL0` với cùng tick count.

3. Hằng số `TIMER_INTID: u32 = 30`.

Cập nhật [src/boot.s](src/boot.s): thêm trước `eret` (trong đoạn EL2→EL1 drop):
```
mrs x0, CNTHCTL_EL2
orr x0, x0, #3        // EL1PCTEN + EL1PCEN
msr CNTHCTL_EL2, x0
msr CNTVOFF_EL2, xzr
```

Cập nhật [src/exception.rs](src/exception.rs) `irq_dispatch()`: đọc `gic_acknowledge()` → nếu INTID == 30 → gọi `timer::tick_handler()` → `gic_end_interrupt(30)`. Hàm `tick_handler()` in `"T"` ra UART (debug) + tăng biến đếm `TICK_COUNT`.

Unmask IRQ trong `SPSR_EL1` khi boot: giá trị `0x3C5` → đổi thành `0x345` (clear I-bit, bit 7) trong [src/boot.s](src/boot.s) `spsr_el2` setup. Hoặc sau khi vào EL1, chạy `msr DAIFClr, #2` để unmask IRQ.

**Checkpoint:** boot → dòng cũ + `[AegisOS] timer started (10ms)\n` + dòng `TTTTTTTT...` liên tục trên UART.

---

### Sub-phase C3 — Scheduler: 2 Task + Idle, Round-Robin

Tạo [src/sched.rs](src/sched.rs) — scheduler tối thiểu:

1. Định nghĩa `ThreadState`: `Ready`, `Running`, `Blocked`, `Inactive`.

2. Định nghĩa `Tcb` struct (`#[repr(C)]`): `context: TrapFrame` (288 byte), `state: ThreadState`, `priority: u8`, `id: u16`, `stack_top: u64` (đỉnh kernel stack riêng), con trỏ `next: Option<usize>` (index trong mảng tĩnh).

3. Static allocation: `static mut TCBS: [Tcb; 3]` — index 0 = `task_a`, 1 = `task_b`, 2 = `idle`. `static mut CURRENT: usize = 0`.

4. Cập nhật [linker.ld](linker.ld): thêm 3 kernel stack riêng (mỗi cái 4KB, aligned 4096):
   ```
   . = ALIGN(4096);
   __task_stacks_start = .;
   .task_stacks (NOLOAD) : { . += 3 * 4096; }
   __task_stacks_end = .;
   ```

5. Hàm `sched_init()`: khởi tạo 3 TCB:
   - `task_a`: `elr_el1 = task_a_entry as u64`, `spsr_el1 = 0x3C5` (EL1h, IRQ masked sẽ được unmask sau), `sp` = top of stack 0, state = `Ready`, priority = 1.
   - `task_b`: tương tự, entry = `task_b_entry`, stack 1, priority = 1.
   - `idle`: entry = `idle_entry` (loop `wfi`), stack 2, priority = 255.

6. Hàm `schedule(frame: &mut TrapFrame)`: lưu `*frame` vào `TCBS[CURRENT].context` → set state `Ready` → tìm task Ready có priority cao nhất (round-robin nếu bằng nhau) → set `CURRENT` = index mới → set state `Running` → copy `TCBS[CURRENT].context` vào `*frame` → return (exception restore sẽ `eret` vào task mới).

7. Viết 3 task entry trong [src/main.rs](src/main.rs):
   ```rust
   fn task_a_entry() -> ! { loop { uart_print("A"); /* busy wait ~100ms */ } }
   fn task_b_entry() -> ! { loop { uart_print("B"); /* busy wait ~100ms */ } }
   fn idle_entry() -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
   ```

8. Cập nhật `timer::tick_handler()`: bỏ in `"T"`, thay bằng gọi `sched::schedule(frame)`.

9. Cập nhật `kernel_main()`: gọi `sched_init()` → `gic_init()` → `timer_init(10)` → unmask IRQ → load `TCBS[0].context` vào registers → `eret` vào `task_a`. (`kernel_main` **không return** — nó trở thành bootstrap rồi biến mất.)

**Checkpoint:** boot → `[AegisOS] scheduler started\n` + output `AAAAABBBBBAAAAABBBBB...` xen kẽ.

---

### Sub-phase C4 — SVC Syscall + Yield

Cập nhật [src/exception.rs](src/exception.rs) `handle_svc(frame)`:

1. Đọc `frame.x[7]` làm syscall number. Định nghĩa `SYS_YIELD = 0`.

2. `sys_yield(frame)`: gọi `sched::schedule(frame)` — task hiện tại nhường CPU, scheduler chọn task tiếp theo.

3. Viết macro hoặc inline function cho userland:
   ```rust
   #[inline(always)]
   pub fn syscall_yield() {
       unsafe { core::arch::asm!("mov x7, #0", "svc #0", options(nomem, nostack)); }
   }
   ```

4. Cập nhật `task_a` / `task_b`: thay busy-wait bằng `syscall_yield()` — in chữ rồi yield, tạo pattern `ABABAB` rõ ràng hơn.

**Checkpoint:** boot → `ABABABABAB...` đều đặn, mỗi chữ một lần yield.

---

### Sub-phase C5 — IPC: Synchronous Endpoint

Tạo [src/ipc.rs](src/ipc.rs):

1. Định nghĩa `Endpoint` struct: `send_queue: [Option<usize>; 4]` (task indices chờ send), `recv_queue: [Option<usize>; 4]` (task indices chờ recv), `head_send/tail_send/head_recv/tail_recv: usize`.

2. Static allocation: `static mut ENDPOINTS: [Endpoint; 2]` — endpoint 0 cho ping-pong test.

3. Syscall numbers: `SYS_SEND = 1`, `SYS_RECV = 2`, `SYS_CALL = 3` (send + recv), `SYS_REPLY_RECV = 4`.

4. `sys_send(frame, ep_id)`: kiểm tra `ep_id` hợp lệ → nếu có task đang chờ recv trên endpoint → copy `frame.x[0..4]` (message registers) vào receiver TCB `context.x[0..4]` → unblock receiver (state = `Ready`) → return. Nếu không có receiver → block sender (state = `Blocked`), enqueue vào `send_queue`, gọi `schedule(frame)`.

5. `sys_recv(frame, ep_id)`: nếu có task đang chờ send → copy message registers → unblock sender → return với message. Nếu không → block receiver, enqueue, schedule.

6. `sys_call(frame, ep_id)`: = `sys_send` + tự động chuyển sang chờ reply. Tạo implicit reply endpoint (đơn giản: sender ID lưu trong receiver badge).

7. Test trong [src/main.rs](src/main.rs):
   ```
   task_a (client): loop { sys_call(EP_0, "PING") → nhận reply → in "A:reply" }
   task_b (server): loop { sys_recv(EP_0) → in "B:got msg" → sys_reply(...) }
   ```

**Checkpoint:** boot → `A:PING B:PONG A:PING B:PONG...` trên UART.

---

### Chi tiết kỹ thuật quan trọng

**TrapFrame layout (ABI cố định cho toàn Phase C):**

| Offset | Field | Size |
|---|---|---|
| 0–240 | `x[0]`–`x[30]` | 31 × 8 = 248 |
| 248 | `sp_el0` | 8 |
| 256 | `elr_el1` | 8 |
| 264 | `spsr_el1` | 8 |
| 272 | `_pad` | 8 |
| 280 | `_pad2` | 8 |
| **Total** | | **288 bytes** (16-byte aligned) |

**GICv2 register map (QEMU virt):**

| Register | Address | Dùng cho |
|---|---|---|
| `GICD_CTLR` | `0x0800_0000` | Enable/disable distributor |
| `GICD_ISENABLER[0]` | `0x0800_0100` | Enable INTIDs 0–31 (bit 30 = timer) |
| `GICD_IPRIORITYR[30]` | `0x0800_041E` | Priority cho INTID 30 (byte) |
| `GICC_CTLR` | `0x0801_0000` | Enable/disable CPU interface |
| `GICC_PMR` | `0x0801_0004` | Priority mask (set `0xFF`) |
| `GICC_IAR` | `0x0801_000C` | Acknowledge IRQ → INTID |
| `GICC_EOIR` | `0x0801_0010` | End-of-interrupt |

**Timer (ARM Generic Timer):**

| Register | Giá trị | Ghi chú |
|---|---|---|
| `CNTFRQ_EL0` | 62,500,000 (QEMU) | Tần số timer |
| `CNTP_TVAL_EL0` | 625,000 (cho 10ms) | `freq × 0.01` |
| `CNTP_CTL_EL0` | `0x1` | Enable + unmask |
| INTID | 30 | PPI 14 — EL1 Physical Timer |

**Context switch — critical path:**

```
Timer IRQ → save TrapFrame (asm) → irq_dispatch() →
  gic_ack(30) → timer_rearm() → schedule(frame) →
    save frame → TCBS[old].context
    pick next ready task (round-robin)
    load TCBS[new].context → frame
  → gic_eoi(30) → restore TrapFrame (asm) → eret
```

**EL2→EL1 boot.s additions (cho timer access):**

Thêm 3 dòng trước `eret` trong đoạn EL2→EL1 drop:
- `mrs x0, CNTHCTL_EL2` → `orr x0, x0, #3` → `msr CNTHCTL_EL2, x0` (enable EL1 physical timer access)
- `msr CNTVOFF_EL2, xzr` (zero virtual offset)

**Linker script additions (cho C3):**

Thêm `.task_stacks` section (NOLOAD, 3 × 4096 = 12KB, aligned 4096) sau `.page_tables`, trước `__stack_guard`. Symbols: `__task_stacks_start`, `__task_stacks_end`. Mỗi task dùng `__task_stacks_start + i * 4096 + 4096` làm stack top.

---

### Quyết định kiến trúc đã khóa

1. **TrapFrame 288 byte** — mọi assembly (exception entry, context switch, syscall) dùng cùng offset. Định nghĩa một lần, không đổi.
2. **Static allocation only** — không heap trong kernel. TCB, endpoint, stack đều static. Đúng chuẩn safety-critical.
3. **Single address space** — tất cả task dùng chung TTBR0 (identity map). Per-task page table để Phase D.
4. **EL1 tasks trước** — task chạy ở EL1 (cùng privilege với kernel). Chuyển EL0 ở Phase D.
5. **Synchronous IPC** — không async, không buffer queue. Sender block đến khi receiver sẵn sàng. Dễ verify.

---

### Lưu ý quan trọng

1. **Thứ tự bắt buộc**: C1 → C2 → C3 → C4 → C5. Không thể nhảy bước — scheduler cần timer, IPC cần scheduler.
2. **IRQ unmask timing**: DAIF.I phải clear **sau** khi GIC và timer đã init xong, nếu không IRQ trước khi handler sẵn sàng → crash.
3. **Kernel stack per task**: exception entry dùng SP_EL1 hiện tại. Nếu 2 task dùng chung SP_EL1, TrapFrame sẽ ghi đè nhau. Mỗi task cần SP_EL1 riêng — load khi context switch.
4. **Re-arm timer trước EOI**: nếu EOI trước re-arm, có khoảng trống timer không đếm → jitter. Re-arm trước, EOI sau.
5. **`eret` là atomic**: restore `ELR_EL1` + `SPSR_EL1` **trước** `eret`, không có instruction nào chen giữa ghi SPSR và eret.
6. **FP/SIMD disabled**: set `CPACR_EL1.FPEN = 0b00` trong boot.s — bất kỳ FP instruction nào sẽ trap, bắt lỗi compiler tự sinh FP code.
