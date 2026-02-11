---
lang: vi
title: "🔔 Chuông Cửa Và Hàng Đợi — Nói Chuyện Không Cần Chờ"
tags: "ipc, notification, async, queue, aegisos"
description: "Bài #9 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Notification (tín hiệu không chặn) và Multi-sender Queue — làm sao để các chương trình nói chuyện mà không ai phải đứng đợi?"
date: 2026-02-11
---
# 🔔 Chuông Cửa Và Hàng Đợi — Nói Chuyện Không Cần Chờ

> *Bài #9 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Notification và Multi-sender Queue — hai cách giúp chương trình "giao tiếp" mà không ai phải đứng yên một chỗ.*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2048. Em là kỹ sư thiết kế phần mềm cho xe cứu thương tự lái.

Chiếc xe đang lao nhanh trên đường phố đông đúc. Bên trong, năm chương trình hoạt động cùng lúc:

- **Chương trình radar** — quét vật cản xung quanh.
- **Chương trình phanh** — điều khiển bánh xe.
- **Chương trình định vị** — tìm đường nhanh nhất đến bệnh viện.
- **Chương trình y tế** — theo dõi nhịp tim bệnh nhân.
- **Chương trình liên lạc** — báo cáo về trung tâm điều hành.

Bệnh nhân trên xe đang trong tình trạng nguy kịch. Mỗi giây đều quý giá.

Bỗng nhiên — chương trình radar phát hiện một chiếc xe khác đang chạy ngược chiều!

Radar cần **thông báo ngay lập tức** cho chương trình phanh. Nhưng lúc này, chương trình phanh đang **bận** xử lý lệnh giảm tốc từ chương trình định vị.

Nếu hệ thống giao tiếp là **đồng bộ** — ai muốn nói chuyện phải đợi người kia rảnh — thì radar sẽ bị **đứng yên**, không làm gì được, trong khi xe đang lao thẳng vào nguy hiểm.

*Nhưng nếu radar có thể "bấm chuông cửa" mà không cần đợi ai mở?*

Đó chính là **Notification** — và AegisOS vừa học được kỹ năng mới này.

---

## 📞 Hai Cách Nói Chuyện

### Cách cũ: Gọi điện thoại

Em còn nhớ bài trước không? Trong AegisOS, các chương trình nói chuyện bằng **IPC** (Inter-Process Communication — giao tiếp giữa các tiến trình).

Cách IPC cũ giống như **gọi điện thoại**:

1. Task A nhấc máy gọi cho Task B.
2. Nếu Task B **chưa nhấc máy** → Task A phải **đứng đợi**, không làm gì được.
3. Khi Task B nhấc máy → hai bên nói chuyện → xong → cúp máy.

Cách này gọi là **đồng bộ** (synchronous) — hai người phải *cùng lúc* sẵn sàng.

| Ưu điểm | Nhược điểm |
|---|---|
| Đơn giản, dễ hiểu | Người gọi bị **chặn** nếu người nhận bận |
| Tin nhắn được giao chính xác | Không gửi được cho nhiều người cùng lúc |
| Biết chắc ai đã nhận | Chương trình quan trọng có thể bị "kẹt" chờ |

Với xe cứu thương, điều này **nguy hiểm**. Radar không được phép đứng đợi. Bệnh nhân trên xe không có thời gian cho việc "xin lỗi, đường dây bận".

### Cách mới: Bấm chuông cửa 🔔

Bây giờ AegisOS có thêm cách thứ hai: **Notification** — giống như **bấm chuông cửa**.

1. Task A bấm chuông nhà Task B. **Xong.** A đi làm việc khác ngay lập tức.
2. Task B tùy lúc mới ra mở cửa — khi nào B sẵn sàng.
3. Nếu nhiều người cùng bấm chuông trước khi B mở → B vẫn biết **tất cả** ai đã bấm.

Không đợi. Không chặn. Không kẹt.

| Đời thật | Kỹ thuật |
|---|---|
| Bấm chuông cửa | `SYS_NOTIFY` — gửi tín hiệu |
| Ra mở cửa xem ai bấm | `SYS_WAIT_NOTIFY` — đợi và đọc tín hiệu |
| Chuông reo 3 lần trước khi mở | 3 notification gộp lại thành 1 bitmask |
| Ai cũng có thể bấm chuông | Sender không bao giờ bị chặn |

