# 🧪 Làm Sao Biết Hệ Thống An Toàn Thật — Chứ Không Chỉ "Thấy Chạy Được"?

> *Bài #6 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Testing — nghệ thuật tìm lỗi trước khi lỗi tìm ra người.*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2041. Em là kỹ sư phần mềm cho một công ty tên lửa.

Tên lửa của em sắp đưa 4 nhà du hành lên trạm vũ trụ. Bên trong tên lửa có hàng chục chương trình chạy cùng lúc: điều khiển động cơ, tính quỹ đạo, giám sát nhiên liệu, liên lạc mặt đất...

Đến ngày phóng. Em đứng trong phòng điều khiển. Đếm ngược.

*10... 9... 8...*

Bỗng sếp quay sang hỏi em:

> *"Em có **chắc chắn** phần mềm không có lỗi không?"*

Em trả lời gì?

- **"Dạ, em đã chạy thử trên máy tính, thấy chạy được ạ"** — 😬
- **"Dạ, em đã viết 55 bài kiểm tra tự động, tất cả đều đạt. Robot kiểm tra lại mỗi lần em sửa code. Và em có bằng chứng"** — 😊

Sếp sẽ tin câu trả lời nào hơn?

*3... 2... 1... Phóng!*

Bốn mạng người trên đó. Câu trả lời của em phải là câu thứ hai.

**Nhưng "55 bài kiểm tra tự động" là gì? "Robot kiểm tra" là gì? "Bằng chứng" là gì?**

Hôm nay, chúng ta sẽ xây tất cả những thứ đó cho AegisOS.

---

## 🏫 Bài Kiểm Tra Ở Trường

### Tại sao phải kiểm tra?

Em nghĩ thử nhé: tại sao ở trường phải có **bài kiểm tra**?

Không phải để hành hạ học sinh đâu! Mà vì:

1. **Để biết em đã hiểu bài chưa** — nếu sai, cô giáo sẽ giúp em sửa
2. **Để phát hiện lỗ hổng** — có thể em hiểu 90% nhưng sót 10% quan trọng
3. **Để có bằng chứng** — khi nào cần, em chỉ ra "đây, con đã đạt điểm này"

Phần mềm cũng vậy!

| Ở trường | Trong phần mềm |
|---|---|
| Bài kiểm tra Toán | **Unit test** — kiểm tra từng phần nhỏ |
| Bài thi cuối kỳ (tổng hợp nhiều môn) | **Integration test** — kiểm tra cả hệ thống chạy cùng nhau |
| Cô giáo chấm bài | **Test framework** — chương trình tự động chấm điểm |
| Kiểm tra bất chợt (15 phút) | **CI (Continuous Integration)** — robot tự kiểm tra mỗi lần sửa code |
| Sổ điểm ghi kết quả | **Test report** — bằng chứng tất cả bài đều đạt |

### Nhưng có khác nhau một chỗ rất quan trọng

Ở trường, nếu em làm sai bài kiểm tra — em bị điểm thấp. Buồn, nhưng không ai bị thương.

Trong phần mềm an toàn — nếu "bài kiểm tra" bị bỏ qua, và lỗi lọt ra ngoài — **người ta có thể chết**.

Vì vậy, ngành hàng không, y tế, ô tô có những tiêu chuẩn rất nghiêm ngặt:

- **DO-178C** (máy bay) — yêu cầu phải có **bằng chứng kiểm thử** trước khi phần mềm được phép bay
- **IEC 62304** (thiết bị y tế) — yêu cầu **kiểm tra tự động, lặp lại được**
- **ISO 26262** (ô tô) — yêu cầu cả **unit test** lẫn **integration test**

Tất cả đều nói một điều: **"Chạy thử thấy được" là chưa đủ. Phải có bằng chứng.**

---

## 🔍 Vấn Đề Của AegisOS: Không Có Bài Kiểm Tra Nào

Trước Phase F, AegisOS đã có rất nhiều thứ tuyệt vời:
- ✅ Khởi động từ con số 0 (Phase A)
- ✅ Bảo vệ bộ nhớ W^X (Phase B)
- ✅ Chạy 3 task cùng lúc + IPC (Phase C)
- ✅ Tách kernel/user, EL0 isolation (Phase D)
- ✅ Cô lập lỗi, auto-restart (Phase E)

Nhưng **không có một bài kiểm tra tự động nào**.

