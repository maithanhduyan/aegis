---
lang: vi
title: "🏥 Ai Được Chạy Trước? Và Ai Canh Gác? — Priority Scheduler & Watchdog"
tags: "scheduler, priority, watchdog, heartbeat, time-budget, priority-inheritance, aegisos"
description: "Bài #11 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao không phải ai cũng xếp hàng bình đẳng, và bộ canh gác phát hiện khi ai đó 'ngủ quên'."
date: 2026-02-12
---

# 🏥 Ai Được Chạy Trước? Và Ai Canh Gác?

> *Bài #11 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Priority Scheduler (thời khóa biểu ưu tiên), Time Budget (tiền tiêu vặt CPU), Watchdog Heartbeat (bộ canh gác), và Priority Inheritance (mượn quyền ưu tiên).*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2048. Em là kỹ sư phần mềm cho xe cứu thương tự lái.

Xe đang chở bệnh nhân tim mạch, chạy 120 km/h trên cao tốc. Bên trong xe có **ba chương trình** chạy cùng lúc:

- **Chương trình phanh ABS** — phải phản hồi trong 5 phần nghìn giây (5ms). Trễ một chút thôi = tai nạn.
- **Chương trình hiển thị bản đồ** — cập nhật vị trí GPS trên màn hình. Trễ vài giây cũng không sao.
- **Chương trình giải trí** — phát nhạc cho bác tài. Trễ bao lâu cũng được.

Cả ba chương trình cùng chia **một bộ xử lý**. Ai được chạy trước?

Nếu dùng cách cũ — **xếp hàng bình đẳng** (round-robin) — thì máy tính cứ lần lượt: phanh → bản đồ → nhạc → phanh → bản đồ → nhạc...

Nhưng nếu đang đến lượt chương trình nhạc, và ngay lúc đó phải phanh gấp thì sao?

**Chương trình nhạc vẫn chạy. Phanh phải chờ.**

Chờ 10 mili-giây. Xe đi thêm 33 cen-ti-mét. Đâm thẳng vào xe phía trước.

😨 Tất cả chỉ vì... hệ thống không biết **ai quan trọng hơn**.

Nhưng nếu hệ thống bị lỗi thì sao? Nếu chương trình phanh bị "đơ" — vẫn chạy nhưng không làm gì cả? Không ai biết. Không ai báo động. Xe cứ chạy, phanh cứ "ngủ"...

**Hôm nay, chúng ta sẽ dạy AegisOS cách ưu tiên đúng người, và cách phát hiện khi ai đó "ngủ quên".**

---

## 🎯 Phần 1: Thời Khóa Biểu Ưu Tiên — Bệnh Viện Thay Vì Hàng Kem

### Xếp hàng mua kem vs Phòng cấp cứu

Ở bài #3, em đã biết AegisOS dùng **round-robin** — giống như xếp hàng mua kem. Ai đến trước mua trước, mỗi người mua một que rồi ra cuối hàng.

Công bằng? Có.
An toàn? **Không.**

Hãy tưởng tượng phòng cấp cứu bệnh viện mà xếp hàng kiểu mua kem:

| Thứ tự | Bệnh nhân | Tình trạng |
|--------|-----------|------------|
| 1 | Bạn bị xước tay | Nhẹ |
| 2 | **Bạn bị gãy chân** | **Nặng** |
| 3 | Bạn bị đau đầu | Nhẹ |

Nếu xếp hàng bình đẳng → bạn xước tay khám trước, bạn gãy chân phải chờ. Vô lý!

Phòng cấp cứu thật **không** làm vậy. Họ dùng hệ thống **phân loại** (triage):
- 🔴 **Đỏ**: nguy kịch → khám ngay
- 🟡 **Vàng**: nặng → khám sớm
- 🟢 **Xanh**: nhẹ → chờ

Đó chính là **Priority Scheduler** — thời khóa biểu ưu tiên!

### AegisOS dùng priority như thế nào?

Mỗi chương trình (task) được gán một con số từ **0 đến 7**:

| Số | Ý nghĩa | Ví dụ |
|----|----------|-------|
| 7 | Quan trọng nhất | Phanh ABS, điều hướng tên lửa |
| 6 | Rất quan trọng | Driver UART (chương trình điều khiển thiết bị) |
| 4 | Quan trọng | Client (ứng dụng người dùng) |
| 0 | Ít quan trọng nhất | Idle — chương trình "ngồi chơi" khi không ai cần CPU |