---

## 🧠 Notification Hoạt Động Như Thế Nào?

### Bitmask — Bảng đèn tín hiệu

Mỗi chương trình có một **bảng đèn** gồm 64 bóng đèn nhỏ. Mỗi bóng đèn đại diện cho một loại tín hiệu.

Ví dụ:

| Đèn số | Ý nghĩa |
|---|---|
| Đèn 0 | "Có dữ liệu mới từ radar" |
| Đèn 1 | "Timer đã kêu" |
| Đèn 2 | "Có tin nhắn từ trung tâm" |
| Đèn 3 | "Cảm biến nhiệt độ báo động" |
| … | (còn 60 đèn nữa cho tương lai) |

Khi radar muốn thông báo cho phanh, nó **bật đèn số 0** trên bảng đèn của phanh. Radar không cần đợi phanh nhìn — nó bật xong rồi đi làm việc tiếp.

Trong AegisOS, "bảng đèn" này là một con số 64-bit gọi là `notify_pending`. Mỗi bit là một "bóng đèn".

**Kỹ thuật hay:** nếu radar bấm chuông 3 lần trước khi phanh mở cửa, các tín hiệu được **gộp lại** bằng phép OR. Giống như bảng đèn — đèn đã bật thì bật thêm cũng vẫn bật. Khi phanh "mở cửa" (gọi `SYS_WAIT_NOTIFY`), nó đọc **toàn bộ bảng đèn** một lần, rồi tắt hết.

### Hai lệnh mới

AegisOS giờ có 7 lệnh (syscall) thay vì 5:

| Lệnh | Số | Làm gì |
|---|---|---|
| `SYS_YIELD` | 0 | Nhường CPU |
| `SYS_SEND` | 1 | Gửi tin nhắn (đồng bộ, có chờ) |
| `SYS_RECV` | 2 | Nhận tin nhắn (đồng bộ, có chờ) |
| `SYS_CALL` | 3 | Gửi rồi nhận (gọi hàm từ xa) |
| `SYS_WRITE` | 4 | Ghi chữ ra màn hình |
| 🔔 `SYS_NOTIFY` | 5 | **MỚI:** Bấm chuông — gửi tín hiệu không chờ |
| 🔔 `SYS_WAIT_NOTIFY` | 6 | **MỚI:** Mở cửa — đợi và đọc tín hiệu |

### Kịch bản minh họa

Hãy xem radar và phanh "nói chuyện" bằng notification:

> **Radar:** *"Bấm chuông nhà Phanh! Bật đèn số 0: CÓ VẬT CẢN!"*
> *(Radar tiếp tục quét — không đợi.)*
>
> **Định vị:** *"Bấm chuông nhà Phanh! Bật đèn số 2: ĐỔI HƯỚNG!"*
> *(Định vị tiếp tục tính đường — không đợi.)*
>
> **Phanh:** *(đang xử lý xong lệnh trước)*
> *"Mở cửa xem nào…"*
> *(Đọc bảng đèn: đèn 0 BẬT, đèn 2 BẬT → vật cản + đổi hướng)*
> *"OK! Phanh gấp + rẽ phải!"*
> *(Tắt hết đèn. Tiếp tục.)*

Không ai bị chặn. Không ai phải đợi. Tín hiệu **không bao giờ bị mất** — chúng gộp lại trên bảng đèn cho đến khi người nhận đọc.

---

## 🏢 Hàng Đợi Gửi Tin — Nhiều Người, Một Quầy

### Vấn đề cũ: Chỉ một người gửi

Trước Phase I, mỗi "quầy giao dịch" (endpoint) chỉ cho **một người** đứng chờ gửi. Nếu người thứ hai đến — xin lỗi, không có chỗ.

Tưởng tượng bưu điện chỉ có **1 chỗ xếp hàng**. Nếu 2 người cùng đến gửi thư → người thứ 2 phải đi về. Thật bất tiện!

### Giải pháp: Hàng đợi xoay vòng 🔄

AegisOS giờ có **hàng đợi** tại mỗi quầy, chứa được tối đa 4 người.

Hoạt động giống hàng đợi ở siêu thị:

