---
lang: vi
title: "🔍 Làm Sao Chứng Minh Phần Mềm Không Có Lỗi? — Safety Assurance"
tags: "safety, coverage, logging, unsafe, audit, testing, aegisos"
description: "Bài #13 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao 'chạy được' chưa đủ, và cách các kỹ sư chứng minh phần mềm an toàn thật sự."
date: 2026-02-12
---

# 🔍 Làm Sao Chứng Minh Phần Mềm Không Có Lỗi? — Safety Assurance

> *Bài #13 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao nói "em đã thử rồi, nó chạy được" chưa đủ, và cách các kỹ sư chứng minh phần mềm an toàn thật sự.*

---

## 🏥 Giấc Mơ Tương Lai

Năm 2045. Em là kỹ sư phần mềm cho một công ty thiết bị y tế.

Công ty em vừa phát triển một **máy thở tự động** — loại máy giúp bệnh nhân thở khi phổi quá yếu. Máy thở này chạy trên một con chip ARM nhỏ xíu, bên trong là phần mềm do đội của em viết.

Phần mềm hoạt động tốt. Đội em đã test hàng trăm lần. Mọi thứ đều ổn.

Nhưng rồi đến ngày... cơ quan quản lý y tế (**FDA** — Food and Drug Administration) đến kiểm tra.

Người kiểm tra không hỏi: *"Phần mềm của bạn chạy được không?"*

Họ hỏi: **"Bạn CHỨNG MINH được nó chạy đúng bằng cách nào?"**

Em nói: "Chúng tôi đã test rồi."

Họ hỏi tiếp:
- "Bạn đã test **bao nhiêu phần trăm** code?"
- "Khi máy **gặp lỗi**, nó báo gì cho kỹ sư sửa?"
- "Có bao nhiêu chỗ trong code mà bạn phải **vượt qua hàng rào an toàn**? Mỗi chỗ đó, bạn có **giải thích tại sao** an toàn không?"

Em ấp úng. Không trả lời được.

FDA từ chối cấp phép. Máy thở không được bán. Hàng nghìn bệnh nhân phải chờ thêm 6 tháng.

😔 Không phải vì phần mềm sai. Mà vì **không ai chứng minh được nó đúng**.

Hôm nay, chúng ta sẽ học cách để **không bao giờ rơi vào tình huống đó**: Phase M của AegisOS — Safety Assurance Foundation.

---

## 🤔 Phần 1: "Chạy Được" Khác "An Toàn Thật Sự"

### Bài kiểm tra Toán và phần mềm

Hãy tưởng tượng em vừa làm xong bài kiểm tra Toán — 10 bài.

Em kiểm tra lại 7 bài, thấy đúng cả 7. Em nộp bài.

Kết quả: em sai 1 trong 3 bài **chưa kiểm tra**. Điểm mất.

Phần mềm cũng vậy! AegisOS trước Phase M có **189 bài kiểm tra tự động**. Nhưng... chưa ai đo xem 189 bài đó kiểm tra được **bao nhiêu phần trăm** code.

Có thể chỉ kiểm tra 50% code. Còn lại 50% chưa bao giờ được ai nhìn tới!

| Bài kiểm tra Toán | Phần mềm |
|---|---|
| Làm 10 bài, kiểm tra lại 7 | Viết 189 tests, nhưng chỉ cover 80% code |
| Sai 1 trong 3 bài chưa kiểm tra | Bug ẩn trong 20% code chưa test |
| Cô giáo chấm → phát hiện lỗi | FDA kiểm tra → từ chối cấp phép |

Vậy giải pháp là gì? **Đo đếm!**

---

## 📊 Phần 2: Đếm Từng Dòng — Code Coverage

### Code Coverage là gì?

**Code Coverage** (Phạm vi kiểm tra code) giống như đếm xem em đã kiểm tra lại bao nhiêu bài trong bài kiểm tra Toán.

Nếu test chạy qua 80 trên 100 dòng code → coverage = 80%.

