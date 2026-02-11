# 🎫 Giấy Phép Cho Phần Mềm — Ai Được Làm Gì?

> *Bài #7 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Capability — hệ thống "giấy phép" kiểm soát từng hành động của từng chương trình.*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2043. Em là kỹ sư phần mềm cho một hãng xe tự lái.

Chiếc xe của em có hàng chục chương trình chạy cùng lúc. Mỗi chương trình lo một việc:

- **Chương trình camera** — nhận hình ảnh từ camera, nhận diện biển báo.
- **Chương trình phanh** — điều khiển phanh, giữ an toàn hành khách.
- **Chương trình nhạc** — phát bài hát cho tài xế nghe.

Ba chương trình này chạy trên **cùng một máy tính** bên trong xe.

Một hôm, em phát hiện một lỗi kinh hoàng:

> Chương trình nhạc... có thể gửi lệnh đến **hệ thống phanh**.

Tưởng tượng đi. Em đang nghe nhạc trên xe. Bài hát chuyển sang bản tiếp theo. Và đúng lúc đó — do một lỗi nhỏ trong phần mềm nhạc — nó gửi một tin nhắn sai đến hệ thống phanh. Xe **bỗng dưng dừng lại** giữa đường cao tốc.

Xe phía sau không kịp phanh.

*Tại sao chương trình nhạc lại được phép nói chuyện với hệ thống phanh?* Nó **không nên** có quyền đó. Giống như một học sinh lớp 5 không nên có chìa khóa phòng hiệu trưởng vậy.

**Nhưng nếu không ai kiểm soát, thì ai cũng có thể làm bất cứ điều gì.**

Đó chính là vấn đề mà AegisOS vừa gặp. Và hôm nay, chúng ta sẽ giải quyết nó.

---

## 🏫 Thẻ Ra Vào Ở Trường Học

### Trường học có quy tắc

Em thử nghĩ về trường mình nhé.

Ở trường, không phải ai muốn đi đâu cũng được:

- **Phòng thí nghiệm** — chỉ ai có giờ thực hành mới được vào. Phải có **thẻ ra vào** do thầy cô cấp.
- **Phòng y tế** — ai cũng được vào khi bị ốm, nhưng không ai được tự ý lấy thuốc.
- **Phòng máy tính** — chỉ vào đúng tiết Tin học, và phải có thầy giám sát.
- **Sân trường** — ai cũng được chơi trong giờ ra chơi.

Mỗi học sinh có một **tập quyền hạn** khác nhau. Lớp trưởng có thể vào phòng giáo viên lấy sổ đầu bài, nhưng các bạn khác thì không.

Bây giờ tưởng tượng: nếu trường **không có bất kỳ quy tắc nào**. Ai muốn vào đâu cũng được. Ai muốn lấy gì cũng được.

Hỗn loạn đúng không?

**Máy tính cũng vậy.**

### AegisOS trước Phase G: "Ai cũng được làm mọi thứ"

Trước hôm nay, AegisOS có 5 loại "hành động" mà chương trình có thể làm (gọi là **syscall** — lời gọi hệ thống):

| Syscall | Ý nghĩa | Ví dụ đời thật |
|---|---|---|
| **YIELD** | "Tôi nghỉ, cho bạn khác chạy" | Giơ tay nhường lượt phát biểu |
| **SEND** | Gửi tin nhắn cho chương trình khác | Gửi thư cho bạn cùng lớp |
| **RECV** | Nhận tin nhắn | Mở thư ra đọc |
| **CALL** | Gửi thư rồi đợi thư trả lời | Hỏi bạn một câu, đợi trả lời |
| **WRITE** | In chữ ra màn hình (UART) | Viết lên bảng |

Và AegisOS có 2 **endpoint** (hòm thư) — nơi các chương trình gửi/nhận tin nhắn.