1. Người đến trước đứng trước. Người đến sau đứng sau. (**FIFO** — First In, First Out — Vào trước, Ra trước.)
2. Khi nhân viên quầy (receiver) sẵn sàng → phục vụ người **đứng đầu hàng**.
3. Nếu hàng đầy (4 người) → người thứ 5 được báo "quầy đang đông, quay lại sau".

| Đời thật | Kỹ thuật |
|---|---|
| Hàng đợi ở siêu thị | `SenderQueue` — mảng vòng tròn 4 phần tử |
| Người đứng đầu hàng | `head` — vị trí đầu hàng |
| Số người đang chờ | `count` — đếm số task trong hàng |
| Phục vụ 1 người | `pop()` — lấy task đầu tiên ra |
| Vào xếp hàng | `push()` — thêm task vào cuối hàng |

**Tại sao "xoay vòng"?** Vì khi người đầu hàng được phục vụ xong, vị trí `head` di chuyển lên. Khi đến cuối mảng, nó quay lại đầu — giống kim đồng hồ quay một vòng. Kỹ thuật này gọi là **circular buffer** (bộ đệm xoay vòng) — rất phổ biến trong hệ thống thực.

### Phía nhận: Vẫn chỉ một người

Một câu hỏi thú vị: tại sao hàng đợi chỉ dành cho **sender** (người gửi), còn **receiver** (người nhận) vẫn chỉ một?

Vì mô hình phổ biến trong microkernel là **nhiều client → một server**:

- Nhiều cảm biến gửi dữ liệu → một bộ xử lý trung tâm nhận.
- Nhiều ứng dụng gửi yêu cầu in → một trình quản lý máy in nhận.
- Nhiều task gửi log → một task ghi log nhận.

Server là "quầy phục vụ". Chỉ cần 1 quầy, nhưng phải có hàng đợi cho khách.

---

## 📬 Bốn Quầy Thay Vì Hai

### Mở rộng hệ thống giao tiếp

Trước đây, AegisOS chỉ có **2 endpoint** (2 quầy giao dịch). Đủ cho demo PING–PONG giữa 2 task, nhưng hệ thống thật cần nhiều hơn.

Giờ AegisOS có **4 endpoint**: EP0, EP1, EP2, EP3.

Tưởng tượng tòa nhà của em có 4 cửa:
- **Cửa 0** — dành cho liên lạc giữa radar và phanh.
- **Cửa 1** — dành cho liên lạc giữa định vị và phanh.
- **Cửa 2** — dành cho liên lạc giữa y tế và trung tâm.
- **Cửa 3** — dự phòng cho tương lai.

Và mỗi cửa đều có **giấy phép riêng**. Task muốn gửi qua cửa 2 phải có giấy phép `CAP_IPC_SEND_EP2`. Không có giấy → không vào được.

---

## 🎫 Giấy Phép Mới

Nhớ bài #7 không? Mỗi task có một **bitmask giấy phép** (capability). Mỗi bit = một quyền.

Phase I thêm 6 giấy phép mới:

| Bit | Quyền | Ý nghĩa |
|---|---|---|
| 6 | `CAP_NOTIFY` | Được bấm chuông nhà người khác |
| 7 | `CAP_WAIT_NOTIFY` | Được mở cửa đọc chuông |
| 8 | `CAP_IPC_SEND_EP2` | Được gửi qua cửa 2 |
| 9 | `CAP_IPC_RECV_EP2` | Được nhận qua cửa 2 |
| 10 | `CAP_IPC_SEND_EP3` | Được gửi qua cửa 3 |
| 11 | `CAP_IPC_RECV_EP3` | Được nhận qua cửa 3 |

Tổng cộng AegisOS giờ có **12 loại giấy phép** (trước đó chỉ 6). Mỗi giấy phép nằm gọn trong 1 bit của con số 64-bit. Vẫn còn dư **52 bit** cho tương lai.

Phần hay nhất? Task idle (task ngồi chơi khi không ai làm gì) vẫn chỉ có đúng **1 giấy phép**: `CAP_YIELD`. Nó không được gọi điện, không được bấm chuông, không được ghi chữ. An toàn tuyệt đối.

---

## 🔧 Chúng Ta Đã Làm Được Gì Trong AegisOS?

Phase I thay đổi 7 file trong dự án. Hãy cùng xem từng phần:

### 1. TCB — Thẻ căn cước có thêm "bảng đèn"

Trong [src/sched.rs](src/sched.rs), mỗi task giờ có thêm hai trường:

- `notify_pending: u64` — bảng đèn 64 bóng, ghi lại ai đã bấm chuông.
- `notify_waiting: bool` — task có đang ngồi chờ chuông không?

Khi task bị lỗi và được khởi động lại, hai trường này tự động **bị xóa sạch**. Giống như khi em chuyển nhà — bảng đèn cũ không theo em sang nhà mới.

### 2. Hai lệnh mới trong bộ xử lý ngoại lệ

Trong [src/exception.rs](src/exception.rs), bộ phận "tổng đài" của kernel giờ biết xử lý 7 loại lệnh thay vì 5:

- Lệnh số 5 (`SYS_NOTIFY`): đọc **ai** là người nhận (từ thanh ghi x6) và **tín hiệu gì** (từ thanh ghi x0). Bật đèn tương ứng trên bảng đèn của người nhận. Nếu người nhận đang ngồi đợi → đánh thức ngay.

- Lệnh số 6 (`SYS_WAIT_NOTIFY`): kiểm tra bảng đèn của task hiện tại. Nếu có đèn bật → trả về ngay và tắt hết. Nếu không → ngủ và đợi ai đó bấm chuông.

### 3. Hàng đợi xoay vòng trong IPC

Trong [src/ipc.rs](src/ipc.rs), mỗi endpoint giờ có `SenderQueue` thay vì chỉ một slot. Hàng đợi này có 4 thao tác:

- `push()` — xếp vào cuối hàng
- `pop()` — lấy người đứng đầu ra
- `remove()` — lôi một người cụ thể ra khỏi hàng (khi task bị lỗi)
- `contains()` — kiểm tra ai đó có đang xếp hàng không

### 4. Giấy phép mở rộng

Trong [src/cap.rs](src/cap.rs), 6 giấy phép mới được thêm vào. Hàm `cap_for_syscall()` giờ biết kiểm tra cả lệnh NOTIFY và endpoint 2, 3.

### 5. Syscall wrapper cho EL0

Trong [src/main.rs](src/main.rs), hai hàm mới giúp task ở chế độ người dùng (EL0) gọi notification:

- `syscall_notify(target, bits)` — bấm chuông
- `syscall_wait_notify()` — đợi chuông reo

### Cây thư mục sau Phase I:

```
src/
├── boot.s          ← khởi động (không đổi)
├── main.rs         ← +2 syscall wrappers, cập nhật caps
├── exception.rs    ← +2 handler mới (notify, wait_notify)
├── sched.rs        ← +2 trường TCB (notify_pending, notify_waiting)
├── ipc.rs          ← SenderQueue, 4 endpoints
├── cap.rs          ← 12 giấy phép (trước: 6)
├── mmu.rs          ← bảng trang (không đổi)
├── gic.rs          ← GIC driver (không đổi)
├── timer.rs        ← timer (không đổi)
├── uart.rs         ← UART driver (không đổi)
└── lib.rs          ← module declarations
tests/
├── host_tests.rs   ← 94 tests (trước: 79) ← +15 mới!
├── qemu_boot_test.ps1  ← 12 checkpoints (trước: 11)
└── qemu_boot_test.sh   ← 12 checkpoints
```

---

## 🏎️ Tại Sao Điều Này Quan Trọng Ngoài Đời Thật?

### Notification ở khắp nơi

Em có biết notification không chỉ có trong AegisOS?

**Mọi hệ điều hành lớn đều dùng cơ chế tương tự:**

- **Linux** có `eventfd`, `signal`, `epoll` — tất cả đều là biến thể của "bấm chuông không chờ".
- **seL4** (microkernel đã được chứng minh toán học) dùng **notification object** — gần giống hệt cách AegisOS làm: u64 bitmask, OR merge, non-blocking.
- **QNX** (dùng trong xe hơi thật — Audi, BMW, Toyota) có **pulse** — tín hiệu nhẹ, không chặn sender.

Khi thiết bị phần cứng (ví dụ card mạng) có dữ liệu mới, nó gửi **interrupt** (ngắt) cho kernel. Kernel không xử lý dữ liệu — nó chỉ "bấm chuông" cho chương trình driver. Driver tỉnh dậy, xử lý dữ liệu. Đây chính là **interrupt routing** — và notification là nền tảng để làm điều đó.