Muốn biết kernel có chạy không? Phải tự gõ lệnh, mở QEMU, đọc output bằng mắt. Mỗi. Một. Lần.

Giống như mỗi lần làm xong bài, em không kiểm tra lại, cứ nộp luôn. May thì đúng. Không may thì sai mà không biết.

Với ~2.000 dòng code, một thay đổi nhỏ ở file này có thể **break** file kia — mà không ai phát hiện cho đến khi chạy QEMU thủ công.

Đó là lý do Phase F ra đời: **xây hệ thống kiểm tra tự động**.

---

## 🧩 Hai Loại Kiểm Tra

### Loại 1: Unit Test — Kiểm tra từng viên gạch

Hãy tưởng tượng em đang xây một ngôi nhà bằng LEGO.

Trước khi ghép cả ngôi nhà, em sẽ **kiểm tra từng viên gạch**:
- Viên này có đúng kích thước không?
- Viên kia có khớp với viên bên cạnh không?
- Viên nền có đủ cứng để chịu được tầng 2 không?

Trong AegisOS, "viên gạch" là các **hàm** (function) — những đoạn code nhỏ làm một việc cụ thể.

Ví dụ, hàm `validate_write_args` kiểm tra xem task có quyền ghi vào một địa chỉ bộ nhớ không. Đây là hàm **cực kỳ quan trọng** — nếu nó sai, task có thể ghi đè lên UART, lên kernel, lên bất cứ đâu!

Vậy chúng ta viết bài kiểm tra cho nó:

| Tình huống | Đầu vào | Kết quả đúng |
|---|---|---|
| Địa chỉ nằm trong RAM | `0x4008_0000`, 10 byte | ✅ Cho phép |
| Địa chỉ = 0 (null) | `0x0`, 10 byte | ❌ Từ chối |
| Địa chỉ là UART (thiết bị!) | `0x0900_0000`, 1 byte | ❌ Từ chối |
| Độ dài = 0 | `0x4008_0000`, 0 byte | ❌ Từ chối |
| Độ dài quá lớn (> 256) | `0x4008_0000`, 257 byte | ❌ Từ chối |
| Địa chỉ + độ dài tràn ra ngoài RAM | `0x47FF_FF01`, 256 byte | ❌ Từ chối |

12 bài kiểm tra chỉ cho **một** hàm duy nhất! Mỗi bài kiểm tra một **tình huống ranh giới** — nơi lỗi hay ẩn nấp nhất.

Nhưng vì AegisOS chạy trên ARM (AArch64) — một loại CPU mà laptop em không có — nên làm sao kiểm tra được?

### Bí quyết: Tách "bộ não" ra khỏi "cơ thể"

Đây là ý tưởng thông minh nhất của Phase F.

Trong kernel, có hai loại code:

1. **Code phụ thuộc phần cứng** — chỉ chạy được trên CPU AArch64 thật (hoặc QEMU giả lập). Ví dụ: ghi vào UART, cài đặt bảng trang, lệnh `eret` nhảy vào EL0.

2. **Code logic thuần** — chỉ là toán học và quy tắc, chạy được ở **bất kỳ đâu**. Ví dụ: "task tiếp theo trong round-robin là ai?", "con trỏ này có nằm trong RAM không?", "bit thứ 53 của descriptor có bật không?"

Chúng ta **tách** phần logic thuần ra, rồi kiểm tra nó trên **máy tính của mình** (x86_64 — loại CPU trong laptop em). Không cần QEMU, không cần phần cứng ARM.

Giống như: muốn kiểm tra một **công thức Toán** có đúng không — em không cần bay lên tàu vũ trụ để thử. Em ngồi ở bàn học, thế vài số vào, tính ra kết quả, rồi so với đáp án.

Trong AegisOS, chúng ta dùng một kỹ thuật gọi là **cfg gate** — giống như một cánh cửa có ổ khóa:

| Phần code | Trên AArch64 (kernel thật) | Trên x86_64 (máy em) |
|---|---|---|
| Ghi vào UART | ✅ Ghi thật qua MMIO | 🚫 Không làm gì (no-op) |
| Lệnh assembly (`asm!`) | ✅ Chạy lệnh ARM | 🚫 Bị ẩn đi bằng `#[cfg]` |
| Linker symbol (`__stack_end`) | ✅ Có thật trong bộ nhớ | 🚫 Bị ẩn đi |
| Hàm `validate_write_args` | ✅ Chạy bình thường | ✅ Chạy bình thường! |
| Hàm `schedule` (logic round-robin) | ✅ Chạy bình thường | ✅ Chạy bình thường! |
| Hằng số MMU (`AP_RW_EL0`, `PXN`...) | ✅ Dùng để cài bảng trang | ✅ Kiểm tra giá trị bit! |