Vấn đề là: **tất cả** chương trình đều có thể gọi **tất cả** syscall, trên **tất cả** hòm thư.

Chương trình nhạc có thể gửi tin nhắn trên hòm thư của hệ thống phanh. Chương trình idle (chương trình "ngủ", chỉ đợi) có thể in chữ ra UART. Bất kỳ ai cũng làm được bất kỳ điều gì.

> Giống như một trường học mà **mọi học sinh đều có chìa khóa của mọi phòng**. 🔑🔑🔑

Nguy hiểm!

---

## 🎫 Giải Pháp: Hệ Thống Giấy Phép (Capability)

### Ý tưởng

Giải pháp rất đơn giản — giống hệt cách trường học hoạt động:

> **Mỗi chương trình được cấp một "thẻ"**. Trên thẻ ghi rõ chương trình đó được phép làm những gì. Trước khi làm bất kỳ điều gì, kernel kiểm tra thẻ. Không có quyền? **Từ chối.**

Trong khoa học máy tính, "thẻ" này gọi là **Capability** (đọc là "kê-pơ-bi-li-ti", nghĩa là "khả năng" hay "quyền hạn").

### Thẻ trông như thế nào?

Thẻ của mỗi chương trình là **một dãy 64 ô**, mỗi ô đánh dấu ✅ hoặc ❌.

Mỗi ô đại diện một quyền cụ thể:

| Ô số | Quyền | Ý nghĩa |
|---|---|---|
| 0 | `IPC_SEND_EP0` | Được gửi thư trên hòm thư #0 |
| 1 | `IPC_RECV_EP0` | Được nhận thư từ hòm thư #0 |
| 2 | `IPC_SEND_EP1` | Được gửi thư trên hòm thư #1 |
| 3 | `IPC_RECV_EP1` | Được nhận thư từ hòm thư #1 |
| 4 | `WRITE` | Được in chữ ra màn hình |
| 5 | `YIELD` | Được nhường lượt |
| 6–63 | *Dự trữ* | Để dành cho tương lai |

Ví dụ, thẻ của **task_a** (chương trình PING) trông như thế này:

```
Ô:    0  1  2  3  4  5
      ✅ ✅ ❌ ❌ ✅ ✅
```

Nghĩa là: task_a được gửi/nhận trên hòm thư #0, được in chữ, được nhường lượt. Nhưng **không được** dùng hòm thư #1.

Còn thẻ của **idle** (chương trình ngủ):

```
Ô:    0  1  2  3  4  5
      ❌ ❌ ❌ ❌ ❌ ✅
```

Idle **chỉ** được nhường lượt. Không được gửi thư. Không được in chữ. Không được làm gì khác. Đúng rồi — nó chỉ cần ngủ thôi mà!

### Tại sao dùng dãy ô 0/1?

Trong máy tính, mỗi ô 0/1 gọi là một **bit**. Một dãy 64 bit = một số `u64` (số nguyên không dấu 64 bit).

Vậy thẻ capability chỉ là **một con số**! Chỉ tốn **8 byte** cho mỗi chương trình. Ba chương trình = 24 byte. Ít hơn một dòng chữ em đang đọc.

Và việc kiểm tra quyền? Chỉ cần **một phép tính** mà CPU làm trong chưa đầy 1 nano-giây (một phần tỷ giây):

```
(thẻ_của_task & quyền_cần_có) == quyền_cần_có
```

Nếu kết quả đúng → được phép. Nếu sai → từ chối.

Phép tính `&` ở đây gọi là **AND** (phép VÀ) — giống như kiểm tra: "Ô này có đánh dấu ✅ **VÀ** ô kia cũng ✅ không?"

Phần này hơi khó, nhưng em cứ nhớ ý chính: **kiểm tra giấy phép cực kỳ nhanh** — nhanh đến mức không ảnh hưởng gì đến tốc độ hệ thống.

---

## 🔍 Đi Sâu Hơn — Tại Sao Điều Này Cứu Mạng Người?

