# 🛡️ Khi Một Task Ngã, Cả Hệ Thống Không Được Ngã Theo

> *Bài #5 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Fault Isolation — nghệ thuật giữ cho hệ thống vẫn chạy khi có thứ gì đó hỏng.*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2042. Em là kỹ sư phần mềm y tế.

Bệnh viện vừa lắp một cỗ máy xạ trị mới — loại dùng tia bức xạ để tiêu diệt tế bào ung thư. Bên trong máy có hàng chục chương trình nhỏ chạy cùng lúc: một chương trình đo liều lượng, một chương trình điều khiển tia, một chương trình hiển thị thông tin cho bác sĩ.

Mỗi ngày, hàng trăm bệnh nhân nằm dưới cỗ máy ấy. Họ tin tưởng rằng phần mềm sẽ bảo vệ họ.

Nhưng nếu chương trình đo liều lượng bị lỗi — và kéo theo **toàn bộ hệ thống sập** thì sao?

Đây không phải chuyện tưởng tượng. Nó đã xảy ra thật.

---

## 💀 Câu Chuyện Therac-25 — Khi Phần Mềm Giết Người

Năm 1985–1987, có một cỗ máy xạ trị tên **Therac-25** được sản xuất ở Canada.

Phần mềm của Therac-25 có lỗi. Khi người vận hành nhấn phím quá nhanh — chỉ trong vòng 8 giây — máy chuyển chế độ nhưng phần mềm **không bắt kịp**. Kết quả: tia bức xạ bắn ra với liều lượng **gấp 100 lần** bình thường.

Ít nhất 6 vụ tai nạn xảy ra. 3 bệnh nhân qua đời.

Điều đáng sợ nhất? Khi máy báo lỗi, nó chỉ hiện chữ "Malfunction 54" — rồi **cho phép người vận hành bấm nút để tiếp tục**. Không ai biết "Malfunction 54" nghĩa là bệnh nhân đang bị chiếu xạ quá liều.

Phần mềm Therac-25 mắc một sai lầm nghiêm trọng: **khi có lỗi, nó không cô lập lỗi**. Nó cứ chạy tiếp, như thể không có gì xảy ra.

Sau thảm kịch này, ngành công nghiệp y tế tạo ra tiêu chuẩn **IEC 62304** — bắt buộc phần mềm thiết bị y tế phải được thiết kế để **cô lập lỗi**, ngăn một lỗi nhỏ trở thành thảm họa.

Ngành hàng không cũng có tiêu chuẩn riêng: **DO-178C** — chia phần mềm máy bay thành 5 cấp độ an toàn, từ Level A (lỗi = rơi máy bay) đến Level E (lỗi = không ảnh hưởng gì). Với Level A, có đến **71 mục tiêu** phải đạt trước khi phần mềm được phép bay.

Tất cả đều dẫn đến một nguyên tắc vàng:

> **Một thành phần hỏng không được phép kéo cả hệ thống chết theo.**

Và đó chính là điều chúng ta sẽ dạy cho AegisOS hôm nay.

---

## 🏫 Trường Học Và Bạn Bị Ngã

Để hiểu **Fault Isolation** (cô lập lỗi), hãy tưởng tượng ngôi trường của em.

Đang giờ ra chơi, một bạn chạy ngoài sân bị **ngã, trầy đầu gối**. Chuyện gì xảy ra?

**Cách 1 — Trường "kém":**
Chuông báo động vang lên. **TẤT CẢ** học sinh phải dừng mọi thứ. Tất cả các lớp ngừng học. Cả trường đóng cửa cho đến khi bạn đó khỏi.

Nghe vô lý phải không? Nhưng đây chính xác là cách nhiều hệ thống máy tính hoạt động — một chương trình lỗi → toàn bộ hệ thống sập.

**Cách 2 — Trường "thông minh":**
1. Y tá chạy ra **băng bó** cho bạn bị ngã
2. Các lớp khác **vẫn học bình thường** — các bạn khác thậm chí không biết có ai bị ngã
3. Sau khi bạn đó khỏe lại, bạn ấy **quay lại lớp tiếp tục học**

