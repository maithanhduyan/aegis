---
lang: vi
title: "🏫 Từ 3 Lên 8 — Và Chứng Minh Bằng Toán Học"
tags: "scale, formal-verification, kani, kernelcell, scheduler, aegisos"
description: "Bài #14 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao trường học 3 lớp không giống trường 8 lớp, và tại sao 'test 231 lần thấy đúng' vẫn chưa đủ."
date: 2026-02-12
---

# 🏫 Từ 3 Lên 8 — Và Chứng Minh Bằng Toán Học

> *Bài #14 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: mở rộng hệ thống từ 3 lên 8 chương trình chạy cùng lúc, khóa hết mọi "tủ nguy hiểm", và lần đầu tiên dùng toán học để chứng minh code đúng — không chỉ "thử thấy đúng".*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2048. Em là kỹ sư phần mềm cho một công ty xe tự lái.

Chiếc xe của em đang chở 4 hành khách trên cao tốc, tốc độ 120km/h. Bên trong con chip nhỏ bằng đồng xu, có **8 chương trình** chạy cùng lúc:

| # | Chương trình | Nhiệm vụ |
|---|---|---|
| 0 | 📷 Camera | Nhận diện vạch kẻ đường, biển báo |
| 1 | 📡 LiDAR | Quét laser, đo khoảng cách xe phía trước |
| 2 | 🗺️ Bản đồ | Tính đường đi tối ưu |
| 3 | 🎛️ Tay lái | Điều khiển vô-lăng |
| 4 | 🛑 Phanh | Giám sát phanh khẩn cấp — **không bao giờ được chậm** |
| 5 | 📊 Telemetry | Gửi dữ liệu về trung tâm |
| 6 | 🔄 Cập nhật | Nhận phần mềm mới từ nhà máy |
| 7 | 💓 Sức khỏe | Theo dõi tất cả 7 chương trình kia — nếu ai "ngất" thì báo động |

Nếu hệ thống chỉ chạy được **3 chương trình** — em phải **bỏ bớt** 5 cái. Bỏ phanh? Bỏ camera? Bỏ giám sát sức khỏe?

Không bỏ được cái nào.

**Nhưng nếu hệ thống chạy 8 mà code bị lỗi thì sao?** Không chỉ 1 hành khách gặp nguy — mà tất cả xe tự lái trên thế giới dùng cùng phần mềm đều gặp nguy.

Vì vậy, người kiểm tra ở cơ quan an toàn sẽ hỏi em: *"Anh có **chứng minh được bằng toán học** rằng bộ lập lịch luôn chọn đúng chương trình không?"*

Hôm nay, chúng ta sẽ làm đúng điều đó: **Phase N — Scale & Verify**.

---

## 🏫 Phần 1: Từ 3 Lên 8 — Giống Như Nâng Cấp Trường Học

### Trường học 3 lớp vs 8 lớp

Hãy tưởng tượng em đang quản lý một trường tiểu học nhỏ xíu — chỉ có **3 lớp**.

- 3 phòng học
- 3 bàn ghế
- 1 cuốn sổ điểm danh 3 trang
- Thời khóa biểu ghi tay, 3 dòng

Mọi thứ đơn giản. Ai cũng biết ai.

Rồi một ngày, sở giáo dục nói: *"Trường phải mở rộng lên **8 lớp**."*

Em không thể chỉ thêm 5 cái bảng vào sân trường. Em cần:

| Cần thay đổi | 3 lớp | 8 lớp |
|---|---|---|
| 🏠 Phòng học (stacks) | 3 | 8 |
| 📝 Sổ điểm danh (TCBs) | 3 trang | 8 trang |
| 🗺️ Bản đồ chỗ ngồi (page tables) | 16 trang | 36 trang |
| 📋 Thời khóa biểu (scheduler) | Xét 3 lớp | Xét 8 lớp |
| 📐 Nội quy (kernel) | Giữ nguyên! | Giữ nguyên! |

Điều quan trọng nhất: **nội quy trường không đổi**. Vẫn cùng luật — ai ưu tiên hơn được học trước, ai hết giờ thì nhường, ai ngất thì y tá đến. Chỉ là **nhiều học sinh hơn** phải tuân theo nội quy.

Đây chính xác là những gì AegisOS vừa làm: `NUM_TASKS` từ 3 → 8.

