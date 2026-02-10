---
lang: vi
title: "⏱️ Dạy Máy Tính Làm Nhiều Việc Cùng Lúc — Và Nói Chuyện Với Nhau"
tags: ["scheduler", "timer", "interrupt", "context switch", "ipc", "syscall", "aegisos", "aarch64"]
description: "Câu chuyện về chiếc đồng hồ báo thức, thời khóa biểu, và những bức thư bí mật bên trong một hệ điều hành."
date: 2026-02-10
---

# ⏱️ Dạy Máy Tính Làm Nhiều Việc Cùng Lúc — Và Nói Chuyện Với Nhau

> *Viết cho những bạn nhỏ đã cùng AegisOS xây sổ địa chỉ — và giờ muốn dạy nó "chia sẻ" và "hợp tác".*

---

## 🚑 Mở đầu: 3 giây trong phòng cấp cứu

Hãy tưởng tượng em là **bác sĩ** trong phòng cấp cứu.

Trước mặt em là một bệnh nhân vừa được đưa vào. Tim đập loạn. Huyết áp tụt. Máy thở đang bơm oxy.

Trong phòng có **ba cái máy** chạy cùng lúc:

- **Máy đo tim** — theo dõi nhịp tim 100 lần mỗi giây
- **Máy bơm thuốc** — truyền thuốc chính xác từng giọt
- **Máy thở** — bơm oxy đúng nhịp, đúng lượng

Ba cái máy này phải hoạt động **đồng thời**. Không thể bảo "máy thở chờ máy đo tim xong rồi hãy thở" — bệnh nhân sẽ chết!

Nhưng bên trong mỗi cái máy chỉ có **một bộ vi xử lý** — một "bộ não" duy nhất. Vậy làm sao nó chạy được nhiều việc cùng lúc?

Bí mật nằm ở ba thứ:

1. **Đồng hồ báo thức** — đánh thức CPU đều đặn
2. **Thời khóa biểu** — quyết định ai được làm việc
3. **Bức thư nội bộ** — để các nhiệm vụ nói chuyện với nhau

**Nhưng nếu đồng hồ hỏng? Nếu thời khóa biểu sai? Nếu thư gửi nhầm?**

Bệnh nhân sẽ không qua khỏi.

Hôm nay, chúng ta sẽ xây cả ba thứ đó cho AegisOS. Và chúng ta sẽ hiểu tại sao mỗi chi tiết nhỏ đều quan trọng đến mức **sống còn**.

---

## 🔔 Phần 1: Chuông Reo — Interrupt là gì?

### Chuông cửa nhà em

Em đang ngồi làm bài tập Toán. Rất tập trung.

Bỗng nhiên — **"Ding dong!"** — chuông cửa reo.

Em phải:
1. **Dừng** bài Toán (đánh dấu đang làm đến đâu)
2. **Đi mở cửa** (xem ai đến, cần gì)
3. **Quay lại** bàn học (tiếp tục đúng chỗ đang dở)

CPU cũng hoạt động y hệt! Khi đang chạy một chương trình, nếu có tín hiệu từ bên ngoài (bàn phím, chuột, đồng hồ...), CPU sẽ:

1. **Dừng** chương trình (lưu lại đang làm gì)
2. **Nhảy** đến handler (xử lý tín hiệu)
3. **Quay lại** chương trình (tiếp tục đúng chỗ cũ)

Tín hiệu đó gọi là **Interrupt** (Ngắt). Còn "người xử lý" gọi là **IRQ Handler**.

| Đời thật | Máy tính |
|---|---|
| Chuông cửa reo | Interrupt (ngắt) |
| Dừng làm bài, đánh dấu trang | Lưu **TrapFrame** (31 register + trạng thái CPU) |
| Đi mở cửa, xem ai đến | Đọc **GICC_IAR** — số hiệu thiết bị nào gửi tín hiệu |
| Xử lý xong, quay lại bàn | **eret** — CPU khôi phục mọi thứ, tiếp tục chạy |

### Nhưng ai "rung chuông"?