Đây là **Fault Isolation** — cô lập lỗi. Và đây chính là cách AegisOS hoạt động từ bây giờ.

| Ở trường | Trong AegisOS |
|---|---|
| Bạn bị ngã | Task bị lỗi (ví dụ: truy cập bộ nhớ cấm) |
| Y tá băng bó | Kernel đánh dấu task là "Faulted" (đã hỏng) |
| Các lớp khác vẫn học | Các task khác vẫn chạy bình thường |
| Bạn khỏe lại, quay lại lớp | Task được restart (khởi động lại) sau 1 giây |
| Sổ liên lạc ghi chú sự cố | Kernel in thông báo lên UART |

---

## 🧠 Kernel Phải Phân Biệt: Lỗi Của Ai?

Phần này hơi khó, nhưng em cứ đọc chậm lại nhé.

Khi CPU gặp lỗi (gọi là **Exception** — ngoại lệ), kernel cần trả lời một câu hỏi quan trọng:

**"Lỗi này do task gây ra, hay do chính kernel gây ra?"**

Giống như ở trường:
- Nếu **học sinh** ngã ngoài sân → y tá băng bó, trường vẫn hoạt động
- Nếu **hiệu trưởng** (người quản lý trường) ngất xỉu → CẢ TRƯỜNG có vấn đề, phải gọi cấp cứu

Trong AegisOS, CPU cho chúng ta biết điều này qua một mã gọi là **EC** (Exception Class — loại ngoại lệ):

| Mã EC | Nghĩa là | Kernel làm gì |
|---|---|---|
| `0x24` | Task (EL0) truy cập bộ nhớ sai — **Data Abort** | Đánh dấu task "Faulted", chạy task khác |
| `0x25` | Kernel (EL1) truy cập bộ nhớ sai — **Data Abort** | **DỪNG HẾT** — đây là bug của kernel! |
| `0x20` | Task (EL0) chạy lệnh sai — **Instruction Abort** | Đánh dấu task "Faulted", chạy task khác |
| `0x21` | Kernel (EL1) chạy lệnh sai — **Instruction Abort** | **DỪNG HẾT** — kernel bị lỗi! |

Quy tắc rất rõ ràng:
- **Lỗi của task** → cô lập, task khác không bị ảnh hưởng
- **Lỗi của kernel** → dừng hẳn, vì kernel là "bộ não" — nếu bộ não hỏng, không ai có thể ra lệnh nữa

---

## 🔄 Vòng Đời Của Một Task Bị Lỗi

Hãy theo dõi một task từ lúc bị lỗi đến lúc được cứu sống:

### Bước 1: Task Gây Ra Lỗi

Tưởng tượng task B (chương trình PONG) cố đọc địa chỉ bộ nhớ `0x0900_0000` — đây là địa chỉ của UART, thiết bị phần cứng. Nhưng task chạy ở **EL0** (chế độ người dùng), không có quyền đụng vào phần cứng!

CPU lập tức "giơ tay" báo lỗi: **Permission Fault** (lỗi quyền truy cập).

### Bước 2: Kernel "Bắt" Lỗi

Kernel nhận được tín hiệu từ CPU. Nó kiểm tra mã EC = `0x24` → "À, lỗi từ task, không phải lỗi kernel."

Kernel gọi hàm `fault_current_task()` — nghĩa là "xử lý task đang lỗi". Hàm này làm 4 việc:

1. **In thông báo** lên UART: `[AegisOS] TASK 1 FAULTED` ("Task số 1 đã hỏng")
2. **Đổi trạng thái** của task từ `Running` (đang chạy) sang `Faulted` (đã hỏng)
3. **Ghi nhớ thời điểm** task bị lỗi (để đếm thời gian chờ)
4. **Dọn dẹp IPC** — nếu task đang gửi/nhận tin nhắn với task khác, phải hủy kết nối (sẽ nói thêm ở phần sau)

Sau đó, kernel gọi `schedule()` — bộ lịch trình — để chuyển sang task khác.

### Bước 3: Các Task Khác Vẫn Chạy