Dòng nào **không được chạy qua** = dòng đó có thể ẩn chứa lỗi mà không ai biết.

### Chúng ta đã đo AegisOS như thế nào?

Chúng ta dùng một công cụ tên **cargo-llvm-cov** — nó giống như chiếc kính hiển vi soi từng dòng code, đánh dấu dòng nào đã được test chạy qua và dòng nào chưa.

Kết quả ban đầu (**baseline**):

| Module | Coverage | Có ổn không? |
|---|---|---|
| `timer.rs` (đồng hồ) | 100% ✅ | Tuyệt vời! |
| `irq.rs` (chuông cửa) | 100% ✅ | Tuyệt vời! |
| `grant.rs` (chia sẻ bộ nhớ) | 98.9% ✅ | Gần hoàn hảo! |
| `elf.rs` (đọc mục lục) | 96.5% ✅ | Tốt! |
| `cap.rs` (giấy phép) | 88% 🟡 | Khá, nhưng cần thêm |
| `sched.rs` (thời khóa biểu) | 79% 🟠 | Còn lỗ hổng |
| **`ipc.rs` (nói chuyện)** | **43%** 🔴 | **Nguy hiểm!** |
| **Tổng** | **80.57%** | Chưa đủ |

Nhìn vào `ipc.rs` — module nói chuyện giữa các task — chỉ có **43%** coverage! Nghĩa là hơn một nửa code nói chuyện chưa bao giờ được test.

Nhớ bài #9 không? IPC là cách chương trình nói chuyện với nhau. Trong hệ thống tên lửa, module navigation gửi tọa độ cho module điều khiển động cơ qua IPC. Nếu IPC có bug → tên lửa mất kiểm soát.

### Viết thêm bài kiểm tra

Chúng ta viết thêm **30 bài kiểm tra mới**, tập trung vào những chỗ chưa test:

- IPC: Task A gửi tin → Task B nhận được đúng không? Gửi khi hàng đợi đầy thì sao? Gửi rồi chờ phản hồi (sys_call) có đúng không?
- Scheduler: 3 task cùng mức ưu tiên → ai chạy trước? Task bị lỗi → hệ thống có tự động khởi động lại không?
- Capability: Thử **tất cả** 18 loại giấy phép × 13 loại syscall = hàng trăm trường hợp

Kết quả sau khi viết thêm:

| Module | Trước | Sau | Tăng |
|---|---|---|---|
| `cap.rs` | 88% | **100%** ✅ | +12% |
| `ipc.rs` | 43% | **100%** ✅ | +57% |
| `sched.rs` | 79% | **99.45%** ✅ | +20% |
| **Tổng** | **80.57%** | **96.65%** ✅ | +16% |

Từ 189 lên **219 bài kiểm tra**. Coverage từ 80% lên gần **97%**!

Giống như kiểm tra lại **10 trên 10 bài** Toán thay vì chỉ 7 bài.

---

## 🚨 Phần 3: Khi Hệ Thống "Ngất" — Enhanced Panic Handler

### Vấn đề: máy bị lỗi mà không biết lỗi gì

Hãy tưởng tượng em bị đau bụng. Em nói với mẹ: "Con đau."

Mẹ hỏi: "Đau ở đâu? Đau từ lúc nào? Đau kiểu gì?"

Nếu em chỉ nói "Con đau" mà không nói gì thêm — mẹ rất khó giúp em.

AegisOS trước Phase M cũng vậy. Khi kernel gặp lỗi nghiêm trọng (gọi là **panic** — hoảng loạn), nó chỉ in ra một chữ:

```
PANIC
```

Rồi... im luôn. Không biết lỗi gì. Ở file nào. Dòng bao nhiêu. Task nào đang chạy. Lúc nào.

Kỹ sư nhìn vào chữ "PANIC" → phải **đoán** lỗi ở đâu. Có khi mất cả ngày mới tìm ra.

