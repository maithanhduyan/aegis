# Kế hoạch Phase L — Tách Arch + ELF Loader

> **Trạng thái: ✅ COMPLETE (L1–L6 ✅, blog ✅, test ✅, docs ✅)** — Tái cấu trúc codebase thành `arch/aarch64/` + `kernel/` + `platform/` để tách biệt code phụ thuộc kiến trúc khỏi logic portable, sau đó xây dựng ELF64 parser + loader để load task từ binary thay vì hardcode trong kernel. Đây là bước nền tảng để AegisOS hướng tới portability (RISC-V tương lai) và modularity (yêu cầu DO-178C/IEC 62304).

---

## Tại sao Phase L?

### Lỗ hổng hiện tại: "Mọi thứ trộn lẫn, mọi task bị đóng cứng"

AegisOS sau Phase K có đầy đủ cơ chế microkernel: priority scheduler, time budget, watchdog, IPC, capability, per-task address space, user-mode driver. Nhưng codebase có **hai vấn đề cấu trúc nghiêm trọng**:

**Vấn đề 1 — Code trộn lẫn arch-specific và portable:**

Hiện tại, 13 file `.rs` nằm phẳng trong `src/`. Mỗi file dùng `#[cfg(not(test))]` rải rác để tách code AArch64 khỏi host test. Kết quả:

- `exception.rs` (780 dòng) — 95% là AArch64 asm + register access, chỉ 5% logic portable
- `mmu.rs` (507 dòng) — 70% AArch64 page table manipulation, 30% constants/validation
- `sched.rs` (330 dòng) — 75% portable nhưng chứa `msr ttbr0_el1` và linker symbols
- `timer.rs` (113 dòng) — 85% AArch64 register access, chỉ `TICK_COUNT` là portable
- Tổng cộng **~50 blocks `cfg(not(test))`** rải khắp 10 file

Điều này vi phạm nguyên tắc **separation of concerns** — yêu cầu cơ bản của DO-178C §6.3 (modular design) và IEC 62304 §5.3 (software architecture).

Trong hệ thống safety-critical thực tế:
- **Tên lửa**: Lockheed Martin F-35 dùng kiến trúc `platform/` + `core/` để cùng 1 flight software chạy trên nhiều biến thể phần cứng. Code trộn lẫn → không thể chứng minh module independence → không qua DO-178C DAL A.
- **Y tế**: Philips IntelliVue monitors chạy trên ARM và x86. Kernel phải tách arch → test logic trên host mà không cần hardware → giảm 60% thời gian verification.
- **Ô tô**: AUTOSAR OS tách `Os_Arch_*` và `Os_Kernel_*` — cho phép cùng 1 scheduler chạy trên Infineon AURIX (TriCore) và Renesas RH850.

**Vấn đề 2 — Task hardcode trong kernel binary:**

Ba task (`uart_driver_entry`, `client_entry`, `idle_entry`) được viết trực tiếp trong `main.rs` và link cùng kernel. Điều này có nghĩa:
- Thay đổi bất kỳ task nào → phải rebuild **toàn bộ kernel**
- Không thể load task mới sau khi boot
- Kernel chứa application code → vi phạm separation of concerns
- Không thể independent verification: kernel và application cùng binary

Trong hệ thống thật:
- **Xe tự lái**: OTA update chỉ cần thay đổi application (camera AI model) mà không rebuild kernel → giảm rủi ro regression
- **Máy thở**: FDA yêu cầu OS và application là separate software items (IEC 62304 §5.3) → phải có ranh giới rõ ràng
- **Vệ tinh**: Payload software được upload riêng sau khi satellite đã trên quỹ đạo → ELF loader là bắt buộc

### Bảng tóm tắt vấn đề

| # | Vấn đề | Ảnh hưởng |
|---|---|---|
| 1 | 13 file phẳng trong `src/`, arch-specific trộn lẫn portable | Không thể chứng minh module independence cho safety certification. Khó port sang RISC-V. Host test phải dùng `cfg(not(test))` rải rác |
| 2 | ~370 dòng inline asm nằm trong file logic (exception.rs, timer.rs, mmu.rs, sched.rs, main.rs) | Khó review, khó audit. Asm nên nằm riêng để chuyên gia asm review |
| 3 | ~50 blocks `cfg(not(test))` rải khắp 10 file | Fragile — thêm 1 function quên cfg → host test compile fail hoặc panic |
| 4 | MMIO addresses hardcode trong nhiều file (gic.rs, uart.rs, main.rs, mmu.rs, exception.rs) | Đổi platform (QEMU virt → Raspberry Pi) → phải sửa 5+ file |
| 5 | Task entries hardcode trong kernel binary | Không thể load task mới, không thể OTA update application riêng |
| 6 | Linker script ở root, tightly coupled với mmu.rs | Thêm arch mới cần linker script riêng nhưng không có cấu trúc cho nó |

### Giải pháp đề xuất

| Cơ chế | Mô tả | Giải quyết vấn đề # |
|---|---|---|
| **Tách `arch/aarch64/`** | Gom tất cả inline asm, system register access, page table manipulation vào `src/arch/aarch64/` | #1, #2, #3 |
| **Tách `kernel/`** | Logic portable (scheduler, IPC, capability, grant, IRQ) vào `src/kernel/` | #1, #3 |
| **Tách `platform/qemu_virt/`** | MMIO addresses, memory map constants, linker script vào 1 nơi | #4, #6 |
| **Arch interface** | `arch::current` module cung cấp API chuẩn → portable code gọi qua interface, không gọi trực tiếp | #1, #3 |
| **ELF64 parser** | Parse ELF header + PT_LOAD segments từ byte slice, no heap | #5 |
| **ELF loader** | Load parsed ELF vào user address space, setup entry+stack | #5 |

---

## Phân tích hiện trạng

### Cấu trúc thư mục hiện tại

```
src/
├── boot.s              [ARCH 100%]    118 dòng
├── main.rs             [ARCH ~99%]    ~500 dòng
├── lib.rs              [PORTABLE]     25 dòng
├── exception.rs        [ARCH ~95%]    780 dòng
├── sched.rs            [MIXED 75/25]  ~330 dòng
├── ipc.rs              [PORTABLE 100%] ~310 dòng
├── mmu.rs              [MIXED 70/30]  507 dòng
├── cap.rs              [PORTABLE 100%] ~172 dòng
├── timer.rs            [ARCH ~85%]    ~113 dòng
├── gic.rs              [ARCH 100%]    ~100 dòng
├── grant.rs            [MIXED 80/20]  222 dòng
├── irq.rs              [MIXED 65/35]  285 dòng
└── uart.rs             [MIXED 50/50]  ~41 dòng
```

### Phân loại chi tiết: Arch-Specific vs Portable

**100% ARCH (di chuyển nguyên vẹn):**
- `boot.s` — entry point, EL2→EL1, BSS clear, MMU enable
- `gic.rs` — GICv2 driver (GICD+GICC MMIO)
- `main.rs` — kernel_main, syscall wrappers (`svc #0`), task entries, panic