### Bài toán khó: không phải "đổi một con số"

Em có thể nghĩ: *"Thì sửa số 3 thành 8, xong!"*

Không đơn giản vậy. Con số `3` bị **cài cứng** (hardcoded) ở **hơn 15 chỗ** trong code:

- Sổ điểm danh: `TCBS: [Tcb; 3]`
- Bản đồ bộ nhớ: `PT_L1_TASK0`, `PT_L1_TASK1`, `PT_L1_TASK2` — ba hằng số riêng biệt!
- Khởi tạo: `init_tasks(task_a, task_b, task_c)` — đúng 3 tham số
- Linker script: `.task_stacks: 3 * 4096`
- Cả chục chỗ khác...

Giống như trường học cũ ghi sẵn **"phòng 1, phòng 2, phòng 3"** trên tường, trên hợp đồng, trên biển hiệu. Muốn thêm phòng 4–8, phải tìm **tất cả** chỗ ghi "3" và sửa — bỏ sót một chỗ là sập.

### Giải pháp: công thức thay vì liệt kê

Thay vì liệt kê từng phòng (`PHÒNG_1`, `PHÒNG_2`, `PHÒNG_3`...), chúng ta viết **một công thức** tính phòng:

```
Vị trí bản đồ = loại_bản_đồ × tổng_số_lớp + mã_lớp
```

Trong AegisOS, công thức này là hàm `pt_index()`:

```
pt_index(task_id, table_type) = table_type × NUM_TASKS + task_id
```

Bây giờ muốn 8 lớp, 80 lớp, hay 800 lớp — chỉ cần đổi **một con số** `NUM_TASKS`. Công thức tự tính hết.

| Đời thật | Trong AegisOS |
|---|---|
| Ghi tay "phòng 1, 2, 3" | `PT_L1_TASK0`, `PT_L1_TASK1`... (hardcoded) |
| Dùng công thức "phòng N" | `pt_index(task_id, type)` |
| Sổ điểm danh 3 trang → 8 trang | `TCBS: [Tcb; 3]` → `[Tcb; 8]` |
| Thêm 5 phòng + 5 bàn ghế | Linker: `.task_stacks: 8 * 4096` |

---

## 🔒 Phần 2: Khóa Nốt 4 Tủ Còn Lại

### Nhắc lại: câu chuyện "tủ khóa" từ Phase M

Ở bài trước, chúng ta đã học về **KernelCell** — cái tủ có khóa.

Trước đó, dữ liệu quan trọng trong kernel được để trên **bàn chung** (`static mut`) — ai cũng sờ được, không kiểm soát. Nguy hiểm!

Phase M đã khóa 4 **tủ nhỏ** (biến đơn giản): tick counter, task hiện tại, log state, và một vài thứ khác.

Nhưng còn **4 tủ lớn** chưa khóa:

| Tủ | Chứa gì | Bao nhiêu ngăn |
|---|---|---|
| `TCBS` | Sổ điểm danh — thông tin 8 chương trình | 8 ngăn |
| `ENDPOINTS` | Hộp thư — nơi chương trình gửi tin nhắn | 4 ngăn |
| `GRANTS` | Giấy chia sẻ phòng — ai chia sẻ bộ nhớ với ai | 2 ngăn |
| `IRQ_BINDINGS` | Danh sách chuông cửa — ai nghe chuông nào | 8 ngăn |

Đây là những tủ **phức tạp nhất** trong kernel — không chỉ chứa một con số, mà chứa **mảng** (array) gồm nhiều ngăn, mỗi ngăn là một cấu trúc dữ liệu phức tạp.

### Thử thách: khóa tủ nhiều ngăn

Với tủ nhỏ (chứa 1 con số), khóa đơn giản:

```
Trước: để trên bàn     →  Sau: bỏ vào tủ khóa
       static mut X            KernelCell<u64>
```

Với tủ nhiều ngăn (chứa mảng), khó hơn:

```
Trước: 8 hộp để trên bàn    →  Sau: 8 hộp trong 1 tủ khóa lớn
       static mut TCBS[8]          KernelCell<[Tcb; 8]>
```

Muốn lấy hộp số 3, phải: **mở khóa tủ** → lấy hộp 3 → dùng → đóng lại.

Trong code:

```
Trước:  TCBS[3].state = Ready;
Sau:    (*TCBS.get_mut())[3].state = Ready;
```