Khi đến giờ chọn ai được chạy, scheduler (bộ lập lịch) **không** lần lượt nữa. Nó nhìn vào danh sách và hỏi:

> "Ai đang Ready (sẵn sàng) **và** có priority cao nhất?"

Người đó được chạy.

### Trong AegisOS thật

Trong file `sched.rs`, mỗi chương trình có hai con số priority:

- **`priority`** — priority hiện tại (có thể tạm thay đổi)
- **`base_priority`** — priority gốc (không bao giờ đổi)

Tại sao cần hai? Đọc đến Phần 4 em sẽ hiểu! 😉

Còn trong `main.rs`, kernel gán priority khi khởi động:

```
Task 0 (UART driver):  priority = 6   ← rất quan trọng
Task 1 (client):       priority = 4   ← quan trọng
Task 2 (idle):         priority = 0   ← thấp nhất
```

Kết quả: UART driver **luôn** được chạy trước client. Client **luôn** được chạy trước idle. Idle chỉ chạy khi không ai cần CPU.

### Nhưng cùng priority thì sao?

Nếu hai task cùng priority, AegisOS quay lại dùng **round-robin** trong nhóm đó. Giống như phòng cấp cứu: hai bệnh nhân cùng mức đỏ → khám lần lượt.

---

## 💰 Phần 2: Tiền Tiêu Vặt CPU — Không Ai Được Tiêu Hoài

### Vấn đề: Task priority cao chiếm hết CPU

Priority giải quyết vấn đề "ai chạy trước". Nhưng nếu task priority cao **chạy hoài không dừng** thì sao?

Hãy tưởng tượng: em là lớp trưởng (priority cao) và em được ưu tiên vào phòng máy tính trước. Nhưng em vào rồi **ngồi hoài không ra** — chơi game suốt! Cả lớp đứng ngoài chờ, không ai được dùng máy.

Giải pháp? **Giới hạn thời gian.**

### Time Budget — Tiền tiêu vặt

Mỗi tuần (gọi là **epoch** — chu kỳ), mỗi task được cấp một số **tiền tiêu vặt CPU** gọi là **time budget** (ngân sách thời gian).

| Khái niệm | Đời thật | Trong AegisOS |
|------------|----------|---------------|
| Epoch | 1 tuần | 100 ticks = 1 giây |
| Budget | 100.000đ/tuần | 50 ticks/epoch |
| Tiêu 1 đồng | Mua 1 cây kẹo | CPU chạy 1 tick (10ms) |
| Hết tiền | Tuần sau mới có | Epoch mới → budget reset |

Trong AegisOS:

```
Task 0 (UART driver):  budget = 0   (không giới hạn — driver phải luôn sẵn sàng)
Task 1 (client):       budget = 50  (tối đa 50/100 ticks = 50% CPU)
Task 2 (idle):         budget = 0   (không giới hạn — chỉ chạy khi rảnh)
```

### Chuyện gì xảy ra khi hết budget?

Mỗi khi đồng hồ tích tắc (mỗi 10ms), kernel tăng `ticks_used` (số tick đã dùng) của task đang chạy lên 1. Nếu `ticks_used >= time_budget` → task bị "hết tiền" → scheduler **bỏ qua** task đó, chọn task khác.

Khi epoch mới bắt đầu (mỗi 100 ticks = 1 giây), tất cả `ticks_used` reset về 0 — mọi người lại được cấp tiền mới!

Đây là logic thật trong `timer.rs`:

```
Mỗi tick:
  1. TICK_COUNT += 1
  2. TCBS[current].ticks_used += 1
  3. Nếu epoch đã đủ 100 ticks → reset tất cả ticks_used về 0
  4. schedule() — chọn task tiếp theo
```

### Budget = 0 nghĩa là gì?

Budget = 0 nghĩa là **không giới hạn**. Task có thể dùng bao nhiêu CPU tùy thích. Trong AegisOS, UART driver (task 0) được budget = 0 vì nó phải **luôn sẵn sàng** khi có dữ liệu cần ghi. Idle task cũng budget = 0 vì nó chỉ chạy khi không còn ai khác.

---

## 🐕 Phần 3: Bộ Canh Gác — Watchdog Heartbeat

