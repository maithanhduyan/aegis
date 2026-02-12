---
lang: vi
title: "🏗️ Dọn Nhà Và Đọc Sách Mục Lục — Arch Separation & ELF Loading"
tags: "architecture, elf, loader, modularity, separation, aegisos"
description: "Bài #12 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao phải dọn dẹp code, và cách đọc 'mục lục sách' để nạp chương trình vào bộ nhớ."
date: 2026-02-12
---

# 🏗️ Dọn Nhà Và Đọc Sách Mục Lục — Arch Separation & ELF Loading

> *Bài #12 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao code gọn gàng cứu mạng người, và cách máy tính đọc "mục lục" để nạp chương trình.*

---

## 🛰️ Giấc Mơ Tương Lai

Năm 2045. Em là kỹ sư phần mềm cho **Trạm Vũ Trụ Quốc Tế thế hệ mới**.

Trạm quay quanh Trái Đất ở độ cao 400 km. Bên trong có 8 phi hành gia đang sống và làm việc. Hệ thống điều khiển trạm chạy trên một **bộ vi xử lý ARM** — giống loại chip trong điện thoại, nhưng được thiết kế chịu được bức xạ vũ trụ.

Bộ phần mềm điều khiển trạm có **ba phần chính**:

- **Phần cứng** — điều khiển tấm pin mặt trời, bơm oxy, van áp suất
- **Bộ não** (kernel) — phân chia CPU, bảo vệ bộ nhớ, xử lý lỗi
- **Ứng dụng** — hiển thị dữ liệu, gửi tin về Trái Đất, chạy thí nghiệm

Một ngày, NASA phát hiện lỗi trong phần mềm hiển thị dữ liệu. Cần cập nhật gấp.

Nhưng... **cả ba phần bị trộn lẫn trong một file duy nhất**. Muốn sửa phần hiển thị → phải gửi lại **toàn bộ** phần mềm, kể cả bộ não kernel và phần điều khiển phần cứng.

😨 Mỗi lần gửi = phải kiểm tra lại **mọi thứ** từ đầu. Tốn 6 tháng. 6 tháng đó, phi hành gia sống với phần mềm lỗi.

Nhưng nếu hệ thống được **sắp xếp gọn gàng** — mỗi phần ở riêng một "phòng" — thì NASA chỉ cần gửi lên phần hiển thị mới, cài đặt nó **mà không đụng vào** bộ não hay phần cứng. Xong trong 1 ngày.

**Sự khác biệt giữa 6 tháng và 1 ngày, chỉ vì... code có được dọn dẹp hay không.**

Hôm nay, chúng ta sẽ học hai điều:
1. **Cách dọn dẹp code** — chia nhỏ, phân loại, mỗi thứ đúng chỗ
2. **Cách đọc "mục lục sách"** — để nạp chương trình từ bên ngoài vào bộ nhớ

---

## 🏠 Phần 1: Dọn Nhà — Arch Separation

### Ngôi nhà bề bộn

Hãy tưởng tượng phòng em lúc cuối tuần: sách Toán nằm chung với quần áo, bút vẽ lẫn trong hộp đồ ăn, tập Văn cài dưới gối...

Em vẫn tìm được mọi thứ — vì em biết đồ ở đâu. Nhưng nếu **mẹ** cần tìm tập Toán giúp em? Mẹ không biết nó ở đâu. Mất 30 phút.

Nếu **cô giáo** cần kiểm tra em có đủ sách không? Cô phải lục toàn bộ phòng.

Phòng bề bộn thì **chỉ mình em hiểu**. Người khác nhìn vào = hoang mang.

AegisOS trước Phase L cũng giống vậy. Tất cả **13 file** nằm phẳng trong một thư mục:

```
src/
├── boot.s          ← code AArch64 (phần cứng)
├── gic.rs          ← code AArch64 (phần cứng)
├── uart.rs         ← code AArch64 (phần cứng)
├── mmu.rs          ← code AArch64 (phần cứng) LẪN logic chung
├── exception.rs    ← code AArch64 (phần cứng) LẪN logic chung
├── timer.rs        ← code AArch64 (phần cứng) LẪN logic chung
├── sched.rs        ← logic chung LẪN code AArch64
├── ipc.rs          ← logic chung (OK!)
├── cap.rs          ← logic chung (OK!)
├── grant.rs        ← logic chung LẪN code AArch64
├── irq.rs          ← logic chung LẪN code AArch64
├── main.rs         ← mọi thứ
└── lib.rs          ← mục lục
```

