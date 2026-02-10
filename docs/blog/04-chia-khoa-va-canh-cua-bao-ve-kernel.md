---
lang: vi
title: "🔐 Chìa Khóa Và Cánh Cửa — Tại Sao Phần Mềm Cũng Cần 'Quyền Hạn'?"
tags: ["el0", "el1", "user mode", "kernel mode", "privilege level", "syscall", "isolation", "aegisos", "aarch64", "safety"]
description: "Câu chuyện về bức tường vô hình ngăn cách 'học sinh' và 'hiệu trưởng' bên trong mọi hệ điều hành — và cách AegisOS xây bức tường ấy để bảo vệ mạng người."
date: 2026-02-11
---

# 🔐 Chìa Khóa Và Cánh Cửa — Tại Sao Phần Mềm Cũng Cần "Quyền Hạn"?

> *Viết cho những bạn nhỏ lớp 5 đã cùng AegisOS học cách "nhớ", "chia sẻ thời gian", và "nói chuyện" — giờ đến lúc học cách "giữ khoảng cách an toàn".*

---

## 🚀 Mở đầu: Chiếc xe không người lái giữa Sài Gòn

Hãy tưởng tượng.

Năm 2040. Em là **kỹ sư phần mềm** tại một công ty xe tự lái ở Việt Nam. Chiếc xe của em chở bốn người — một gia đình nhỏ đang đi du lịch Đà Lạt.

Bên ngoài trời mưa. Đường đèo quanh co. Chiếc xe tự lái phải xử lý hàng trăm thứ cùng lúc: camera nhận diện làn đường, radar đo khoảng cách xe phía trước, GPS tìm đường, phanh tự động khi có chướng ngại vật...

Mỗi thứ đó là một **chương trình** — một "task" chạy trong máy tính của xe.

Bỗng nhiên, chương trình hiển thị nhạc bị lỗi. Một bug nhỏ xíu. Bình thường thì không sao — chỉ là nhạc bị tắt.

**Nhưng nếu chương trình nhạc có thể "sờ" vào chương trình phanh?**

Nếu nó vô tình ghi đè lên bộ nhớ của hệ thống phanh — chiếc xe mất phanh giữa đèo.

Bốn mạng người.

Chỉ vì một chương trình nhạc.

Đó là lý do vì sao các tiêu chuẩn an toàn quốc tế — **DO-178C** (máy bay), **ISO 26262** (ô tô), **IEC 62304** (thiết bị y tế) — đều yêu cầu một thứ gọi là **fault containment**: lỗi ở một phần không được lan sang phần khác.

Và cách đơn giản nhất để làm điều đó? **Phân quyền.**

Hôm nay, chúng ta sẽ tìm hiểu cách AegisOS xây **bức tường vô hình** giữa kernel và task — để dù một task có sập, kernel vẫn đứng vững.

---

## 🏫 Trường học và quyền hạn

### Hiệu trưởng vs. Học sinh

Em thử nghĩ nhé: ở trường em, ai cũng có vai trò riêng.

**Thầy hiệu trưởng** có quyền:
- Mở cửa mọi phòng
- Dùng loa phát thanh
- Thay đổi thời khóa biểu
- Quyết định ai được vào, ai phải ra

**Học sinh** thì không:
- ❌ Không được vào phòng giáo viên
- ❌ Không được tự ý đổi thời khóa biểu
- ❌ Không được dùng loa phát thanh
- ❌ Không được mở tủ hồ sơ

Nhưng học sinh **muốn** làm một số việc — ví dụ muốn gọi điện cho phụ huynh. Thì sao?

**Giơ tay xin phép.**

Học sinh nói: "Thưa thầy, em muốn gọi điện cho mẹ ạ." Thầy hiệu trưởng kiểm tra — ok, hợp lệ — rồi thầy **làm giúp**. Học sinh không tự lấy điện thoại của trường.

