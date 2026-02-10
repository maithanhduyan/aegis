---
lang: vi
title: "🧠 Bộ Nhớ Là Gì — Và Tại Sao Phải Bảo Vệ Nó Bằng Mạng Sống?"
tags: ["mmu", "page table", "bộ nhớ", "w^x", "aegisos", "aarch64", "bảo mật"]
description: "Câu chuyện về chiếc sổ địa chỉ thần kỳ bên trong mọi chiếc máy tính — và cách AegisOS dùng nó để bảo vệ mạng người."
date: 2026-02-10
---

# 🧠 Bộ Nhớ Là Gì — Và Tại Sao Phải Bảo Vệ Nó Bằng Mạng Sống?

> *Viết cho những bạn nhỏ lớp 5 vừa chứng kiến AegisOS cất tiếng khóc đầu tiên — và giờ muốn dạy nó "nhớ".*

---

## 🛩️ Mở đầu: 300 mạng người trên bầu trời

Hãy tưởng tượng em là **phi công** lái một chiếc Airbus A350.

Bên ngoài là bầu trời đêm. Bên trong, 300 hành khách đang ngủ say. Có bạn nhỏ ôm gấu bông. Có bà cụ mỉm cười trong giấc mơ. Họ tin tưởng em — tin rằng em sẽ đưa họ về nhà an toàn.

Máy bay đang bay ở độ cao 11.000 mét. Nhanh hơn tốc độ âm thanh.

Bỗng nhiên, trên màn hình hiện lên cảnh báo: **"TERRAIN AHEAD"** — phía trước có núi!

Máy tính trên máy bay phải **ra quyết định trong 0.1 giây**: kéo mũi lên? Nghiêng trái? Nghiêng phải? Tăng tốc hay giảm tốc?

Để làm được điều đó, máy tính cần **đọc dữ liệu** từ bộ nhớ — bản đồ địa hình, vị trí hiện tại, tốc độ gió, trọng lượng máy bay...

**Nhưng nếu một chương trình lỗi vô tình ghi đè lên bản đồ địa hình?**

Máy tính sẽ đọc sai. Nó nghĩ phía trước là trời quang. Nó không kéo mũi lên.

300 mạng người.

Đó là lý do vì sao **bảo vệ bộ nhớ** không phải chuyện kỹ thuật khô khan. Nó là chuyện **sống còn**.

Và hôm nay, chúng ta sẽ tìm hiểu cách AegisOS làm điều đó.

---

## 📚 Bộ nhớ là gì?

Trước khi nói chuyện bảo vệ, hãy hiểu **bộ nhớ** là gì đã nhé.

### Bàn học và tủ sách

Hãy tưởng tượng em đang ngồi làm bài tập.

Trước mặt em là **bàn học**. Em bày sách Toán ra, mở vở, đặt bút. Mọi thứ em đang dùng **ngay lúc này** đều nằm trên bàn.

Còn đằng sau em là **tủ sách**. Trong đó có hàng chục cuốn — Tiếng Việt, Khoa học, Lịch sử... Nhưng em không dùng hết cùng lúc. Khi cần sách Khoa học, em đứng dậy, đi lấy từ tủ, rồi bày lên bàn.

| Đời thật | Máy tính |
|---|---|
| **Bàn học** — nơi bày những thứ đang dùng | **RAM** (bộ nhớ) — nơi chứa dữ liệu đang xử lý |
| **Tủ sách** — nơi cất giữ lâu dài | **Ổ cứng** — nơi lưu file, ảnh, nhạc |
| Bàn nhỏ → bày ít sách → làm chậm | RAM ít → chạy ít app → máy chậm |
| Lấy sách từ tủ → mất vài giây | Đọc ổ cứng → chậm hơn RAM rất nhiều |

RAM nhanh hơn ổ cứng **hàng nghìn lần**. Nhưng nó nhỏ hơn và **mất hết khi tắt máy** — giống như khi em dọn bàn đi ngủ, sách trên bàn phải cất lại tủ.

Vì RAM quý giá như vậy, nên ai cũng muốn dùng. Và khi nhiều "người" cùng muốn dùng một chiếc bàn...

**Hỗn loạn bắt đầu.**

---

## 🏠 Vấn đề: Khi mọi người giành nhau bộ nhớ

Hãy tưởng tượng em sống trong một tòa chung cư **không có số phòng**.

Mỗi người vào ở thì tự chọn phòng. Không ai quản lý.