### Nguyên tắc "Least Privilege" — Ít quyền nhất có thể

Trong an toàn phần mềm, có một nguyên tắc vàng:

> **Least Privilege — Mỗi chương trình chỉ được cấp đúng những quyền mà nó cần. Không hơn.**

Tại sao? Vì nếu chương trình bị lỗi (và chương trình **luôn** có thể bị lỗi), thì:

- Lỗi ít quyền → thiệt hại nhỏ 🟢
- Lỗi nhiều quyền → thiệt hại lớn 🔴

Quay lại ví dụ xe tự lái:

| Tình huống | Chương trình nhạc bị lỗi | Hậu quả |
|---|---|---|
| **Không có capability** | Nhạc gửi lệnh sai → phanh dừng xe | 💀 Tai nạn |
| **Có capability** | Nhạc cố gửi lệnh → kernel kiểm tra thẻ → DENIED → nhạc bị "đuổi" ra → phanh vẫn hoạt động bình thường | ✅ An toàn |

Đó là sức mạnh của capability: **một lỗi nhỏ không thể lan ra thành thảm họa lớn**.

### Thế giới thật dùng capability

- **seL4** — hệ điều hành microkernel nổi tiếng nhất thế giới (được dùng trong trực thăng quân sự, drone, xe tự lái) — dùng hệ thống capability cực kỳ phức tạp gọi là **CSpace** (Capability Space). Mỗi quyền là một "vé" riêng biệt, có thể truyền từ chương trình này sang chương trình khác.

- **AegisOS** của chúng ta dùng cách đơn giản hơn: **bitmask** (dãy bit). Tại sao? Vì AegisOS chỉ có 3 chương trình tĩnh, không cần phức tạp như seL4. Nhưng **nguyên tắc giống hệt nhau**: kiểm tra quyền trước khi cho phép hành động.

- **Android** cũng dùng capability! Mỗi ứng dụng phải **xin quyền**: quyền dùng camera, quyền đọc danh bạ, quyền truy cập vị trí... Em đã bao giờ thấy cửa sổ "Cho phép ứng dụng truy cập camera?" chưa? Đó chính là capability system!

---

## ⚙️ Kỹ Thuật — Nhưng Dễ Hiểu

### Ba bước bảo vệ

Khi một chương trình muốn làm gì đó (ví dụ: gửi tin nhắn), đây là những gì xảy ra bên trong AegisOS:

**Bước 1: Chương trình gọi syscall**

Chương trình task_a muốn gửi tin nhắn trên hòm thư #0. Nó gọi: "Hệ thống ơi, cho tôi SEND trên endpoint 0!"

**Bước 2: Kernel kiểm tra giấy phép**

Kernel tra "thẻ" của task_a. Hàm `cap_for_syscall` nói: "SEND trên endpoint 0 cần quyền `IPC_SEND_EP0` (ô số 0)." Kernel nhìn thẻ — ô số 0 có ✅ không? **Có!** → Cho phép.

**Bước 3: Thực thi hoặc từ chối**

- Nếu **có quyền** → kernel chạy syscall bình thường.
- Nếu **không có quyền** → kernel in ra: `"!!! CAP DENIED"` → đánh dấu chương trình đó là **bị lỗi** (Faulted) → tự động khởi động lại sau 1 giây.

### Đoạn hội thoại minh họa

Hãy tưởng tượng kernel là **bảo vệ trường học**, và các chương trình là **học sinh**:

> **Task_a (PING):** Bác bảo vệ ơi, cho em vào phòng thư #0 gửi thư ạ!
>
> **Kernel (Bảo vệ):** *(nhìn thẻ)* Em có quyền "Gửi thư phòng #0"... ✅ Được, vào đi.
>
> **Task_a:** Cảm ơn bác!

---

