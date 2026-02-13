# AegisOS
[![Deploy Static Blog](https://github.com/maithanhduyan/aegis/actions/workflows/static-page.yml/badge.svg)](https://github.com/maithanhduyan/aegis/actions/workflows/static-page.yml)
[![AegisOS CI](https://github.com/maithanhduyan/aegis/actions/workflows/ci.yml/badge.svg)](https://github.com/maithanhduyan/aegis/actions/workflows/ci.yml)

> 🇬🇧 [Read in English](../README.md)

**Microkernel AArch64 bare-metal cho hệ thống an toàn cao.**

AegisOS là một microkernel Rust `#![no_std]` chạy trên QEMU `virt` machine (Cortex-A53). Không dùng heap, không phụ thuộc thư viện bên ngoài — được xây dựng từ đầu cho những môi trường mà sự cố là không thể chấp nhận: tên lửa, thiết bị y tế, xe tự lái.

---

## ✨ Tính năng

| Tính năng | Trạng thái | Phase | Mô tả |
|---|---|---|---|
| Khởi động AArch64 | ✅ | A | Chuyển EL2 → EL1, xóa BSS, thiết lập stack |
| MMU + W^X | ✅ | B | Bảng trang identity-mapped (L1→L2→L3, 4KB pages), WXN enforced |
| GICv2 | ✅ | C | Driver bộ điều khiển ngắt (GICD + GICC) |
| Generic Timer | ✅ | C | ARM CNTP_EL0, tick 10ms, INTID 30 |
| Bộ lập lịch Preemptive | ✅ | C | 8 task tĩnh, ưu tiên + ngân sách thời gian + watchdog, chuyển ngữ cảnh qua TrapFrame |
| Tách User/Kernel | ✅ | D | Task chạy ở EL0, kernel ở EL1, cách ly bằng AP-bit |
| Cách ly lỗi | ✅ | E | Lỗi EL0 → task bị dừng + tự khởi động lại (chờ 1s), kernel tiếp tục chạy |
| IPC đồng bộ | ✅ | C | Gửi/nhận chặn trên 4 endpoint, tin nhắn 4 từ |
| Kiểm soát truy cập Capability | ✅ | G | Bitmask u64 cho mỗi task (19 bit: 0–18), kiểm tra quyền tối thiểu mọi syscall |
| Không gian địa chỉ riêng | ✅ | H | Bảng trang L3 cho mỗi task, TTBR0 gắn ASID |
| Thông báo bất đồng bộ | ✅ | I | Notify/wait bằng bitmask, không chặn |
| Chia sẻ bộ nhớ (Grants) | ✅ | J | Trang grant owner/peer, có thể thu hồi |
| Định tuyến IRQ | ✅ | J | Gắn GIC INTID → bit thông báo của task |
| Driver ở User-mode | ✅ | J | UART driver chạy ở EL0 qua MMIO map + IRQ |
| Bộ lập lịch ưu tiên | ✅ | K | 8 mức ưu tiên, ngân sách thời gian, epoch reset |
| Watchdog | ✅ | K | Giám sát heartbeat, lỗi khi hết thời gian |
| Tách kiến trúc | ✅ | L | Cấu trúc module `arch/aarch64/` + `kernel/` + `platform/` |
| ELF64 Loader | ✅ | L | Parse + tải binary ELF, W^X enforced, nhúng bằng `include_bytes!` |
| Tải đa ELF | ✅ | O | 6 slot ELF (16 KiB mỗi slot), `load_elf_to_task()`, `const_assert!` |
| libsyscall | ✅ | O | Thư viện syscall dùng chung cho mọi user binary — nguồn chính duy nhất |
| SYS_EXIT | ✅ | O | Thoát task có kiểm soát, `TaskState::Exited`, `cleanup_task_resources()` |
| Hạ tầng kiểm thử | ✅ | F–P | 250 unit test + 32 QEMU boot checkpoint + 18 chứng minh Kani |
| CI/CD | ✅ | F | GitHub Actions — host tests + QEMU integration mỗi lần push |

## 📐 Kiến trúc

```
boot.s (_start)
  │
  ├── Chuyển EL2 → EL1
  ├── Xóa BSS
  └── kernel_main()
        ├── Khởi tạo MMU (identity map, W^X)
        ├── Cài đặt vector ngoại lệ
        ├── Khởi tạo GICv2
        ├── Khởi tạo Scheduler (8 tasks, ưu tiên)
        ├── Gán Capability (19 bits)
        ├── Tải ELF (hello/sensor/logger → task 2–4)
        ├── Khởi động Timer (tick 10ms)
        └── bootstrap() ── ERET ──► uart_driver @ EL0
                                      │
              ┌───────┴───────────────────────────────────┐
              │         │         │         │         │
           task 0    task 1    task 2    task 3    task 4
         (UART drv) (client) (ELF hello)(ELF sensor)(ELF logger)
          prio=10    prio=5   prio=5    prio=5     prio=5
              │         │         │         │         │
              └─────── IPC + Notify + Grants ───────┘
                              task 7 = IDLE (wfi)
```

### Cấu trúc mã nguồn

```
src/
├── arch/
│   ├── mod.rs              # cfg(aarch64) → pub use aarch64 as current
│   └── aarch64/
│       ├── mod.rs           # Re-export tất cả module arch
│       ├── boot.s           # Điểm vào, EL2→EL1, thiết lập SP + BSS
│       ├── exception.rs     # Bảng vector, TrapFrame (288B), xử lý SVC (14 syscall)
│       ├── mmu.rs           # Bảng trang, identity map, W^X (WXN + AP bits)
│       └── gic.rs           # Driver GICv2 (GICD + GICC)
│
├── kernel/
│   ├── mod.rs               # Re-export tất cả module kernel
│   ├── cell.rs              # KernelCell<T> — wrapper an toàn cho UnsafeCell globals
│   ├── sched.rs             # Bộ lập lịch ưu tiên, 8 TCB, ngân sách, watchdog, 6 trạng thái
│   ├── ipc.rs               # IPC endpoint đồng bộ, gửi/nhận chặn
│   ├── cap.rs               # Kiểm soát truy cập capability (bitmask u64, 19 bit: 0–18)
│   ├── timer.rs             # Bộ đếm tick + logic xử lý tick
│   ├── grant.rs             # Chia sẻ bộ nhớ grant (owner/peer)
│   ├── irq.rs               # Gắn + định tuyến IRQ → thông báo
│   └── elf.rs               # Parser + loader ELF64 (không dùng heap)
│
├── platform/
│   ├── mod.rs               # Cổng module platform
│   └── qemu_virt.rs         # Địa chỉ MMIO, hằng số bản đồ bộ nhớ
│
├── main.rs                  # kernel_main(), 14 wrapper syscall, tải đa ELF
├── lib.rs                   # Gốc crate — cây module + re-export
├── exception.rs             # Stub cho host (test x86_64)
├── mmu.rs                   # Stub cho host (test x86_64)
└── uart.rs                  # PL011 UART (dual cfg: phần cứng thật + stub host)

user/                            # Workspace Cargo riêng (target aarch64-user.json)
├── Cargo.toml               # workspace = ["libsyscall", "hello", "sensor", "logger"]
├── aarch64-user.json        # Target spec dùng chung cho mọi user crate
├── libsyscall/              # Thư viện syscall dùng chung (14 wrapper, nguồn chính duy nhất)
├── hello/                   # Task EL0 → slot 0 (task 2), WRITE + YIELD
├── sensor/                  # Task EL0 → slot 1 (task 3), SEND + YIELD + HEARTBEAT
└── logger/                  # Task EL0 → slot 2 (task 4), RECV + WRITE + YIELD

tests/
├── host_tests.rs            # 250 unit test (x86_64, logic thuần)
├── qemu_boot_test.sh        # Tích hợp QEMU (Linux/CI) — 32 checkpoint
└── qemu_boot_test.ps1       # Tích hợp QEMU (Windows) — 32 checkpoint

docs/
├── blog/                    # 15 bài viết giải thích khái niệm OS (tiếng Việt, cho học sinh)
├── plan/                    # Kế hoạch các phase (A đến P)
├── standard/                # Tham chiếu DO-178C, IEC 62304, ISO 26262 + ánh xạ chứng minh FM.A-7
└── discussions/             # Bản ghi tranh luận thiết kế đa tác tử
```

## 🔧 Build & Chạy

### Yêu cầu

- **Rust nightly** với component `rust-src`
- **QEMU** với `qemu-system-aarch64`

```bash
# Toolchain Rust được cố định trong rust-toolchain.toml (nightly + rust-src)
rustup show   # xác nhận nightly đang active
```

### Build

Phase O yêu cầu build user crate trước, rồi kernel:

```bash
# 1. Build user crate (libsyscall + hello + sensor + logger)
cd user && cargo build --release -Zjson-target-spec

# 2. Build kernel (nhúng user binary qua include_bytes!)
cargo build --release -Zjson-target-spec

# Hoặc dùng script tiện lợi:
./scripts/build-all.sh       # Linux/macOS
.\scripts\build-all.ps1      # Windows PowerShell
```

Kết quả: `target/aarch64-aegis/release/aegis_os`

### Chạy trên QEMU

```bash
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a53 \
  -nographic \
  -kernel target/aarch64-aegis/release/aegis_os
```

Kết quả mong đợi:
```
[AegisOS] boot
[AegisOS] MMU enabled (identity map)
[AegisOS] W^X enforced (WXN + 4KB pages)
[AegisOS] exceptions ready
[AegisOS] scheduler ready (3 tasks)
[AegisOS] capabilities assigned
[AegisOS] timer started (10ms tick)
[AegisOS] bootstrapping into task_a (EL0)...
A:PING B:PONG A:PING B:PONG A:PING B:PONG ...
```

Nhấn `Ctrl+A`, sau đó `X` để thoát QEMU.

## 🧪 Kiểm thử

### Unit Test trên Host (250 test)

Test logic thuần chạy trên x86_64 — không cần QEMU:

```bash
# Linux
cargo test --target x86_64-unknown-linux-gnu --lib --test host_tests -- --test-threads=1

# Windows
cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1
```

| Nhóm test | Số lượng | Kiểm tra gì |
|---|---|---|
| TrapFrame Layout | 4 | Kích thước (288B), alignment, offset field khớp assembly |
| MMU Descriptors | 18 | Tổ hợp bit, bất biến W^X, quyền AP, XN, AF |
| SYS_WRITE Validation | 12 | Kiểm tra phạm vi con trỏ, ranh giới, tràn, null |
| Scheduler | 30 | Ưu tiên, round-robin, ngân sách, epoch, watchdog, lỗi/khôi phục, Exited |
| IPC | 14 | Dọn dẹp endpoint, sao chép tin nhắn, hàng đợi sender FIFO, chặn |
| Capabilities | 20 | Kiểm tra bit, ánh xạ syscall (0–13), quyền tối thiểu, CAP_EXIT |
| Notifications | 7 | Bit chờ xử lý, merge, cờ chờ, xóa khi khôi phục |
| Grants | 17 | Tạo, thu hồi, dọn dẹp, địa chỉ trang, tạo lại, cạn kiệt slot, logic thuần |
| IRQ Routing | 15 | Gắn, xác nhận, định tuyến, dọn dẹp, gắn lại, tích lũy, không trùng lặp, logic thuần |
| Không gian địa chỉ | 10 | ASID, TTBR0, base bảng trang, bảo toàn khi schedule |
| Device Map | 4 | Task/device hợp lệ/không hợp lệ, chỉ số L2 UART |
| ELF Parser | 14 | Magic, class, arch, segment, giới hạn, entry point |
| ELF Loader | 5 | Sao chép segment, BSS zero, validate, quyền W^X |
| Tải đa ELF | 17 | load_elf_to_task, const_assert, chồng chéo, giới hạn kích thước |
| Phase P Logic thuần | 9 | Hàm thuần tương đương grant/IRQ/watchdog/budget |
| L6 Integration | 6 | Module arch, export kernel, platform, tách cfg |
| Khác | 48 | Vòng đời SYS_EXIT, sender queue, hằng bảng trang, UART, logging |
| **Tổng** | **250** | |

### Tích hợp QEMU Boot (32 checkpoint)

```bash
# Linux
bash tests/qemu_boot_test.sh

# Windows (PowerShell)
.\tests\qemu_boot_test.ps1
```

| # | Checkpoint | Phase |
|---|---|---|
| 1–6 | Khởi động kernel, MMU, W^X, ngoại lệ, scheduler, capability | A–G |
| 7–9 | Bộ lập lịch ưu tiên, ngân sách thời gian, watchdog | K |
| 10–14 | Thông báo, grant, định tuyến IRQ, device MMIO, không gian địa chỉ | H–J |
| 15–16 | Tách kiến trúc L1, L2 | L |
| 17–19 | ELF parser, loader, task đã tải | L |
| 20–25 | Binary ELF, timer, bootstrap EL0, UART driver, output task ELF | A–L |
| 26–32 | Đa ELF (hello/sensor/logger), SYS_EXIT, libsyscall, IPC xuyên task | O |

### CI

GitHub Actions chạy cả hai bộ test mỗi lần push vào `main`/`develop`:
- **Host Unit Tests** — `x86_64-unknown-linux-gnu` (250 test)
- **QEMU Boot Test** — Build kernel AArch64 + kiểm tra 32 boot checkpoint
- **Kani Formal Verification** — 18 chứng minh (Docker container `aegis-dev`)

## 🗺️ Bản đồ bộ nhớ (QEMU virt)

| Địa chỉ | Vùng |
|---|---|
| `0x0800_0000` | GIC Distributor (GICD) |
| `0x0801_0000` | GIC CPU Interface (GICC) |
| `0x0900_0000` | UART0 (PL011) |
| `0x4008_0000` | Địa chỉ tải kernel (`_start`) |
| `0x4010_0000` | Vùng tải ELF (6 slot × 16 KiB) |
| Do linker đặt | `.text` → `.rodata` → `.data` → `.bss` → `.page_tables` (16KB) → `.grant_pages` (8KB) → `.task_stacks` (8×4KB) → `.user_stacks` (8×4KB) → guard page (4KB) → boot stack (16KB) |

## 🔐 Syscall ABI

| Thanh ghi | Mục đích |
|---|---|
| `x7` | Số syscall |
| `x6` | ID Endpoint (cho IPC) |
| `x0`–`x3` | Dữ liệu tin nhắn |

| # | Syscall | Mô tả | Phase |
|---|---|---|---|
| 0 | `SYS_YIELD` | Tự nguyện nhường CPU | C |
| 1 | `SYS_SEND` | Gửi tin nhắn trên endpoint | C |
| 2 | `SYS_RECV` | Nhận (chặn) từ endpoint | C |
| 3 | `SYS_CALL` | Gửi + chờ phản hồi (SEND + RECV) | C |
| 4 | `SYS_WRITE` | Ghi chuỗi ra UART | D |
| 5 | `SYS_NOTIFY` | Gửi bitmask thông báo đến task | I |
| 6 | `SYS_WAIT_NOTIFY` | Chặn cho đến khi có thông báo | I |
| 7 | `SYS_GRANT_CREATE` | Tạo grant chia sẻ bộ nhớ | J |
| 8 | `SYS_GRANT_REVOKE` | Thu hồi grant chia sẻ bộ nhớ | J |
| 9 | `SYS_IRQ_BIND` | Gắn IRQ INTID → bit thông báo | J |
| 10 | `SYS_IRQ_ACK` | Xác nhận IRQ, bật lại INTID | J |
| 11 | `SYS_DEVICE_MAP` | Ánh xạ MMIO thiết bị vào user-space | J |
| 12 | `SYS_HEARTBEAT` | Đăng ký/làm mới heartbeat watchdog | K |
| 13 | `SYS_EXIT` | Thoát task có kiểm soát (dọn dẹp + không tự khởi động lại) | O |

## 🛡️ Ràng buộc thiết kế

- **Không dùng heap.** Mọi cấp phát đều tĩnh (mảng `static mut`, linker section). Không có crate `alloc`.
- **Không FP/SIMD ở EL0.** `CPACR_EL1.FPEN = 0b01` — FP cho phép ở EL1 (compiler memcpy), bẫy ở EL0.
- **TrapFrame bị khóa ABI.** 288 byte, dùng chung giữa struct Rust và macro assembly.
- **W^X ở mọi nơi.** Không trang nào vừa ghi được vừa thực thi được.
- **Kiểm soát bằng Capability.** Mọi syscall đều được kiểm tra quyền với bitmask capability của task trước khi xử lý.

## 🔬 Xác minh hình thức (Formal Verification)

AegisOS sử dụng [Kani](https://model-checking.github.io/kani/) cho bounded model checking, cung cấp bằng chứng toán học về tính đúng đắn của logic kernel quan trọng:

- **18 chứng minh Kani** bao phủ 7 module kernel (cap, sched, ipc, mmu, grant, irq, platform)
- **Thuộc tính đã xác minh**: Logic capability, đảm bảo scheduler, giới hạn hàng đợi IPC, toàn vẹn tin nhắn, dọn dẹp hoàn chỉnh, grant không chồng chéo, định tuyến IRQ đúng, phát hiện watchdog, công bằng ngân sách
- **Ánh xạ chứng minh**: [`docs/standard/05-proof-coverage-mapping.md`](standard/05-proof-coverage-mapping.md) (DO-333 FM.A-7)

```bash
# Chạy tất cả chứng minh Kani (yêu cầu Docker container aegis-dev)
docker exec -w /workspaces/aegis aegis-dev cargo kani --tests
# Kỳ vọng: 18 harness, 18 passed, 0 failed
```

> Tài liệu kiến trúc đầy đủ: [`.github/copilot-instructions.md`](../.github/copilot-instructions.md)

## 📚 Chuỗi bài viết blog (tiếng Việt, 15 bài)

Giải thích các khái niệm hệ điều hành viết cho học sinh lớp 5 — giúp phát triển kernel trở nên dễ tiếp cận:

1. [Tại sao chúng ta cần một Hệ Điều Hành?](blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh.md)
2. [Bộ nhớ là gì và tại sao phải bảo vệ nó?](blog/02-bo-nho-la-gi-va-tai-sao-phai-bao-ve-no.md)
3. [Dạy máy tính làm nhiều việc cùng lúc](blog/03-day-may-tinh-lam-nhieu-viec-cung-luc.md)
4. [Chìa khóa và cánh cửa — Bảo vệ Kernel](blog/04-chia-khoa-va-canh-cua-bao-ve-kernel.md)
5. [Khi một task ngã, cả hệ thống không được ngã theo](blog/05-khi-mot-task-nga-ca-he-thong-khong-duoc-nga-theo.md)
6. [Làm sao biết hệ thống an toàn thật?](blog/06-lam-sao-biet-he-thong-an-toan-that.md)
7. [Giấy phép cho phần mềm — Ai được làm gì?](blog/07-giay-phep-cho-phan-mem-ai-duoc-lam-gi.md)
8. [Mỗi chương trình một bản đồ riêng](blog/08-moi-chuong-trinh-mot-ban-do-rieng.md)
9. [Chuông cửa và hàng đợi — Nói chuyện không cần chờ](blog/09-chuong-cua-va-hang-doi-noi-chuyen-khong-can-cho.md)
10. [Khi chương trình tự nói chuyện với phần cứng](blog/10-khi-chuong-trinh-tu-noi-chuyen-voi-phan-cung.md)
11. [Ai được chạy trước? Và ai canh gác?](blog/11-ai-duoc-chay-truoc-va-ai-canh-gac.md)
12. [Dọn Nhà Và Đọc Sách Mục Lục — Arch Separation & ELF Loading](blog/12-don-nha-va-doc-sach-muc-luc.md)
13. [Làm Sao Chứng Minh Phần Mềm Không Có Lỗi? — Safety Assurance](blog/13-lam-sao-chung-minh-phan-mem-khong-co-loi.md)
14. [Từ 3 Lên 8 — Và Chứng Minh Bằng Toán Học](blog/14-tu-3-len-8-va-chung-minh-bang-toan-hoc.md)
15. [Ba Chương Trình, Một Hệ Sinh Thái — Multi-ELF & User Ecosystem](blog/15-ba-chuong-trinh-mot-he-sinh-thai.md)

## 📜 Tham chiếu tiêu chuẩn an toàn

AegisOS được phát triển với nhận thức về các tiêu chuẩn an toàn công nghiệp:

- **DO-178C** — Phần mềm cho hệ thống hàng không
- **IEC 62304** — Vòng đời phần mềm thiết bị y tế
- **ISO 26262** — An toàn chức năng ô tô

Xem [docs/standard/](standard/) cho các bản tóm tắt tiếng Việt.

## 💎 Nhà tài trợ

### 🏆 Nhà tài trợ chính

<table>
  <tr>
    <td align="center">
      <a href="https://tayafood.com">
        <img src="https://tayafood.com/favicon.ico" width="80" alt="TAYAFOOD.COM" /><br />
        <b>TAYAFOOD.COM</b>
      </a>
    </td>
  </tr>
</table>

> **Cảm ơn [TAYAFOOD.COM](https://tayafood.com)** đã tin tưởng và tài trợ cho dự án AegisOS.
> Sự hỗ trợ của TAYAFOOD.COM giúp chúng tôi duy trì và phát triển một hệ điều hành mã nguồn mở an toàn, phục vụ cộng đồng nghiên cứu và giáo dục.

---

### 🤝 Trở thành nhà tài trợ

AegisOS là dự án mã nguồn mở phi lợi nhuận. Nếu bạn hoặc tổ chức của bạn muốn hỗ trợ:

| Hạng | Quyền lợi | Liên hệ |
|---|---|---|
| 🥇 **Vàng** | Logo trên README + Blog + trang docs | [Liên hệ qua GitHub Issues](https://github.com/maithanhduyan/aegis/issues) |
| 🥈 **Bạc** | Tên trên README + cảm ơn trong blog | [Liên hệ qua GitHub Issues](https://github.com/maithanhduyan/aegis/issues) |
| 🥉 **Đồng** | Tên trong danh sách cảm ơn | [Liên hệ qua GitHub Issues](https://github.com/maithanhduyan/aegis/issues) |

> ⭐ Bạn cũng có thể hỗ trợ bằng cách **star repo**, **chia sẻ dự án**, hoặc **đóng góp code**. Mọi sự giúp đỡ đều có ý nghĩa!

## 📄 Giấy phép

Dự án này dành cho mục đích giáo dục và nghiên cứu.