Bên trong máy tính, chuyện xảy ra **y hệt vậy**.

| Ở trường | Trong máy tính |
|---|---|
| Thầy hiệu trưởng | **Kernel** — bộ não điều hành |
| Học sinh | **Task** (chương trình) — ứng dụng đang chạy |
| Phòng giáo viên, tủ hồ sơ | **Bộ nhớ kernel**, thanh ghi hệ thống, thiết bị phần cứng |
| "Giơ tay xin phép" | **Syscall** — lời gọi hệ thống |
| Kiểm tra hợp lệ, làm giúp | Kernel xử lý syscall, trả kết quả |

---

## 🏰 Hai "tầng lầu" trong chip ARM

### EL1 và EL0 — Tầng VIP và Tầng thường

Chip ARM (loại chip mà điện thoại em đang dùng, và cũng là chip AegisOS chạy trên) có một thiết kế rất thông minh: nó chia quyền hạn thành **các tầng**, gọi là **Exception Level** (mức ngoại lệ — tức là "mức quyền hạn").

Hai tầng quan trọng nhất:

| Tầng | Tên | Ai sống ở đây? | Quyền hạn |
|---|---|---|---|
| **EL1** | Kernel mode | Kernel — "thầy hiệu trưởng" | Toàn quyền: đọc/ghi mọi bộ nhớ, dùng mọi thiết bị, thay đổi cài đặt hệ thống |
| **EL0** | User mode | Task — "học sinh" | Hạn chế: chỉ đọc/ghi bộ nhớ **của mình**, không sờ được thiết bị, không thay đổi hệ thống |

Phần cứng — con chip silicon thật sự — **kiểm tra** mỗi lệnh. Nếu một task ở EL0 cố đọc bộ nhớ kernel, chip sẽ **dừng task đó ngay lập tức** và báo cho kernel: "Có đứa vi phạm!"

Giống như cửa phòng giáo viên **có khóa điện tử** — không phải thầy hiệu trưởng đứng canh, mà **ổ khóa tự động** ngăn cản. Nhanh, chắc chắn, không bao giờ ngủ gật.

### Tại sao phải dùng phần cứng?

Em hỏi: "Sao không dùng phần mềm để kiểm tra?"

Câu trả lời: vì phần mềm có thể bị lỗi. Nếu chương trình kiểm tra bị hack hoặc bị ghi đè, thì không còn ai canh nữa. Nhưng phần cứng — con chip — không thể bị sửa bằng code. Nó được "đúc" sẵn khi sản xuất.

Đó gọi là **hardware-enforced isolation** — cách ly bằng phần cứng. Và đó là lý do chip ARM có EL0/EL1.

---

## 🔑 Giơ tay xin phép — Syscall là gì?

OK, vậy task ở EL0 bị "nhốt" — không được sờ thiết bị, không được đọc bộ nhớ kernel. Nhưng task vẫn cần **làm việc** chứ! Ví dụ, task muốn in chữ ra màn hình (qua UART — thiết bị giao tiếp nối tiếp).

Task không thể tự ghi vào UART vì UART nằm ở vùng nhớ **chỉ EL1 mới sờ được** (địa chỉ `0x0900_0000`, với quyền `AP_RW_EL1`).

Vậy task làm thế nào?

**Gọi syscall.**

Hãy tưởng tượng cuộc hội thoại này:

> **Task A** (đang ở EL0): "Thưa kernel, em muốn in chữ 'A:PING' ra màn hình ạ."
>
> *Task A giơ tay — thực hiện lệnh `SVC` (Supervisor Call). CPU lập tức "nâng cấp" lên EL1.*
>
> **Kernel** (ở EL1): "Để tôi kiểm tra... Chữ nằm ở đâu? Địa chỉ `0x4008_xxxx`. Hợp lệ — nằm trong vùng nhớ cho phép. Độ dài 7 byte. OK, dưới giới hạn 256 byte. Tôi sẽ in giúp."
>
> *Kernel đọc từng byte từ bộ nhớ của task, ghi vào UART.*
>
> **Kernel**: "Xong rồi. Em về đi."
>
> *Kernel thực hiện `ERET` (Exception Return). CPU "hạ cấp" về EL0. Task A tiếp tục chạy.*