> **Idle (Ngủ):** Bác bảo vệ ơi, cho em vào phòng thư #0 gửi thư ạ!
>
> **Kernel (Bảo vệ):** *(nhìn thẻ)* Em ơi, thẻ em chỉ có quyền "Nhường lượt" thôi. Phòng thư #0 em không được vào.
>
> **Idle:** Nhưng em muốn...
>
> **Kernel:** ❌ DENIED. Em ra ngoài ngồi đợi. Và bác sẽ báo cho thầy hiệu trưởng.
>
> *(Idle bị đánh dấu "Faulted" — 1 giây sau sẽ được khởi động lại, nhưng vẫn chỉ có quyền cũ)*

### Tại sao từ chối = lỗi nghiêm trọng?

Em có thể hỏi: "Sao không trả lỗi cho chương trình tự xử lý?"

Lý do: trong hệ thống an toàn cao (safety-critical), nếu một chương trình gọi syscall mà nó **không có quyền**, điều đó có nghĩa là **chương trình bị lỗi thiết kế**. Đây không phải tình huống bình thường — đây là **lỗi phần mềm** (software defect).

Giống như nếu một học sinh cố phá khóa phòng hiệu trưởng — đó không phải "xin phép bị từ chối". Đó là **vi phạm nghiêm trọng** cần xử lý ngay.

Vì vậy, AegisOS **fault** (đánh dấu lỗi) chương trình vi phạm, rồi tự động khởi động lại nó. An toàn hơn là để nó tiếp tục chạy với trạng thái không xác định.

---

## 🛠️ Chúng Ta Đã Làm Được Gì Trong AegisOS?

### Cấu trúc code mới

Phase G thêm một module hoàn toàn mới vào AegisOS:

```
src/
├── boot.s          ← Khởi động (Phase A)
├── mmu.rs          ← Bộ nhớ (Phase B)
├── exception.rs    ← Xử lý ngoại lệ (Phase C)
├── sched.rs        ← Thời khóa biểu (Phase C)
├── ipc.rs          ← Gửi/nhận thư (Phase C)
├── timer.rs        ← Đồng hồ báo thức (Phase C)
├── gic.rs          ← Bộ điều phối ngắt (Phase C)
├── uart.rs         ← "Hai cái lon nối dây" (Phase A)
├── cap.rs          ← 🆕 Giấy phép! (Phase G)
├── main.rs         ← Hàm chính + task entries
└── lib.rs          ← Khai báo module
```

### File mới: cap.rs — "Quyển sổ giấy phép"

File [src/cap.rs](src/cap.rs) chỉ khoảng 100 dòng, nhưng chứa toàn bộ hệ thống capability:

- **6 hằng số quyền** — mỗi quyền là một bit riêng biệt (bit 0, bit 1, ..., bit 5)
- **`cap_check()`** — hàm kiểm tra: "Task này có đủ quyền không?" Chỉ một phép AND, cực nhanh.
- **`cap_for_syscall()`** — hàm tra bảng: "Syscall này cần quyền gì?" Ví dụ: SEND trên endpoint 0 → cần bit 0. CALL trên endpoint 0 → cần **cả** bit 0 và bit 1 (vì CALL = gửi + nhận).
- **`cap_name()`** — hàm đặt tên: khi từ chối, in ra "IPC_SEND_EP0" thay vì "bit 0" cho dễ đọc.

### Thay đổi trong sched.rs — "Dán thẻ lên mỗi task"

Mỗi chương trình (task) có một **TCB** (Task Control Block — "hồ sơ chương trình"). Chúng ta thêm một trường mới vào hồ sơ:

```
TCB (trước Phase G):          TCB (sau Phase G):
┌──────────────┐              ┌──────────────┐
│ context      │              │ context      │
│ state        │              │ state        │
│ id           │              │ id           │
│ stack_top    │              │ stack_top    │
│ entry_point  │              │ entry_point  │
│ user_stack   │              │ user_stack   │
│ fault_tick   │              │ fault_tick   │
└──────────────┘              │ caps  🆕     │  ← 8 byte, dãy 64 bit
                              └──────────────┘
```