Task A (chương trình PING) hoàn toàn không biết task B bị lỗi. Nó vẫn chạy bình thường, gửi tin nhắn, in chữ lên màn hình.

### Bước 4: Đếm Ngược Và Khởi Động Lại

Mỗi 10 mili-giây, đồng hồ hẹn giờ (timer) "tích" một lần. Sau **100 tích** (= 1 giây), bộ lịch trình kiểm tra:

> "Task B bị hỏng đã đủ 1 giây chưa? Đủ rồi → khởi động lại!"

Hàm `restart_task()` được gọi. Nó làm gì?

1. **Xóa sạch bộ nhớ stack** của task — giống như lau bảng sạch sẽ
2. **Xóa toàn bộ trạng thái** (thanh ghi, con trỏ) — quay về "như mới sinh"
3. **Nạp lại địa chỉ bắt đầu** — chỉ cho task biết "hãy bắt đầu lại từ dòng lệnh đầu tiên"
4. **Đổi trạng thái** sang `Ready` (sẵn sàng) — task quay lại hàng đợi

Kernel in: `[AegisOS] TASK 1 RESTARTED` ("Task số 1 đã khởi động lại")

Và task B sống lại, như chưa có gì xảy ra!

---

## 📬 Dọn Dẹp Thư — IPC Cleanup

Phần này quan trọng mà dễ quên.

Nhớ ở bài trước không? Các task giao tiếp bằng **IPC** (Inter-Process Communication — giao tiếp liên tiến trình). Task A gửi tin nhắn "PING", task B nhận và trả lời "PONG".

Nhưng nếu task B **bị hỏng giữa chừng** — đang chờ nhận tin nhắn thì crash?

Tưởng tượng thế này: em viết thư cho bạn B, bỏ vào hộp thư chung. Em đứng đợi bạn B đến lấy. Nhưng bạn B **bị ốm nghỉ học** — không đến lấy thư. Em cứ đứng đó đợi mãi... mãi... mãi...

Đây gọi là **deadlock** — kẹt không lối thoát.

AegisOS giải quyết bằng hàm `cleanup_task()` trong module IPC. Khi một task bị hỏng, kernel sẽ:

1. Kiểm tra tất cả các **endpoint** (hộp thư)
2. Nếu task bị hỏng đang là **người gửi** ở hộp thư nào → xóa tên task đó ra
3. Nếu task bị hỏng đang là **người nhận** ở hộp thư nào → xóa tên task đó ra

Giống như lớp trưởng thông báo: "Bạn B nghỉ ốm rồi. Ai đang đợi thư của bạn B thì đừng đợi nữa nhé."

---

## 🏰 Task "Bất Tử" — Idle Task

Trong AegisOS có 3 task: task 0, task 1, và task 2. Nhưng task 0 đặc biệt — nó là **idle task** (task rảnh rỗi).

Idle task giống như **bảo vệ trường** — khi không có ai cần làm gì, bảo vệ vẫn phải ở đó. Nếu bảo vệ cũng nghỉ, trường không có ai trông coi!

Vì vậy, AegisOS có một quy tắc đặc biệt cho idle task:

> **Nếu idle task bị lỗi → khởi động lại NGAY LẬP TỨC, không đợi 1 giây.**

Idle task **bất tử**. Nó không bao giờ được phép nằm im.

Trong code, bộ lịch trình kiểm tra: nếu idle task (index 0) đang ở trạng thái `Faulted` → gọi `restart_task(0)` lập tức, không cần đếm ngược.

---

## 🎬 AegisOS Trong Thực Tế — Output Trên UART

Khi chúng ta cố tình cho task B đọc bộ nhớ phần cứng (để test), đây là những gì UART in ra:

```
[AegisOS] TASK 1 FAULTED (at tick 42)
A:PING
A:PING
A:PING
...
[AegisOS] TASK 1 RESTARTED (after 100 ticks)
B:PONG
A:PING
B:PONG
```

Hãy đọc từng dòng:

1. `TASK 1 FAULTED` — Task B (số 1) vừa crash
2. `A:PING` — Task A vẫn chạy bình thường, không bị ảnh hưởng!
3. `TASK 1 RESTARTED` — Sau 1 giây, task B được khởi động lại
4. `B:PONG` — Task B sống lại, tiếp tục trả lời tin nhắn