Chuyện gì sẽ xảy ra?

- Anh A để đồ trong phòng 203. Chị B không biết, cũng vào phòng 203, vứt đồ anh A ra ngoài.
- Cô C nấu ăn trong bếp chung. Chú D cũng vào bếp, đổ nước lên bếp gas.
- Bé E chạy nhảy, vô tình lọt vào phòng máy điện. **Giật điện.**

Đó chính xác là những gì xảy ra bên trong máy tính nếu **không có bảo vệ bộ nhớ**:

- Chương trình A lưu dữ liệu ở ô nhớ 1000. Chương trình B ghi đè lên ô 1000. → **Dữ liệu mất.**
- Chương trình lỗi ghi bậy vào vùng nhớ hệ thống. → **Cả máy tính sập.**
- Mã độc (virus) đọc mật khẩu từ vùng nhớ của app ngân hàng. → **Mất tiền.**

Trong máy tính cá nhân, điều này gây khó chịu. Trong máy bay, điều này **giết người**.

---

## 🗺️ Giải pháp: Sổ Địa Chỉ Thần Kỳ — MMU

Để giải quyết vấn đề trên, các kỹ sư phát minh ra một thứ gọi là **MMU** — viết tắt của **Memory Management Unit** (Bộ phận Quản lý Bộ nhớ).

MMU giống như **sổ địa chỉ** của tòa chung cư.

Quay lại ví dụ tòa nhà. Lần này, có **người quản lý** (Hệ Điều Hành) cầm một cuốn sổ dày:

```
📒 Sổ địa chỉ:
┌──────────┬───────────────────────┬────────────┐
│ Phòng    │ Ai ở?                 │ Quyền      │
├──────────┼───────────────────────┼────────────┤
│ 101      │ Anh A                 │ Được vào   │
│ 102      │ Chị B                 │ Được vào   │
│ 103      │ (Phòng máy điện)      │ CẤM VÀO!  │
│ 201      │ Chương trình Y        │ Chỉ đọc    │
│ 202      │ (Phòng trống)         │ CẤM VÀO!  │
└──────────┴───────────────────────┴────────────┘
```

Bây giờ, khi anh A muốn vào phòng 101, người quản lý mở sổ kiểm tra: "Phòng 101 — Anh A — Được vào." ✅

Khi anh A muốn vào phòng 102 (phòng chị B), người quản lý nói: "Không! Phòng này không phải của anh!" ❌

Khi bé E chạy đến phòng máy điện (103), người quản lý chặn lại ngay: "DỪNG! Phòng này CẤM VÀO!" 🛑

**MMU chính là người quản lý đó — nhưng nằm bên trong con chip CPU.**

| Sổ địa chỉ tòa nhà | MMU trong máy tính |
|---|---|
| Mỗi phòng có số | Mỗi ô nhớ có **địa chỉ** |
| Ghi ai được vào phòng nào | Ghi chương trình nào được dùng ô nhớ nào |
| Cấm vào phòng máy điện | Cấm truy cập vùng nhớ nguy hiểm |
| Kiểm tra mỗi khi có người muốn vào | Kiểm tra **mỗi lần** CPU đọc/ghi bộ nhớ |

Và "cuốn sổ" mà MMU dùng có tên chính thức: **Page Table** (Bảng Trang).

---

## 📖 Page Table — Cuốn sổ ghi nhớ mọi thứ

### Tại sao gọi là "Trang"?

Em biết cuốn sách được chia thành **trang** đúng không? Trang 1, trang 2, trang 3...

Bộ nhớ cũng được chia thành **trang** — mỗi trang có kích thước cố định. Trong AegisOS, chúng ta dùng trang **4 KB** (4096 byte) — tức mỗi "trang" chứa được khoảng 4000 chữ cái.

Tại sao phải chia thành trang? Vì nếu quản lý **từng ô nhớ một** (từng byte) thì cuốn sổ sẽ **dài hàng tỷ dòng** — chẳng ai tra nổi!

Giống như nếu quản lý tòa nhà ghi **từng viên gạch** thì sổ sách sẽ dày bằng cả tòa nhà luôn. Thay vào đó, người ta quản lý **từng phòng** — dễ hơn nhiều.

| Quản lý từng viên gạch | Quản lý từng phòng |
|---|---|
| Sổ dày hàng triệu trang | Sổ chỉ vài trăm dòng |
| Tra cứu rất lâu | Tra cứu nhanh |
| Không ai làm thế! | ← Cách MMU hoạt động |