**95% ARCH (tách ra, giữ lại phần portable):**
- `exception.rs` — vector table + asm macros + dispatch = ARCH. Chỉ `is_valid_user_buffer()` = portable
- `timer.rs` — register access = ARCH. Chỉ `TICK_COUNT` + `tick_count()` = portable

**70% ARCH (tách nhiều):**
- `mmu.rs` — page table build/map/unmap + TLB ops = ARCH. Constants + `descriptor_for_section()` + `DeviceInfo` = portable

**75% PORTABLE (tách ít):**
- `sched.rs` — scheduler logic portable. Chỉ TTBR0 switch + linker symbols + bootstrap = ARCH
- `grant.rs` — grant logic portable. Chỉ `mmu::map_grant_page()` calls = ARCH
- `irq.rs` — irq logic portable. Chỉ `gic::*` calls = ARCH

**100% PORTABLE (không cần di chuyển logic):**
- `ipc.rs` — hoàn toàn portable, zero asm, zero cfg
- `cap.rs` — hoàn toàn portable, zero asm, zero cfg

### Inline asm inventory: ~370 dòng trong 5 file

| File | Dòng asm | Loại |
|---|---|---|
| `exception.rs` | ~228 | `global_asm!` — vector table, SAVE/RESTORE macros |
| `main.rs` | ~80 | `asm!` — 13 syscall wrappers + `wfi` + `global_asm!(boot.s)` |
| `mmu.rs` | ~12 | `asm!` — `dsb ish; isb`, `tlbi aside1is` |
| `timer.rs` | ~8 | `asm!` — `mrs CNTFRQ_EL0`, `msr CNTP_*` |
| `sched.rs` | ~12 | `asm!` — `msr ttbr0_el1`, `eret`, `msr spsel` |

### MMIO addresses phân tán

| Address | File(s) chứa |
|---|---|
| `0x0800_0000` (GICD) | `gic.rs` |
| `0x0801_0000` (GICC) | `gic.rs` |
| `0x0900_0000` (UART) | `uart.rs`, `main.rs`, `exception.rs` |
| `0x4008_0000` (kernel load) | `linker.ld` |
| `0x4000_0000` (RAM base) | `mmu.rs`, `exception.rs` |

### `cfg(not(test))` distribution: ~50 blocks

| File | Số blocks | Mô tả |
|---|---|---|
| `exception.rs` | ~18 | Gần như toàn bộ file |
| `mmu.rs` | ~10 | Functions + linker externs + constants |
| `main.rs` | ~6 | kernel_main, panic, task entries |
| `sched.rs` | ~5 | TTBR0 switch, linker symbols, bootstrap |
| `timer.rs` | ~4 | init, rearm, tick_handler |
| `grant.rs` | ~4 | mmu calls |
| `irq.rs` | ~4 | gic calls |
| `uart.rs` | ~2 | UART0 address, write impl |
| `lib.rs` | ~1 | module declarations |

### Capability bits còn trống

```
Bit  0–17:  ĐÃ DÙNG (18 bits)
Bit 18–63:  TRỐNG (46 bits)
Phase L cần: ~1 bit (CAP_TASK_CREATE) nếu thêm syscall
```

---

## Thiết kế Phase L

### L1 — Tạo Cấu Trúc `arch/` + `kernel/` + `platform/`

#### Khái niệm

Bước đầu tiên: tạo cấu trúc thư mục mới và di chuyển **các module nguyên vẹn** (không tách file). Mục tiêu: build thành công + tất cả test pass mà chỉ thay đổi `mod` paths.

Hình ảnh: Giống dọn nhà — bước 1 là kê tủ mới (tạo phòng `arch/`, `kernel/`, `platform/`), chưa dọn đồ bên trong tủ cũ.

#### Cấu trúc thư mục mục tiêu L1

```
src/
├── arch/
│   ├── mod.rs                  ← #[cfg(target_arch = "aarch64")] pub mod aarch64;
│   │                              pub use aarch64 as current;
│   └── aarch64/
│       ├── mod.rs              ← re-export boot, gic, uart
│       ├── boot.s              ← di chuyển từ src/boot.s (nguyên vẹn)
│       ├── gic.rs              ← di chuyển từ src/gic.rs (nguyên vẹn)
│       └── uart.rs             ← di chuyển từ src/uart.rs (nguyên vẹn)
│
├── kernel/
│   ├── mod.rs                  ← re-export ipc, cap
│   ├── ipc.rs                  ← di chuyển từ src/ipc.rs (nguyên vẹn)
│   └── cap.rs                  ← di chuyển từ src/cap.rs (nguyên vẹn)
│
├── platform/
│   └── qemu_virt.rs            ← MỚI: tập hợp MMIO addresses (GICD, GICC, UART0, RAM base)
│
├── exception.rs                ← giữ nguyên vị trí (tách ở L2)
├── mmu.rs                      ← giữ nguyên vị trí (tách ở L2)
├── sched.rs                    ← giữ nguyên vị trí (tách ở L2)
├── timer.rs                    ← giữ nguyên vị trí (tách ở L2)
├── grant.rs                    ← giữ nguyên vị trí (tách ở L2)
├── irq.rs                      ← giữ nguyên vị trí (tách ở L2)
├── main.rs                     ← cập nhật use paths
└── lib.rs                      ← cập nhật module tree
```

#### Nguyên tắc L1