**Không có lúc nào hệ thống dừng hẳn.** Đó là sức mạnh của Fault Isolation.

---

## 🗂️ Chúng Ta Đã Làm Gì Trong AegisOS?

Hãy nhìn lại cấu trúc project:

```
src/
├── main.rs        ← Task entries (task_a, task_b, task_c)
├── sched.rs       ← 🔥 Scheduler + fault_current_task() + restart_task()
├── exception.rs   ← 🔥 Phân biệt lỗi task vs lỗi kernel
├── ipc.rs         ← 🔥 cleanup_task() — dọn dẹp hộp thư
├── timer.rs       ← Đồng hồ 10ms, đếm tick
├── gic.rs         ← Bộ điều khiển ngắt
├── mmu.rs         ← Bảng trang, bảo vệ bộ nhớ
└── boot.s         ← Khởi động máy
```

Ba file đánh dấu 🔥 là nơi Fault Isolation sống:

### Trong [sched.rs](../../src/sched.rs) — Trái Tim Của Fault Isolation

- **Trạng thái mới:** `TaskState::Faulted = 4` — thêm vào danh sách trạng thái (Inactive, Ready, Running, Blocked, **Faulted**)
- **Thông tin mới trong TCB:** Mỗi task bây giờ nhớ `entry_point` (điểm bắt đầu) và `user_stack_top` (đỉnh stack) — để khi restart, biết đường quay về
- **`fault_current_task()`:** In lỗi → đánh dấu Faulted → dọn IPC → chuyển task
- **`restart_task()`:** Xóa stack → xóa trạng thái → nạp lại điểm bắt đầu → Ready
- **Auto-restart:** Trong `schedule()`, kiểm tra mỗi task Faulted — đủ 100 tick (1 giây) thì restart

### Trong [exception.rs](../../src/exception.rs) — Bộ Lọc Thông Minh

- **EC `0x24`/`0x20`** (lỗi từ task) → gọi `fault_current_task()`, hệ thống tiếp tục
- **EC `0x25`/`0x21`** (lỗi từ kernel) → dừng hẳn, in thông báo bug
- **Syscall không hợp lệ** → fault task luôn (trước đây chỉ in cảnh báo rồi bỏ qua)
- **Dùng phép tính FP/SIMD** (cấm trong hệ thống này) → nếu task vi phạm thì fault, nếu kernel vi phạm thì halt

### Trong [ipc.rs](../../src/ipc.rs) — Người Dọn Dẹp

- **`cleanup_task(task_idx)`:** Quét toàn bộ endpoint, xóa task bị hỏng khỏi hàng đợi gửi/nhận — tránh deadlock

---

## 🌍 Tại Sao Điều Này Quan Trọng Ngoài Đời Thật?

### Xe tự lái 🚗

Xe Tesla có hàng chục module phần mềm: camera trước, camera sau, radar, GPS, phanh tự động... Nếu module camera sau bị lỗi, xe **KHÔNG ĐƯỢC** mất phanh tự động. Module camera sau phải bị "cô lập" — tắt nó đi, các module khác vẫn chạy.

### Máy bay ✈️

Tiêu chuẩn **DO-178C** của ngành hàng không chia phần mềm thành 5 cấp. Phần mềm điều khiển cánh lái là Level A — "Catastrophic" (thảm họa). Nếu phần mềm này lỗi, máy bay có thể rơi. Vì vậy, phần mềm Level A phải qua **71 bài kiểm tra** với **30 bài phải do người độc lập kiểm tra** (không phải người viết code).

Nhưng ngay cả với bấy nhiêu kiểm tra, lỗi vẫn có thể xảy ra. Cho nên, nguyên tắc **Fault Isolation** — cô lập lỗi — là tuyến phòng thủ cuối cùng.

### Thiết bị y tế 🏥

Sau thảm kịch Therac-25, tiêu chuẩn **IEC 62304** chia phần mềm y tế thành 3 lớp an toàn (Class A, B, C). Class C — nơi lỗi phần mềm có thể gây chết người — yêu cầu tài liệu thiết kế chi tiết đến từng dòng code.