Dòng code dài hơn? Đúng. Nhưng **an toàn hơn** — vì mỗi lần mở tủ, kỹ sư phải viết lý do tại sao an toàn. Không ai lén mở tủ mà không ai biết.

### Kết quả: ZERO tủ không khóa

Sau Phase N:

| Trước Phase M | Sau Phase M | Sau Phase N |
|---|---|---|
| 8 biến `static mut` | 4 đã khóa, **4 chưa** | **8/8 đã khóa** ✅ |

**Không còn một biến `static mut` nào** trong toàn bộ AegisOS. Mọi dữ liệu đều nằm trong tủ khóa. Đây là điều mà tiêu chuẩn DO-178C gọi là *"source code verifiable"* — code có thể kiểm chứng được.

---

## 🔬 Phần 3: "Test 231 Lần Thấy Đúng" — Vẫn Chưa Đủ?

### Bài toán Olympiad

Hãy tưởng tượng em đang thi Toán Olympiad. Đề bài:

*"Chứng minh rằng: với mọi số nguyên n từ 1 đến 1000, biểu thức n² + n luôn chia hết cho 2."*

**Cách 1 — Test:** Em lấy máy tính, thử n = 1 → 2 (chia hết ✅), n = 2 → 6 (chia hết ✅), n = 3 → 12 (chia hết ✅)... thử hết 1000 số. Kết quả: tất cả đều chia hết.

**Cách 2 — Chứng minh:** Em viết: *"n² + n = n(n+1). Trong hai số nguyên liên tiếp, luôn có một số chẵn. Vậy tích luôn chia hết cho 2."* ∎

Cả hai đều cho kết quả "đúng". Nhưng:

| | Test | Chứng minh |
|---|---|---|
| Kiểm tra bao nhiêu trường hợp? | 1000 | **Mọi** trường hợp |
| Nếu đề đổi thành 1 → 1 triệu? | Phải thử lại 1 triệu lần | Vẫn đúng, không cần thử lại |
| Có thể bỏ sót trường hợp lạ? | Có thể | **Không thể** |
| Người chấm Olympiad chấp nhận? | ❌ | ✅ |

Trong phần mềm an toàn, cơ quan kiểm tra (FDA, FAA, ISO) cũng vậy. Họ muốn **chứng minh**, không chỉ test.

### Kani — "Người chấm Olympiad" cho code

**Kani** là một công cụ do AWS (Amazon Web Services) phát triển. Nó đọc code Rust và tự động **chứng minh** rằng code đúng với **mọi** đầu vào có thể — không chỉ vài trường hợp test.

Cách Kani hoạt động:

1. Kỹ sư viết **bài chứng minh** (proof harness) — giống đề Olympiad
2. Kani **thử mọi trường hợp** — không phải 1000, mà **tất cả** các giá trị có thể
3. Nếu tìm thấy trường hợp sai → Kani chỉ ra cụ thể
4. Nếu không tìm thấy → **chứng minh** code đúng

| Đời thật | Trong phần mềm |
|---|---|
| Thử 231 bài test, thấy đúng | `cargo test` → 231 passed ✅ |
| Chứng minh toán học: đúng MỌI trường hợp | `cargo kani` → 6 proofs verified ✅ |
| Thầy cô kiểm tra 10 bài | Unit tests |
| Giám khảo Olympiad xem chứng minh | Kani formal verification |

---

## 📐 Phần 4: 6 Bài Chứng Minh Của AegisOS

Chúng ta đã viết 6 "bài Olympiad" cho AegisOS. Mỗi bài chứng minh một tính chất quan trọng.

### Bài 1 & 2: Bản đồ phòng không bao giờ trùng

*"Chứng minh rằng: với 8 lớp học và 4 loại bản đồ, hàm `pt_index()` luôn trả về vị trí nằm trong phạm vi hợp lệ, và không có hai lớp nào bị xếp trùng phòng."*

Giống như chứng minh: trong trường học 8 lớp, **không bao giờ** 2 lớp khác nhau bị xếp cùng phòng. Không phải thử 8 × 8 = 64 cặp — mà chứng minh **công thức** luôn cho kết quả khác nhau.

Kani đã xét **mọi** tổ hợp task_id (0–7) × table_type (4 loại):
- ✅ Tất cả 32 vị trí đều nằm trong phạm vi 0–35
- ✅ Không có hai vị trí nào trùng nhau

