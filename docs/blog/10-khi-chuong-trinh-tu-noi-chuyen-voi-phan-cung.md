---
lang: vi
title: "🔌 Khi Chương Trình Tự Nói Chuyện Với Phần Cứng"
tags: "driver, shared-memory, irq, mmio, user-mode, aegisos"
description: "Bài #10 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Shared Memory, IRQ Routing, và User-Mode Driver — làm sao để chương trình ngoài kernel tự điều khiển thiết bị?"
date: 2026-02-12
---
# 🔌 Khi Chương Trình Tự Nói Chuyện Với Phần Cứng

> *Bài #10 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Shared Memory (bộ nhớ chung), IRQ Routing (chuyển tín hiệu phần cứng), và User-Mode Driver — ba phép thuật giúp chương trình bình thường tự nói chuyện trực tiếp với thiết bị!*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2048. Em là kỹ sư phần mềm cho tàu ngầm nghiên cứu đại dương sâu.

Dưới đáy biển 3.000 mét, tàu có rất nhiều thiết bị: **camera** quan sát san hô, **cảm biến nhiệt độ**, **đèn pha**, **cánh tay robot** gắp mẫu đá... Mỗi thiết bị cần một **chương trình riêng** để điều khiển.

Nhưng có một quy tắc vàng: **kernel — bộ não trung tâm của tàu — không được viết code cho từng thiết bị**. Tại sao?

- Kernel phải **nhỏ gọn, không lỗi**. Nếu nhét tất cả vào kernel, một lỗi nhỏ ở driver camera có thể làm cả tàu mất kiểm soát!
- Mỗi thiết bị do **đội khác nhau** thiết kế. Không ai muốn sửa kernel mỗi lần thêm thiết bị mới.

Vậy làm sao? **Cho mỗi chương trình tự nói chuyện với thiết bị của mình, nhưng kernel vẫn kiểm soát ai được nói với cái gì.** Đó chính là Phase J!

---

## 📦 Phần 1: Bộ Nhớ Chung — Chia Sẻ Giấy Ghi Chú

### Vấn đề: Chương trình không thể đọc giấy của nhau

Ở bài trước, em đã biết mỗi chương trình có **bộ nhớ riêng**. Chương trình A không thể nhìn vào bộ nhớ của chương trình B — giống như hai bạn ngồi hai phòng kín, không thể liếc bài nhau.

Nhưng đôi khi hai chương trình **cần chia sẻ dữ liệu lớn**. Ví dụ: chương trình camera chụp ảnh (1 triệu pixel!) và cần gửi cho chương trình phân tích. Gửi qua IPC thì mỗi lần chỉ được 32 byte — như cố nhét bức tranh khổ lớn qua khe cửa thư!

### Giải pháp: Trang giấy chung (Grant Page)

Kernel cho phép hai chương trình **dùng chung một trang bộ nhớ** gọi là **Grant Page** (trang được cấp quyền).

```
┌─────────────┐    ┌─────────────┐
│ Chương trình │    │ Chương trình │
│   Camera     │    │  Phân tích   │
│  (chủ sở hữu)│    │  (khách)     │
└──────┬───────┘    └──────┬───────┘
       │                   │
       │   Trang chung     │
       ▼   (Grant Page)    ▼
    ╔══════════════════════╗
    ║  Dữ liệu ảnh pixel  ║
    ║  Cả hai đều đọc/ghi  ║
    ╚══════════════════════╝
```

Cách hoạt động:
1. **Camera** gọi `SYS_GRANT_CREATE(0, bạn_phân_tích)` → "Kernel ơi, cho bạn phân tích cùng dùng trang 0 với tôi!"
2. Kernel kiểm tra quyền, rồi **mở khóa trang** cho cả hai bên.
3. Camera ghi dữ liệu vào trang → Phân tích đọc được ngay lập tức!
4. Khi xong, camera gọi `SYS_GRANT_REVOKE(0)` → "Kernel ơi, khóa lại trang, không cho bạn kia đọc nữa."

### An toàn: Chủ sở hữu kiểm soát tất cả

- **Chỉ chủ mới thu hồi được.** Khách không thể tự khóa trang.
- **Nếu ai đó bị lỗi**, kernel tự động thu hồi tất cả trang chung liên quan — không để dữ liệu lỏng lẻo.
- **Tối đa 2 trang** trong AegisOS nhỏ bé của chúng ta (đủ để minh chứng ý tưởng).

---

## ⚡ Phần 2: Chuyển Tín Hiệu Phần Cứng — Chuông Báo Từ Thiết Bị