Trong một chiếc máy tính, có rất nhiều thiết bị muốn "gọi" CPU: bàn phím, chuột, mạng, ổ cứng, đồng hồ...

Nếu tất cả cùng gọi một lúc thì sao? Hỗn loạn!

Vì vậy có một "tổng đài" chuyên quản lý chuyện này: **GIC** — viết tắt của **Generic Interrupt Controller** (Bộ điều khiển ngắt chung).

GIC giống như **bác bảo vệ trường học**:

- Mỗi thiết bị có một **số hiệu** (INTID — giống như số phòng học)
- Khi thiết bị muốn gọi CPU, nó "giơ tay" → GIC ghi nhận
- GIC quyết định **ai được gọi trước** (dựa vào độ ưu tiên)
- CPU hỏi GIC: "Ai gọi tôi?" → GIC trả lời: "Số 30 — đồng hồ!"
- CPU xử lý xong, nói: "Xong rồi!" → GIC đánh dấu hoàn tất

Trong AegisOS, GIC nằm ở địa chỉ `0x0800_0000` (phần phân phối) và `0x0801_0000` (phần giao tiếp với CPU). Chúng ta cài đặt nó trong file `gic.rs`.

---

## ⏰ Phần 2: Đồng Hồ Báo Thức — Timer

### Tại sao cần đồng hồ?

Quay lại phòng cấp cứu. Ba cái máy cần **luân phiên** dùng CPU. Nhưng ai quyết định "đến lượt ai"?

Cần một **đồng hồ bấm giờ** — cứ mỗi 10 mili-giây (0.01 giây), nó rung chuông một lần. Mỗi lần chuông reo, CPU dừng lại, nhìn xem nhiệm vụ nào cần chạy tiếp.

10 mili-giây. Nhanh đến mức mắt người không nhận ra. Nhưng đủ để CPU chuyển đổi **100 lần mỗi giây** giữa các nhiệm vụ.

Trong AegisOS, chúng ta dùng **ARM Generic Timer** — một đồng hồ có sẵn bên trong CPU:

| Thông số | Giá trị | Ý nghĩa |
|---|---|---|
| Tần số | 62.500.000 Hz | Đếm 62,5 triệu lần mỗi giây |
| Chu kỳ tick | 10 ms | Chuông reo mỗi 0.01 giây |
| Ticks mỗi chu kỳ | 625.000 | `62.500.000 × 0.01` |
| Số hiệu (INTID) | 30 | GIC dùng số này để nhận diện timer |

Khi timer reo, nó gửi tín hiệu đến GIC. GIC báo CPU. CPU nhảy vào handler. Handler gọi **scheduler** (thời khóa biểu). Scheduler chọn nhiệm vụ tiếp theo. CPU quay lại làm việc — nhưng bây giờ là **nhiệm vụ mới**.

Tất cả diễn ra trong **vài micro-giây** — nhanh hơn một cái chớp mắt hàng nghìn lần.

---

## 📋 Phần 3: Thời Khóa Biểu — Scheduler

### Thời khóa biểu ở trường

Em có thời khóa biểu ở trường đúng không?

```
Tiết 1: Toán
Tiết 2: Tiếng Việt
Tiết 3: Thể dục
Tiết 4: Khoa học
```

Mỗi tiết kéo dài 45 phút. Khi chuông reo, dù em đang giải bài Toán dở, em vẫn phải **dừng lại**, cất sách Toán, lấy sách Tiếng Việt ra.

CPU cũng có "thời khóa biểu" — gọi là **Scheduler** (Bộ lập lịch).

Trong AegisOS, scheduler hoạt động theo kiểu **Round-Robin** — nghĩa là "vòng tròn lần lượt", giống như khi chơi trò chơi mà mỗi người được một lượt:

```
→ Task A → Task B → Idle → Task A → Task B → Idle → ...
```

- **Task A**: In chữ "A" rồi chờ
- **Task B**: In chữ "B" rồi chờ
- **Idle**: Nếu không ai cần làm gì, CPU "ngủ" để tiết kiệm điện