Toàn bộ quá trình diễn ra trong **vài micro-giây** — nhanh hơn một cái chớp mắt hàng nghìn lần.

### Các syscall trong AegisOS

AegisOS hiện có **5 syscall** — 5 cách để task "giơ tay xin phép":

| Số | Tên | Ý nghĩa | Ví dụ đời thật |
|---|---|---|---|
| 0 | `SYS_YIELD` | "Em nhường lượt cho bạn khác" | Giơ tay nói "Em xong rồi, bạn khác làm đi" |
| 1 | `SYS_SEND` | "Em gửi thư cho bạn" | Đưa thư cho thầy để thầy chuyển |
| 2 | `SYS_RECV` | "Em chờ thư" | Giơ tay hỏi "Có thư cho em không ạ?" |
| 3 | `SYS_CALL` | "Em gửi thư và chờ thư trả lời" | Gửi + chờ — như hỏi thầy câu hỏi và đợi trả lời |
| 4 | `SYS_WRITE` | "Em muốn in chữ ra màn hình" | Nhờ thầy viết lên bảng |

Mỗi syscall hoạt động theo cùng một nguyên tắc: task **không tự làm** — task **nhờ kernel làm giúp**.

---

## 🧱 Xây bức tường — Kỹ thuật chi tiết (đọc chậm lại nhé!)

Phần này hơi khó, nhưng em cứ đọc chậm lại. Chúng ta sẽ đi qua **ba bước** mà AegisOS dùng để xây bức tường giữa kernel và task.

### Bước 1: Nói cho chip biết "task là học sinh"

Khi kernel khởi tạo một task, nó đặt một giá trị đặc biệt gọi là **SPSR** (Saved Program Status Register — thanh ghi trạng thái chương trình đã lưu).

SPSR giống như **thẻ học sinh** — nó cho chip biết: "Khi task này chạy, hãy đặt nó ở EL0."

Trong AegisOS, giá trị này là `0x000` — nghĩa là **EL0t** (Exception Level 0, dùng SP_EL0). Trước đây, tất cả task đều chạy ở EL1 với giá trị `0x345` — giống như mọi học sinh đều có chìa khóa phòng hiệu trưởng. Nguy hiểm!

Khi kernel thực hiện lệnh **ERET** (Exception Return — "quay về từ ngoại lệ"), chip đọc SPSR và tự động "hạ cấp" xuống EL0. Task bắt đầu chạy mà **không biết** mình đang bị giới hạn — giống như học sinh vào trường, không thấy ổ khóa điện tử ở cửa phòng giáo viên, cứ vui vẻ học bài.

### Bước 2: Chia bộ nhớ thành "vùng an toàn"

Ở bài trước, chúng ta đã biết MMU (Memory Management Unit — bộ phận quản lý bộ nhớ) giống như **sổ địa chỉ**. Bây giờ, sổ địa chỉ đó có thêm một cột mới: **"Ai được vào?"**

| Vùng nhớ | Chứa gì? | EL1 (Kernel) | EL0 (Task) |
|---|---|---|---|
| `.text` | Code chương trình | ✅ Đọc + Chạy | ✅ Đọc + Chạy |
| `.rodata` | Dữ liệu chỉ đọc (chuỗi chữ) | ✅ Đọc | ✅ Đọc |
| `.data`, `.bss` | Dữ liệu kernel | ✅ Đọc + Ghi | ❌ Cấm hoàn toàn |
| `.task_stacks` | Stack kernel (dùng khi xử lý syscall) | ✅ Đọc + Ghi | ❌ Cấm hoàn toàn |
| `.user_stacks` | Stack của task (biến cục bộ) | ✅ Đọc + Ghi | ✅ Đọc + Ghi |
| UART, GIC | Thiết bị phần cứng | ✅ Đọc + Ghi | ❌ Cấm hoàn toàn |