### Page Table nhiều tầng — như tìm nhà bằng bản đồ

Nhưng bộ nhớ thì rộng mênh mông. Nếu chỉ dùng một cuốn sổ thì vẫn quá dài.

Giải pháp? **Chia sổ thành nhiều tầng** — giống như cách em tìm nhà bạn:

1. **Tầng 1** (L1 — Level 1): "Bạn ở **quận** nào?" → Quận 1. Mỗi ô trong sổ L1 chỉ đến **1 tỷ byte** (1 GiB).
2. **Tầng 2** (L2 — Level 2): "Bạn ở **phường** nào?" → Phường 5. Mỗi ô chỉ đến **2 triệu byte** (2 MiB).
3. **Tầng 3** (L3 — Level 3): "Bạn ở **nhà số** mấy?" → Nhà 42. Mỗi ô chỉ đến chính xác **một trang 4 KB**.

Hãy hình dung thế này:

```
🏙️ L1: "Thành phố có mấy quận?"
   ├── Quận 0 → 📁 L2_device  (thiết bị: UART, chip điều khiển...)
   └── Quận 1 → 📁 L2_ram     (bộ nhớ chính: code, dữ liệu...)

📁 L2_ram: "Quận 1 có mấy phường?"
   ├── Phường 0 → 📁 L3_kernel (chi tiết từng nhà!)
   ├── Phường 1 → 🏗️ Khối 2MiB (cả phường cùng quyền)
   ├── Phường 2 → 🏗️ Khối 2MiB
   └── ... (tổng cộng 64 phường = 128 MiB RAM)

📁 L3_kernel: "Phường 0 có mấy nhà?"
   ├── Nhà 0-127:   📦 (chưa dùng — CẤM VÀO)
   ├── Nhà 128-...: 📕 Code AegisOS (CHỈ ĐỌC + CHẠY ĐƯỢC)
   ├── Nhà ...:     📗 Dữ liệu cố định (CHỈ ĐỌC)
   ├── Nhà ...:     📘 Dữ liệu thay đổi (ĐỌC + GHI)
   └── Nhà ...:     🚫 Trang bảo vệ (CẤM HOÀN TOÀN!)
```

Cách này cực kỳ thông minh vì:

- **L1 chỉ có 2 dòng** → siêu nhỏ
- **L2 có 512 dòng** → vẫn nhỏ
- **L3 chỉ cần tạo cho vùng cần quản lý chi tiết** → tiết kiệm
- Tổng bộ nhớ dùng cho sổ: chỉ **16 KB** — bằng một bức ảnh nhỏ xíu trên điện thoại!

---

## 🔐 W^X — Luật "Hoặc Viết, Hoặc Chạy, Không Được Cả Hai!"

Bây giờ đến phần hay nhất.

Em biết không, hầu hết virus máy tính hoạt động thế nào không?

1. Virus tìm một vùng nhớ mà nó **được phép ghi** (viết dữ liệu vào)
2. Virus viết **mã độc** vào đó — giống như nhét một tờ giấy có ghi "chuyển hết tiền cho tôi" vào hồ sơ
3. Rồi virus **bảo CPU chạy** đoạn mã đó

Nếu một vùng nhớ vừa **ghi được** vừa **chạy được**, thì virus vào thoải mái. Giống như một căn phòng vừa là **nhà kho** (ai cũng bỏ đồ vào được) vừa là **phòng điều khiển** (mọi lệnh từ đây đều được thực hiện). Nguy hiểm cực!

AegisOS dùng một quy tắc thép: **W^X** — đọc là "Write XOR Execute".

**XOR** (Exclusive OR) nghĩa là "hoặc cái này, hoặc cái kia, KHÔNG ĐƯỢC cả hai":

| Vùng nhớ | Viết được? | Chạy được? | An toàn? |
|---|---|---|---|
| Code (`.text`) | ❌ Không | ✅ Có | ✅ Virus không ghi đè được code |
| Dữ liệu (`.data`, `.bss`, stack) | ✅ Có | ❌ Không | ✅ Dù virus ghi vào, CPU sẽ **từ chối chạy** |
| Dữ liệu cố định (`.rodata`) | ❌ Không | ❌ Không | ✅ Không ai đụng được, không ai chạy được |

Hãy nghĩ thế này:

- **Phòng đọc sách** (thư viện): em được **đọc** nhưng **không được viết bậy** lên sách. → Giống vùng code.
- **Bảng đen trong lớp**: em được **viết** lên, nhưng bảng đen **không thể tự chạy** lệnh nào. → Giống vùng dữ liệu.
- **Không bao giờ** có thứ vừa là thư viện vừa là bảng đen.

AegisOS bật một cờ đặc biệt trong CPU gọi là **WXN** (Write implies Execute Never). Khi cờ này bật, bất kỳ trang nào **ghi được** thì CPU tự động **cấm chạy** luôn. Không cần kiểm tra từng trang. Luật này áp dụng cho toàn bộ bộ nhớ. **Không có ngoại lệ.**

---

## 🚧 Trang Bảo Vệ — Chiếc Lan Can Chống Ngã

Em biết **Stack** (ngăn xếp) là gì không?

Stack giống **chồng đĩa** trên bàn ăn. Mỗi khi gọi một hàm (function), máy tính "đặt thêm một cái đĩa lên". Khi hàm xong, "lấy đĩa ra". Đĩa nào đặt sau thì lấy trước.

Nhưng chồng đĩa thì có giới hạn. Nếu em cứ chất đĩa lên mãi mà không lấy ra — **chồng đĩa sẽ đổ!**

Trong máy tính, chuyện này gọi là **Stack Overflow** (tràn ngăn xếp). Nó xảy ra khi chương trình gọi hàm quá nhiều lần (thường do lỗi lập trình). Khi stack tràn, nó **ghi đè** lên dữ liệu phía dưới — giống đĩa đổ xuống đập vỡ mọi thứ trên bàn.

AegisOS giải quyết bằng cách đặt một **Guard Page** (Trang Bảo Vệ) ngay bên dưới stack:

```
          ┌────────────────────┐  ← __stack_end (đỉnh stack)
          │   Stack 16 KB      │
          │   ↓ Mọc xuống ↓    │
          ├────────────────────┤  ← __stack_start
          │ 🚫 GUARD PAGE 🚫  │  ← __stack_guard (4KB — CẤM TRUY CẬP!)
          │   (vô hiệu hóa)    │
          ├────────────────────┤
          │   Page Tables...   │
          └────────────────────┘
```

Trang bảo vệ này được đánh dấu là **vô hiệu** (invalid) trong Page Table — tức là **không có quyền gì cả**. Không đọc, không ghi, không chạy.

Nếu stack tràn xuống trang này, CPU sẽ phát hiện ngay và báo **Data Abort** (Lỗi Truy Cập Dữ Liệu) — giống như chuông báo cháy vang lên.

Giống như lan can cầu thang — em không thể vô tình đi lọt qua mà rơi xuống. Lan can sẽ chặn em lại.

---

## ⚡ Exception — Chuông Báo Động

Khi có ai vi phạm — truy cập trang cấm, cố ghi vào vùng chỉ đọc, hay cố chạy code từ vùng dữ liệu — CPU sẽ kích hoạt một **Exception** (Ngoại lệ).

Exception giống **chuông báo cháy** trong trường học:

1. Chuông kêu → Mọi người **dừng** mọi việc ngay lập tức
2. Nhìn lên bảng → Biết **loại** báo động gì (cháy? động đất? diễn tập?)
3. Làm theo hướng dẫn → Sơ tán / Ẩn nấp / Chờ đợi

CPU cũng vậy:

1. Exception xảy ra → CPU **dừng** chương trình đang chạy
2. CPU nhảy đến **Exception Vector Table** (Bảng Hướng Dẫn Khẩn Cấp)
3. Handler (người xử lý) kiểm tra: lỗi gì? Ở đâu? Rồi quyết định xử lý

AegisOS có một bảng hướng dẫn dài 2048 byte, chia thành 16 ô, mỗi ô 128 byte — mỗi ô dành cho một loại tình huống khác nhau. Khi có Data Abort (ai đó truy cập trang cấm), CPU đọc 3 thông tin:

- **ESR_EL1**: "Chuyện gì xảy ra?" — mã lỗi
- **FAR_EL1**: "Ở địa chỉ nào?" — nơi xảy ra vi phạm
- **ELR_EL1**: "Đang chạy lệnh nào?" — dòng code gây lỗi

Rồi AegisOS in thông báo ra UART:

```
!!! EXCEPTION !!!
  Type: Data Abort (same EL)
  ESR_EL1: 0x96000047
  FAR_EL1: 0x40089000
  ELR_EL1: 0x40080124
  HALTED.
```

Và dừng lại. Trong hệ thống an toàn, **dừng lại đúng lúc** tốt hơn **tiếp tục chạy sai**. Giống như khi chuông báo cháy kêu, dừng lại sơ tán tốt hơn là tiếp tục ngồi học.

---

## 🔧 Chúng Ta Đã Làm Được Gì Trong AegisOS?

Hãy nhìn lại cấu trúc project bây giờ:

```
aegis/
├── src/
│   ├── boot.s          ← "Chuông báo thức" — giờ biết dọn nhà + bật MMU
│   ├── main.rs         ← "Bộ não" — ra lệnh và in trạng thái
│   ├── mmu.rs          ← ⭐ MỚI: "Sổ địa chỉ" — xây Page Table
│   └── exception.rs    ← ⭐ MỚI: "Chuông báo cháy" — bắt lỗi
├── linker.ld            ← "Bản đồ thành phố" — giờ có thêm ranh giới
├── aarch64-aegis.json
└── Cargo.toml
```

| File | Bài trước | Bài này |
|---|---|---|
| `boot.s` | Thức dậy, dọn BSS, gọi kernel | + Hạ từ EL2 xuống EL1, bật MMU + WXN |
| `main.rs` | In `[AegisOS] boot` | + In 4 checkpoint, cài Exception vectors |
| `mmu.rs` | *(chưa có)* | Xây 4 bảng Page Table, identity map, W^X |
| `exception.rs` | *(chưa có)* | Bảng vector 2048 byte, handler in lỗi qua UART |
| `linker.ld` | 3 section đơn giản | + Ranh giới mỗi section, Page Tables, Guard Page |

### EL2 → EL1: "Xuống cấp" để an toàn hơn

Phần này hơi khó, nhưng em cứ đọc chậm lại nhé.

CPU ARM có nhiều **Exception Level** (Cấp Độ Đặc Quyền) — giống như các tầng trong tòa nhà:

- **EL3** — Tầng hầm bí mật: Firmware, chỉ nhà sản xuất chip mới vào
- **EL2** — Tầng quản lý: Hypervisor, quản lý nhiều Hệ Điều Hành
- **EL1** — Tầng làm việc: Hệ Điều Hành chạy ở đây
- **EL0** — Tầng trệt: App bình thường

QEMU khởi động ở **EL2** (cấp cao). Nhưng AegisOS là Hệ Điều Hành, nên nó chỉ cần ở **EL1**.

Tại sao phải "xuống cấp"? Vì **càng ít quyền thì càng ít nguy hiểm**. Giống như:

- Giám đốc có chìa khóa **mọi phòng** → nếu bị hack, mất hết
- Nhân viên chỉ có chìa khóa **phòng mình** → nếu bị hack, chỉ mất một phòng

AegisOS tự nguyện "xuống cấp" từ EL2 xuống EL1, rồi bật MMU ở EL1. Từ đó, ngay cả chính nó cũng phải tuân theo luật của Page Table. **Không ai được đứng trên luật.**

### Quá trình khởi động bây giờ

```
🔌 Bật máy
   ↓
🏁 boot.s: Core 0 thức dậy, các core khác ngủ
   ↓
📏 Kiểm tra: đang ở EL2? → Hạ xuống EL1
   ↓
🧹 Xóa BSS + vùng Page Table (dọn dẹp sạch sẽ)
   ↓
📒 mmu_init(): Xây sổ địa chỉ (Page Table)
   │  ├── init_tables_2mib()     — vẽ bản đồ thô (quận, phường)
   │  ├── refine_kernel_pages()  — vẽ chi tiết (từng nhà)
   │  └── set_guard_page()       — đặt lan can
   ↓
⚙️ Ghi MAIR, TCR, TTBR0 vào CPU (nạp sổ địa chỉ)
   ↓
🟢 Bật MMU + WXN trong SCTLR_EL1
   ↓
🧠 kernel_main(): In trạng thái + cài chuông báo cháy
   ↓
💤 WFI — chờ lệnh tiếp theo
```

Và khi chạy, UART in ra:

```
[AegisOS] boot
[AegisOS] MMU enabled (identity map)
[AegisOS] W^X enforced (WXN + 4KB pages)
[AegisOS] memory isolation active
```

**Bốn dòng.** Mỗi dòng là một lời hứa:

1. "Tôi đã sống." 💓
2. "Tôi biết ai ở đâu." 🗺️
3. "Tôi không cho ai vừa viết vừa chạy." 🔐
4. "Tôi đã dựng chuông báo cháy." 🔔

---

## 🌍 Tại Sao Điều Này Quan Trọng Ngoài Đời Thật?

Mọi Hệ Điều Hành "nghiêm túc" đều dùng MMU + Page Table + W^X:

- **Máy bay Airbus A350**: Hệ thống ARINC 653 dùng MMU để cách ly từng phân vùng — nếu app giải trí (phim, nhạc) bị lỗi, nó **không ảnh hưởng** đến app lái máy bay.

- **Xe Tesla**: MMU đảm bảo camera AI và hệ thống phanh chạy trên **vùng nhớ riêng biệt** — dù AI nhìn sai, phanh khẩn cấp vẫn hoạt động.

- **Tàu vũ trụ NASA**: Curiosity rover trên Sao Hỏa dùng bộ nhớ được bảo vệ chặt chẽ — nếu tia vũ trụ gây lỗi bit (bit flip), hệ thống phát hiện và sửa ngay.

- **Máy y tế**: Máy bơm insulin tự động dùng MMU để đảm bảo đoạn code tính liều thuốc **không bao giờ** bị ghi đè bởi bất kỳ thứ gì khác.

AegisOS nhỏ xíu, nhưng nó dùng **đúng kỹ thuật** mà các hệ thống trên dùng. Không phải đồ chơi — mà là nền tảng thật.

---

## 🌟 Người Thật, Chuyện Thật

Em biết **Sophie Wilson** không?

Năm 1983, khi mới **26 tuổi**, bà ấy thiết kế tập lệnh cho bộ vi xử lý **ARM** — chính là loại CPU mà AegisOS đang chạy trên đó!

Hồi bà ấy bắt đầu, ARM chỉ là một dự án nhỏ trong công ty Acorn Computers. Không ai nghĩ nó sẽ thành công.

43 năm sau, **hơn 200 tỷ chip ARM** đã được sản xuất. ARM có mặt trong mọi điện thoại, mọi máy tính bảng, hầu hết thiết bị IoT, và ngày càng nhiều xe hơi, máy bay, vệ tinh.

200 tỷ. Đó là gấp **25 lần** số người trên Trái Đất.

Và tất cả bắt đầu từ một người trẻ dám nghĩ: "Tôi có thể thiết kế một bộ vi xử lý tốt hơn."

MMU — thứ mà chúng ta vừa cài đặt cho AegisOS — cũng là một phần trong thiết kế ARM. Mỗi lần CPU kiểm tra Page Table, nó đang sử dụng kiến trúc mà Sophie Wilson đặt nền móng.

---

## 🎯 Bước Tiếp Theo

AegisOS giờ đã biết **"nhớ"** — nó biết ai ở đâu, ai được làm gì, và kêu lên khi có ai vi phạm.

Nhưng nó vẫn chỉ làm **một việc** tại một thời điểm.

Bước tiếp theo, chúng ta sẽ dạy nó **"làm nhiều việc cùng lúc"** — giống như em vừa nghe nhạc vừa làm bài tập. Trong khoa học máy tính, đó gọi là **Scheduling** (Lập lịch) — giống thời khóa biểu ở trường: Toán tiết 1, Văn tiết 2, Thể dục tiết 3...

CPU chỉ có một "bộ não", nhưng nó chuyển đổi giữa các nhiệm vụ **nhanh đến mức** em tưởng nó đang làm tất cả cùng lúc.

Cách nó chuyển đổi? Gọi là **Context Switch** (Chuyển ngữ cảnh) — giống như chuyển từ làm Toán sang làm Văn: cất sách Toán, lấy sách Văn, mở đúng trang đang dở...

Nghe thú vị không? 🚀

---

> *"Bộ nhớ là nơi mà quá khứ gặp tương lai — dữ liệu cũ được đọc lại, và dữ liệu mới được ghi xuống. Bảo vệ bộ nhớ chính là bảo vệ cả quá khứ lẫn tương lai."*

---

*Nếu em đọc đến đây, em đã hiểu được MMU, Page Table, W^X, Guard Page, và Exception — những thứ mà nhiều sinh viên đại học năm 3 vẫn còn thấy khó. Em không bình thường đâu. Em phi thường đấy.* ✨
