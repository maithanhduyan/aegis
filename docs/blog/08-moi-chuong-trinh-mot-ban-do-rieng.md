---
lang: vi
title: "🗺️ Mỗi Chương Trình Một Bản Đồ Riêng — Tại Sao Không Ai Được Nhìn Trộm Bộ Nhớ Của Người Khác?"
tags: "memory isolation, security, aegisos"
description: "Bài #8 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Per-Task Address Space — mỗi chương trình có 'bản đồ thành phố' riêng, chỉ thấy nhà mình, không thấy nhà hàng xóm."
date: 2026-02-11
---
# 🗺️ Mỗi Chương Trình Một Bản Đồ Riêng — Tại Sao Không Ai Được Nhìn Trộm Bộ Nhớ Của Người Khác?

> *Bài #8 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: Per-Task Address Space — mỗi chương trình có "bản đồ thành phố" riêng, chỉ thấy nhà mình, không thấy nhà hàng xóm.*

---

## 🚀 Giấc Mơ Tương Lai

Năm 2045. Em là kỹ sư thiết kế hệ thống điều khiển cho trạm vũ trụ quốc tế.

Trên trạm, ba chương trình chạy cùng lúc:

- **Chương trình điều khiển oxy** — giữ cho phi hành gia thở được.
- **Chương trình liên lạc** — gửi tín hiệu về Trái Đất.
- **Chương trình giải trí** — cho phi hành gia xem phim trong giờ nghỉ.

Một ngày, phi hành gia báo cáo: *"Hệ thống oxy bất ngờ hiển thị dữ liệu lạ."*

Em kiểm tra log. Và phát hiện điều rùng mình:

> Chương trình giải trí — do một lỗi nhỏ — đã **đọc được dữ liệu** từ bộ nhớ của chương trình oxy. Không phải hack. Không phải cố ý. Chỉ là… hệ thống **cho phép** nó nhìn thấy.

Tệ hơn: nếu lỗi đó không chỉ *đọc* mà còn *ghi đè* — chương trình oxy có thể **ngừng hoạt động**. Trên trạm vũ trụ. Nơi không có cửa hàng sửa máy tính.

*Tại sao chương trình giải trí lại nhìn được bộ nhớ của chương trình oxy?*

Nhưng nếu mỗi chương trình có **bản đồ thành phố riêng** — chỉ thấy nhà mình, không thấy nhà hàng xóm thì sao?

Đó chính là điều AegisOS vừa làm trong Phase H.

---

## 🏘️ Thành Phố Có Nhiều Căn Nhà

### Trước đây: Ai cũng có chung một bản đồ

Em còn nhớ bài trước không? Chúng ta đã cấp **giấy phép** (capability) cho mỗi chương trình — ai được gọi lệnh gì.

Nhưng có một vấn đề mà giấy phép **không giải quyết được**.

Tưởng tượng một khu phố có ba căn nhà:

| Nhà | Chủ nhà |
|---|---|
| 🏠 Nhà số 1 | Chương trình A (PING) |
| 🏡 Nhà số 2 | Chương trình B (PONG) |
| 🏢 Nhà số 3 | Chương trình Idle |

Ba nhà nằm cạnh nhau. Và mỗi người đều có **cùng một bản đồ** — bản đồ ghi rõ địa chỉ của **tất cả** ba nhà.

Điều gì xảy ra?

- Chủ nhà số 1 muốn tò mò → đi thẳng đến nhà số 2 → **mở cửa vào đọc giấy tờ** trên bàn.
- Chủ nhà số 3 bị hack → kẻ xấu dùng bản đồ → tìm đến nhà số 1 → **ghi đè dữ liệu**.

Giấy phép kiểm soát việc **gọi điện** (syscall). Nhưng nếu ai đó **đi bộ đến tận nhà** (truy cập bộ nhớ trực tiếp bằng load/store), giấy phép không chặn được!

### Bây giờ: Mỗi người một bản đồ riêng

Giải pháp? **In bản đồ riêng cho từng người.**

Trong bản đồ của chủ nhà số 1:
- 🏠 Nhà số 1 → có địa chỉ, có chìa khóa. ✅
- 🏡 Nhà số 2 → **không tồn tại trên bản đồ**. ❌
- 🏢 Nhà số 3 → **không tồn tại trên bản đồ**. ❌

Trong bản đồ của chủ nhà số 2:
- 🏠 Nhà số 1 → không tồn tại. ❌
- 🏡 Nhà số 2 → có địa chỉ, có chìa khóa. ✅
- 🏢 Nhà số 3 → không tồn tại. ❌