Trường `caps` nằm cuối struct, nên **không làm xáo trộn** vị trí các trường cũ. Tất cả code cũ vẫn hoạt động bình thường.

### Thay đổi trong exception.rs — "Bảo vệ kiểm tra thẻ"

Trong hàm `handle_svc` (nơi kernel xử lý mọi syscall), chúng ta thêm một bước kiểm tra **ngay trước** khi thực thi:

```
TRƯỚC Phase G:                      SAU Phase G:

Task gọi syscall                    Task gọi syscall
       ↓                                  ↓
                                    Tra thẻ capability
                                          ↓
                                    Có quyền? ─── Không ──→ FAULT!
                                       │
                                      Có
                                       ↓
Thực thi syscall                    Thực thi syscall
```

Chỉ thêm 10 dòng code. Nhưng 10 dòng đó **bảo vệ toàn bộ hệ thống**.

### Thay đổi trong main.rs — "Phát thẻ cho mỗi task"

Trong hàm `kernel_main()`, sau khi khởi tạo scheduler, kernel **phát thẻ** cho từng task:

| Task | Vai trò | Quyền được cấp |
|---|---|---|
| task_a | PING (gửi/nhận thư) | Gửi EP0 ✅ • Nhận EP0 ✅ • Viết ✅ • Nhường ✅ |
| task_b | PONG (nhận/gửi thư) | Gửi EP0 ✅ • Nhận EP0 ✅ • Viết ✅ • Nhường ✅ |
| idle | Ngủ | Nhường ✅ *(chỉ vậy thôi!)* |

Khi QEMU khởi động, em sẽ thấy dòng mới:

```
[AegisOS] capabilities assigned
```

Nghĩa là: mọi thẻ đã được phát. Hệ thống sẵn sàng.

### Điều kỳ diệu: thẻ tồn tại sau khi restart

Nhớ bài #5 không? Khi một task bị lỗi, AegisOS tự động restart nó sau 1 giây.

Câu hỏi: khi restart, thẻ capability có bị mất không?

**Không!** Hàm `restart_task()` chỉ xóa **context** (trạng thái CPU) và đặt lại **entry point** (điểm bắt đầu). Trường `caps` nằm ngoài phạm vi xóa → **tự động được giữ nguyên**.

Điều này rất quan trọng: capability là **chính sách tĩnh** — được gán một lần bởi kernel, tồn tại suốt đời task. Task bị lỗi restart lại vẫn chỉ có đúng những quyền cũ. Không hơn, không kém.

---

## 🧪 Kiểm Tra — 69 Bài Test Đều Đạt!

Nhớ bài #6 không? Chúng ta có 55 bài test. Phase G thêm **14 bài test mới**:

| Nhóm test | Kiểm tra gì | Số test |
|---|---|---|
| Bit constants | 6 quyền phải khác nhau, mỗi quyền đúng 1 bit | 1 |
| cap_check logic | Kiểm tra phép AND hoạt động đúng | 3 |
| cap_for_syscall | Mỗi syscall + endpoint → đúng quyền cần thiết | 5 |
| cap_name | Tên quyền in ra đúng tiếng Anh | 1 |
| TCB integration | Thẻ mới = 0 trong EMPTY_TCB | 1 |
| Restart survival | Thẻ vẫn còn sau khi task restart | 1 |
| CAP_ALL / CAP_NONE | Bao gồm tất cả / không có gì | 2 |

Tổng: **69 test, tất cả đạt** ✅

Và trên QEMU thật: **9 checkpoint boot đều pass** — bao gồm checkpoint mới `[AegisOS] capabilities assigned`.

---

## 💡 Truyền Cảm Hứng — Người Phát Minh Ra Capability