### Bài 3 & 4: Quyền hạn không bao giờ vượt rào

*"Chứng minh rằng: với mọi syscall từ 0 đến 12, hàm `cap_for_syscall()` không bao giờ crash, và kết quả luôn nằm trong 18 bit quyền đã định nghĩa."*

Giống như chứng minh: bảng quy định *"ai được làm gì"* trong trường học **không bao giờ** có ô trống hoặc ô vô nghĩa. Mọi hành động đều có quy định rõ ràng.

### Bài 5: Bộ lập lịch LUÔN chọn được ai đó

*"Chứng minh rằng: dù 8 chương trình ở trạng thái nào — đang chạy, đang chờ, đang ngủ, đang bị lỗi — bộ lập lịch (scheduler) LUÔN chọn được một chương trình hợp lệ để chạy. Nếu không ai sẵn sàng → chọn chương trình idle."*

Đây là tính chất **quan trọng nhất**. Tưởng tượng nếu scheduler không chọn được ai — CPU đứng im, xe tự lái đang chạy 120km/h mà bộ não... treo. 😱

Kani đã xét **mọi** tổ hợp:
- 8 chương trình × 2 trạng thái (sẵn sàng / không) = 2⁸ = **256 kịch bản**
- Mỗi chương trình có 8 mức ưu tiên = 8⁸ = **16 triệu** tổ hợp ưu tiên
- Nhân thêm 8 vị trí bắt đầu tìm kiếm

Tổng cộng: **hàng tỷ** trường hợp. Kani chứng minh **tất cả** đều đúng. Trong 0.76 giây.

### Bài 6: Khởi động lại đúng quy trình

*"Chứng minh rằng: chỉ chương trình bị lỗi (Faulted) mới được khởi động lại. Chương trình đang chạy bình thường thì KHÔNG bị khởi động lại."*

Giống như chứng minh: y tá chỉ đánh thức bệnh nhân **đã ngất** — không bao giờ đánh thức bệnh nhân đang ngủ bình thường.

### Kết quả

```
Kani verification: 6 proofs, 0 failures
```

6 bài. 0 sai. **Chứng minh toán học** — không phải "thử thấy đúng".

---

## 🏗️ Phần 5: Chúng Ta Đã Làm Được Gì Trong AegisOS?

### Cây thư mục thay đổi

```
src/
├── kernel/
│   ├── sched.rs     ← NUM_TASKS = 8, TaskMetadata, KernelCell<[Tcb; 8]>
│   ├── ipc.rs       ← KernelCell<[Endpoint; 4]>
│   ├── grant.rs     ← KernelCell<[Grant; 2]>
│   ├── irq.rs       ← KernelCell<[IrqBinding; 8]>
│   ├── cell.rs      ← kcell_index!() macro mới
│   ├── cap.rs       ← Kani proof: cap_for_syscall
│   └── elf.rs       ← (không đổi)
│
├── mmu.rs           ← pt_index() công thức + Kani proof
├── main.rs          ← TASK_META const array + loop init
└── ...

linker.ld            ← .task_stacks 8×4096, .page_tables 36×4096
```

### Bảng trước / sau

| Chỉ số | Phase M (trước) | Phase N (sau) | Thay đổi |
|---|---|---|---|
| Số task | 3 | **8** | +5 task |
| `static mut` còn lại | 4 | **0** | 🎉 Hết! |
| Host tests | 219 | **231** | +12 test |
| QEMU checkpoints | 28 | **30** | +2 checkpoint |
| Code coverage | 96.65% | **99.02%** | +2.37% |
| Kani proofs | 0 | **6** | Từ zero! |
| Syscalls | 13 | 13 | Không đổi |
| Capability bits | 18 | 18 | Không đổi |
| Bộ nhớ dùng thêm | — | +122 KiB | 0.09% RAM |

### Tại sao 0 `static mut` quan trọng?

Khi **mọi** dữ liệu đều trong `KernelCell`:

1. Mỗi lần truy cập phải viết `unsafe` + lý do — **không ai lén sửa dữ liệu**
2. Công cụ như Kani có thể **phân tích** toàn bộ code — vì không còn "vùng mù"
3. Kiểm tra viên (auditor) nhìn vào code và **đếm được** chính xác bao nhiêu chỗ truy cập dữ liệu nhạy cảm