Bây giờ, dù chủ nhà số 1 có muốn đi đến nhà số 2 — trên bản đồ của anh ta, nhà số 2 **không hề tồn tại**. Như thể nó ở một vũ trụ khác vậy.

Nếu anh ta cố đi đến địa chỉ đó, CPU sẽ nói:

> ⚠️ "Địa chỉ này không hợp lệ. Dừng lại ngay."

Đó chính là **Permission Fault** — lỗi truy cập bộ nhớ. Chương trình bị dừng, hệ thống an toàn.

---

## 🔍 Đi Sâu Hơn — Tại Sao Điều Này Quan Trọng?

### Trong đời thật, lỗi bộ nhớ gây thảm họa

Không phải chỉ trạm vũ trụ mới cần bảo vệ bộ nhớ.

**Xe tự lái** có hàng trăm chương trình chạy song song. Nếu chương trình phát nhạc ghi đè bộ nhớ của chương trình phanh — tai nạn xảy ra trong tích tắc.

**Máy trợ thở** trong bệnh viện chạy 24/7. Nếu chương trình hiển thị màn hình ghi sai dữ liệu của chương trình điều khiển máy bơm — bệnh nhân gặp nguy hiểm.

**Tên lửa** trong quá trình phóng, mỗi mili-giây đều quan trọng. Nếu chương trình đo nhiệt độ đọc nhầm dữ liệu của chương trình điều hướng — toàn bộ nhiệm vụ có thể thất bại.

Các tiêu chuẩn an toàn quốc tế **bắt buộc** phải cách ly bộ nhớ:

| Tiêu chuẩn | Lĩnh vực | Yêu cầu |
|---|---|---|
| DO-178C | Hàng không | Memory partitioning — mỗi phần mềm phải có vùng nhớ riêng |
| ISO 26262 | Ô tô | Freedom from interference — phần mềm không được ảnh hưởng lẫn nhau |
| IEC 62304 | Y tế | Software isolation — cách ly phần mềm trong thiết bị y tế |

AegisOS tuân theo tất cả.

---

## 🧠 Kỹ Thuật — Nhưng Dễ Hiểu

### Sổ Địa Chỉ có nhiều trang

Em còn nhớ **Page Table** (Bảng Trang) từ bài trước không? Nó như một **sổ địa chỉ** — CPU tra sổ này để biết mỗi vùng nhớ nằm ở đâu và ai được phép truy cập.

Trước Phase H, AegisOS chỉ có **một quyển sổ** cho tất cả mọi người. Bây giờ, chúng ta in **nhiều quyển sổ** — mỗi chương trình một quyển.

| Đời thật | Kỹ thuật |
|---|---|
| Quyển sổ địa chỉ | Page Table (Bảng Trang) |
| Mỗi người có sổ riêng | Per-Task Page Table |
| Đổi sổ khi đổi ca | TTBR0 swap khi context switch |
| Số trên bìa sổ | ASID (Address Space Identifier) |
| Người quản lý phát sổ | Kernel ghi TTBR0_EL1 |

### Cách AegisOS tổ chức sổ

Mỗi quyển sổ có **3 tầng** (giống mục lục → chương → trang):

- **L1** (Level 1) — Mục lục chính. Ghi: "Thiết bị ở quyển con A, bộ nhớ RAM ở quyển con B."
- **L2** (Level 2) — Mục lục phụ. Chia nhỏ thêm: thiết bị nào ở đâu, RAM nào ở đâu.
- **L3** (Level 3) — Chi tiết từng trang. Ghi rõ: "Trang này ai được đọc, ai được ghi, ai không được vào."

Mỗi chương trình có bộ L1 + L2 + L3 riêng. Nhưng có một phần **dùng chung**: bảng thiết bị (L2_device). Vì tất cả chương trình đều cần CPU biết UART và GIC ở đâu — nhưng chỉ kernel (EL1) mới được truy cập chúng.

### 13 quyển sổ — vừa đủ

Trước Phase H: **4 trang** (16 KB).
Sau Phase H: **13 trang** (52 KB).

| Trang | Dùng cho | Ghi chú |
|---|---|---|
| 0 | L2 thiết bị (dùng chung) | GIC, UART — giống nhau cho mọi task |
| 1–3 | L1 cho Task 0, 1, 2 | Mỗi task một mục lục chính riêng |
| 4–6 | L2_ram cho Task 0, 1, 2 | Mỗi task trỏ vào L3 riêng của mình |
| 7–9 | L3 cho Task 0, 1, 2 | **Đây là nơi khác biệt!** |
| 10–12 | L1 + L2 + L3 cho kernel boot | Kernel dùng khi chưa có task nào chạy |