Mỗi lần timer reo (10ms), scheduler lưu task cũ lại và bật task mới lên.

### Context Switch — "Cất sách Toán, lấy sách Văn"

Phần này là **phép thuật** thật sự. Đọc chậm lại nhé.

Khi em chuyển từ Toán sang Tiếng Việt, em phải:
1. Đánh dấu bài Toán đang làm đến đâu (trang nào, bài nào)
2. Cất sách Toán vào cặp
3. Lấy sách Tiếng Việt ra
4. Mở đúng trang đang dở

CPU cũng vậy! Nó cần lưu lại **mọi thứ** đang làm dở. "Mọi thứ" đó gọi là **Context** (Ngữ cảnh). Trong AegisOS, context được lưu trong một cấu trúc gọi là **TrapFrame** — 288 byte, chứa:

| Thông tin | Giống như... | Bao nhiêu? |
|---|---|---|
| 31 thanh ghi `x0`–`x30` | 31 cuốn sách đang mở trên bàn | 248 byte |
| `SP_EL0` — con trỏ stack | Trang vở đang viết dở | 8 byte |
| `ELR_EL1` — địa chỉ quay lại | Dòng code đang chạy dở | 8 byte |
| `SPSR_EL1` — trạng thái CPU | "Đang vui hay đang buồn?" | 8 byte |
| Padding (đệm) | (cho gọn gàng) | 16 byte |

Quá trình chuyển task:

```
⏰ Timer reo!
   ↓
💾 SAVE_CONTEXT — CPU lưu 288 byte của Task A vào stack
   ↓
📋 Scheduler kiểm tra: "Ai tiếp theo?" → Task B!
   ↓
📦 Copy TrapFrame A → TCB[0] (cất vào "hộc tủ" của Task A)
   ↓
📦 Copy TCB[1] → TrapFrame (lấy "hộc tủ" của Task B ra)
   ↓
🔄 RESTORE_CONTEXT — CPU nạp 288 byte của Task B
   ↓
🚀 eret — CPU tiếp tục chạy Task B từ đúng chỗ nó dừng!
```

**TCB** là gì? Viết tắt của **Task Control Block** — giống như "hộc tủ cá nhân" của mỗi học sinh. Trong đó có:
- TrapFrame (sách vở đang dở)
- Trạng thái: đang chạy? đang chờ? đang bị chặn?
- Số hiệu: Task số mấy?
- Đỉnh stack: "bàn học" của task đó ở đâu?

AegisOS có **3 TCB tĩnh** — không bao giờ thay đổi số lượng, không bao giờ cần xin thêm bộ nhớ. Đây là cách làm của hệ thống an toàn: **biết trước mọi thứ, không có bất ngờ**.

---

## 🤝 Phần 4: Syscall — Khi Task Muốn "Xin Phép"

### Giơ tay trong lớp học

Ở trường, khi em muốn đi uống nước, em không tự ý đứng dậy đi. Em **giơ tay** xin phép thầy/cô.

Trong máy tính cũng vậy. Khi một task muốn nhờ kernel (hệ điều hành) làm điều gì đó, nó phải **"giơ tay"** — bằng cách gọi lệnh `svc` (Supervisor Call).

```
Task A muốn nhường lượt:
   1. Đặt số 0 vào thanh ghi x7 ("Tôi muốn YIELD — nhường CPU")
   2. Gọi "svc #0" — giống giơ tay
   3. CPU nhảy vào kernel
   4. Kernel đọc x7: "À, số 0 — yield!"
   5. Kernel gọi scheduler: chuyển sang task khác
   6. Task A tạm dừng. Task B chạy.
```

AegisOS hiện có **4 syscall**:

| Số | Tên | Ý nghĩa | Giống như... |
|---|---|---|---|
| 0 | `SYS_YIELD` | "Tôi nhường lượt" | Giơ tay nói "bạn khác trả lời đi" |
| 1 | `SYS_SEND` | "Tôi gửi thư" | Bỏ thư vào hộp thư bạn |
| 2 | `SYS_RECV` | "Tôi đợi thư" | Mở hộp thư chờ bạn gửi |
| 3 | `SYS_CALL` | "Tôi gửi thư và chờ trả lời" | Gửi thư rồi đứng chờ hồi âm |