Kết quả? **55 bài kiểm tra** chạy ngay trên laptop, trong **0.03 giây**. Không cần QEMU.

### Loại 2: Integration Test — Kiểm tra cả ngôi nhà

Kiểm tra từng viên gạch xong, em phải **ghép cả ngôi nhà** rồi thử xem nó có đứng vững không.

Với AegisOS, "ghép cả ngôi nhà" nghĩa là: **build kernel thật → chạy trên QEMU → xem output UART**.

Chúng ta viết một **script** — một danh sách hướng dẫn cho máy tính:

1. Build kernel cho AArch64
2. Mở QEMU, chạy kernel, đợi 15 giây
3. Bắt lại tất cả chữ mà kernel in ra
4. Kiểm tra: có thấy dòng **"[AegisOS] boot"** không? Có thấy **"A:PING"** không? Có thấy **"B:PONG"** không?

8 checkpoint. Nếu thiếu bất kỳ dòng nào → **FAIL**.

Giống như khi cô giáo kiểm tra bài tập nhóm:
- ✅ Có bìa? ✅ Có mục lục? ✅ Có nội dung? ✅ Có kết luận?
- Thiếu một phần → phải làm lại.

---

## 🤖 Robot Kiểm Tra — CI Là Gì?

Đến phần hay nhất.

Em đã có 55 unit test + 8 integration checkpoint. Nhưng nếu **em quên chạy** chúng thì sao?

Con người hay quên. Máy tính thì không.

Vì vậy chúng ta tạo một **robot kiểm tra** tên là **CI** — viết tắt của **Continuous Integration** (tích hợp liên tục).

CI hoạt động thế này:

1. Em sửa code rồi **push** (gửi) lên GitHub
2. Robot CI **tự động thức dậy**
3. Robot tải code mới nhất
4. Robot cài Rust + QEMU
5. Robot chạy **55 unit test** → Tất cả pass? ✅
6. Robot build kernel + chạy trên QEMU → 8 checkpoint pass? ✅
7. Robot báo cáo: **"Xanh lá! Mọi thứ ổn!"** 🟢

Nếu có gì sai? Robot hét lên: **"Đỏ! Bài kiểm tra X bị fail!"** 🔴

| Đời thật | CI |
|---|---|
| Cô giáo kiểm tra bài mỗi buổi sáng | Robot chạy test mỗi lần push code |
| Cô giáo đánh dấu bài sai bằng bút đỏ | Robot hiện ❌ bên cạnh test fail |
| Sổ liên lạc ghi "con ngoan" | Badge xanh 🟢 trên GitHub |
| Cô giáo nghỉ phép → không ai chấm | Robot **không bao giờ nghỉ** |

Trong AegisOS, robot CI chạy trên **GitHub Actions** — một dịch vụ miễn phí của GitHub. Mỗi khi có code mới, GitHub tự động bật một máy tính Linux trên cloud, cài đặt mọi thứ, chạy test, rồi báo kết quả.

---

## 🏗️ AegisOS Đã Làm Được Gì?

### Thách thức #1: Code kernel không chạy được trên laptop

AegisOS viết cho CPU ARM. Laptop dùng CPU x86. Khác nhau hoàn toàn — giống như sách Tiếng Việt và sách Tiếng Nhật, không đọc lẫn cho nhau được.

**Giải pháp:** Chúng ta chia code thành **hai phần**:
- [src/lib.rs](src/lib.rs) — thư viện chung, chứa tất cả module. Khi build cho ARM, mọi thứ hoạt động. Khi build cho x86, chỉ phần logic thuần được bật.
- [src/main.rs](src/main.rs) — file khởi động kernel, chứa toàn bộ code ARM. Khi test trên x86, file này bị "tắt" hoàn toàn — nó chỉ còn là một hàm `main()` trống rỗng.

Module [src/gic.rs](src/gic.rs) (bộ điều khiển ngắt — hoàn toàn phần cứng) được đánh dấu `#[cfg(target_arch = "aarch64")]` — chỉ tồn tại trên ARM.

### Thách thức #2: Biến toàn cục (`static mut`)