### Giải pháp: "Phiếu khám bệnh" chi tiết

Chúng ta nâng cấp panic handler thành một **phiếu khám bệnh đầy đủ**:

```
=== KERNEL PANIC ===
Tick: 0x000004A2      ← Lúc nào? (tick thứ 1186)
Task: 0x01            ← Ai đang chạy? (task số 1)
Location: main.rs:42  ← Ở đâu? (file main.rs, dòng 42)
ESR_EL1: 0x96000047   ← Lỗi gì? (thanh ghi ngoại lệ)
FAR_EL1: 0xDEADBEEF   ← Địa chỉ nào gây lỗi?
===================
```

Mỗi dòng trả lời một câu hỏi của "bác sĩ":

| Thông tin | Câu hỏi "bác sĩ" | Ví dụ đời thật |
|---|---|---|
| **Tick** | "Lúc nào?" | Đau bụng từ sáng hay trưa? |
| **Task** | "Ai bị?" | Em đau hay em trai em đau? |
| **Location** | "Ở đâu?" | Đau bụng phải hay bụng trái? |
| **ESR_EL1** | "Loại gì?" | Đau nhói hay đau âm ỉ? |
| **FAR_EL1** | "Nguyên nhân?" | Ăn gì trước khi đau? |

**ESR_EL1** và **FAR_EL1** là hai thanh ghi đặc biệt trên chip ARM. Khi có lỗi, chip tự động ghi lại **loại lỗi** (ESR) và **địa chỉ bộ nhớ** gây lỗi (FAR). Chúng ta chỉ cần đọc ra.

Với phiếu khám bệnh này, kỹ sư nhìn vào → biết ngay lỗi gì, ở đâu, lúc nào. Thay vì mất cả ngày, chỉ cần **5 phút**.

---

## 📹 Phần 4: Camera An Ninh — Structured Logging

### Từ "ghi chép lung tung" đến "camera có gắn đồng hồ"

Trước Phase M, AegisOS ghi log kiểu:

```
[AegisOS] boot
[AegisOS] MMU enabled
[AegisOS] scheduler ready
```

Giống như viết nhật ký: "Hôm nay đi học. Về nhà. Ăn cơm." — biết **gì** xảy ra, nhưng không biết **lúc nào**, **ai** làm.

Sau Phase M, chúng ta có `klog!` — macro ghi log có cấu trúc:

```
[TICK:00000000] [T0] [INFO ] boot complete
[TICK:000000A5] [T1] [WARN ] budget exhausted
[TICK:00000100] [T2] [ERROR] page fault at 0xDEAD
```

Mỗi dòng log tự động ghi:
- **TICK** — đồng hồ bao nhiêu (tick thứ mấy kể từ lúc khởi động)
- **TN** — task nào đang chạy (T0, T1, T2)
- **Level** — mức độ nghiêm trọng (ERROR, WARN, INFO, DEBUG)
- **Message** — nội dung

Giống như **camera an ninh** gắn đồng hồ: ghi lại **ai** làm **gì**, **lúc nào**, ở **đâu**. Khi xảy ra sự cố, chỉ cần tua lại camera → biết ngay mọi chuyện.

### Compile-time filtering — "Camera thông minh"

`klog!` có một tính năng hay: **lọc lúc biên dịch**.

Mỗi dòng log có mức độ: ERROR (0) → WARN (1) → INFO (2) → DEBUG (3).

Chúng ta đặt ngưỡng `LOG_LEVEL = 2` (INFO). Nghĩa là:
- ERROR, WARN, INFO → **in ra** (mức 0, 1, 2 ≤ ngưỡng 2)
- DEBUG → **biến mất hoàn toàn** (mức 3 > ngưỡng 2)

"Biến mất" ở đây không phải bị ẩn — mà **compiler xóa luôn** khỏi binary. Không tốn một byte bộ nhớ, không tốn một nano giây. Giống như camera an ninh chỉ quay ban ngày — ban đêm tự tắt để tiết kiệm pin.