Và quy tắc số 1 luôn là: **Khi có lỗi, phải cô lập. Không được lan ra.**

---

## 🧩 Tổng Kết — Bảng So Sánh Lớn

| Khái niệm | Đời thật | Trong AegisOS |
|---|---|---|
| **Fault Isolation** | Bạn ngã ở sân, các lớp vẫn học | Task crash, các task khác vẫn chạy |
| **Task State: Faulted** | "Bạn B đang nghỉ ốm" | `TaskState::Faulted = 4` |
| **Auto Restart** | Bạn B khỏe lại, quay lại lớp | `restart_task()` sau 100 ticks |
| **IPC Cleanup** | Lớp trưởng báo "đừng đợi bạn B" | `cleanup_task()` xóa sender/receiver |
| **Idle Task bất tử** | Bảo vệ trường không được nghỉ | Task 0 restart ngay, không đợi |
| **Phân biệt lỗi** | Học sinh ngã vs hiệu trưởng ngất | EC 0x24 (task) vs EC 0x25 (kernel) |
| **Kernel halt** | Gọi cấp cứu cho hiệu trưởng | Kernel lỗi → dừng hẳn, in bug report |

---

## ✨ Tại Sao Em Nên Quan Tâm?

Em biết **Margaret Hamilton** không?

Bà là người viết phần mềm cho tàu Apollo 11 — con tàu đưa con người lên Mặt Trăng lần đầu tiên vào năm 1969. Bà bắt đầu quan tâm đến máy tính từ khi còn rất trẻ.

Khi tàu Apollo 11 đang hạ cánh xuống Mặt Trăng, máy tính bỗng **quá tải** — có quá nhiều chương trình chạy cùng lúc. Nhưng phần mềm của Margaret Hamilton đã được thiết kế để **ưu tiên** các chương trình quan trọng nhất (điều khiển hạ cánh) và **tạm dừng** các chương trình không quan trọng.

Kết quả? Con tàu hạ cánh an toàn. Hai phi hành gia Neil Armstrong và Buzz Aldrin bước đi trên Mặt Trăng.

Margaret Hamilton đã hiểu một điều từ rất sớm: **phần mềm sẽ gặp lỗi — điều quan trọng là hệ thống phải sống sót khi lỗi xảy ra.**

Đó chính xác là điều chúng ta đang xây dựng trong AegisOS.

---

## 🔮 Bước Tiếp Theo

AegisOS đã biết cách:
- ✅ Khởi động và nói "Hello World"
- ✅ Bảo vệ bộ nhớ bằng bảng trang (MMU)
- ✅ Chạy nhiều task cùng lúc (scheduler + timer)
- ✅ Cho task giao tiếp qua IPC
- ✅ Ngăn task xâm nhập kernel (EL0/EL1)
- ✅ **Cô lập lỗi và tự khởi động lại task** ← MỚI!

Nhưng có một câu hỏi mà chúng ta chưa trả lời:

> **"Làm sao chúng ta CHỨNG MINH rằng AegisOS thật sự an toàn?"**

Không phải "test thấy đúng" — mà là "chứng minh toán học rằng nó đúng". Giống như trong môn Toán, không phải chỉ thử vài số rồi nói "công thức đúng" — mà phải **chứng minh** nó đúng cho MỌI trường hợp.

Đây gọi là **Formal Verification** — xác minh hình thức. Và đó là nơi AegisOS sẽ đi tiếp.

Hẹn gặp em ở bài tiếp theo! 🚀

---

> *"Thất bại không phải là ngã xuống — thất bại là nằm im không đứng dậy."*
> — Nelson Mandela

---

*Nếu em đọc đến đây, em đã hiểu được một trong những nguyên tắc quan trọng nhất của kỹ thuật an toàn: **Fault Isolation**. Đây là thứ mà kỹ sư hàng không, y tế, và ô tô phải học suốt nhiều năm đại học — và em vừa nắm được ý tưởng cốt lõi chỉ trong một bài đọc. Tuyệt lắm!* 👏