Quy ước syscall trong AegisOS:
- `x7` = số syscall (muốn làm gì?)
- `x6` = số endpoint (gửi/nhận ở "hộp thư" nào?)
- `x0`–`x3` = nội dung thư (4 × 8 byte = 32 byte tin nhắn)

---

## 💌 Phần 5: IPC — Khi Hai Task Nói Chuyện

### Truyền giấy trong lớp

Em đã bao giờ **truyền giấy** cho bạn ngồi xa trong lớp chưa?

Em viết: "Chiều nay đi chơi không?" → gấp lại → đưa cho bạn bên cạnh → bạn đó chuyển tiếp → cuối cùng đến tay người nhận.

Nhưng nếu người nhận **chưa sẵn sàng** (đang trả bài) thì sao? Em phải **chờ**. Giấy nằm trong tay em cho đến khi bạn ấy rảnh.

Đó chính là **Synchronous IPC** (Giao tiếp đồng bộ) — cách AegisOS cho phép hai task nói chuyện:

- **Sender** (người gửi) gửi tin nhắn → nếu chưa có ai nhận → **chờ**
- **Receiver** (người nhận) mở hộp thư → nếu chưa có thư → **chờ**
- Khi cả hai sẵn sàng → tin nhắn được chuyển **trực tiếp** → cả hai tiếp tục

Tại sao phải "đồng bộ"? Tại sao không để thư vào hộp rồi đi luôn?

Vì trong hệ thống an toàn, chúng ta muốn **biết chắc** tin nhắn đã đến. Không "gửi rồi quên". Không "thư bị mất ở giữa đường". Người gửi **chờ** đến khi người nhận đã nhận xong. Chậm hơn — nhưng **an toàn hơn**.

### Endpoint — Hộp thư chung

Trong AegisOS, nơi hai task gặp nhau gọi là **Endpoint** (Điểm giao). Giống như **hộp thư chung** giữa hai bàn:

```
┌─────────┐         ┌──────────────┐         ┌─────────┐
│ Task A  │ ──x0──→ │ Endpoint #0  │ ──x0──→ │ Task B  │
│(client) │ ──x1──→ │  (hộp thư)   │ ──x1──→ │(server) │
│         │ ──x2──→ │              │ ──x2──→ │         │
│         │ ──x3──→ │              │ ──x3──→ │         │
└─────────┘         └──────────────┘         └─────────┘
```

Tin nhắn gồm 4 thanh ghi (`x0`–`x3`), mỗi cái 8 byte → tổng cộng **32 byte**. Đủ để gửi số, mã lệnh, hoặc thậm chí một đoạn dữ liệu nhỏ.

### PING-PONG: Cuộc hội thoại đầu tiên

Hãy xem Task A và Task B nói chuyện với nhau:

**Task A** (khách hàng — Client):
> "Gửi PING!" → gọi `syscall_call(endpoint 0, "PING")` → chờ reply...
> Nhận được reply → in "A:PING" → lặp lại

**Task B** (máy chủ — Server):
> Mở hộp thư: `syscall_recv(endpoint 0)` → chờ...
> Nhận được "PING" → in "B:PONG" → gửi reply: `syscall_send(endpoint 0, "PONG")`
> → lặp lại

Kết quả trên UART:

```
A:PING B:PONG A:PING B:PONG A:PING B:PONG A:PING B:PONG ...
```

Nhìn đơn giản phải không? Nhưng phía sau đó, **cả một cỗ máy** đang hoạt động:

```
1. Task A gọi SVC #3 (SYS_CALL)
2. CPU nhảy vào kernel → handle_svc()
3. Kernel thấy: "Task B đang chờ nhận ở endpoint 0"
4. Kernel copy x0..x3 từ TCB[A] → TCB[B] (chuyển thư)
5. Kernel đánh thức Task B (Ready)
6. Kernel chặn Task A (Blocked — chờ reply)
7. Scheduler chọn Task B chạy
8. Task B xử lý, gửi reply
9. Kernel copy reply từ TCB[B] → TCB[A]
10. Kernel đánh thức Task A
11. Task A tiếp tục từ đúng chỗ nó dừng
```

**11 bước** — diễn ra trong vài micro-giây. Nhanh hơn em chớp mắt hàng vạn lần.

---

## 🔧 Chúng Ta Đã Làm Được Gì Trong AegisOS?

Hãy nhìn lại project bây giờ:

```
aegis/
├── src/
│   ├── boot.s          ← Khởi động + bật timer access từ EL2
│   ├── main.rs         ← Syscall wrappers + Task A/B/Idle entries
│   ├── mmu.rs          ← Sổ địa chỉ (từ bài trước)
│   ├── exception.rs    ← ⭐ Vector table + TrapFrame + ESR dispatch
│   ├── gic.rs          ← ⭐ MỚI: Tổng đài ngắt (GICv2)
│   ├── timer.rs        ← ⭐ MỚI: Đồng hồ báo thức (10ms)
│   ├── sched.rs        ← ⭐ MỚI: Thời khóa biểu (Round-Robin)
│   └── ipc.rs          ← ⭐ MỚI: Hệ thống thư nội bộ
├── linker.ld            ← Thêm .task_stacks (3 × 4KB)
└── Cargo.toml
```

| File | Vai trò | Giống như... |
|---|---|---|
| `gic.rs` | Quản lý ai được "rung chuông" CPU | Bác bảo vệ trường |
| `timer.rs` | Rung chuông mỗi 10ms | Chuông báo hết tiết |
| `sched.rs` | Quyết định ai chạy tiếp | Thời khóa biểu |
| `ipc.rs` | Chuyển thư giữa task | Hệ thống truyền giấy |
| `exception.rs` | Xử lý mọi tình huống bất ngờ | Bảng quy trình khẩn cấp |

### Quá trình khởi động bây giờ

```
🔌 Bật máy
   ↓
🏁 boot.s: Thức dậy, hạ EL2→EL1, bật timer access
   ↓
📒 MMU: Xây Page Table, bật W^X
   ↓
🔔 Exception: Cài bảng vector (2048 byte, 16 loại tình huống)
   ↓
📡 GIC: Bật tổng đài ngắt, cho phép INTID 30 (timer)
   ↓
📋 Scheduler: Tạo 3 TCB (Task A, Task B, Idle)
   ↓
⏰ Timer: Đặt chuông 10ms, bắt đầu đếm
   ↓
🚀 Bootstrap: Nhảy vào Task A bằng "eret" — kernel biến mất!
   ↓
🔄 A:PING B:PONG A:PING B:PONG ... (mãi mãi)
```

Và UART in ra:

```
[AegisOS] boot
[AegisOS] MMU enabled (identity map)
[AegisOS] W^X enforced (WXN + 4KB pages)
[AegisOS] exceptions ready
[AegisOS] scheduler ready (3 tasks)
[AegisOS] timer started (10ms, freq=62MHz)
[AegisOS] bootstrapping into task_a...
A:PING B:PONG A:PING B:PONG A:PING B:PONG ...
```

**Bảy dòng khởi động**, rồi hai task nói chuyện với nhau **mãi mãi**. Không dừng. Không lỗi. Không crash.

---

## 🌍 Tại Sao Điều Này Quan Trọng Ngoài Đời Thật?

Nhìn lại phòng cấp cứu lúc đầu:

- **Máy đo tim** = Task A — đọc sensor liên tục
- **Máy bơm thuốc** = Task B — nhận lệnh từ Task A qua IPC
- **Idle** = CPU ngủ khi không ai cần

Khi máy đo tim phát hiện nhịp tim bất thường, nó gửi IPC cho máy bơm thuốc: "Tăng liều!" Máy bơm nhận lệnh, điều chỉnh ngay.