### Vấn đề: Thiết bị gõ cửa, nhưng ai nghe?

Khi **cảm biến nhiệt độ** có dữ liệu mới, nó gửi một **tín hiệu phần cứng** gọi là **IRQ** (Interrupt Request — yêu cầu ngắt). Tín hiệu này đi thẳng đến kernel.

Nhưng kernel không biết cảm biến đang nói gì! Kernel chỉ biết: "Có ai đó gõ cửa số 33." Kernel cần chuyển tiếng gõ cửa đến **đúng chương trình** đang phụ trách thiết bị đó.

### Giải pháp: Bảng đăng ký (IRQ Binding Table)

```
┌───────────────────────────────────────┐
│        Bảng đăng ký IRQ (Kernel)      │
├────────┬──────────┬───────────────────┤
│ Cửa # │ Ai nghe? │ Tín hiệu gì?     │
├────────┼──────────┼───────────────────┤
│   33   │ Task 0   │ Bít 0x01          │
│   34   │ Task 1   │ Bít 0x02          │
│  ...   │  ...     │  ...              │
└────────┴──────────┴───────────────────┘
```

Cách hoạt động:
1. Chương trình UART gọi `SYS_IRQ_BIND(33, 0x01)` → "Kernel ơi, khi cửa 33 có người gõ, hãy bật bít 0x01 cho tôi!"
2. Kernel ghi vào bảng, rồi **bật thiết bị ngắt** trong bộ điều khiển (GIC).
3. Khi thiết bị gõ cửa → kernel nhận IRQ → tra bảng → gửi **notification** đến đúng chương trình.
4. Chương trình xử lý xong → gọi `SYS_IRQ_ACK(33)` → "Kernel ơi, tôi xong rồi, mở cửa lại cho lần gõ tiếp!"

### An toàn: Không ai nghe nhầm cửa

- **Chỉ cửa ≥ 32** (tín hiệu thiết bị ngoài — SPI). Cửa nhỏ hơn là của kernel (bộ hẹn giờ, v.v.)
- **Mỗi cửa chỉ 1 người nghe.** Hai chương trình không thể tranh nhau cùng một thiết bị.
- **Nếu chương trình bị lỗi trước khi ACK**, kernel tự động mở cửa lại — không để thiết bị bị "khóa vĩnh viễn".

---

## 🗺️ Phần 3: Bản Đồ Riêng Cho Thiết Bị — Mỗi Người Một Cửa Hàng

### Vấn đề: Chương trình muốn ghi trực tiếp vào thiết bị

Trước Phase J, mọi chương trình muốn ghi ra UART (cổng in chữ) phải **nhờ kernel**:

```
Chương trình → SYS_WRITE → Kernel → UART
                  (chậm!)
```

Mỗi ký tự phải "đi vòng" qua kernel. Nếu cần ghi 1000 ký tự thì 1000 lần nhờ kernel — chậm!

### Giải pháp: Cho chương trình tự ghi (Device MMIO Mapping)

Kernel cho chương trình **một chiếc bản đồ riêng** để thấy thiết bị:

```
TRƯỚC: Tất cả dùng chung bản đồ (mọi thiết bị bị khóa)
  Task 0: [🔒 GIC] [🔒 UART] [🔒 ...]
  Task 1: [🔒 GIC] [🔒 UART] [🔒 ...]

SAU Phase J3: Mỗi task bản đồ riêng
  Task 0: [🔒 GIC] [🔓 UART] [🔒 ...]  ← UART mở cho task 0
  Task 1: [🔒 GIC] [🔒 UART] [🔒 ...]  ← UART vẫn khóa cho task 1
```

Khi chương trình gọi `SYS_DEVICE_MAP(0)` (0 = UART), kernel:
1. Kiểm tra quyền (có `CAP_DEVICE_MAP` không?)
2. Mở khóa **chỉ UART** trong bản đồ riêng của chương trình đó.
3. Bộ điều khiển ngắt (GIC) **luôn bị khóa** — không ai được phá rào!

Giờ chương trình ghi trực tiếp:
```
Chương trình → UART  (nhanh! không cần kernel!)
```

---

## 🏗️ Phần 4: Ghép Tất Cả Lại — Chương Trình Tự Điều Khiển UART!

Đây là phần kỳ diệu nhất. Chúng ta ghép **ba phép thuật** trên thành một **chương trình UART chạy ngoài kernel**:

```
╔══════════════════════════════════════════════════════╗
║  UART Driver (Task 0)              Client (Task 1)  ║
║                                                      ║
║  1. SYS_DEVICE_MAP(UART)           1. SYS_GRANT_CREATE║
║     → Mở UART trong bản đồ           → Chia sẻ trang ║
║                                       với driver      ║
║  2. DRV:ready!                                       ║
║                                    2. Ghi "J4:UserDrv"║
║  3. SYS_RECV ← ─ ─ ─ ─ ─ ─ ─ ─ ─ 3. SYS_CALL ─ ─ →║
║     Nhận địa chỉ + độ dài             (gửi địa chỉ  ║
║                                        trang chung)  ║
║  4. Đọc từ trang chung                              ║
║     Ghi trực tiếp vào UART!                         ║
║     (EL0 MMIO — không qua kernel!)                   ║
║                                                      ║
║  5. SYS_SEND("OK") ─ ─ ─ ─ ─ ─ → 5. Client mở khóa ║
║     Lặp lại từ bước 3                Lặp lại       ║
╚══════════════════════════════════════════════════════╝
```

Và đây là kết quả thật trên QEMU:

```
[AegisOS] boot
[AegisOS] DEVICE MAP: UART0 -> task 0
DRV:ready
[AegisOS] GRANT: task 1 -> task 0 (grant 0)
J4:UserDrv J4:UserDrv J4:UserDrv J4:UserDrv ...
```

Dòng `J4:UserDrv` được **chính chương trình UART (Task 0)** ghi trực tiếp vào thiết bị — **không có syscall nào!** Kernel chỉ làm trung gian lúc đầu (cấp quyền, chia sẻ bộ nhớ), rồi đứng sang một bên.

---

## 🔐 Bài Học An Toàn

Phase J thêm **5 syscall mới** (7–11), **3 module** mới (`grant.rs`, `irq.rs`, device registry), và **3 tầng bảo vệ**:

| Tầng | Cơ chế | Bảo vệ gì? |
|------|--------|-------------|
| Capability | `CAP_GRANT_CREATE`, `CAP_IRQ_BIND`, `CAP_DEVICE_MAP` | Chỉ ai có quyền mới được dùng |
| Per-task L2 | Mỗi task bản đồ thiết bị riêng | Không ai thấy thiết bị của người khác |
| Cleanup | `cleanup_task()` cho grant, IRQ, device | Lỗi ở một chỗ không lan ra |

Trong tàu ngầm nghiên cứu: nếu chương trình camera bị lỗi, kernel tự động thu hồi trang chung, mở khóa IRQ, và khởi động lại camera — **không ảnh hưởng đến cánh tay robot hay cảm biến nhiệt!**

---

## 📊 AegisOS Sau Phase J

| Thống kê | Giá trị |
|----------|---------|
| Syscalls | 12 (0–11) |
| Capability bits | 17 (0–16) |
| Page table pages | 16 (từ 13) |
| Module Rust | 10 (`cap`, `exception`, `gic`, `grant`, `ipc`, `irq`, `mmu`, `sched`, `timer`, `uart`) |
| Host tests | 135 |
| QEMU checkpoints | 15 |
| Dòng code Rust | ~2500 |

---

## 🤔 Câu Hỏi Cho Bạn Nhỏ

1. **Tại sao kernel không cho chương trình tự mở bản đồ đến GIC (bộ điều khiển ngắt)?** Gợi ý: nếu chương trình tắt GIC, điều gì xảy ra với timer và scheduler?

2. **Nếu chương trình UART bị lỗi giữa lúc ghi dữ liệu, kernel làm gì?** Gợi ý: nhìn vào `cleanup_task()` — nó chạy khi nào?

3. **Tại sao cần `SYS_IRQ_ACK` thay vì kernel tự mở lại IRQ?** Gợi ý: nếu kernel mở IRQ ngay, nhưng chương trình chưa xử lý xong thiết bị, điều gì xảy ra?

---

## 🔮 Tiếp Theo: Phase K — ???

Phase J hoàn thành bộ ba nền tảng: **bộ nhớ chung + ngắt phần cứng + thiết bị cho user-mode**. Đây là những viên gạch để xây **bất kỳ driver nào** — SPI, I²C, Ethernet, GPU...

Phase tiếp theo sẽ làm gì? Có thể là:
- **Per-task page tables cho user code** — mỗi chương trình có vùng code riêng
- **ELF loader** — load chương trình từ file thay vì hardcode trong kernel
- **Watchdog timer** — bộ canh gác tự khởi động lại nếu hệ thống treo

Hẹn gặp bạn nhỏ ở bài tiếp theo! 🚀