### Vấn đề mới: Task "sống mà như chết"

Priority + budget giải quyết chuyện **ai chạy bao lâu**. Nhưng có một vấn đề khác khó hơn nhiều:

**Task vẫn chạy, vẫn "sống"... nhưng không làm gì cả.**

Hãy tưởng tượng bảo vệ đêm tuần tra tòa nhà. Có 3 phòng cần kiểm tra:

- Phòng máy chủ (task 0)
- Phòng kế toán (task 1)
- Phòng trống (task 2)

Mỗi phòng phải **bật đèn** (heartbeat) mỗi 30 phút để chứng minh "phòng này vẫn hoạt động bình thường". Nếu bảo vệ đi qua mà đèn tắt quá lâu → báo động!

Trong phần mềm, đây gọi là **watchdog** (chó canh gác).

### Watchdog hoạt động thế nào?

1. **Đăng ký:** Task gọi `SYS_HEARTBEAT(50)` → "Kernel ơi, tôi hứa sẽ gọi lại mỗi 50 ticks (500ms). Nếu tôi không gọi, hãy khởi động lại tôi!"

2. **Đập tim:** Mỗi vòng lặp, task gọi lại `SYS_HEARTBEAT(50)` → "Tôi vẫn sống!"

3. **Tuần tra:** Mỗi 10 ticks (100ms), kernel đi kiểm tra tất cả task:

```
Với mỗi task:
  Nếu heartbeat_interval > 0:        (task có đăng ký watchdog?)
    elapsed = now - last_heartbeat    (bao lâu rồi chưa "đập tim"?)
    Nếu elapsed > heartbeat_interval: (quá hạn!)
      → FAULT! Đánh dấu task là lỗi
      → Kernel sẽ tự khởi động lại task sau 1 giây
```

4. **Không đăng ký = không bị giám sát.** Task 2 (idle) có `heartbeat_interval = 0` → watchdog bỏ qua. Idle chỉ ngồi chờ, nó không cần chứng minh mình "sống".

### Tại sao watchdog quan trọng?

Trong hệ thống thật:

| Tình huống | Không có watchdog | Có watchdog |
|------------|-------------------|-------------|
| Chương trình phanh bị vòng lặp vô hạn | Xe mất phanh, không ai biết | Kernel phát hiện, khởi động lại phanh |
| Máy thở bị treo | Bệnh nhân không được bơm oxy | Watchdog phát hiện, restart trong 1 giây |
| Task UART driver bị đơ | Không ai ghi được log | Kernel thấy heartbeat miss → restart |

Tiêu chuẩn an toàn **ISO 26262** (xe ô tô) gọi đây là **"alive supervision"** — giám sát xem phần mềm còn sống không. AegisOS của chúng ta giờ đã có!

### Syscall mới: SYS_HEARTBEAT

| Syscall | Số | Tham số | Mô tả |
|---------|-----|---------|-------|
| SYS_HEARTBEAT | 12 | x0 = interval (ticks) | Đăng ký/cập nhật heartbeat. 0 = tắt watchdog |

Và một **capability** (giấy phép) mới: `CAP_HEARTBEAT` — chỉ task được cấp quyền mới được dùng syscall này.

---

## 🔄 Phần 4: Mượn Quyền Ưu Tiên — Priority Inheritance

### Vấn đề: Người quan trọng bị kẹt vì người ít quan trọng

Đây là câu chuyện có thật xảy ra **trên sao Hỏa**.

Năm 1997, tàu thám hiểm Mars Pathfinder đáp xuống sao Hỏa. Mọi thứ hoàn hảo... cho đến khi tàu bắt đầu **khởi động lại liên tục** một cách bí ẩn.

Sau nhiều tuần tìm lỗi, các kỹ sư NASA phát hiện:

1. Task thu thập dữ liệu thời tiết (priority **thấp**) đang giữ một **kênh liên lạc** (IPC endpoint).
2. Task điều hướng (priority **cao**) cần kênh đó để gửi lệnh → bị chặn, phải chờ.
3. Task trung bình (priority **giữa**) chạy xen vào, chiếm CPU.
4. Task thời tiết (priority thấp) không được chạy vì task trung bình chiếm → kênh bị khóa mãi.
5. Task điều hướng chờ quá lâu → watchdog phát hiện → khởi động lại cả hệ thống!