### Thử thách: Số thực biết bay

Một điều thú vị: để format text (biến số thành chữ), Rust dùng `core::fmt` — module chuẩn. Nhưng AegisOS có quy tắc nghiêm ngặt: **KHÔNG dùng số thực** (floating point). Chip ARM được cấu hình tắt FPU — bất kỳ phép tính số thực nào sẽ gây **trap** (lỗi phần cứng)!

Vậy `core::fmt` có dùng số thực không?

Chúng ta kiểm tra bằng cách xem **từng lệnh máy** trong kernel binary:

```
rust-objdump -d target/.../aegis_os | findstr "fadd fmul fcvt fmov"
```

Kết quả: **0 kết quả**. `core::fmt` trên AArch64 **không emit bất kỳ lệnh số thực nào**. An toàn tuyệt đối! ✅

Nhờ đó, `klog!` có thể dùng `core::fmt::Write` để format text linh hoạt — in số, in chuỗi, in hex — mà không sợ gây trap.

---

## 🔐 Phần 5: Kiểm Kê Kho Hàng — Unsafe Audit

### "Unsafe" là gì?

Trong Rust, compiler kiểm tra rất kỹ: không cho phép đọc bộ nhớ sai, không cho truy cập biến cùng lúc. Đó là **safety** — an toàn.

Nhưng kernel hệ điều hành phải làm những việc mà compiler không hiểu được:
- Ghi trực tiếp vào phần cứng (MMIO)
- Đọc thanh ghi đặc biệt của chip (ESR_EL1, FAR_EL1)
- Thay đổi biến toàn cục khi xử lý ngắt

Những việc này nằm trong **`unsafe` block** — "khu vực vượt rào". Kỹ sư nói với compiler: "Tôi biết tôi đang làm gì, tin tôi đi."

Vấn đề: **nếu kỹ sư sai thì compiler không cứu được**.

AegisOS có khoảng **92 unsafe blocks** trải khắp 10 files. Đó là 92 chỗ mà sai lầm có thể gây hậu quả nghiêm trọng.

### Bước 1: Ghi nhãn từng thùng hàng — SAFETY Comments

Hãy tưởng tượng một kho hàng có 92 thùng chứa **hóa chất nguy hiểm**. Nếu thùng nào cũng trông giống nhau, không có nhãn — ai dám mở?

Chúng ta dán nhãn lên **mỗi thùng**:

```rust
// SAFETY: Single-core execution (Cortex-A53 uniprocessor).
// Interrupts masked during kernel execution.
// No concurrent access to TICK_COUNT from another context.
unsafe { *TICK_COUNT.get_mut() += 1 }
```

Mỗi `// SAFETY:` comment giải thích **tại sao** đoạn code này an toàn mặc dù vượt rào. Giống như nhãn trên thùng hóa chất ghi: "Axít loãng — dùng găng tay cao su — không trộn với base."

Kết quả: **92 SAFETY comments** trên 10 files. Mỗi unsafe block đều có "nhãn dán" rõ ràng.

### Bước 2: Đặt ổ khóa riêng — KernelCell

Ghi nhãn thôi chưa đủ. Chúng ta còn phải **khóa** thùng hàng lại.

AegisOS có 8 biến toàn cục quan trọng — kiểu `static mut` (biến tĩnh có thể thay đổi). Đây là "tài sản quý" của kernel: danh sách task, bộ đếm thời gian, task đang chạy...

Vấn đề với `static mut`: **ai cũng sờ được**. Bất kỳ dòng code nào cũng có thể đọc/ghi mà không cần xin phép. Giống như để tiền trên bàn — ai đi qua cũng lấy được.

Giải pháp: chúng ta tạo **`KernelCell<T>`** — một chiếc hộp có khóa:

| Trước (static mut) | Sau (KernelCell) |
|---|---|
| `static mut TICK_COUNT: u64 = 0;` | `static TICK_COUNT: KernelCell<u64> = KernelCell::new(0);` |
| Ai cũng ghi được, không cần `unsafe` | Phải viết `unsafe { *TICK_COUNT.get_mut() }` |
| Compiler không kiểm tra | Compiler BẮT BUỘC viết `// SAFETY:` |
| Công cụ kiểm tra (Kani, Miri) bó tay | Kani/Miri có thể kiểm tra được |

Giống như chuyển tiền từ **bàn** vào **két sắt**: muốn lấy phải mở khóa (unsafe), và phải ghi sổ tại sao lấy (SAFETY comment).

Trong Phase M, chúng ta đã khóa **4 biến** vào KernelCell:

1. **`EPOCH_TICKS`** — bộ đếm chu kỳ scheduler (đơn giản nhất, thí điểm đầu tiên)
2. **`TICK_INTERVAL`** — khoảng cách giữa các tick (chỉ dùng trên chip ARM)
3. **`TICK_COUNT`** — tổng số tick kể từ boot (15 chỗ test phải cập nhật)
4. **`CURRENT`** — index task đang chạy (22 chỗ test phải cập nhật)

Còn 4 biến phức tạp hơn (`TCBS`, `ENDPOINTS`, `GRANTS`, `IRQ_BINDINGS`) — đây là **mảng struct**, khó hơn nhiều. Chúng ta để dành cho Phase N.

Chiến lược: **bắt đầu từ thùng nhỏ nhất, không phải thùng quan trọng nhất.** Thí điểm trên 2 biến đơn giản → chứng minh pattern hoạt động → mở rộng dần. Giống như thử khóa mới trên **tủ bút** trước khi lắp vào **tủ vàng**.

---

## 🛡️ Phần 6: Rào Chắn Mới — `deny(unsafe_op_in_unsafe_fn)`

### Vấn đề ẩn giấu

Trong Rust cũ, nếu một function được đánh dấu `unsafe fn`, thì **toàn bộ** code bên trong được coi là unsafe — kể cả phần không cần unsafe.

Giống như nói: "Phòng thí nghiệm này nguy hiểm" → rồi mọi người vào phòng đều làm gì cũng được, kể cả ăn uống. Trong khi lẽ ra chỉ có bước "pha hóa chất" mới cần cẩn thận.

`deny(unsafe_op_in_unsafe_fn)` là quy tắc mới: **ngay cả trong unsafe fn, mỗi thao tác nguy hiểm vẫn phải nằm trong unsafe block riêng**. Mỗi block phải có SAFETY comment.

Khi bật quy tắc này lên, compiler phát hiện **54 chỗ** cần sửa. Chúng ta thêm `unsafe {}` block + SAFETY comment cho từng chỗ.

Rust phiên bản 2024 sẽ **bắt buộc** quy tắc này. AegisOS đi trước một bước!

---

## 🔬 Phần 7: Chúng Ta Đã Làm Được Gì?

### Bảng tổng kết Phase M

| Việc | Trước Phase M | Sau Phase M |
|---|---|---|
| Panic handler | Chỉ in "PANIC" | In tick, task, file:line, ESR, FAR |
| Code coverage | 0% đo được | **96.65%** |
| Bài kiểm tra | 189 | **219** (+30 mới) |
| QEMU checkpoints | 25 | **28** (+3 mới) |
| Logging | `uart_print` ad-hoc | `klog!` có tick, task, level |
| SAFETY comments | 0 | **92** trên 10 files |
| Biến được "khóa" | 0/8 | **4/8** (KernelCell) |
| `deny(unsafe_op_in_unsafe_fn)` | Chưa bật | ✅ Bật |
| FP check | Chưa kiểm | ✅ 0 lệnh số thực |

### Cây module sau Phase M