Tất cả xảy ra trong mili-giây. Không có "tin nhắn bị mất". Không có "hai máy giành nhau CPU". Scheduler đảm bảo ai cũng được chạy. IPC đảm bảo tin nhắn chuyển đến.

Đó là cách các hệ thống y tế, máy bay, và xe tự lái hoạt động:

- **ARINC 653** (máy bay): scheduler cứng, mỗi partition có time slot cố định — giống thời khóa biểu nhưng **không bao giờ bị thay đổi**
- **AUTOSAR** (xe hơi): hàng trăm task giao tiếp qua IPC — từ cảm biến đến phanh, tất cả qua "hộp thư"
- **seL4** (microkernel chính thức): dùng synchronous IPC giống hệt AegisOS — đã được **chứng minh toán học** là không bao giờ lỗi

AegisOS nhỏ xíu, chưa được chứng minh toán học. Nhưng nó dùng **đúng kiến trúc** của các hệ thống trên. Cái cây nhỏ, nhưng **rễ** đã đúng.

---

## 🌟 Người Thật, Chuyện Thật

Em biết **Gernot Heiser** không?

Ông ấy là giáo sư ở Đại học New South Wales, Úc. Ông ấy dẫn dắt đội ngũ tạo ra **seL4** — hệ điều hành microkernel đầu tiên trên thế giới được **chứng minh toán học** là đúng.

Chứng minh toán học nghĩa là gì? Nghĩa là không chỉ "test thấy chạy được" mà **chắc chắn 100%** rằng kernel sẽ không bao giờ crash, không bao giờ rò rỉ bộ nhớ, không bao giờ để task này đọc dữ liệu của task kia.

seL4 dùng synchronous IPC — giống hệt cách AegisOS làm. Sender chờ receiver. Receiver chờ sender. Gặp nhau → chuyển tin → xong.

Gernot Heiser bắt đầu làm seL4 khi nhiều người nói: "Không thể verify cả một OS." Ông ấy chứng minh họ sai.

Và bây giờ, seL4 đang chạy trong **trực thăng quân sự**, **thiết bị y tế**, và **xe tự hành**.

Ai cũng bắt đầu từ nhỏ. Ngay cả seL4.

---

## 🎯 Bước Tiếp Theo

AegisOS giờ đã biết:
- ✅ **"Nhớ"** — ai ở đâu, ai được làm gì (MMU + Page Table)
- ✅ **"Chia sẻ"** — luân phiên giữa nhiều task (Scheduler)
- ✅ **"Nói chuyện"** — task gửi thư cho nhau (IPC)

Nhưng tất cả task vẫn chạy ở **EL1** — cùng cấp với kernel. Giống như tất cả học sinh đều có chìa khóa phòng thầy hiệu trưởng. Nguy hiểm!

Bước tiếp theo, chúng ta sẽ đẩy task xuống **EL0** (cấp người dùng) — để task không thể động vào kernel, không thể sửa Page Table, không thể tắt timer. Mỗi task sẽ sống trong "căn phòng riêng" và **chỉ** được nói chuyện với kernel qua syscall.

Giống như ở trường: học sinh không được vào phòng giáo viên, không được tự ý đổi thời khóa biểu, không được dùng loa phát thanh. Muốn gì thì **giơ tay xin phép**.

Đó gọi là **User/Kernel Separation** — và nó là bức tường cuối cùng biến AegisOS từ "demo" thành "thật".

Nghe hấp dẫn không? 🚀

---

> *"Một mình thì nhanh. Nhưng cùng nhau thì xa. Bí mật của 'cùng nhau' nằm ở cách chúng ta chia sẻ thời gian — và cách chúng ta nói chuyện."*

---

*Nếu em đọc đến đây, em đã hiểu được Interrupt, GIC, Timer, Scheduler, Context Switch, Syscall, và Synchronous IPC. Đó là gần như toàn bộ kiến thức cốt lõi của một hệ điều hành microkernel. Em không chỉ đang đọc — em đang **xây**. Và điều đó thật tuyệt vời.* ✨