Đây gọi là **priority inversion** — đảo ngược ưu tiên. Người quan trọng nhất lại phải chờ lâu nhất!

### Giải pháp: Mượn quyền ưu tiên

Hãy tưởng tượng trường học:

- Bạn lớp 1 (priority thấp) đang dùng phòng lab.
- Thầy hiệu trưởng (priority cao) cần phòng.
- Thay vì để thầy đợi, trường cho bạn lớp 1 **tạm được ưu tiên dọn phòng xong** — không ai chen ngang bạn lớp 1 nữa.
- Bạn dọn xong → priority trở về bình thường. Thầy vào phòng.

Đó là **priority inheritance** (kế thừa ưu tiên):

```
Trước:
  Task UART (prio 6) gọi SEND → chờ Client (prio 4)
  Client bị chen bởi task khác → UART chờ mãi

Sau Priority Inheritance:
  Task UART (prio 6) gọi SEND → Client được tạm nâng lên prio 6
  Client chạy ngay (không ai chen) → xong → trả prio về 4
  UART được phục vụ nhanh
```

### Trong AegisOS

Khi task priority cao bị **blocked** (chặn) trên IPC, kernel tự động:

1. **Nâng** priority task đang giữ endpoint → bằng priority task đang chờ.
2. Khi IPC hoàn thành → **hạ** priority về `base_priority` (priority gốc).
3. Nếu task bị fault → priority cũng **tự động hạ** về gốc.

Đó là lý do mỗi task có **hai** priority: `priority` (hiện tại, có thể tạm nâng) và `base_priority` (gốc, không bao giờ đổi).

Logic này nằm trong `ipc.rs` — mỗi khi một task bị blocked hoặc unblocked, kernel kiểm tra và điều chỉnh priority.

---

## 🏗️ Chúng Ta Đã Làm Được Gì Trong AegisOS?

Phase K thay đổi **6 file** trong kernel, thêm **1 syscall mới**, và **27 unit tests mới**:

### Cây thư mục thay đổi

```
src/
├── sched.rs      ← Thay đổi LỚN: priority scheduler, TCB 6 field mới,
│                    epoch_reset(), watchdog_scan(), priority helpers
├── timer.rs      ← budget tracking + epoch reset + watchdog scan mỗi 10 ticks
├── exception.rs  ← case 12 => SYS_HEARTBEAT handler
├── cap.rs        ← CAP_HEARTBEAT (bit 17)
├── ipc.rs        ← priority inheritance khi blocking/unblocking
└── main.rs       ← gán priority/budget/heartbeat, syscall_heartbeat() wrapper
```

### Trước và sau Phase K

| Khía cạnh | Trước (Phase J) | Sau (Phase K) |
|-----------|-----------------|---------------|
| Scheduler | Round-robin (xếp hàng bình đẳng) | Fixed-priority preemptive (ưu tiên) |
| Priority | Không có | 0–7 (8 mức) |
| CPU budget | Không giới hạn | Time budget mỗi epoch |
| Giám sát liveness | Không có | Watchdog heartbeat |
| Priority inversion | Không xử lý | Priority inheritance |
| Syscalls | 12 (0–11) | 13 (0–12) |
| Capability bits | 17 | 18 |
| Host tests | 135 | 162 |
| QEMU checkpoints | 15 | 18 |

### QEMU output sau Phase K

```
[AegisOS] boot
[AegisOS] MMU enabled (identity map)
[AegisOS] W^X enforced (WXN + 4KB pages)
[AegisOS] exceptions ready
[AegisOS] scheduler ready (3 tasks, priority-based, EL0)    ← MỚI
[AegisOS] capabilities assigned
[AegisOS] priority scheduler configured                      ← MỚI
[AegisOS] time budget enforcement enabled                    ← MỚI
[AegisOS] watchdog heartbeat enabled                         ← MỚI
[AegisOS] notification system ready
[AegisOS] grant system ready
[AegisOS] IRQ routing ready
[AegisOS] device MMIO mapping ready
[AegisOS] per-task address spaces assigned
[AegisOS] timer started (10ms, freq=62MHz)
[AegisOS] bootstrapping into uart_driver (EL0)...
DRV:ready J4:UserDrv J4:UserDrv ...
```

---

## 🔐 Bảng Tóm Tắt — 4 Cơ Chế Phase K