Nhìn thấy không? Task ở EL0 **chỉ** sờ được code chung, chuỗi chữ, và stack riêng của mình. Mọi thứ khác — dữ liệu kernel, thiết bị — đều bị **khóa**.

Cách khóa? Bằng **AP bits** (Access Permission — quyền truy cập) trong Page Table. Mỗi trang nhớ 4KB có 2 bit AP quyết định ai được đọc, ai được ghi:

- `AP = 00` → Chỉ EL1 đọc/ghi. EL0? **Permission Fault** — bị bắt.
- `AP = 01` → Cả EL1 và EL0 đọc/ghi.
- `AP = 11` → Cả EL1 và EL0 chỉ đọc.

AegisOS dùng `AP = 00` cho dữ liệu kernel và thiết bị, `AP = 01` cho user stack, và `AP = 11` cho code chung.

### Bước 3: Hai ngăn kéo — Stack kernel và Stack task

Đây là phần tinh tế nhất.

Mỗi task có **hai** stack (ngăn xếp — giống chồng đĩa):

- **User stack** (`SP_EL0`): Task dùng khi chạy bình thường ở EL0. Nằm trong vùng `.user_stacks`.
- **Kernel stack** (`SP_EL1`): Kernel dùng khi xử lý syscall hoặc interrupt cho task đó.

Tại sao cần hai? Vì khi task gọi syscall, CPU "nâng cấp" lên EL1. Lúc đó, nếu kernel dùng **cùng stack** với task, thì task có thể phá stack kernel bằng cách để tràn user stack trước khi gọi syscall. Nguy hiểm!

Giống như: thầy hiệu trưởng có **ngăn kéo riêng** để cất hồ sơ. Học sinh không biết ngăn kéo đó ở đâu, không sờ được, không phá được.

Trong AegisOS, khi exception xảy ra từ EL0, macro `SAVE_CONTEXT_LOWER` sẽ **ngay lập tức** chuyển SP sang kernel boot stack (16KB, ở địa chỉ `__stack_end`), **trước khi** làm bất kỳ thứ gì khác. Mọi thứ kernel làm sau đó đều nằm trên "ngăn kéo của thầy".

---

## 💥 Chuyện gì xảy ra khi task "vi phạm"?

Đây là lúc mọi thứ trở nên thú vị.

Khi AegisOS lần đầu chạy task ở EL0, task cố in chữ ra UART bằng cách **trực tiếp ghi vào địa chỉ `0x0900_0000`** — giống như học sinh cầm phấn tự lên bảng viết, không xin phép.

Và chip ARM **bắt ngay lập tức**.

Kernel nhận được thông báo: "**Permission Fault — Data Abort — Lower EL**". Nghĩa là: "Có một chương trình ở EL0 cố ghi vào vùng nhớ mà nó không có quyền."

Lúc đó, AegisOS in ra thông báo lỗi rồi dừng lại. Nhưng điều quan trọng là: **kernel không bị ảnh hưởng**. Kernel vẫn sống. Kernel vẫn khỏe. Chỉ task vi phạm bị "bắt".

Đó chính là **fault containment** — gói gọn lỗi. Lỗi ở đâu, ở yên đó. Không lan.

Và đó là điều mà **DO-178C** (tiêu chuẩn phần mềm máy bay), **ISO 26262** (tiêu chuẩn xe ô tô), và **IEC 62304** (tiêu chuẩn thiết bị y tế) đều yêu cầu. Ba bộ tiêu chuẩn này được viết bởi hàng trăm kỹ sư trên thế giới, qua hàng chục năm kinh nghiệm xương máu, và tất cả đều nói cùng một điều:

**"Lỗi ở một phần không được giết chết toàn bộ hệ thống."**