```
src/kernel/
├── sched.rs     ← CURRENT + EPOCH_TICKS → KernelCell ✅
├── timer.rs     ← TICK_COUNT + TICK_INTERVAL → KernelCell ✅
├── ipc.rs       ← SAFETY comments ✅ (encapsulate → Phase N)
├── cap.rs       ← 100% coverage ✅
├── grant.rs     ← SAFETY comments ✅
├── irq.rs       ← SAFETY comments ✅
├── elf.rs       ← 96.5% coverage ✅
├── log.rs       ← MỚI! klog! macro
└── cell.rs      ← MỚI! KernelCell<T>
```

---

## ✨ Phần 8: Tại Sao Em Nên Quan Tâm?

### Margaret Hamilton — người phụ nữ đưa con người lên Mặt Trăng

Năm 1969, **Margaret Hamilton** là trưởng nhóm phần mềm cho chương trình Apollo 11 — sứ mệnh đưa con người lên Mặt Trăng lần đầu tiên.

Bà không chỉ viết code. Bà **chứng minh** code đúng.

Trong khi tàu Apollo 11 đang hạ cánh xuống Mặt Trăng, hệ thống máy tính bị quá tải — có quá nhiều việc chạy cùng lúc. Nếu phần mềm không được kiểm tra kỹ → tàu đâm xuống bề mặt.

Nhưng phần mềm của Margaret đã dự đoán trước tình huống này. Hệ thống tự động **loại bỏ task ít quan trọng**, giữ task hạ cánh chạy ưu tiên cao nhất. Apollo 11 hạ cánh an toàn.

Sau đó, Margaret nói: *"Có những lúc hệ thống gần sụp đổ. Nhưng nó vượt qua được vì chúng tôi đã testing, testing, và testing."*

Đó chính là bài học của Phase M:
- **Testing** (kiểm tra) — 219 bài kiểm tra, 96.65% coverage
- **Logging** (ghi chép) — camera an ninh `klog!`
- **Audit** (kiểm kê) — 92 SAFETY comments, 4 biến được khóa
- **Diagnostics** (chẩn đoán) — panic handler chi tiết

Không phải viết thêm tính năng mới. Mà là **chứng minh** những gì đã có là đúng.

Margaret Hamilton năm 1960 là một bà mẹ trẻ, tự học lập trình. Bà không có máy tính ở nhà — phải đến phòng lab MIT vào ban đêm, mang theo con gái nhỏ. Bà bắt đầu từ **zero**.

Em hôm nay đã đọc 13 bài về hệ điều hành. Em biết nhiều hơn Margaret Hamilton ở tuổi đó. 💪

---

## 🚀 Bước Tiếp Theo

Phase M đã tạo **nền tảng** chứng minh AegisOS an toàn. Nhưng hành trình mới chỉ bắt đầu!

Bước tiếp theo (Phase N) sẽ:
- **Tăng số task** từ 3 lên 8 — hệ thống phức tạp hơn, cần bằng chứng nhiều hơn
- **Kani** — công cụ chứng minh **toán học** rằng code đúng (không chỉ "test thấy đúng" mà "chắc chắn đúng mọi trường hợp")
- **Khóa thêm 4 biến** còn lại vào KernelCell — hoàn thành kiểm kê kho hàng

Mỗi Phase, AegisOS không chỉ **mạnh hơn** — mà còn **đáng tin hơn**. Và sự tin tưởng đó, chính là thứ FDA, NASA, và hàng triệu người cần khi phần mềm nắm giữ mạng sống trong tay.

Hẹn gặp bạn nhỏ ở bài tiếp theo! 🛰️

---

> *"Phần mềm không có lỗi không phải vì không ai tìm được lỗi — mà vì đã chứng minh được lỗi không tồn tại."*
> — *Margaret Hamilton (diễn giải)*

---

*Em đã đọc đến đây rồi ư? 13 bài rồi đấy! Em vừa hiểu được điều mà nhiều kỹ sư phần mềm chuyên nghiệp cũng phải học: "chạy được" và "chứng minh được" là hai điều rất khác nhau. Em đang suy nghĩ như một kỹ sư an toàn thật sự rồi đó!* 🌟