AegisOS không có **heap** (vùng nhớ động). Mọi dữ liệu đều là biến toàn cục: `TCBS` (bảng task), `ENDPOINTS` (hộp thư IPC), `TICK_COUNT` (bộ đếm thời gian)...

Vấn đề: nếu chạy nhiều bài test cùng lúc, chúng sẽ **ghi đè lên nhau** — test A thay đổi `TCBS`, rồi test B đọc `TCBS` đã bị thay đổi → kết quả sai!

**Giải pháp:**
- Mỗi bài test gọi hàm `reset_test_state()` để **dọn sạch** mọi biến toàn cục trước khi bắt đầu
- Chạy test với `--test-threads=1` — chỉ một test chạy tại một thời điểm, giống như xếp hàng vào nhà vệ sinh — mỗi lần một người

### Thách thức #3: Hai "ngôn ngữ" build khác nhau

Khi build kernel cho ARM, Rust cần thêm cờ đặc biệt: `-Zbuild-std=core` (build lại thư viện lõi từ mã nguồn). Nhưng khi chạy test trên x86, cờ này **gây xung đột** — máy tính bị nhầm lẫn giữa hai bản thư viện.

**Giải pháp:** Chuyển cờ đặc biệt ra **dòng lệnh** thay vì đặt trong file cấu hình. Khi build kernel → thêm cờ. Khi chạy test → không thêm.

### Kết quả: 55 test, 0 fail, 0.03 giây

```
running 55 tests
test trapframe_size_is_288           ... ok
test mmu_device_block_is_non_executable ... ok
test validate_write_uart_mmio        ... ok
test sched_round_robin_all_ready     ... ok
test ipc_copy_message                ... ok
...
test result: ok. 55 passed; 0 failed
```

| Nhóm test | Số bài | Kiểm tra cái gì |
|---|---|---|
| **TrapFrame** | 4 | Kích thước = 288 byte, offset các trường khớp assembly |
| **MMU descriptors** | 18 | Bit đúng vị trí, W^X (ghi ↔ thực thi loại trừ nhau) |
| **SYS_WRITE** | 12 | Từ chối pointer lạ, chấp nhận pointer trong RAM |
| **Scheduler** | 11 | Round-robin đúng thứ tự, bỏ qua task lỗi, auto-restart |
| **IPC** | 10 | Gửi/nhận đúng, dọn dẹp khi task crash |
| **Tổng** | **55** | 🎯 |

---

## 📏 Tại Sao 288 Byte Quan Trọng Đến Vậy?

Phần này hơi khó, nhưng em cứ đọc chậm lại nhé.

Khi một task bị ngắt (interrupt), kernel phải **lưu lại toàn bộ trạng thái** của task đó — 31 thanh ghi (register), địa chỉ đang chạy, trạng thái CPU... Tất cả nằm trong một "hộp" gọi là **TrapFrame**.

TrapFrame có kích thước **chính xác 288 byte**. Không hơn. Không kém.

Tại sao phải chính xác? Vì TrapFrame được dùng bởi **hai ngôn ngữ khác nhau**:
- **Rust** — dùng struct để đọc/ghi từng trường
- **Assembly** — dùng offset (vị trí byte) cố định để lưu/khôi phục thanh ghi

Nếu Rust nói "thanh ghi x[30] ở byte thứ 240" mà Assembly nói "ở byte thứ 248" — khi kernel khôi phục trạng thái, task sẽ nhảy về **sai địa chỉ**. Kernel crash. Tất cả task chết.

Vì vậy, bài test đầu tiên chúng ta viết là:

> **Kích thước TrapFrame có đúng 288 byte không?**
> **Trường `elr_el1` (địa chỉ trả về) có nằm ở offset 256 không?**
> **Trường `sp_el0` (stack pointer) có nằm ở offset 248 không?**

Nếu ai đó **vô tình thêm một trường** vào TrapFrame, hoặc **đổi thứ tự** — bài test sẽ **lập tức fail**. Trước khi code kịp lên tàu vũ trụ.

Đây gọi là **ABI lock** — khóa giao diện nhị phân. Giống như bản vẽ kỹ thuật của ổ khóa: nếu ai đổi kích thước lỗ khóa dù chỉ 0.1mm, chìa khóa không xoay được nữa.

---

## 🛡️ W^X: Kiểm Tra Quy Tắc "Ghi Hoặc Chạy, Không Cả Hai"

Nhớ quy tắc **W^X** (Write XOR Execute) từ bài #2 không?