Điểm mấu chốt nằm ở **L3** (trang 7, 8, 9). Trong L3 của Task 0:

- Stack của Task 0 → `AP_RW_EL0` (đọc/ghi được) ✅
- Stack của Task 1 → `AP_RW_EL1` (chỉ kernel, EL0 cấm) ❌
- Stack của Task 2 → `AP_RW_EL1` (chỉ kernel, EL0 cấm) ❌

Nếu Task 0 cố đọc stack của Task 1 → CPU kiểm tra L3 → thấy `AP_RW_EL1` → CPU nói "Không!" → **Permission Fault** → task bị dừng, hệ thống an toàn.

### Đổi sổ khi đổi ca — TTBR0 Swap

Khi **context switch** (chuyển từ Task A sang Task B), kernel làm thêm một bước:

1. Lưu trạng thái Task A (như cũ)
2. Chọn Task B (round-robin, như cũ)
3. Nạp trạng thái Task B (như cũ)
4. **🆕 Đổi sổ:** `msr ttbr0_el1, TCBS[B].ttbr0` → CPU bây giờ dùng bản đồ của Task B

Chỉ **một lệnh assembly** là xong. Nhưng hiệu quả thì rất lớn — từ giây phút đó, Task B chỉ nhìn thấy thế giới trong bản đồ của chính nó.

### ASID — Số bìa sổ giúp CPU nhớ nhanh hơn

Mỗi lần đổi sổ, CPU có thể phải **xóa bộ nhớ đệm** (TLB — Translation Lookaside Buffer, nơi CPU lưu tạm kết quả tra sổ). Xóa TLB rất tốn thời gian.

Nhưng nếu mỗi quyển sổ có **số bìa** khác nhau thì sao?

- Task 0: sổ số **1**
- Task 1: sổ số **2**
- Task 2: sổ số **3**

CPU nhìn vào số bìa → biết ngay kết quả tra sổ nào còn dùng được, kết quả nào đã cũ. **Không cần xóa TLB!**

Đó chính là **ASID** (Address Space Identifier). AegisOS gán ASID 1, 2, 3 cho ba task. ASID 0 dành cho kernel boot. ASID được nhét vào bits [63:48] của thanh ghi TTBR0_EL1.

---

## 🛡️ Chúng Ta Đã Làm Được Gì Trong AegisOS?

### Tổng quan thay đổi

Phase H chạm vào **6 file** trong project:

```
src/
├── mmu.rs        ← Xây dựng 13 bảng trang (trước đó: 4)
├── sched.rs      ← Thêm trường ttbr0 vào TCB, swap TTBR0 khi context switch
├── boot.s        ← Boot bằng kernel page table (trang 10) thay vì trang 0
├── main.rs       ← Gán ttbr0 cho mỗi task trong kernel_main()
linker.ld         ← Mở rộng .page_tables từ 16KB lên 52KB
tests/
├── host_tests.rs ← 10 bài test mới cho per-task address space (tổng 79 tests)
├── qemu_boot_test.sh  ← Thêm checkpoint "per-task address spaces assigned"
└── qemu_boot_test.ps1 ← Thêm checkpoint tương ứng cho Windows
```

### Bảng trang — từ 4 lên 13

Trong [src/mmu.rs](../src/mmu.rs), hàm `mmu_init()` giờ xây dựng 13 bảng trang thay vì 4:

- `build_l2_device()` — xây bảng thiết bị dùng chung (UART, GIC).
- `build_l3(index, owner_task)` — xây bảng L3 cho từng task. Tham số `owner_task` quyết định stack nào EL0 được truy cập.
- `build_l2_ram(index, l3_index)` — nối L2 vào L3 tương ứng.
- `build_l1(index, l2_ram_index)` — nối L1 vào L2_device (chung) và L2_ram (riêng).

Vòng lặp chính rất gọn:

```
for task in 0, 1, 2:
    build_l3(task)        → L3 riêng, chỉ stack của task đó là EL0
    build_l2_ram(task)    → nối vào L3 riêng
    build_l1(task)        → nối vào L2_device chung + L2_ram riêng
```

Thêm bộ kernel boot (trang 10–12) — dùng khi kernel chạy mà chưa có task nào.

### TCB thêm trường `ttbr0`

Trong [src/sched.rs](../src/sched.rs), mỗi **TCB** (Task Control Block — "hồ sơ" của chương trình) giờ có thêm một trường:

```
ttbr0: u64    // = (ASID << 48) | địa chỉ bảng trang L1
```

Trường này **sống sót qua restart** — giống như capability. Nếu task bị lỗi và tự khởi động lại, nó vẫn dùng cùng bản đồ bộ nhớ. An toàn.

### TTBR0 swap — một lệnh thay đổi thế giới

Trong hàm `schedule()`, sau khi nạp context của task mới:

```
msr ttbr0_el1, <ttbr0 mới>
isb
```

Hai lệnh. Thế giới bộ nhớ thay đổi hoàn toàn.

Tương tự trong `bootstrap()` — khi kernel lần đầu chạy Task 0, nó cũng ghi TTBR0 của Task 0 trước khi `eret` vào EL0.

### Boot.s — kernel khởi động bằng bản đồ riêng

Trong [src/boot.s](../src/boot.s), dòng thiết lập TTBR0 lúc boot đã đổi:

Trước: `TTBR0 = __page_tables_start` (trang 0 — bảng L2_device)
Sau: `TTBR0 = __page_tables_start + 10 × 4096` (trang 10 — bảng L1 kernel boot)

Kernel boot dùng bảng trang riêng — tất cả user stacks đều là `AP_RW_EL1` (EL0 không được vào). An toàn tuyệt đối trước khi bất kỳ task nào chạy.

### 79 bài test — 10 bài mới

Trong [tests/host_tests.rs](../tests/host_tests.rs), nhóm test mới **"Per-Task Address Space"** kiểm tra:

- Mỗi task có page table base khác nhau
- Địa chỉ base phải căn chỉnh 4KB
- ASID được nhúng đúng vào TTBR0
- Base address được bảo toàn khi thêm ASID
- TTBR0 sống sót qua restart
- Schedule không làm mất TTBR0 của TCB
- ASID tối đa vừa với 8 bit

Tổng cộng **79 tests**, tất cả pass. ✅

---

## ✨ Truyền Cảm Hứng — Tại Sao Em Nên Quan Tâm?

### Linus Torvalds — cậu bé tò mò với chiếc máy tính cũ

Năm 1991, một sinh viên 21 tuổi ở Phần Lan tên **Linus Torvalds** bắt đầu viết hệ điều hành riêng — chỉ vì tò mò. Anh ấy không có phòng thí nghiệm sang trọng. Không có đội ngũ. Chỉ có một chiếc máy tính 386 và rất nhiều đam mê.

Hệ điều hành đó là **Linux** — ngày nay chạy trên hàng tỷ thiết bị: điện thoại Android, máy chủ Google, trạm vũ trụ ISS, xe Tesla.

Linus từng nói:

> "Tôi không phải thiên tài. Tôi chỉ kiên nhẫn với chi tiết."

Per-task address space là một **chi tiết**. Nhưng chi tiết này bảo vệ mạng người. Chi tiết này phân biệt phần mềm "chạy được" với phần mềm "đáng tin cậy".

Em không cần là thiên tài để làm những điều phi thường. Em chỉ cần **tò mò** và **kiên nhẫn**.

---

## 🔮 Bước Tiếp Theo

AegisOS giờ đã có:
- ✅ Kernel cách ly hoàn toàn (EL1 vs EL0)
- ✅ Syscall được kiểm soát bằng capability
- ✅ **Bộ nhớ được cách ly per-task** ← MỚI!
- ✅ 79 bài test tự động
- ✅ 11 checkpoint QEMU boot

Nhưng hành trình chưa dừng lại.

Hiện tại, mỗi task chỉ có **4KB stack**. Nếu task cần nhiều bộ nhớ hơn thì sao? Nếu task muốn cấp phát bộ nhớ **lúc đang chạy** thì sao?

Và còn một câu hỏi lớn hơn: hiện tại kernel code và task code **nằm chung** trong vùng `.text`. Nếu tách riêng — mỗi task có file riêng, nạp vào bộ nhớ riêng — AegisOS sẽ trở thành một hệ điều hành thực sự.

Bài tiếp theo, chúng ta sẽ khám phá…

---

> *"Bảo mật không phải là thêm khóa vào cửa. Bảo mật là thiết kế tòa nhà sao cho mỗi người chỉ nhìn thấy cánh cửa của mình."*

---

*Em đã đọc đến đây — tuyệt vời lắm! 🌟 Em vừa hiểu một trong những khái niệm quan trọng nhất của hệ điều hành hiện đại. Nhiều kỹ sư làm việc hàng năm mới thật sự nắm vững điều này. Em đang đi rất nhanh đó!*