1. **Chỉ di chuyển module 100% ARCH hoặc 100% PORTABLE** — không tách file
2. `boot.s`, `gic.rs`, `uart.rs` → `arch/aarch64/` (100% arch)
3. `ipc.rs`, `cap.rs` → `kernel/` (100% portable)
4. Tạo `platform/qemu_virt.rs` tập hợp MMIO addresses — nhưng chưa đổi code dùng nó
5. Cập nhật `lib.rs` module tree + tất cả `use` paths
6. **KHÔNG thay đổi logic** — chỉ `move` + `pub use`

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/arch/mod.rs` | Tạo mới | `cfg` gate + `pub use aarch64 as current` |
| `src/arch/aarch64/mod.rs` | Tạo mới | Re-export `boot`, `gic`, `uart` |
| `src/arch/aarch64/boot.s` | Di chuyển | Từ `src/boot.s` |
| `src/arch/aarch64/gic.rs` | Di chuyển | Từ `src/gic.rs` |
| `src/arch/aarch64/uart.rs` | Di chuyển | Từ `src/uart.rs` |
| `src/kernel/mod.rs` | Tạo mới | Re-export `ipc`, `cap` |
| `src/kernel/ipc.rs` | Di chuyển | Từ `src/ipc.rs` |
| `src/kernel/cap.rs` | Di chuyển | Từ `src/cap.rs` |
| `src/platform/mod.rs` | Tạo mới | `pub mod qemu_virt` |
| `src/platform/qemu_virt.rs` | Tạo mới | Tập hợp `GICD_BASE`, `GICC_BASE`, `UART0`, `RAM_BASE` |
| `src/lib.rs` | Sửa | Thêm `mod arch`, `mod kernel`, `mod platform`. Cập nhật re-exports |
| `src/main.rs` | Sửa | Cập nhật `use` paths: `aegis_os::kernel::ipc`, `aegis_os::arch::current::gic`, ... |
| `src/exception.rs` | Sửa nhẹ | Cập nhật `use` paths cho `gic`, `ipc`, `cap` |
| `src/sched.rs` | Sửa nhẹ | Cập nhật `use` paths cho `ipc`, `cap` |
| `src/grant.rs` | Sửa nhẹ | Cập nhật `use` paths |
| `src/irq.rs` | Sửa nhẹ | Cập nhật `use` paths cho `gic` |
| `tests/host_tests.rs` | Sửa | Cập nhật `use` paths cho `ipc`, `cap`, v.v. |
| `linker.ld` | Không đổi | Giữ nguyên vị trí (di chuyển ở phase sau) |

#### Checkpoint L1

```
[AegisOS] arch separation: phase L1 — module structure created
```

Xác nhận: QEMU boot đầy đủ (18 checkpoints cũ vẫn pass) + 162 host tests pass.

---

### L2 — Tách Arch-Specific Code Từ Mixed Files

#### Khái niệm

Bước lớn nhất: tách phần arch-specific ra khỏi 6 file "mixed" (`exception.rs`, `mmu.rs`, `timer.rs`, `sched.rs`, `grant.rs`, `irq.rs`). Code arch di chuyển vào `arch/aarch64/`. Code portable ở lại hoặc vào `kernel/`.

Hình ảnh: Bước 2 dọn nhà — mở từng thùng đồ ra, phân loại: quần áo mùa đông (arch) vào tủ riêng, quần áo quanh năm (portable) vào tủ chung.

#### Cấu trúc thư mục mục tiêu L2

```
src/
├── arch/
│   ├── mod.rs
│   └── aarch64/
│       ├── mod.rs              ← re-export tất cả arch modules
│       ├── boot.s              ← (từ L1)
│       ├── gic.rs              ← (từ L1)
│       ├── uart.rs             ← (từ L1)
│       ├── vectors.rs          ← MỚI: global_asm! vector table + SAVE/RESTORE macros
│       ├── trap.rs             ← MỚI: dispatch_sync, dispatch_irq, abort handlers
│       ├── context.rs          ← MỚI: TrapFrame struct, init_context()
│       ├── mmu.rs              ← MỚI: page table build, map/unmap, TLB ops, mmu_exports()
│       ├── timer.rs            ← MỚI: init(), rearm(), register access
│       ├── syscall.rs          ← MỚI: 13 syscall wrappers (svc #0)
│       └── bootstrap.rs        ← MỚI: bootstrap() — TTBR0 set + eret vào EL0
│
├── kernel/
│   ├── mod.rs
│   ├── sched.rs                ← MỚI: portable scheduler (TaskState, Tcb, schedule(), epoch, watchdog)
│   ├── ipc.rs                  ← (từ L1)
│   ├── cap.rs                  ← (từ L1)
│   ├── grant.rs                ← MỚI: grant logic (gọi arch::current::mmu qua function)
│   ├── irq.rs                  ← MỚI: irq logic (gọi arch::current::gic qua function)
│   └── timer.rs                ← MỚI: TICK_COUNT, tick_count(), tick_handler logic
│
├── platform/
│   ├── mod.rs
│   └── qemu_virt.rs            ← MMIO addresses + memory map constants
│
├── main.rs                     ← kernel_main, task entries (vẫn arch-specific)
└── lib.rs                      ← module tree
```

#### Chi tiết tách từng file

**`exception.rs` (780 dòng) → 3 file arch + 0 file kernel:**

| Nội dung hiện tại | Đích | Lý do |
|---|---|---|
| `global_asm!` (SAVE/RESTORE macros, vector table, stubs) ~228 dòng | `arch/aarch64/vectors.rs` | 100% AArch64 asm |
| `TrapFrame` struct + `init_context()` | `arch/aarch64/context.rs` | ABI-locked to AArch64 registers |
| `handle_sync_*`, `handle_irq_*`, abort handlers, `init()` | `arch/aarch64/trap.rs` | Đọc ESR/FAR registers, gọi GIC |
| `handle_svc()` + các `handle_*` dispatch | `arch/aarch64/trap.rs` | Syscall dispatch (gọi kernel modules) |
| `is_valid_user_buffer()` | `kernel/sched.rs` hoặc `kernel/mod.rs` | Pure validation, portable |

**`mmu.rs` (507 dòng) → 1 file arch + constants giữ lại:**

| Nội dung hiện tại | Đích | Lý do |
|---|---|---|
| Descriptor constants (`VALID`, `TABLE`, `AP_*`, ...) | `arch/aarch64/mmu.rs` | Semantics AArch64-specific |
| Composed templates (`KERN_CODE_ATTR`, ...) | `arch/aarch64/mmu.rs` | Dùng trong page table build |
| `init()`, `build_*`, `map_*`, `unmap_*` | `arch/aarch64/mmu.rs` | Page table manipulation |
| `MAIR_VALUE`, `TCR_VALUE`, `SCTLR_VALUE`, `mmu_exports()` | `arch/aarch64/mmu.rs` | Boot.s cần |
| `descriptor_for_section()` | `arch/aarch64/mmu.rs` | Dùng linker symbols |
| `DeviceInfo`, `DEVICE_TABLE`, `device_lookup()` | `platform/qemu_virt.rs` | Device registry = platform-specific |
| Error constants | Có thể ở `kernel/` | Portable nhưng nhỏ, có thể giữ ở arch |

**`timer.rs` (113 dòng) → 1 file arch + 1 file kernel:**

| Nội dung hiện tại | Đích | Lý do |
|---|---|---|
| `init()`, `rearm()` — system register access | `arch/aarch64/timer.rs` | `msr CNTP_*` |
| `TICK_COUNT`, `tick_count()` | `kernel/timer.rs` | Pure static counter |
| `tick_handler()` logic (budget, epoch, watchdog, schedule) | `kernel/timer.rs` | Logic portable, gọi `arch::timer::rearm()` |

**`sched.rs` (330 dòng) → phần lớn vào kernel + nhỏ vào arch:**

| Nội dung hiện tại | Đích | Lý do |
|---|---|---|
| `TaskState`, `Tcb`, `TCBS`, `CURRENT_TASK`, `NUM_TASKS` | `kernel/sched.rs` | Portable data structures |
| `schedule()` (priority selection, context save/load) | `kernel/sched.rs` | Logic portable |
| `fault_current_task()`, `restart_task()`, epoch/budget/watchdog | `kernel/sched.rs` | Logic portable |
| TTBR0 switch trong `schedule()` (`msr ttbr0_el1`) | Gọi `arch::current::switch_ttbr0(val)` | 1 dòng asm |
| `bootstrap()` (TTBR0 + SPSR + eret) | `arch/aarch64/bootstrap.rs` | AArch64-specific: `msr spsel`, `eret` |
| `is_valid_user_buffer()` (từ exception.rs) | `kernel/sched.rs` | Validation logic |

**`grant.rs` (222 dòng) → kernel/ nhưng gọi arch:**

| Nội dung hiện tại | Đích | Lý do |
|---|---|---|
| `Grant` struct, `create/revoke/cleanup` logic | `kernel/grant.rs` | Portable |
| `mmu::map_grant_page()` calls | Thay bằng `arch::current::mmu::map_grant_page()` | Arch call qua interface |
| Linker symbol `__grant_pages_start` | `arch::current::mmu::grant_page_base()` | Arch-provided address |

**`irq.rs` (285 dòng) → kernel/ nhưng gọi arch:**

| Nội dung hiện tại | Đích | Lý do |
|---|---|---|
| `IrqBinding` struct, `bind/ack/route/cleanup` logic | `kernel/irq.rs` | Portable |
| `gic::enable/disable/set_priority` calls | Thay bằng `arch::current::gic::*` | Arch call qua interface |

#### Arch Interface — Hàm mà kernel/ gọi từ arch/

```
// arch::current cung cấp các hàm sau cho kernel/ gọi:

// context.rs
pub struct TrapFrame { ... }               // 288 bytes, ABI-locked
pub fn init_context(...) -> TrapFrame      // Tạo context cho task mới

// mmu.rs
pub fn map_grant_page(task, grant_idx, page_addr)  // Map shared page
pub fn unmap_grant_page(task, grant_idx)            // Unmap shared page
pub fn map_device(task, l2_index)                   // Map MMIO cho EL0
pub fn switch_ttbr0(ttbr0_val: u64)                 // Switch address space
pub fn grant_page_base() -> u64                     // Linker-provided address

// timer.rs
pub fn timer_init()                        // Setup CNTP + enable
pub fn timer_rearm()                       // Reset countdown

// gic.rs
pub fn gic_init()                          // GIC init
pub fn gic_enable_irq(intid)               // Enable interrupt
pub fn gic_disable_irq(intid)              // Disable interrupt
pub fn gic_ack() -> u32                    // Read IAR
pub fn gic_eoi(intid)                      // Write EOIR
pub fn gic_set_priority(intid, prio)       // Set priority

// bootstrap.rs
pub fn bootstrap(tcb: &Tcb)                // Set TTBR0 + eret → EL0

// uart.rs
pub fn uart_putc(c: u8)                    // Write byte to UART
```

#### Host Test — Thay thế `cfg(not(test))`

Hiện tại: ~50 `cfg(not(test))` blocks rải rác.

Sau L2: `arch/` module **không được compile khi host test** (vì `#[cfg(target_arch = "aarch64")]`). Kernel modules gọi arch functions qua `arch::current::*` — trên host, `arch::current` không tồn tại.

**Giải pháp**: Trong `kernel/*.rs`, các function cần arch call sẽ có parameter hoặc dùng `cfg`:

```
// Cách 1: Conditional call (đơn giản nhất, giữ hiện trạng)
fn schedule_inner(...) {
    // ... portable logic ...
    #[cfg(target_arch = "aarch64")]
    arch::current::switch_ttbr0(new_ttbr0);
}

// Cách 2 (tương lai): Arch trait — dùng khi port RISC-V
```

Ưu tiên **Cách 1** cho Phase L — tập trung `cfg` vào ít điểm nhất thay vì rải khắp nơi. Mục tiêu: giảm từ ~50 blocks → ~15 blocks (chỉ ở ranh giới arch/kernel).

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/arch/aarch64/vectors.rs` | Tạo mới | Tách `global_asm!` từ `exception.rs` |
| `src/arch/aarch64/trap.rs` | Tạo mới | Tách dispatch + handlers từ `exception.rs` |
| `src/arch/aarch64/context.rs` | Tạo mới | `TrapFrame` + `init_context()` |
| `src/arch/aarch64/mmu.rs` | Tạo mới | Tách từ `src/mmu.rs` |
| `src/arch/aarch64/timer.rs` | Tạo mới | `init()`, `rearm()` |
| `src/arch/aarch64/syscall.rs` | Tạo mới | 13 syscall wrappers từ `main.rs` |
| `src/arch/aarch64/bootstrap.rs` | Tạo mới | `bootstrap()` từ `sched.rs` |
| `src/arch/aarch64/mod.rs` | Sửa | Thêm re-exports |
| `src/kernel/sched.rs` | Tạo mới | Portable scheduler từ `src/sched.rs` |
| `src/kernel/timer.rs` | Tạo mới | `TICK_COUNT` + tick handler logic |
| `src/kernel/grant.rs` | Di chuyển + sửa | Từ `src/grant.rs`, đổi arch calls |
| `src/kernel/irq.rs` | Di chuyển + sửa | Từ `src/irq.rs`, đổi arch calls |
| `src/platform/qemu_virt.rs` | Sửa | Thêm `DeviceInfo`, `DEVICE_TABLE` từ `mmu.rs` |
| `src/main.rs` | Sửa | Cập nhật paths, dùng `arch::current::*` |
| `src/lib.rs` | Sửa | Cập nhật module tree |
| Xóa `src/exception.rs` | Xóa | Đã tách thành vectors.rs + trap.rs + context.rs |
| Xóa `src/mmu.rs` | Xóa | Đã di chuyển vào arch/aarch64/mmu.rs |
| Xóa `src/sched.rs` | Xóa | Đã di chuyển vào kernel/sched.rs |
| Xóa `src/timer.rs` | Xóa | Đã tách vào arch + kernel |
| Xóa `src/grant.rs` | Xóa | Đã di chuyển vào kernel/grant.rs |
| Xóa `src/irq.rs` | Xóa | Đã di chuyển vào kernel/irq.rs |
| `tests/host_tests.rs` | Sửa | Cập nhật tất cả `use` paths |

#### Checkpoint L2

```
[AegisOS] arch separation: complete (arch/aarch64 + kernel + platform)
```

Xác nhận: QEMU boot đầy đủ (18 checkpoints cũ vẫn pass) + 162 host tests pass.

**Đây là sub-phase rủi ro cao nhất** — nhiều file thay đổi đồng thời. Cần chia nhỏ: di chuyển 1 module → build → test → tiếp.

---

### L3 — Minimal ELF64 Parser

#### Khái niệm

Xây dựng ELF64 parser hoàn toàn `no_std`, no heap. Parser nhận `&[u8]` (byte slice trỏ vào ELF binary trong memory) và trả về struct mô tả entry point + danh sách PT_LOAD segments.

Chỉ hỗ trợ:
- ELF64 (Class = 2)
- Little-endian (Data = 1)
- Executable (Type = ET_EXEC = 2)
- AArch64 (Machine = EM_AARCH64 = 183)
- Segment type PT_LOAD (Type = 1)

Hình ảnh: ELF file giống một **cuốn sách có mục lục**. Trang đầu (ELF header) cho biết sách có bao nhiêu chương. Mỗi chương (program header) nói "copy nội dung trang X–Y vào địa chỉ Z trong bộ nhớ". Parser đọc mục lục, chưa copy.

#### Thiết kế dữ liệu

```rust
// src/kernel/elf.rs (MỚI)

/// Kết quả parse 1 PT_LOAD segment
pub struct ElfSegment {
    pub vaddr: u64,        // Virtual address để load
    pub offset: u64,       // Offset trong ELF file
    pub filesz: u64,       // Bytes cần copy từ file
    pub memsz: u64,        // Bytes cần allocate (memsz >= filesz, phần dư = zero)
    pub flags: u32,        // PF_R=4, PF_W=2, PF_X=1
}

/// Kết quả parse toàn bộ ELF
pub struct ElfInfo {
    pub entry: u64,                        // Entry point address
    pub segments: [Option<ElfSegment>; 4], // Tối đa 4 PT_LOAD segments (static array)
    pub num_segments: usize,               // Số segments thực tế
}

/// Lỗi parse
pub enum ElfError {
    TooSmall,           // File < 64 bytes (ELF64 header size)
    BadMagic,           // Không phải 0x7F 'E' 'L' 'F'
    Not64Bit,           // Class != 2
    NotLittleEndian,    // Data != 1
    NotExecutable,      // Type != ET_EXEC
    WrongArch,          // Machine != EM_AARCH64 (183)
    TooManySegments,    // > 4 PT_LOAD segments
    SegmentOutOfBounds, // Segment offset+size vượt file
}

/// Parse ELF64 từ byte slice. No heap.
pub fn parse_elf64(data: &[u8]) -> Result<ElfInfo, ElfError>;
```

#### Logic parse

```
parse_elf64(data):
  1. Kiểm tra data.len() >= 64 (ELF64 header size)
  2. Kiểm tra magic: data[0..4] == [0x7F, 'E', 'L', 'F']
  3. Kiểm tra class (data[4] == 2), endian (data[5] == 1)
  4. Đọc e_type (offset 16, u16) == 2 (ET_EXEC)
  5. Đọc e_machine (offset 18, u16) == 183 (EM_AARCH64)
  6. Đọc e_entry (offset 24, u64) → entry point
  7. Đọc e_phoff (offset 32, u64) → program header table offset
  8. Đọc e_phentsize (offset 54, u16) → program header entry size
  9. Đọc e_phnum (offset 56, u16) → number of program headers
  10. Iterate program headers:
      for i in 0..e_phnum:
        ph_offset = e_phoff + i * e_phentsize
        p_type = read_u32(data, ph_offset)
        if p_type == 1 (PT_LOAD):
          Đọc p_offset, p_vaddr, p_filesz, p_memsz, p_flags
          Thêm vào segments[]
  11. Trả về ElfInfo { entry, segments, num_segments }
```

#### Helper functions (no heap, no FP)

```rust
fn read_u16_le(data: &[u8], offset: usize) -> u16;  // data[offset] | data[offset+1]<<8
fn read_u32_le(data: &[u8], offset: usize) -> u32;
fn read_u64_le(data: &[u8], offset: usize) -> u64;
```

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/elf.rs` | Tạo mới | ~150 dòng: structs + `parse_elf64()` + helpers |
| `src/kernel/mod.rs` | Sửa | Thêm `pub mod elf` |
| `src/lib.rs` | Sửa | Re-export nếu cần |
| `tests/host_tests.rs` | Sửa | ~10 tests cho ELF parser |

#### Checkpoint L3

```
[AegisOS] ELF64 parser ready
```

Xác nhận: QEMU boot + UART output checkpoint. Host tests parse synthetic ELF binary.

---

### L4 — ELF Loader: Load Binary Vào User Address Space

#### Khái niệm

Dùng ELF parser từ L3 để load binary vào user memory. Trong phase này, ELF binary được **embed vào kernel image** (dùng `include_bytes!`) để đơn giản hóa — chưa cần filesystem. Kernel parse, copy PT_LOAD segments vào user pages, setup entry point + stack, rồi schedule task.

Hình ảnh: Nếu L3 là đọc mục lục sách, L4 là **photocopy từng chương** vào đúng phòng (user address space) theo hướng dẫn mục lục.

#### Flow

```
Khởi tạo task từ ELF:
  1. embed_elf = include_bytes!("../../path/to/user_task.bin")  // hoặc static &[u8]
  2. elf_info = parse_elf64(embed_elf)?
  3. Với mỗi PT_LOAD segment:
     a. Tính page range: vaddr..vaddr+memsz (4KB aligned)
     b. Copy filesz bytes từ elf data → user pages
     c. Zero phần memsz - filesz (BSS)
     d. Set page permissions: PF_X → execute-only, PF_W → read-write, PF_R → read-only
  4. Set TCB entry_point = elf_info.entry
  5. Set TCB user_stack_top = configured stack address
  6. Set TCB state = Ready
```

#### Ràng buộc Phase L4

| Ràng buộc | Lý do | Cách tuân thủ |
|---|---|---|
| NUM_TASKS = 3 cố định | No heap, static TCBs | ELF load vào 1 trong 3 slot hiện có. Không tạo slot mới |
| User stacks cố định (3×4KB) | Linker-placed | Dùng stack hiện có, ELF binary chỉ cung cấp code+data |
| Identity mapping | MMU hiện tại | ELF binary phải link ở địa chỉ trong user region |
| No heap | Parser + loader chỉ dùng static buffers | `include_bytes!` trả về `&[u8]` — no allocation |
| W^X | Bảo vệ bộ nhớ | ELF segment .text → X+R, .data → RW, không bao giờ RWX |

#### Phương án embed ELF

**Phase L4 (đơn giản)**: `include_bytes!` — ELF binary build riêng, rồi embed vào kernel. Đủ để demo concept.

**Tương lai**: Flash/ROM loader hoặc IPC-based loader — task "loader" nhận binary qua IPC rồi gọi syscall.

#### Syscall mới (tùy chọn)

| # | Tên | x7 | x0 | x1 | Mô tả |
|---|---|---|---|---|---|
| 13 | `SYS_TASK_CREATE` | 13 | task_slot (0–2) | elf_base_addr | Yêu cầu kernel load ELF vào task slot. Trả về 0=ok, <0=error |

**Lưu ý**: Syscall này **tùy chọn** cho L4. Có thể chỉ dùng kernel API (gọi trong `kernel_main`) mà chưa expose syscall. Syscall expose cho userspace khi có nhu cầu (task tạo task khác).

Nếu thêm syscall:

#### Capability mới (tùy chọn)

| Bit | Tên | Mô tả |
|---|---|---|
| 18 | `CAP_TASK_CREATE` | Quyền sử dụng `SYS_TASK_CREATE` |

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `src/kernel/elf.rs` | Sửa | Thêm `load_elf()` function — copy segments, setup page permissions |
| `src/arch/aarch64/mmu.rs` | Sửa | Thêm `map_user_code_page()` nếu cần map page mới cho loaded ELF |
| `src/main.rs` | Sửa | Thay hardcode task entry bằng ELF load: `include_bytes!` + `parse_elf64` + `load_elf` |
| `src/kernel/sched.rs` | Sửa nhẹ | Thêm `create_task_from_elf()` — wrapper khởi tạo TCB từ ElfInfo |
| `src/arch/aarch64/trap.rs` | Sửa (nếu syscall) | Thêm case 13 → `handle_task_create()` |
| `src/kernel/cap.rs` | Sửa (nếu syscall) | Thêm `CAP_TASK_CREATE = 1 << 18` |
| `tests/host_tests.rs` | Sửa | Tests cho `load_elf`, `create_task_from_elf`, cap check |

#### Checkpoint L4

```
[AegisOS] ELF loader ready
[AegisOS] task 1 loaded from ELF (entry=0x...)
```

---

### L5 — Demo: Tách Task Thành Binary Riêng

#### Khái niệm

Proof of concept: tách `client_entry` ra khỏi kernel binary, build thành ELF64 riêng, embed vào kernel image, load bằng ELF loader. UART driver và idle vẫn hardcode (đơn giản).

Mục đích: chứng minh separation hoạt động end-to-end.

#### Bước thực hiện

1. Tạo thư mục `user/client/` — chứa `main.rs` + `linker.ld` riêng cho user task
2. Build user task thành ELF64: `cargo build --release --target aarch64-unknown-none`
3. Embed vào kernel: `include_bytes!("../../user/client/target/.../client")`
4. Kernel parse + load + schedule

#### Cấu trúc thư mục mới

```
aegis/
├── src/                    ← kernel source
│   ├── arch/aarch64/
│   ├── kernel/
│   ├── platform/
│   ├── main.rs
│   └── lib.rs
├── user/                   ← MỚI: user task binaries
│   └── client/
│       ├── Cargo.toml      ← no_std, no_main, panic=abort
│       ├── src/
│       │   └── main.rs     ← client_entry + syscall wrappers (copy hoặc shared)
│       └── linker.ld       ← vaddr ở user region
├── linker.ld               ← kernel linker script
└── Cargo.toml              ← workspace (kernel + user tasks)
```

#### Ràng buộc

- User task binary phải link ở địa chỉ trong user-accessible region
- User task **không thể gọi kernel functions** trực tiếp — chỉ qua syscall
- Syscall wrappers phải copy/duplicated hoặc shared qua library crate

#### File cần thay đổi

| File | Thao tác | Chi tiết |
|---|---|---|
| `user/client/Cargo.toml` | Tạo mới | no_std crate cho user task |
| `user/client/src/main.rs` | Tạo mới | client_entry + syscall wrappers |
| `user/client/linker.ld` | Tạo mới | User-space address layout |
| `Cargo.toml` (root) | Sửa | Workspace members |
| `src/main.rs` | Sửa | Dùng `include_bytes!` + ELF load cho client task |

#### Checkpoint L5

```
[AegisOS] client task loaded from ELF binary
```

UART output: task client vẫn hoạt động như trước (IPC, heartbeat, v.v.) nhưng giờ chạy từ ELF loaded binary.

---

### L6 — Tests + Tổng hợp

#### Host unit tests mới (ước lượng: ~25 tests)

| # | Test case | Sub-phase | Mô tả |
|---|---|---|---|
| 1 | `test_arch_module_exports` | L1 | Verify `arch::current::gic` accessible |
| 2 | `test_kernel_module_exports` | L1 | Verify `kernel::ipc`, `kernel::cap` accessible |
| 3 | `test_platform_constants` | L1 | GICD_BASE, UART0, etc. đúng giá trị |
| 4 | `test_use_paths_unchanged` | L1 | Public API unchanged after move |
| 5 | `test_trapframe_in_context` | L2 | TrapFrame from `arch::current::context` |
| 6 | `test_scheduler_portable` | L2 | `kernel::sched::schedule()` works without arch |
| 7 | `test_grant_portable` | L2 | `kernel::grant` logic without mmu calls |
| 8 | `test_irq_portable` | L2 | `kernel::irq` logic without gic calls |
| 9 | `test_cfg_blocks_reduced` | L2 | Verify <20 cfg blocks remain |
| 10 | `test_elf_parse_valid` | L3 | Parse valid ELF64 binary |
| 11 | `test_elf_parse_bad_magic` | L3 | Reject non-ELF |
| 12 | `test_elf_parse_not_64bit` | L3 | Reject ELF32 |
| 13 | `test_elf_parse_wrong_arch` | L3 | Reject x86_64 ELF |
| 14 | `test_elf_parse_not_exec` | L3 | Reject shared library (ET_DYN) |
| 15 | `test_elf_parse_too_small` | L3 | Reject truncated file |
| 16 | `test_elf_parse_segments` | L3 | Correct PT_LOAD extraction |
| 17 | `test_elf_parse_too_many_segments` | L3 | Reject >4 PT_LOAD |
| 18 | `test_elf_parse_segment_bounds` | L3 | Reject out-of-bounds segment |
| 19 | `test_elf_parse_no_segments` | L3 | Handle 0 PT_LOAD gracefully |
| 20 | `test_elf_entry_point` | L3 | Entry point correctly extracted |
| 21 | `test_load_elf_segments` | L4 | Segments copied to correct vaddr |
| 22 | `test_load_elf_bss_zeroed` | L4 | memsz > filesz → zero filled |
| 23 | `test_create_task_from_elf` | L4 | TCB initialized correctly |
| 24 | `test_cap_task_create` | L4 | `cap_for_syscall(13, _) == CAP_TASK_CREATE` (nếu thêm syscall) |
| 25 | `test_elf_wxn_permissions` | L4 | PF_X → exec page, PF_W → write page, never RWX |

#### QEMU boot checkpoints mới

| # | Checkpoint UART output |
|---|---|
| 19 | `[AegisOS] arch separation: complete` |
| 20 | `[AegisOS] ELF64 parser ready` |
| 21 | `[AegisOS] ELF loader ready` |

---

## Ràng buộc & Rủi ro

### Ràng buộc kỹ thuật

| # | Ràng buộc | Lý do | Cách tuân thủ |
|---|---|---|---|
| 1 | TrapFrame = 288 bytes | ABI-locked | Di chuyển vào `arch/aarch64/context.rs` nhưng KHÔNG thay đổi layout |
| 2 | No heap | Bất biến AegisOS | ELF parser dùng static `[Option<ElfSegment>; 4]`. `include_bytes!` = compile-time, no alloc |
| 3 | No FP/SIMD | CPACR_EL1.FPEN=0 | ELF loader tính toán integer only. Loaded ELF cũng không được dùng FP |
| 4 | NUM_TASKS = 3 | Static allocation | ELF load vào slot hiện có. Không tạo slot mới trong Phase L |
| 5 | W^X | Không có page vừa W vừa X | ELF segments: PF_X → AP execute, PF_W → AP write, tách biệt |
| 6 | Identity mapping | MMU hiện tại | User ELF binary phải link ở địa chỉ identity-mapped (trong user region) |
| 7 | Linker script tightly coupled | `linker.ld` → `mmu.rs` | Giữ kernel `linker.ld` ở root (hoặc `platform/`), user task có linker.ld riêng |
| 8 | Host tests must pass | CI requirement | Mỗi bước refactor phải giữ 162 tests pass. Thêm ~25 tests mới |
| 9 | Syscall ABI 0–12 giữ nguyên | Backward compatibility | ELF loader thêm syscall #13 (tùy chọn), không thay đổi 0–12 |
| 10 | Global_asm includes | `boot.s` include via `global_asm!` | Cần cập nhật path trong `main.rs` khi move `boot.s` |

### Rủi ro

| # | Rủi ro | Xác suất | Ảnh hưởng | Giảm thiểu |
|---|---|---|---|---|
| 1 | **L2 refactor break build** — di chuyển nhiều file đồng thời | 🔴 Cao | 🔴 Cao | Di chuyển 1 module/lần → build → test → commit → tiếp. Ước tính 6–8 bước nhỏ |
| 2 | **`use` path cascade** — đổi 1 module → 10 file cần update import | 🔴 Cao | 🟡 Trung bình | Dùng `pub use` re-exports tại `lib.rs` để giữ public API ổn định |
| 3 | **Host test break** — `cfg` gate bị thiếu sau refactor | 🟡 Trung bình | 🟡 Trung bình | Chạy `cargo test` sau mỗi bước. CI sẽ catch |
| 4 | **ELF parser edge cases** — binary không chuẩn, segment overlap | 🟡 Trung bình | 🟢 Thấp | Strict validation, reject non-conforming ELF, 10 unit tests |
| 5 | **User task binary link address** — vaddr conflict với kernel | 🟡 Trung bình | 🔴 Cao | User linker.ld đặt vaddr ở region khác kernel. Per-task address space (Phase H) đã hỗ trợ |
| 6 | **`include_bytes!` tăng kernel binary size** | 🟢 Thấp | 🟢 Thấp | User task nhỏ (~1KB). Kernel binary tăng ~1KB — negligible |
| 7 | **Cargo workspace complexity** | 🟡 Trung bình | 🟡 Trung bình | Workspace members build riêng, chỉ kernel embed final binary |
| 8 | **QEMU checkpoint strings thay đổi** | 🟢 Thấp | 🟡 Trung bình | Cập nhật `qemu_boot_test.sh` + `.ps1` đồng bộ |
| 9 | **Refactor lâu hơn dự kiến** — estimated ~800 dòng code moved | 🟡 Trung bình | 🟡 Trung bình | L1 (1–2h), L2 (3–5h), L3 (1–2h), L4+L5 (2–3h), L6 (1h). Tổng ~8–13h |

---

## Backward Compatibility

### Thay đổi breaking

| Thay đổi | Ảnh hưởng | Giải pháp |
|---|---|---|
| Module paths thay đổi (`sched` → `kernel::sched`, `gic` → `arch::current::gic`) | `tests/host_tests.rs` + any external code | `lib.rs` re-export giữ old paths hoặc update tests |
| File cũ bị xóa (`src/exception.rs`, `src/mmu.rs`, etc.) | Git history fragmented | Git `mv` để giữ history. Commit message rõ ràng |
| Thêm `user/` directory | Build system | Workspace config, không ảnh hưởng kernel build |
| Thêm UART checkpoint strings | QEMU test | Cập nhật test scripts |

### Không thay đổi (backward compatible)

- Syscall ABI 0–12 giữ nguyên
- TrapFrame 288 bytes giữ nguyên layout
- Capability bits 0–17 giữ nguyên
- Memory layout (linker.ld) KHÔNG thay đổi
- QEMU boot output giữ 18 checkpoints cũ + thêm 3 mới
- IPC, notification, grant, IRQ routing behavior giữ nguyên

---

## Test Plan

### Host unit tests mới (ước lượng: ~25 tests)

Xem chi tiết tại [L6 — Tests](#l6--tests--tổng-hợp).

### QEMU boot checkpoints

Sau Phase L: **21 checkpoints** (18 cũ + 3 mới).

---

## Thứ tự triển khai

| Bước | Sub-phase | Phụ thuộc | Thời gian ước tính | Checkpoint xác nhận | Risk |
|---|---|---|---|---|---|
| 1 | **L1: Module Structure** | Không | 1–2h | `[AegisOS] arch separation: phase L1` + 162 tests + 18 QEMU checkpoints | 🟡 Trung bình |
| 2 | **L2: Tách Arch Code** | L1 | 3–5h | `[AegisOS] arch separation: complete` + 162 tests + 18 checkpoints | 🔴 Cao |
| 3 | **L3: ELF Parser** | L1 (không cần L2) | 1–2h | + `[AegisOS] ELF64 parser ready` + 10 host tests | 🟢 Thấp |
| 4 | **L4: ELF Loader** | L2 + L3 | 2–3h | + `[AegisOS] ELF loader ready` + 5 host tests | 🟡 Trung bình |
| 5 | **L5: Demo Binary** | L4 | 1–2h | + `client task loaded from ELF binary` | 🟡 Trung bình |
| 6 | **L6: Tests** | L1-L5 | 1h | ~187 host tests + 21 QEMU checkpoints | 🟢 Thấp |

**Tổng ước tính: 9–15 giờ** (lớn nhất trong các phase, do refactor chiếm ~60%).

**Lưu ý quan trọng**: L3 (ELF parser) **có thể làm song song** với L2 (tách arch) vì ELF parser là module mới, không phụ thuộc arch refactoring. Đề xuất:
- Track A: L1 → L2 (refactor)
- Track B: L3 (ELF parser, song song với L2)
- Merge: L4 → L5 → L6

---

## Cấu trúc thư mục cuối cùng (sau Phase L)

```
aegis/
├── src/
│   ├── arch/
│   │   ├── mod.rs                  ← cfg(aarch64) pub use aarch64 as current
│   │   └── aarch64/
│   │       ├── mod.rs              ← re-export all
│   │       ├── boot.s              ← entry, EL2→EL1, BSS, MMU enable
│   │       ├── vectors.rs          ← global_asm! vector table + SAVE/RESTORE
│   │       ├── trap.rs             ← dispatch_sync, dispatch_irq, handle_svc, faults
│   │       ├── context.rs          ← TrapFrame (288B), init_context()
│   │       ├── mmu.rs              ← page tables, map/unmap, TLB, mmu_exports
│   │       ├── timer.rs            ← init(), rearm(), CNTP register access
│   │       ├── gic.rs              ← GICv2 driver
│   │       ├── uart.rs             ← PL011 UART write
│   │       ├── syscall.rs          ← 13 SVC wrappers
│   │       └── bootstrap.rs        ← TTBR0 + eret → EL0
│   │
│   ├── kernel/
│   │   ├── mod.rs                  ← re-export all
│   │   ├── sched.rs               ← TCB, schedule(), epoch, budget, watchdog
│   │   ├── ipc.rs                 ← 4 endpoints, sync send/recv/call
│   │   ├── cap.rs                 ← 19 capability bits (0–18)
│   │   ├── elf.rs                 ← ELF64 parser + loader (MỚI)
│   │   ├── grant.rs               ← shared memory grants
│   │   ├── irq.rs                 ← IRQ routing → notification
│   │   └── timer.rs               ← TICK_COUNT, tick_handler logic
│   │
│   ├── platform/
│   │   ├── mod.rs
│   │   └── qemu_virt.rs           ← MMIO addresses, DeviceInfo, memory map
│   │
│   ├── main.rs                    ← kernel_main, task entries/ELF load, panic
│   └── lib.rs                     ← module tree
│
├── user/
│   └── client/                    ← User task binary (MỚI)
│       ├── Cargo.toml
│       ├── src/main.rs
│       └── linker.ld
│
├── tests/
│   ├── host_tests.rs             ← ~187 tests
│   ├── qemu_boot_test.sh         ← 21 checkpoints
│   └── qemu_boot_test.ps1        ← 21 checkpoints
│
├── linker.ld                      ← kernel linker script
├── Cargo.toml                     ← workspace
└── docs/...
```

---

## Tổng kết chi phí

| Metric | Giá trị |
|---|---|
| File mới | ~15 (`arch/aarch64/` 7 files, `kernel/` 2 files, `platform/` 2 files, `user/client/` 3 files, `kernel/elf.rs` 1 file) |
| File di chuyển | ~8 (boot.s, gic.rs, uart.rs, ipc.rs, cap.rs, sched.rs, grant.rs, irq.rs) |
| File xóa (sau tách) | ~6 (exception.rs, mmu.rs, timer.rs cũ, sched.rs cũ, grant.rs cũ, irq.rs cũ) |
| Dòng code MỚI (ước lượng) | ~300 (ELF parser ~150, loader ~50, module glue ~50, user task ~50) |
| Dòng code DI CHUYỂN | ~3,000 (gần toàn bộ kernel, restructured) |
| Bộ nhớ thêm | ~2KB (ELF binary embedded via include_bytes) |
| Tests mới | ~25 |
| Tổng tests | ~187 (162 + 25) |
| Syscalls mới | 0–1 (SYS_TASK_CREATE tùy chọn) |
| Tổng syscalls | 13–14 |
| Capability bits mới | 0–1 (CAP_TASK_CREATE tùy chọn) |
| Tổng capability bits | 18–19/64 |
| QEMU checkpoints mới | 3 |
| Tổng checkpoints | 21 |
| Thời gian ước tính | 9–15 giờ |

---

## Syscall ABI sau Phase L (nếu thêm SYS_TASK_CREATE)

| # | Tên | Mô tả | Mới? |
|---|---|---|---|
| 0 | SYS_YIELD | Nhường CPU | |
| 1 | SYS_SEND | Gửi IPC | |
| 2 | SYS_RECV | Nhận IPC | |
| 3 | SYS_CALL | Send+Recv atomic | |
| 4 | SYS_WRITE | Ghi UART | |
| 5 | SYS_NOTIFY | Gửi notification | |
| 6 | SYS_WAIT_NOTIFY | Chờ notification | |
| 7 | SYS_GRANT_CREATE | Tạo grant | |
| 8 | SYS_GRANT_REVOKE | Thu hồi grant | |
| 9 | SYS_IRQ_BIND | Đăng ký IRQ | |
| 10 | SYS_IRQ_ACK | ACK IRQ | |
| 11 | SYS_DEVICE_MAP | Map MMIO | |
| 12 | SYS_HEARTBEAT | Watchdog heartbeat | |
| **13** | **SYS_TASK_CREATE** | **Load ELF vào task slot** | **✅ TÙY CHỌN** |

---

## Capability Bitmap sau Phase L

```
Bit  0–17:  Giữ nguyên từ Phase K (18 bits)
Bit 18:     CAP_TASK_CREATE      ← MỚI (tùy chọn, L4)
Bit 19–63:  Reserved (45–46 bits còn trống)
```

---

## Tham chiếu tiêu chuẩn an toàn

| Tiêu chuẩn | Điều khoản | Yêu cầu liên quan |
|---|---|---|
| **DO-178C** §6.3 | Modular design | Tách `arch/` + `kernel/` = clean module boundaries. Mỗi module có interface rõ ràng → dễ verify độc lập |
| **DO-178C** §6.3.3 | Partitioning — Spatial & SW | ELF loader tách application khỏi kernel binary → independent development & verification |
| **DO-178C** §6.6 | Traceability | Module structure giúp trace requirement → module → test dễ hơn (module boundary = trace point) |
| **IEC 62304** §5.3.1 | Software Architecture — Decomposition | Tách arch/kernel/platform = 3-tier decomposition. Mỗi tier là separate software item |
| **IEC 62304** §5.3.5 | Software Architecture — Interfaces | Arch interface (`arch::current::*`) = documented interface giữa platform-dependent và platform-independent |
| **IEC 62304** §5.4 | Detailed Design | Module-level separation cho phép detailed design document per module |
| **ISO 26262** Part 6 §7.4.1 | Design principles — Modularity | `arch/` tách biệt HAL (Hardware Abstraction Layer) → ISO 26262 khuyến nghị HAL pattern |
| **ISO 26262** Part 6 §7.4.5 | Freedom from interference | ELF loader = separate binary → spatial separation giữa kernel và application |
| **ISO 26262** Part 8 §9 | Software tool qualification | Tách arch → dễ dàng tool-qualify từng layer riêng |

---

## So sánh trước/sau Phase L

| Khía cạnh | Trước (Phase K) | Sau (Phase L) |
|---|---|---|
| Cấu trúc thư mục | 13 file phẳng trong `src/` | `arch/` + `kernel/` + `platform/` + `user/` |
| Inline asm | Rải trong 5 file | Tập trung trong `arch/aarch64/` |
| `cfg(not(test))` blocks | ~50 blocks rải 10 file | ~15 blocks tập trung ở ranh giới arch/kernel |
| MMIO addresses | Hardcode trong 5 file | Tập trung trong `platform/qemu_virt.rs` |
| Task loading | Hardcode trong kernel binary | ELF64 parse + load |
| Portability | Chỉ AArch64 | AArch64. Cấu trúc sẵn cho RISC-V |
| Module independence | Không rõ ràng | Rõ ràng: arch / kernel / platform |
| Safety compliance | Partial (modularity gap) | Đáp ứng DO-178C §6.3, IEC 62304 §5.3, ISO 26262 Part 6 §7.4.1 |

---

## Bước tiếp theo đề xuất

1. [x] **Review kế hoạch** → phản hồi/chỉnh sửa (đặc biệt L2 scope và L4 syscall quyết định)
2. [x] **Triển khai L1** (Module Structure) — ✅ 162 tests + 19 QEMU checkpoints
3. [x] **Triển khai L2** (Tách Arch Code) — ✅ 162 tests + 20 QEMU checkpoints
4. [x] **Triển khai L3** (ELF Parser) — ✅ 174 tests + 21 QEMU checkpoints
5. [x] **Triển khai L4** (ELF Loader) — ✅ 183 tests + 23 QEMU checkpoints
6. [x] **Triển khai L5** (Demo Binary) — ✅ 183 tests + 25 QEMU checkpoints
7. [x] **Triển khai L6** (Tests) — ✅ 189 tests + 25 QEMU checkpoints
8. [x] **Viết blog #12** — ✅ giải thích arch separation + ELF loading cho học sinh lớp 5
9. [x] **Chạy test suite** — ✅ 189 host tests + 25 QEMU checkpoints (2026-02-12)
10. [x] **Cập nhật README.md** — ✅ reflect Phase A–L stats (189 tests, 25 checkpoints, 13 syscalls, 12 blogs)
11. [x] **Cập nhật `copilot-instructions.md`** — ✅ reflect cấu trúc mới arch/kernel/platform