Câu chuyện về capability bắt đầu từ năm **1966** — khi một nhà khoa học máy tính tên **Jack Dennis** ở MIT (Viện Công nghệ Massachusetts) đặt câu hỏi:

> "Làm sao cho mỗi chương trình chỉ truy cập được đúng những gì nó cần?"

Ông phát minh ra khái niệm **capability** — một "vé" mà chương trình phải xuất trình trước khi truy cập bất kỳ tài nguyên nào.

Gần 60 năm sau, ý tưởng của ông vẫn là nền tảng của mọi hệ thống an toàn:

- **seL4** (2009) — microkernel được **chứng minh toán học** là đúng, dùng capability.
- **Google Fuchsia** (2016) — hệ điều hành mới của Google, dùng capability.
- **CHERI** (2014) — chip CPU do Cambridge phát triển, nhúng capability **vào phần cứng**.
- **AegisOS** (2026) — microkernel nhỏ bé của chúng ta, cũng dùng capability! 🎉

Jack Dennis bắt đầu nghiên cứu khi ông là **sinh viên** ở MIT. Ông tò mò, ông đặt câu hỏi, ông thử nghiệm. Và ý tưởng của ông thay đổi cả ngành khoa học máy tính.

Em cũng có thể bắt đầu như vậy. Tò mò. Đặt câu hỏi. Thử nghiệm.

---

## 🔮 Bước Tiếp Theo

AegisOS giờ đã biết:

- ✅ **"Nhớ"** — ai ở đâu, ai được làm gì (MMU + Page Table)
- ✅ **"Chia sẻ"** — luân phiên giữa nhiều task (Scheduler)
- ✅ **"Nói chuyện"** — task gửi thư cho nhau (IPC + Syscall)
- ✅ **"Giữ khoảng cách"** — task không sờ được kernel (EL0 Isolation)
- ✅ **"Đứng dậy khi ngã"** — cô lập lỗi + tự khởi động lại (Fault Isolation)
- ✅ **"Tự kiểm tra"** — 69 unit test + 9 boot checkpoint (Testing)
- ✅ **"Kiểm soát quyền"** — mỗi task chỉ làm được điều nó được phép ← MỚI!

Nhưng vẫn còn một câu hỏi:

> **"Nếu task_a bị hacker chiếm quyền điều khiển — nó có thể đọc bộ nhớ của task_b không?"**

Hiện tại, tất cả task chia chung một **bản đồ bộ nhớ** (identity map). Task_a và task_b nhìn thấy cùng một không gian địa chỉ. Capability kiểm soát **syscall**, nhưng không kiểm soát **bộ nhớ**.

Bước tiếp theo, chúng ta sẽ xây **Per-Task Address Space** — mỗi task có bản đồ bộ nhớ riêng. Task_a không thể nhìn thấy dữ liệu của task_b, dù có cố truy cập cùng địa chỉ.

Giống như mỗi lớp học có **phòng riêng**. Lớp 5A không nhìn thấy bảng của lớp 5B, dù hai phòng ở cạnh nhau.

Hẹn gặp em ở bài tiếp theo! 🚀

---

> *"Nguyên tắc ít quyền nhất (Least Privilege): mỗi chương trình chỉ nên có đúng những quyền cần thiết để hoàn thành nhiệm vụ — không hơn."*
> — Jerome Saltzer & Michael Schroeder, 1975

---

*Nếu em đọc đến đây, em đã hiểu được Capability, Least Privilege, Bitmask, và cách AegisOS kiểm soát quyền truy cập — những khái niệm mà kỹ sư bảo mật chuyên nghiệp dùng hàng ngày. Lần sau ai hỏi em "Tại sao điện thoại hay hỏi 'Cho phép ứng dụng truy cập camera?'", em có thể trả lời: "Vì mỗi ứng dụng cần có giấy phép. Đó gọi là capability." Ngầu lắm!* 👏