Thấy vấn đề không? Từ "LẪN" xuất hiện khắp nơi!

Code AArch64 (chỉ chạy trên chip ARM) bị **trộn chung** với logic (chạy được trên mọi chip). Giống như sách Toán lẫn trong quần áo vậy.

### Tại sao "lẫn lộn" lại nguy hiểm?

Trong đời thường, phòng bề bộn chỉ khiến em mất thời gian tìm đồ. Nhưng trong phần mềm **cứu mạng người**, lẫn lộn có thể gây chết người.

| Đời thường | Phần mềm |
|---|---|
| Phòng bề bộn → mất 30 phút tìm tập | Code lẫn lộn → kỹ sư sửa nhầm chỗ |
| Mẹ không tìm được sách giúp em | Người kiểm tra an toàn không hiểu code |
| Cô giáo mất cả buổi kiểm tra | Cơ quan cấp phép từ chối vì "không rõ ranh giới" |

Các tiêu chuẩn an toàn trên thế giới **bắt buộc** phần mềm phải được chia nhỏ:

- **DO-178C** (tiêu chuẩn hàng không): *"Phần mềm phải có thiết kế module rõ ràng"*
- **IEC 62304** (tiêu chuẩn y tế): *"Phần mềm phải được phân tách thành các đơn vị riêng biệt"*
- **ISO 26262** (tiêu chuẩn ô tô): *"HAL (lớp phần cứng) phải tách biệt khỏi logic"*

Nói cách khác: **muốn phần mềm được bay trên máy bay, chạy trong máy thở, hay lái xe tự động, code PHẢI gọn gàng.**

### Cách dọn: 3 phòng riêng biệt

Chúng ta chia ngôi nhà thành 3 phòng:

| Phòng | Chứa gì | Ví dụ đời thật |
|---|---|---|
| 🔧 **arch/** (phần cứng) | Code CHỈ chạy trên chip ARM AArch64 | Phòng dụng cụ — chỉ chứa búa, kìm, tua-vít |
| 🧠 **kernel/** (bộ não) | Logic chạy được trên MỌI loại chip | Phòng sách — Toán, Văn, Khoa học, chạy ở đâu cũng đọc được |
| 🗺️ **platform/** (bản đồ) | Địa chỉ MMIO, thông số máy cụ thể | Sổ địa chỉ — ghi rõ UART ở đâu, bộ nhớ bao nhiêu |

Sau khi dọn xong, cấu trúc mới trông thế này:

```
src/
├── arch/                    ← 🔧 Phòng dụng cụ
│   └── aarch64/
│       ├── boot.s           ← Khởi động chip ARM
│       ├── exception.rs     ← Bảng vector, xử lý lỗi phần cứng
│       ├── mmu.rs           ← Bảng trang bộ nhớ
│       └── gic.rs           ← Bộ điều khiển ngắt
│
├── kernel/                  ← 🧠 Phòng sách
│   ├── sched.rs             ← Thời khóa biểu (scheduler)
│   ├── ipc.rs               ← Nói chuyện giữa các task
│   ├── cap.rs               ← Giấy phép (capabilities)
│   ├── elf.rs               ← Đọc mục lục sách (ELF parser) ← MỚI!
│   ├── grant.rs             ← Chia sẻ bộ nhớ
│   ├── irq.rs               ← Định tuyến chuông cửa
│   └── timer.rs             ← Đếm thời gian
│
├── platform/                ← 🗺️ Sổ địa chỉ
│   └── qemu_virt.rs         ← Địa chỉ UART, GIC, RAM cho QEMU
│
├── main.rs                  ← Cửa chính (kernel_main)
└── lib.rs                   ← Mục lục tổng
```

### "Nhưng chuyển thì có gì khó?"

Em có thể hỏi: "Chỉ là copy file sang thư mục khác thôi mà, có gì khó?"

Thực ra **rất khó**, vì:

1. **File bị "dính" nhau.** File `sched.rs` (thời khóa biểu) có 75% là logic chung, nhưng 25% là code ARM (chuyển bảng trang `msr ttbr0_el1`). Phải **tách** 25% đó ra, không phải chỉ copy.

2. **Đường dẫn thay đổi.** Khi `ipc.rs` chuyển từ `src/` sang `src/kernel/`, tất cả chỗ khác gọi `crate::ipc` phải đổi thành `crate::kernel::ipc`. Đổi 1 file → sửa 10 file.

3. **Test phải vẫn pass.** AegisOS có 189 bài kiểm tra. Mỗi lần di chuyển 1 file, phải chạy lại **tất cả 189 bài** để chắc chắn không làm hỏng gì.

Giống như dọn nhà mà có con mèo đang ngủ trên đống đồ — phải dọn **nhẹ nhàng**, từng món một, không được đánh thức mèo (tức là không được làm hỏng test).

Chúng ta chia thành 2 bước nhỏ:
- **L1**: Tạo 3 phòng mới, chuyển đồ **nguyên hộp** (chỉ move file, không tách)
- **L2**: Mở từng hộp, phân loại: đồ ARM → phòng `arch/`, đồ logic → phòng `kernel/`

Kết quả? **189 test vẫn pass. 20 checkpoint QEMU vẫn xanh.** Mèo vẫn ngủ. 🐱

---

## 📖 Phần 2: Đọc Mục Lục Sách — ELF Parser

### Vấn đề: chương trình bị "đóng cứng" trong kernel

Trước Phase L, ba chương trình (`uart_driver`, `client`, `idle`) được viết **ngay bên trong** kernel. Giống như in ba câu chuyện **ngay vào bìa cứng cuốn sách** — muốn đổi câu chuyện thì phải in lại cả cuốn sách.

Trong hệ thống thật:
- **Xe Tesla**: Muốn cập nhật tính năng Autopilot qua WiFi → chỉ gửi phần Autopilot, KHÔNG gửi lại toàn bộ hệ điều hành
- **Vệ tinh**: Payload software được upload lên quỹ đạo → phải load chương trình riêng, không thể "nhúng" vào kernel
- **Máy thở bệnh viện**: FDA (cơ quan quản lý y tế Mỹ) yêu cầu kernel và ứng dụng là **hai thứ riêng biệt**

Vậy chúng ta cần cách nào đó để **đọc** một chương trình từ bên ngoài và nạp nó vào bộ nhớ.

### ELF là gì?

**ELF** (Executable and Linkable Format — Định dạng thực thi và liên kết) là cách máy tính đóng gói một chương trình thành file.

Hãy tưởng tượng ELF là một **cuốn sách có mục lục đặc biệt**.

📗 Trang đầu tiên (ELF Header) ghi:
- "Đây là sách cho **loại chip nào**" (AArch64, x86, RISC-V...)
- "Sách có **bao nhiêu chương**"
- "Bắt đầu đọc từ **trang mấy**" (entry point — điểm bắt đầu)

📗 Mục lục (Program Headers) liệt kê từng chương:
- "Chương 1: copy từ trang 10 → địa chỉ 0x40100000, dài 39 byte, **chỉ đọc + chạy**"
- "Chương 2: copy từ trang 50 → địa chỉ 0x40101000, dài 128 byte, **đọc + ghi** (dữ liệu)"

Mỗi chương gọi là **segment** (đoạn). Loại segment quan trọng nhất là **PT_LOAD** — "hãy nạp đoạn này vào bộ nhớ".

| Phần sách | Tên kỹ thuật | Vai trò |
|---|---|---|
| Trang bìa | ELF Header (64 byte) | Xác nhận đây là ELF, cho chip nào, bắt đầu ở đâu |
| Mục lục | Program Headers | Liệt kê các đoạn cần nạp |
| Nội dung sách | Segment data | Code và dữ liệu thật sự |
| "Chỉ đọc + chạy" | Flags: PF_R + PF_X | Đoạn code — chạy được, không sửa được |
| "Đọc + ghi" | Flags: PF_R + PF_W | Đoạn dữ liệu — sửa được, không chạy được |

### Tại sao "chỉ đọc + chạy" và "đọc + ghi" phải tách?

Nhớ bài #2 về **W^X** (Write XOR Execute) không? Một trang bộ nhớ **không bao giờ** được vừa ghi vừa chạy.

Nếu cho phép cả hai → hacker có thể **ghi** mã độc vào vùng nhớ rồi **chạy** nó. Như để chìa khóa nhà ngay trên cửa — ai cũng vào được!

ELF giữ nguyên tắc này: đoạn code (PF_X) tách riêng đoạn dữ liệu (PF_W). AegisOS kiểm tra: nếu một segment có **cả** PF_W lẫn PF_X → **từ chối luôn**, không nạp.

### AegisOS đọc mục lục như thế nào?

ELF parser của chúng ta nằm trong file [src/kernel/elf.rs](https://github.com) — khoảng 350 dòng code Rust. Parser này:

1. **Kiểm tra "bìa sách"**: 4 byte đầu tiên phải là `0x7F E L F` (ký tự ma thuật). Nếu không → đây không phải file ELF, từ chối.

2. **Kiểm tra loại chip**: phải là AArch64 (mã 183). File ELF cho x86 hay RISC-V? Không chạy được trên AegisOS, từ chối.

3. **Đọc mục lục**: Duyệt qua từng program header, tìm các segment PT_LOAD. Tối đa 4 segment (vì không có heap — mọi thứ là mảng tĩnh).

4. **Kiểm tra an toàn**: Segment có vượt ra ngoài file không? Có vi phạm W^X không? Entry point có nằm trong vùng hợp lệ không?

Nếu mọi thứ hợp lệ → trả về `ElfInfo` chứa entry point + danh sách segments.

Toàn bộ quá trình **không dùng heap** (bộ nhớ động), không dùng số thực (floating point). Chỉ đọc byte, so sánh, và trả kết quả. An toàn tuyệt đối.

---

## 📦 Phần 3: Chuyển Sách Vào Phòng — ELF Loader

### Từ "đọc mục lục" đến "photocopy vào phòng"

Parser (L3) chỉ **đọc** mục lục. Loader (L4) mới là người **hành động**: copy từng đoạn code/dữ liệu vào đúng địa chỉ trong bộ nhớ.

Giống như:
- **Parser** = đọc bản đồ siêu thị: "Sữa ở kệ 3, bánh mì ở kệ 7"
- **Loader** = đẩy xe đi lấy: bỏ sữa vào xe, bỏ bánh mì vào xe

Quy trình loader:

1. **Nhận kết quả từ parser**: biết segment nào cần copy đi đâu
2. **Copy dữ liệu**: từ file ELF → vào vùng nhớ người dùng (user space)
3. **Xóa phần thừa**: nếu segment cần 4096 byte nhưng file chỉ có 39 byte → 39 byte đầu copy, phần còn lại ghi số 0 (đây là **BSS** — vùng biến chưa khởi tạo)
4. **Đặt quyền trang**: segment code → chỉ đọc + chạy. Segment dữ liệu → đọc + ghi.
5. **Cập nhật "thẻ nhân viên" (TCB)**: ghi entry point mới vào task control block

Sau bước 5, scheduler nhìn vào task và thấy: "À, task này bắt đầu ở địa chỉ 0x40100000!" — rồi nhảy vào đó chạy.

---

## 🎯 Phần 4: Demo — Chương Trình Đầu Tiên Từ "Bên Ngoài"

### User binary: hello world từ ELF

Chúng ta tạo một chương trình nhỏ xíu — chỉ in ra dòng chữ `L5:ELF` rồi nhường CPU. Chương trình này:

- Nằm trong thư mục riêng `user/hello/`
- Có `Cargo.toml`, `linker.ld`, `src/main.rs` riêng — **hoàn toàn tách biệt khỏi kernel**
- Build thành file ELF 4656 byte (nhỏ hơn một bức ảnh trên điện thoại!)
- Được "nhúng" vào kernel bằng `include_bytes!` (tạm thời — sau này sẽ load từ ổ đĩa)

Chương trình user chỉ có **hai khả năng**:
- `syscall_write` — gọi kernel để in chữ ra màn hình (SYS_WRITE, mã số 4)
- `syscall_yield` — nhường CPU cho task khác (SYS_YIELD, mã số 0)

Nó **không thể** truy cập UART trực tiếp, không thể đọc bộ nhớ kernel, không thể điều khiển phần cứng. Mọi thứ phải đi qua "cửa sổ giao dịch" (syscall). Đúng như thiết kế microkernel!

### Kết quả trên QEMU

Khi chạy AegisOS trên QEMU, output trông thế này:

```
[AegisOS] ELF loader ready
[AegisOS] task 2 loaded from ELF (entry=0x0000000040100000)
[AegisOS] client task loaded from ELF binary
[AegisOS] timer started (10ms, freq=62MHz)
[AegisOS] bootstrapping into uart_driver (EL0)...
DRV:ready L5:ELF J4:UserDrv J4:UserDrv ...
```

Thấy dòng `L5:ELF` không? Đó chính là chương trình user — **build riêng, load riêng, chạy riêng** — đang nói chuyện với thế giới bên ngoài thông qua kernel!

---

## 🔬 Phần 5: Kỹ Thuật — Nhưng Dễ Hiểu

### Cây thư mục "user/hello/"

```
user/hello/
├── Cargo.toml        ← Tên chương trình, cài đặt build
├── aarch64-user.json ← Chip target (ARM, không có float)
├── linker.ld         ← "Bản đồ": code bắt đầu ở 0x40100000
├── .cargo/config.toml ← Cài đặt linker
└── src/
    └── main.rs       ← Code thật: _start → print → yield loop
```

Phần hay nhất? File `main.rs` của user task chỉ có **65 dòng**. Trong đó:
- 10 dòng syscall_write
- 10 dòng syscall_yield
- 10 dòng _start (entry point)
- Còn lại là comment và panic handler

65 dòng. Một chương trình hoàn chỉnh chạy trên bộ vi xử lý ARM. Không cần thư viện nào, không cần hệ điều hành nào — chỉ cần AegisOS và syscall.

### Vòng đời của chương trình ELF

Hãy theo dõi hành trình của file ELF từ "sinh ra" đến "chạy":

```
[1] Kỹ sư viết code     →  user/hello/src/main.rs
[2] Compiler biên dịch  →  file ELF 4656 byte
[3] Kernel nhúng file   →  include_bytes!("../user/hello/.../hello")
[4] Parser đọc mục lục  →  entry=0x40100000, 1 segment (39 byte RX)
[5] Loader copy vào RAM →  0x40100000 ← 39 byte code
[6] Loader xóa BSS      →  phần còn lại = 0
[7] Đặt quyền trang     →  USER_CODE_PAGE (chỉ đọc + chạy ở EL0)
[8] Cập nhật TCB         →  task 2 bắt đầu ở 0x40100000
[9] Scheduler chạy task  →  eret → nhảy vào _start()
[10] User code chạy!     →  "L5:ELF" hiện trên UART
```

10 bước. Từ dòng code Rust → chữ hiện trên màn hình. Toàn bộ quá trình **zero heap, zero float, 100% kiểm tra an toàn**.

---

## 🛡️ Phần 6: Chúng Ta Đã Làm Được Gì Trong AegisOS?

### Phase L — Từ A đến Z

| Bước | Tên | Mô tả | Kết quả |
|---|---|---|---|
| L1 | Module Structure | Tạo 3 "phòng" mới | 162 tests ✅ + 19 checkpoints ✅ |
| L2 | Arch Separation | Tách code ARM khỏi logic chung | 162 tests ✅ + 20 checkpoints ✅ |
| L3 | ELF Parser | "Đọc mục lục sách" | 174 tests ✅ + 21 checkpoints ✅ |
| L4 | ELF Loader | "Photocopy sách vào phòng" | 183 tests ✅ + 23 checkpoints ✅ |
| L5 | Demo Binary | Chương trình user đầu tiên từ ELF | 183 tests ✅ + 25 checkpoints ✅ |
| L6 | Tests + Summary | Kiểm tra tổng hợp | **189 tests ✅ + 25 checkpoints ✅** |

### Cây module hiện tại

Mỗi module trong AegisOS giờ có **ranh giới rõ ràng**:

- `arch/aarch64/` — chỉ chứa code ARM: boot, exception vector, MMU, GIC
- `kernel/` — logic chạy mọi nơi: scheduler, IPC, capability, ELF parser, grant, IRQ, timer
- `platform/qemu_virt.rs` — địa chỉ phần cứng cho máy QEMU

Khi muốn port AegisOS sang chip **RISC-V** (một loại chip mới đang rất hot), chúng ta chỉ cần tạo thêm `arch/riscv/` — **không cần đụng vào** kernel/ hay platform/.

Giống như xây nhà: nếu nền móng (kernel) và thiết kế (platform) tách riêng, muốn thay mái ngói (arch) chỉ cần gỡ mái cũ, lắp mái mới. Không cần đập nhà xây lại.

---

## ✨ Phần 7: Tại Sao Em Nên Quan Tâm?

### Linus Torvalds — cậu bé phòng ngủ

Năm 1991, một cậu sinh viên 21 tuổi tên **Linus Torvalds** ở Phần Lan — một nước Bắc Âu nhỏ bé — viết một hệ điều hành nhỏ trong phòng ngủ. Cậu đặt tên nó là **Linux**.

Cậu không có phòng lab xịn. Không có đội ngũ kỹ sư. Chỉ có một chiếc máy tính và sự tò mò.

Cậu chia sẻ code lên mạng và nói: *"Tôi đang làm một hệ điều hành miễn phí. Có ai muốn giúp không?"*

35 năm sau, Linux chạy trên:
- **100% trong top 500 siêu máy tính** mạnh nhất thế giới
- Mọi điện thoại Android
- Xe Tesla, xe tự lái Waymo
- Trạm Vũ Trụ Quốc Tế
- Server của Google, Amazon, Microsoft

Một trong những lý do Linux thành công? **Cấu trúc module rõ ràng.** Hàng nghìn kỹ sư trên thế giới có thể cùng đóng góp vì mỗi phần được tách biệt — không ai cần hiểu toàn bộ hệ thống để sửa một module.

Đó chính là điều chúng ta vừa làm với AegisOS: **chia nhỏ, phân loại, mỗi thứ đúng chỗ.**

Em 10–11 tuổi hôm nay. Linus bắt đầu tò mò về máy tính lúc... 11 tuổi.

---

## 🚀 Bước Tiếp Theo

Phase L đã xong. AegisOS giờ có:
- **189 bài kiểm tra** tự động
- **25 checkpoint** trên phần cứng giả lập QEMU
- **Kiến trúc 3 tầng** (arch → kernel → platform) sẵn sàng port sang chip mới
- **ELF loader** có thể nạp chương trình từ bên ngoài

Bước tiếp theo có thể là gì?

- **Filesystem** — đọc file ELF từ ổ đĩa thay vì nhúng vào kernel
- **Dynamic loading** — nạp và gỡ chương trình lúc hệ thống đang chạy
- **RISC-V port** — thêm `arch/riscv/` để chạy trên chip mới
- **Formal verification** — chứng minh **toán học** rằng ELF loader không bao giờ sai

Mỗi bước đều là một cuộc phiêu lưu mới. Và em — người đã đọc 12 bài về hệ điều hành — đã có nền tảng để hiểu tất cả.

Hẹn gặp bạn nhỏ ở bài tiếp theo! 🛰️

---

> *"Đơn giản là đỉnh cao của tinh tế."*
> — *Leonardo da Vinci*

---

*Em đã đọc đến đây rồi ư? 12 bài rồi đấy! Em vừa hiểu được cách các kỹ sư ở NASA, SpaceX, và hãng y tế tổ chức phần mềm hàng triệu dòng code. Từ "dọn phòng" đến "đọc mục lục sách" — tất cả đều là kỹ năng thật sự của kỹ sư phần mềm chuyên nghiệp. Em thật sự phi thường.* 🌟