### Multi-sender Queue trong thực tế

Hàng đợi xoay vòng (circular buffer) là một trong những cấu trúc dữ liệu **phổ biến nhất** trong phần mềm hệ thống:

- **Bàn phím** gõ nhanh hơn phần mềm xử lý? → các phím vào **hàng đợi**, không mất ký tự nào.
- **Card mạng** nhận nhiều gói tin cùng lúc? → các gói vào **ring buffer** (bộ đệm vòng tròn).
- **Hệ thống âm thanh** thu âm liên tục? → mẫu âm thanh vào **circular buffer**, chương trình đọc khi sẵn sàng.

Em đang học một kỹ thuật mà các kỹ sư ở Intel, ARM, Google dùng **mỗi ngày**.

---

## 💡 Truyền Cảm Hứng — Cậu Bé Hay Hỏi "Tại Sao?"

Có một cậu bé ở Phần Lan rất hay hỏi "tại sao?".

*"Tại sao máy tính chỉ chạy được một chương trình?"* — Cậu viết thêm cho nó bộ lập lịch.

*"Tại sao các chương trình phải đợi nhau?"* — Cậu thêm tín hiệu không đồng bộ.

*"Tại sao chỉ dùng được 2 endpoint?"* — Cậu mở rộng lên nhiều hơn.

Cậu bé đó là **Linus Torvalds**. Và "bài tập" của cậu trở thành **Linux** — hệ điều hành chạy trên hàng tỷ thiết bị hôm nay.

Điều thú vị là: những câu hỏi Linus đặt ra năm 21 tuổi cũng giống y **những câu hỏi chúng ta đang trả lời** trong AegisOS:

- Bài #3: Làm nhiều việc cùng lúc → Scheduler
- Bài #7: Ai được làm gì → Capability
- Bài #8: Mỗi task một bản đồ riêng → Address Space
- **Bài #9: Nói chuyện không cần chờ → Notification** ← Chúng ta đang ở đây!

Em không cần đợi đến 21 tuổi. Em đang bắt đầu **ngay bây giờ**.

---

## 🔮 Bước Tiếp Theo

AegisOS giờ đã có:
- ✅ Kernel cách ly hoàn toàn (EL1 vs EL0)
- ✅ Syscall được kiểm soát bằng capability (12 giấy phép)
- ✅ Bộ nhớ cách ly per-task
- ✅ **Notification — tín hiệu không chặn** ← MỚI!
- ✅ **Multi-sender Queue — hàng đợi xoay vòng** ← MỚI!
- ✅ **4 endpoint thay vì 2** ← MỚI!
- ✅ 94 bài test tự động
- ✅ 12 checkpoint QEMU boot

Nhưng hệ thống giao tiếp vẫn thiếu một mảnh ghép quan trọng.

Hiện tại, hai task **chỉ gửi được 32 bytes** cho nhau (4 thanh ghi × 8 bytes). Đủ cho "PING–PONG", nhưng hoàn toàn không đủ nếu camera muốn gửi **hình ảnh** cho bộ nhận diện — hàng triệu bytes!

Giải pháp? **Shared Memory Grant** — kernel cấp cho 2 task một "phòng họp chung" nơi cả hai có thể đọc/ghi dữ liệu lớn. Phòng họp này có khóa — kernel kiểm soát ai được vào, ai phải ra.

Và sau đó? Khi phần cứng gửi **interrupt** (ngắt) cho kernel, kernel sẽ dùng chính notification để "bấm chuông" cho chương trình driver. Đó là **interrupt routing** — biến AegisOS thành hệ điều hành có thể điều khiển thiết bị thật.

Hành trình chưa dừng lại. 🚀

---

> *"Giao tiếp tốt không phải là nói to hơn. Giao tiếp tốt là biết khi nào cần đợi — và khi nào không cần."*

---

*Em đã đọc đến đây — tuyệt vời! 🌟 Em vừa hiểu hai kỹ thuật mà mọi hệ điều hành hiện đại đều dùng: tín hiệu bất đồng bộ và hàng đợi xoay vòng. Nhiều sinh viên đại học năm 3 mới học những điều này. Em đang đi trước rất xa đó!*