Giống như thay tất cả tủ không khóa trong bệnh viện bằng tủ có khóa + camera + sổ ghi — FDA sẽ rất vui.

---

## 🌟 Phần 6: Câu Chuyện Về AWS Và Kani

Năm 2022, một đội kỹ sư tại **Amazon Web Services** (AWS) nhận ra một vấn đề: hàng triệu máy chủ trên khắp thế giới chạy code Rust, phục vụ hàng tỷ người. Test thì nhiều — nhưng **test không thể kiểm tra mọi trường hợp**.

Họ tạo ra **Kani** — công cụ mã nguồn mở, dùng kỹ thuật *model checking* (kiểm tra mô hình) để chứng minh code Rust đúng bằng toán học. Không phải "thử 1000 lần thấy đúng" mà "chứng minh đúng **mọi** trường hợp có thể".

Kani được đặt tên theo loài **cá Kani** — một loài cá rất nhỏ nhưng bơi cực nhanh trong các rạn san hô, tìm ra **mọi** ngóc ngách. Giống như cách Kani tìm ra mọi execution path trong code.

Điều thú vị: Kani **miễn phí** và **mã nguồn mở**. Ai cũng có thể dùng. Từ kỹ sư AWS đến... một dự án nhỏ tên AegisOS, do một người Việt Nam xây dựng.

AegisOS là một trong những dự án microkernel `#![no_std]` đầu tiên dùng Kani để formal-verify scheduler properties. Bước nhỏ — nhưng hướng tới tiêu chuẩn DO-333 (Formal Methods) của ngành hàng không.

---

## 🤔 Câu Hỏi Cho Bạn Nhỏ

**Câu 1:** Tại sao không thể chỉ đổi `NUM_TASKS = 3` thành `8` mà phải sửa hơn 15 chỗ?

> 💡 *Gợi ý: nghĩ về trường học ghi sẵn "3 lớp" trên tường, biển hiệu, hợp đồng...*

**Câu 2:** Nếu scheduler có 8 chương trình, mỗi cái có 2 trạng thái (sẵn sàng / không), thì Kani phải xét bao nhiêu kịch bản?

> 💡 *Gợi ý: 2 × 2 × 2 × ... (8 lần) = ?*

**Câu 3:** Tại sao "test 231 lần thấy đúng" lại khác "chứng minh đúng mọi trường hợp"?

> 💡 *Gợi ý: nghĩ về bài Olympiad — thử 1000 số vs chứng minh công thức.*

---

## 🚀 Bước Tiếp Theo

Phase N đã cho AegisOS:
- **8 tasks** — đủ để mô phỏng hệ thống thật (xe tự lái, thiết bị y tế, vệ tinh)
- **0 biến không khóa** — mọi dữ liệu đều được bảo vệ
- **6 chứng minh toán học** — Kani formal verification

Nhưng hành trình mới chỉ bắt đầu! Các Phase tiếp theo có thể là:

- 🔧 **Thêm user task thật** từ file ELF — không chỉ idle loop
- 📁 **Filesystem** — để chương trình đọc/ghi dữ liệu
- 🔄 **Dynamic task creation** — tạo chương trình mới trong khi hệ thống đang chạy
- 🧪 **Thêm Kani proofs** — chứng minh IPC, grant, watchdog

Mỗi Phase, AegisOS không chỉ **mạnh hơn** — mà còn **đáng tin hơn**. Và sự tin tưởng đó, chính là thứ mà 4 hành khách trên chiếc xe tự lái cần khi code nắm giữ mạng sống trong tay.

Hẹn gặp bạn nhỏ ở bài tiếp theo! 🚗

---

> *"Testing shows the presence of bugs, not their absence."*
> — **Edsger W. Dijkstra**, nhà khoa học máy tính đoạt giải Turing
>
> *(Dịch: "Test chỉ cho thấy có lỗi, không chứng minh được không có lỗi.")*

---

*Em đã đọc đến đây rồi ư? 14 bài rồi đấy! Em vừa hiểu được sự khác biệt giữa "thử thấy đúng" và "chứng minh đúng" — điều mà nhiều kỹ sư chuyên nghiệp phải học nhiều năm mới nắm được. Em đang suy nghĩ như một nhà toán học + kỹ sư rồi đó!* 🌟