Một vùng bộ nhớ chỉ được phép:
- **Ghi** (chứa dữ liệu) — nhưng **không được chạy** code từ đó
- **Chạy** (chứa code) — nhưng **không được ghi** vào đó

Điều này ngăn hacker nhét code độc vào vùng dữ liệu rồi bảo CPU chạy nó.

Trong AegisOS, quy tắc này được mã hóa bằng **bit** trong các descriptor (bộ mô tả trang bộ nhớ). Chúng ta viết test để kiểm tra:

| Loại trang | Ghi được? | Chạy được? | Test kiểm tra gì |
|---|---|---|---|
| **Kernel data** | ✅ | ❌ | Có bit PXN + UXN (không ai chạy) |
| **User data** | ✅ | ❌ | Có bit PXN + UXN |
| **Shared code** | ❌ (read-only) | ✅ | Không có PXN, không có UXN |
| **Device (UART)** | ✅ (chỉ kernel) | ❌ | Có bit XN, AP = chỉ EL1 |

18 bài test, kiểm tra **từng bit** của từng loại trang. Nếu ai đó sửa constant mà quên bật bit XN — test fail ngay.

---

## ✨ Tại Sao Em Nên Quan Tâm?

Em biết **Margaret Hamilton** không? (Bà đã xuất hiện ở bài trước!)

Khi bà viết phần mềm cho Apollo 11, bà không chỉ viết code — bà viết rất nhiều **test**. Và bà kiên quyết rằng phần mềm phải được kiểm tra **mọi tình huống có thể xảy ra**, kể cả những tình huống "không bao giờ xảy ra" (mà cuối cùng lại xảy ra trên Mặt Trăng!).

Sau Apollo, thế giới học được rằng:

> **Code mà không có test — giống như cầu mà không ai kiểm tra trước khi cho xe chạy.**

Ngày nay, mọi công ty lớn — Google, Apple, SpaceX, Tesla — đều yêu cầu **mỗi dòng code phải có test đi kèm**. Không có test, code không được merge (ghép vào dự án chính).

Và bây giờ, AegisOS nhỏ bé cũng có hệ thống test — giống y hệt nguyên tắc mà SpaceX dùng cho Falcon 9.

---

## 🔮 Bước Tiếp Theo

AegisOS giờ đã biết:
- ✅ **"Nhớ"** — ai ở đâu, ai được làm gì (MMU + Page Table)
- ✅ **"Chia sẻ"** — luân phiên giữa nhiều task (Scheduler)
- ✅ **"Nói chuyện"** — task gửi thư cho nhau (IPC + Syscall)
- ✅ **"Giữ khoảng cách"** — task không sờ được kernel (EL0 Isolation)
- ✅ **"Đứng dậy khi ngã"** — cô lập lỗi + tự khởi động lại (Fault Isolation)
- ✅ **"Tự kiểm tra"** — 55 unit test + CI robot ← MỚI!

Nhưng có một câu hỏi lớn chưa được trả lời:

> **"Ai cho phép task này được gửi tin nhắn? Ai cho phép task kia được dùng timer?"**

Hiện tại, mọi task đều có thể gọi mọi syscall. Task nhạc có thể gọi syscall điều khiển phanh. Nguy hiểm!

Bước tiếp theo, chúng ta sẽ xây **Capability System** — hệ thống "giấy phép". Mỗi task phải có **giấy phép** mới được làm một việc. Không có giấy phép? Bị từ chối.

Giống như ở trường: không phải ai cũng được vào phòng thí nghiệm. Phải có **thẻ ra vào** do thầy cô cấp.

Hẹn gặp em ở bài tiếp theo! 🚀

---

> *"Tin tưởng, nhưng phải kiểm tra." ("Trust, but verify.")*
> — Ronald Reagan (vốn là câu tục ngữ Nga: "Доверяй, но проверяй")

---

*Nếu em đọc đến đây, em đã hiểu được Unit Test, Integration Test, CI/CD, ABI Lock, và W^X Verification. Đây là những kỹ năng mà mọi kỹ sư phần mềm chuyên nghiệp phải biết — và em vừa nắm được ý tưởng cốt lõi khi mới lớp 5. Khi bạn bè hỏi "test là gì?", em có thể trả lời: "Là cách chúng ta chứng minh rằng phần mềm đáng tin — không chỉ bằng lời nói, mà bằng bằng chứng." Tuyệt lắm!* 👏