---

## 🛠️ Chúng ta đã làm được gì trong AegisOS?

Hãy nhìn lại hành trình của chúng ta:

### Trước Phase D (nguy hiểm!)

```
Task A ──── chạy ở EL1 ──── đọc/ghi MỌI THỨ
Task B ──── chạy ở EL1 ──── đọc/ghi MỌI THỨ
Kernel ──── chạy ở EL1 ──── không ai bảo vệ
```

Mọi task đều là "hiệu trưởng". Ai cũng mở được mọi cửa. Nếu Task A lỗi → cả hệ thống sập.

### Sau Phase D (an toàn!)

```
Task A ──── chạy ở EL0 ──── chỉ sờ được stack riêng + code chung
Task B ──── chạy ở EL0 ──── chỉ sờ được stack riêng + code chung
Kernel ──── chạy ở EL1 ──── được phần cứng bảo vệ
         ↑
    Syscall (SVC) ─── cửa duy nhất để task nói chuyện với kernel
```

Bốn file chính đã thay đổi:

📁 **Cấu trúc project**:
```
src/
  ├── main.rs        ← thêm user_print(), syscall_write() cho EL0
  ├── sched.rs       ← SPSR 0x345→0x000, tách user stack / kernel stack
  ├── exception.rs   ← SAVE_CONTEXT_LOWER, SYS_WRITE handler
  ├── mmu.rs         ← AP bits: SHARED_CODE_PAGE, USER_DATA_PAGE
  ├── ipc.rs
  ├── gic.rs
  └── timer.rs
linker.ld            ← thêm .user_stacks (3×4KB)
```

Và kết quả? Khi chạy trên QEMU:

```
[AegisOS] boot
[AegisOS] MMU enabled (identity map)
[AegisOS] W^X enforced (WXN + 4KB pages)
[AegisOS] exceptions ready
[AegisOS] scheduler ready (3 tasks, EL0)
[AegisOS] bootstrapping into task_a (EL0)...
A:PING B:PONG A:PING B:PONG A:PING B:PONG ...
```

Nhìn bình thường — chẳng khác gì trước. Nhưng bên dưới, **mọi thứ đã thay đổi**. Task không còn là "hiệu trưởng" nữa. Task là "học sinh" — chỉ làm được những gì kernel cho phép, qua syscall.

Giống như: trường vẫn hoạt động bình thường, học sinh vẫn vui vẻ, nhưng giờ đây **cửa phòng giáo viên đã có ổ khóa**.

---

## 🧩 Bài toán khó nhất: Chuyển stack khi bị "gọi lên"

Có một bài toán kỹ thuật rất thú vị mà chúng ta phải giải trong Phase D. Em thử đọc xem nhé — hơi khó, nhưng rất "phê" khi hiểu được!

Khi task đang chạy ở EL0 và chip nhận được interrupt (chuông cửa!) hoặc syscall (giơ tay!), CPU tự động chuyển lên EL1. Nhưng lúc đó, **SP** (Stack Pointer — con trỏ ngăn xếp) vẫn là SP cũ.

Kernel cần **đổi sang stack kernel** ngay lập tức — trước khi lưu bất cứ thứ gì. Nhưng để đổi stack, kernel cần **dùng một thanh ghi** để chứa địa chỉ stack mới. Mà thanh ghi nào cũng đang chứa dữ liệu của task — nếu ghi đè lên, dữ liệu đó mất!

Bài toán con gà - quả trứng: cần đổi stack trước khi lưu, nhưng cần lưu trước khi đổi stack.

**Giải pháp của AegisOS?** Dùng một thanh ghi hệ thống đặc biệt: **TPIDR_EL1** (Thread Pointer ID Register — thanh ghi định danh luồng). Thanh ghi này bình thường kernel không dùng, nên nó là chỗ "ký gửi" hoàn hảo.

Quá trình diễn ra thế này:

1. ✍️ Cất giá trị x9 vào TPIDR_EL1 (ký gửi tạm)
2. 📍 Nạp địa chỉ kernel stack vào x9
3. 🔄 Đổi SP sang kernel stack
4. 💾 Bắt đầu lưu tất cả thanh ghi... đến lượt x9, lấy giá trị **thật** từ TPIDR_EL1 ra

Như vậy, không mất bất cứ dữ liệu nào. Không thanh ghi nào bị ghi đè sai.

Giống như: em cần dọn bàn để bày bài mới. Nhưng trên bàn có ly nước, không có chỗ đặt. Em **đưa ly nước cho bạn cầm hộ**, dọn bàn xong, lấy ly nước lại. Đơn giản nhưng hiệu quả!

---

## 🌟 Người Thật, Chuyện Thật

Em biết **Linus Torvalds** không?

Năm 1991, khi còn là sinh viên 21 tuổi ở Phần Lan, Linus tạo ra **Linux** — hệ điều hành mã nguồn mở nổi tiếng nhất thế giới. Và một trong những thứ **đầu tiên** Linux làm là tách User mode / Kernel mode — chính xác là thứ chúng ta vừa làm trong AegisOS.

Linus viết trên mailing list: "Tôi đang làm một hệ điều hành miễn phí... chỉ là sở thích, sẽ không lớn và chuyên nghiệp như GNU."

Linux giờ chạy trên **96% máy chủ thế giới**, mọi điện thoại Android, mọi siêu máy tính trong top 500, và cả trạm vũ trụ quốc tế ISS.

Tất cả bắt đầu từ một sinh viên, một sở thích, và quyết tâm xây đúng nền tảng — bao gồm User/Kernel Separation.

AegisOS nhỏ bé hơn Linux rất nhiều. Nhưng bức tường EL0/EL1 mà chúng ta vừa xây? **Cùng nguyên lý.** Cùng ý tưởng. Cùng lý do.

---

## 🎯 Bước Tiếp Theo

AegisOS giờ đã biết:
- ✅ **"Nhớ"** — ai ở đâu, ai được làm gì (MMU + Page Table)
- ✅ **"Chia sẻ"** — luân phiên giữa nhiều task (Scheduler)
- ✅ **"Nói chuyện"** — task gửi thư cho nhau (IPC + Syscall)
- ✅ **"Giữ khoảng cách"** — task không sờ được kernel (EL0 Isolation)

Nhưng khi một task **crash** — ví dụ chia cho 0, hoặc nhảy vào địa chỉ sai — hiện tại kernel in lỗi rồi... **dừng toàn bộ hệ thống**. Tất cả task khác cũng chết theo.

Giống như ở trường: một bạn ngã ở sân, và thầy hiệu trưởng tuyên bố **cả trường nghỉ học**. Vô lý phải không?

Bước tiếp theo, chúng ta sẽ dạy AegisOS **Fault Isolation** — khi một task crash, kernel chỉ đánh dấu task đó là "đã hỏng", rồi tiếp tục chạy các task còn lại. Và có thể — **khởi động lại** task bị hỏng từ đầu.

Giống như: bạn ngã ở sân → y tá băng bó → các lớp khác vẫn học bình thường. Đó mới là cách một hệ thống an toàn hoạt động.

Nghe hấp dẫn không? 🚀

---

> *"Tự do thật sự không phải là làm bất cứ thứ gì mình muốn — mà là biết rằng sai lầm của mình sẽ không hại đến người khác."*

---

*Nếu em đọc đến đây, em đã hiểu được Exception Level, User/Kernel Separation, Syscall, Access Permission, và Fault Containment. Đó là nền tảng bảo mật của mọi hệ điều hành hiện đại — từ điện thoại trong túi em đến máy bay trên trời. Em không chỉ đang đọc — em đang **hiểu cách thế giới vận hành**. Và điều đó thật phi thường.* ✨