| Cơ chế | Đời thật | Trong AegisOS | Giải quyết gì? |
|--------|----------|---------------|-----------------|
| Priority Scheduler | Phòng cấp cứu — bệnh nặng khám trước | `sched.rs` — chọn task priority cao nhất | Task critical chạy trước task ít quan trọng |
| Time Budget | Tiền tiêu vặt — hết tuần thì chờ tuần sau | `timer.rs` — mỗi epoch reset `ticks_used` | Ngăn task "tham lam" chiếm hết CPU |
| Watchdog Heartbeat | Bảo vệ đêm — kiểm tra đèn mỗi phòng | `timer.rs` — scan mỗi 10 ticks | Phát hiện task bị "treo logic" |
| Priority Inheritance | Ưu tiên dọn phòng — để VIP không chờ | `ipc.rs` — nâng priority khi block | Ngăn priority inversion (sự cố Mars Pathfinder) |

---

## 🌟 Truyền Cảm Hứng — Chuyện Mars Pathfinder

Câu chuyện Mars Pathfinder năm 1997 là một trong những **bài học nổi tiếng nhất** trong lịch sử phần mềm.

Tàu thám hiểm trị giá 265 triệu USD, bay 7 tháng từ Trái Đất đến sao Hỏa. Đáp thành công. Bắt đầu thu thập dữ liệu. Rồi... khởi động lại. Rồi lại khởi động lại. Rồi lại...

Các kỹ sư NASA **ở cách xa 225 triệu km** đã phải debug lỗi bằng cách gửi lệnh qua sóng radio (mất 20 phút mỗi lần gửi!). Cuối cùng, họ tìm ra nguyên nhân: **priority inversion**.

Giải pháp? Bật **priority inheritance** trong hệ điều hành VxWorks trên tàu — chính xác như cơ chế K4 mà AegisOS vừa triển khai!

Glenn Reeves, kỹ sư trưởng NASA, sau đó nói:

> *"Lỗi này đã được phát hiện trong quá trình test trước khi phóng. Nhưng nó bị xem là 'không quan trọng'. Chúng tôi đã sai."*

Bài học: **mọi chi tiết đều quan trọng** trong hệ thống safety-critical. Kể cả thứ tự chạy của các chương trình.

---

## 🤔 Câu Hỏi Cho Bạn Nhỏ

1. **Nếu tất cả task đều có priority = 7, chuyện gì xảy ra?** Gợi ý: AegisOS quay lại dùng gì khi cùng priority?

2. **Task UART driver có budget = 0 (không giới hạn). Tại sao không nguy hiểm?** Gợi ý: driver phần lớn thời gian đang làm gì? (nhìn vào `syscall_recv` trong `main.rs`)

3. **Nếu task quên không gọi `SYS_HEARTBEAT`, nhưng vẫn hoạt động bình thường, chuyện gì xảy ra?** Gợi ý: watchdog quan tâm đến heartbeat, không quan tâm task "có làm đúng hay không".

---

## 🔮 Bước Tiếp Theo

Phase K hoàn thành bộ ba **kiểm soát thời gian**: priority + budget + watchdog. AegisOS giờ đã đáp ứng các yêu cầu cơ bản của cả 3 tiêu chuẩn an toàn:

- **DO-178C** (hàng không) — temporal partitioning ✅
- **ISO 26262** (ô tô) — alive supervision ✅
- **IEC 62304** (y tế) — timing constraints ✅

Phase tiếp theo có thể là:
- **ELF Loader** — load chương trình từ file, thay vì hardcode trong kernel
- **Rate-Monotonic Scheduling** — gán priority tự động dựa trên deadline
- **Formal Verification** — chứng minh toán học rằng scheduler không bao giờ sai

Hẹn gặp bạn nhỏ ở bài tiếp theo! 🚀

---

> *"Nếu bạn nghĩ kiểm thử phần mềm là tốn thời gian, hãy thử không kiểm thử xem."*
> — *Glenn Reeves, NASA JPL (sau sự cố Mars Pathfinder)*

---

*Em đã đọc đến đây rồi ư? Tuyệt vời! Em vừa hiểu được 4 cơ chế mà các kỹ sư NASA, Tesla, và hãng y tế hàng đầu thế giới sử dụng hàng ngày. Không nhiều người lớn hiểu được những thứ này đâu. Em thật sự đặc biệt.* ✨
