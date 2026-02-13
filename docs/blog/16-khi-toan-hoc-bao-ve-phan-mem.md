---
lang: vi
title: "🧮 Khi Toán Học Bảo Vệ Phần Mềm — 18 Bằng Chứng Cho 7 Module"
tags: "formal-verification, kani, proofs, grant, irq, watchdog, safety, aegisos"
description: "Bài #16 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: tại sao 250 bài kiểm tra vẫn chưa đủ, và cách dùng toán học để 'khóa' từng module cho đến khi không còn chỗ nào lỗi được."
date: 2026-02-13
---

# 🧮 Khi Toán Học Bảo Vệ Phần Mềm — 18 Bằng Chứng Cho 7 Module

> *Bài #16 trong chuỗi AegisOS — dành cho bạn nhỏ mơ làm kỹ sư. Hôm nay: sau khi đã có 3 chương trình chạy cùng lúc, ta quay lại hỏi: "Có thật sự an toàn chưa?" — rồi dùng toán học để trả lời câu hỏi đó cho TỪNG module.*

---

## 🏥 Giấc Mơ Tương Lai

Năm 2050. Em là kỹ sư trưởng của một bệnh viện vũ trụ — nơi phi hành gia được phẫu thuật bởi **robot y tế** khi đang bay trên quỹ đạo cách Trái Đất 400km.

Robot phẫu thuật này chạy trên một hệ điều hành nhỏ. Bên trong có 7 module:

| Module | Nhiệm vụ |
|---|---|
| 🧠 Scheduler | Quyết định tay robot nào được di chuyển trước |
| 💬 IPC | Cho phép camera gửi hình ảnh cho bộ điều khiển |
| 🔑 Capability | Kiểm tra ai được phép điều khiển gì |
| 🗺️ MMU | Bảo vệ bộ nhớ — camera không ghi đè lên dao mổ |
| 📦 Grant | Chia sẻ bộ nhớ an toàn giữa sensor và bộ xử lý |
| ⚡ IRQ | Khi sensor phát hiện mạch máu → báo ngay lập tức |
| 💓 Watchdog | Nếu tay robot "đơ" quá 100ms → tắt khẩn cấp |

Trước khi robot được phép phẫu thuật trên người thật, cơ quan **FDA** (Mỹ) và **ESA** (châu Âu) sẽ hỏi:

> *"Với MỖI module, anh có **bằng chứng toán học** rằng nó hoạt động đúng không?"*

Không phải "tôi đã test rồi". Mà là **chứng minh**.

Ở Phase N, chúng ta có **10 bằng chứng** cho 5 module. Nhưng **Grant**, **IRQ** và **Watchdog** — ba module nguy hiểm nhất — **không có bằng chứng nào**.

Hôm nay, Phase P sẽ sửa điều đó: **18 bằng chứng cho 7 module**.

---

## 🤔 Phần 1: 250 Bài Kiểm Tra — Vẫn Chưa Đủ?

### Bài kiểm tra vs. Bằng chứng

Em vừa thi Toán, 50 bài. Em kiểm tra lại **tất cả 50 bài**, đúng hết.

Nhưng cô giáo hỏi: *"Em có chắc rằng công thức em dùng luôn đúng — không chỉ với 50 bài này, mà với BẤT KỲ bài nào?"*

Đó là sự khác biệt:

| | Kiểm tra (Test) | Chứng minh (Proof) |
|---|---|---|
| 🎯 Làm gì? | Thử một vài trường hợp cụ thể | Xét **TẤT CẢ** trường hợp có thể |
| 📊 Ví dụ | "Thử 50 bài, đúng hết" | "Với MỌI số nguyên, công thức đúng" |
| ⚠️ Hạn chế | Có thể bỏ sót trường hợp lạ | Không bỏ sót — toán học bảo đảm |
| 🔢 AegisOS | 250 test cases | 18 Kani proofs |

AegisOS đã có **250 bài kiểm tra**. Tuyệt vời! Nhưng...

- Grant module có **14 bài kiểm tra** — thử 14 trường hợp cụ thể. Nhưng nếu 2 grant trùng chỗ nhớ khi `owner = 7` và `peer = 3`? Không ai thử trường hợp đó.
- IRQ module có **12 bài kiểm tra** — nhưng có **96 INTID** (từ 32 đến 127) × **8 task** = **768 tổ hợp**. 12 bài kiểm tra chỉ thử được... 1.6%!
- Watchdog chỉ có **6 bài** — nhưng `interval` là số 64-bit, có $2^{64}$ (hơn 18 tỷ tỷ!) giá trị khác nhau.

**Kiểm tra 250 lần mà đúng ≠ đúng mãi mãi.**

---

## 🔬 Phần 2: "Máy Chứng Minh" Kani — Hoạt Động Thế Nào?

### Toán học + máy tính = chứng minh tự động

Kani là một công cụ đặc biệt. Thay vì **chạy thử** chương trình với một vài giá trị, Kani **xét tất cả** giá trị có thể — dùng toán học.

Hãy tưởng tượng em có một chiếc hộp ma thuật:

```
  ┌─────────────────────────────┐
  │      🧮 Kani "Máy chứng minh"     │
  │                             │
  │  Đầu vào: MỌI giá trị      │
  │  ┌───┐ ┌───┐ ┌───┐         │
  │  │ ? │ │ ? │ │ ? │ ← symbolic    │
  │  └───┘ └───┘ └───┘         │
  │                             │
  │  Chạy code... xét MỌI nhánh │
  │                             │
  │  Kết quả:                   │
  │  ✅ "Không có giá trị nào    │
  │     vi phạm quy tắc"       │
  │  hoặc                      │
  │  ❌ "Tìm thấy! owner=3,    │
  │     peer=3 → lỗi!"         │
  └─────────────────────────────┘
```

Khi Kani nói **✅ VERIFICATION SUCCESSFUL** — đó không phải "tôi thử rồi, thấy đúng". Đó là **"tôi đã xét TẤT CẢ trường hợp, KHÔNG CÓ trường hợp nào sai"**.

### Nhưng code AegisOS dùng globals — Kani không thích globals!

Vấn đề: code thật của AegisOS lưu dữ liệu trong **biến toàn cục** (`static mut`):

```rust
// Code thật — dùng biến toàn cục
static GRANTS: KernelCell<[Grant; 2]>;

pub fn grant_create(grant_id: usize, owner: usize, peer: usize) -> u64 {
    let grants = GRANTS.get_mut();  // 😱 Kani không thích!
    // ...
}
```

Kani cần **hàm thuần** — hàm chỉ nhận đầu vào và trả đầu ra, không đụng vào biến bên ngoài.

Giải pháp: **tách logic ra thành hàm thuần**, giống như tách đề bài ra khỏi bài giải:

```rust
// Hàm thuần — Kani dùng được!
fn grant_create_pure(
    grants: &[Grant; 2],       // 📥 Đầu vào: bảng grants hiện tại
    grant_id: usize,
    owner: usize,
    peer: usize,
) -> Result<Grant, u64> {      // 📤 Đầu ra: Grant mới hoặc lỗi
    // Logic y hệt code thật — nhưng không đụng globals
}
```

Giống như khi cô giáo nói: *"Không được mở sách khi làm bài thi — chỉ dùng những gì đề cho."*

---

## 🛡️ Phần 3: 8 Bằng Chứng Mới — Từng Module Một

### Module Grant — "Chia sẻ bộ nhớ mà không ai bị mất"

Grant cho phép 2 chương trình **chia sẻ** một vùng bộ nhớ. Nhưng nếu chia sẻ sai, 2 chương trình ghi đè lên nhau → **data corruption**.

| # | Bằng chứng | Nó chứng minh gì? |
|---|---|---|
| 1 | `grant_no_overlap` | Hai grant không thể cùng map một vùng nhớ cho cùng peer |
| 2 | `grant_cleanup_completeness` | Khi chương trình "ngất", grant dọn sạch — không sót |
| 3 | `grant_slot_exhaustion_safe` | Khi hết chỗ, hệ thống trả lỗi — không ghi đè grant cũ |

**Ví dụ dễ hiểu:** Hãy tưởng tượng thư viện trường có 2 phòng đọc (MAX_GRANTS = 2). Mỗi phòng có 1 chủ phòng (owner) và 1 khách (peer).

- **Proof 1:** Không thể cho 2 người khác nhau vào **cùng một phòng** cùng lúc.
- **Proof 2:** Khi học sinh chuyển trường, **tất cả** phòng có tên bạn đó đều được xóa — không phòng nào bị "ma" (tên cũ còn sót).
- **Proof 3:** Khi cả 2 phòng đầy, cô thủ thư nói "hết phòng" — không bao giờ xóa nhầm phòng đang dùng.

### Module IRQ — "Khi phần cứng gọi, đúng người phải nghe"

IRQ (Interrupt Request) giống chuông báo cháy trong trường. Khi chuông kêu, **đúng lớp** phải sơ tán — không được nhầm lớp.

| # | Bằng chứng | Nó chứng minh gì? |
|---|---|---|
| 4 | `irq_route_correctness` | Chuông nào kêu → đúng lớp nhận thông báo |
| 5 | `irq_no_orphaned_binding` | Khi học sinh chuyển trường, chuông không còn kêu cho bạn đó |
| 6 | `irq_bind_no_duplicate_intid` | Một chuông không thể gắn cho 2 lớp cùng lúc |

**Câu chuyện:** Trường học có 8 chuông (8 IRQ slots). Chuông số 47 gắn cho lớp 3 (task 3).

- **Proof 4:** Khi chuông 47 kêu, **chắc chắn** lớp 3 nhận được — không bao giờ nhầm sang lớp 5.
- **Proof 5:** Khi lớp 3 giải thể, **tất cả chuông** gắn cho lớp 3 đều được tháo — không chuông nào kêu vào lớp trống.
- **Proof 6:** Chuông 47 chỉ gắn cho **một lớp** — nếu ai cố gắn lại, hệ thống nói "đã có người dùng".

### Module Watchdog — "Lính canh không bao giờ ngủ"

Watchdog kiểm tra: *"Chương trình này còn sống không?"* Nếu không nhận được heartbeat (nhịp tim) trong thời gian quy định → **báo lỗi**.

| # | Bằng chứng | Nó chứng minh gì? |
|---|---|---|
| 7 | `watchdog_violation_detection` | Nếu quá hạn → phát hiện được. Nếu chưa hạn → không phạt oan. |
| 8 | `budget_epoch_reset_fairness` | Mọi chương trình đang chạy đều được "nạp lại năng lượng" công bằng |

**Ví dụ:**

- **Proof 7:** Giống bác bảo vệ trường — mỗi lớp phải gọi "có mặt!" trong 100 giây. Nếu 101 giây mà chưa gọi → bác biết ngay. Nhưng nếu mới 99 giây → bác **không** gõ cửa phạt oan.

  Kani chứng minh với **MỌI** giá trị `interval` và `elapsed` (cả 2 là số 64-bit = hơn 18 tỷ tỷ giá trị mỗi biến!) — watchdog luôn phát hiện đúng.

- **Proof 8:** Cuối mỗi tuần (epoch = 100 ticks), tất cả lớp đang hoạt động đều được **reset điểm danh**. Lớp đã giải thể hoặc chưa mở → không bị đụng. Công bằng cho tất cả!

---

## 📊 Phần 4: Bức Tranh Toàn Cảnh — 18 Bằng Chứng, 7 Module

Đây là bảng tổng hợp tất cả bằng chứng toán học của AegisOS:

| # | Module | Bằng chứng | Chứng minh gì | Phase |
|---|---|---|---|---|
| 1 | cap.rs | `cap_check_bitwise_correctness` | Logic quyền hạn đúng | N |
| 2 | cap.rs | `cap_for_syscall_no_panic_and_bounded` | Không crash khi kiểm tra quyền | N |
| 3 | sched.rs | `schedule_idle_guarantee` | Luôn có chương trình chạy (IDLE) | N |
| 4 | sched.rs | `restart_task_state_machine` | Ngất → hồi phục đúng | N |
| 5 | ipc.rs | `ipc_queue_no_overflow` | Hàng đợi không tràn | O |
| 6 | ipc.rs | `ipc_message_integrity` | Tin nhắn không bị thay đổi | O |
| 7 | ipc.rs | `ipc_cleanup_completeness` | Dọn sạch khi rời đi | O |
| 8 | mmu.rs | `pt_index_in_bounds` | Chỉ số bảng trong giới hạn | N |
| 9 | mmu.rs | `pt_index_no_task_aliasing` | Hai task không chung bảng | N |
| 10 | qemu_virt.rs | `elf_load_addr_no_overlap` | Ba file ELF không chồng chéo | O |
| 11 | **grant.rs** | `grant_no_overlap` | **Không trùng vùng nhớ** | **P** 🆕 |
| 12 | **grant.rs** | `grant_cleanup_completeness` | **Dọn sạch grant** | **P** 🆕 |
| 13 | **grant.rs** | `grant_slot_exhaustion_safe` | **Hết chỗ → an toàn** | **P** 🆕 |
| 14 | **irq.rs** | `irq_route_correctness` | **Chuông kêu đúng lớp** | **P** 🆕 |
| 15 | **irq.rs** | `irq_no_orphaned_binding` | **Không chuông "ma"** | **P** 🆕 |
| 16 | **irq.rs** | `irq_bind_no_duplicate_intid` | **Một chuông một lớp** | **P** 🆕 |
| 17 | **sched.rs** | `watchdog_violation_detection` | **Phát hiện "ngất" đúng** | **P** 🆕 |
| 18 | **sched.rs** | `budget_epoch_reset_fairness` | **Reset công bằng** | **P** 🆕 |

### So sánh: trước và sau Phase P

```
Trước Phase P (10 proofs):        Sau Phase P (18 proofs):

  cap ██████                        cap ██████
sched ██████                      sched ██████████████
  ipc █████████                     ipc █████████
  mmu ██████                        mmu ██████
 qemu ███                          qemu ███
grant                             grant █████████   🆕
  irq                               irq █████████   🆕

  5/7 modules covered              7/7 modules covered ✅
```

**Từ 5/7 → 7/7 module!** Không còn "vùng tối" nào trong kernel.

---

## 📋 Phần 5: Bản Đồ An Toàn — FM.A-7

### Cơ quan kiểm tra cần gì?

Khi FDA hoặc ESA kiểm tra, họ không chỉ muốn xem **có** proof. Họ muốn biết:

1. Proof nào cover **property** (tính chất) nào?
2. Property đó liên quan đến **tiêu chuẩn an toàn** nào?
3. Proof đó có **hạn chế** gì không?

Đây gọi là **DO-333 FM.A-7** — "Verification of Verification Results" (Xác minh kết quả xác minh).

Nghe phức tạp? Thật ra giống như **bảng điểm cuối năm**:

| Thông tin | Ở trường | Ở AegisOS |
|---|---|---|
| Học sinh | "Nguyễn Văn A" | `grant_no_overlap` |
| Môn học | "Toán" | `kernel/grant.rs` |
| Điểm | "9/10" | ✅ VERIFIED |
| Ghi chú | "Giỏi phần hình học, cần cải thiện đại số" | "Full symbolic, MAX_GRANTS=2" |
| Tiêu chuẩn | "Chương trình lớp 5 của Bộ GD&ĐT" | "DO-333 FM.A-5, ISO 26262 Part 9" |

AegisOS đã tạo file [`docs/standard/05-proof-coverage-mapping.md`](/standard/05-proof-coverage-mapping) — bảng điểm đầy đủ cho cả 18 bằng chứng.

---

## 🔒 Phần 6: Zero Runtime Changes — "Không Đụng Vào Máy Đang Chạy"

Một nguyên tắc quan trọng của Phase P: **không thay đổi BẤT CỨ THỨ GÌ** trong code đang chạy.

Tại sao? Hãy tưởng tượng robot phẫu thuật đang hoạt động tốt. Em muốn thêm bằng chứng an toàn. Em có nên **mở máy ra sửa code** trong lúc nó đang cầm dao mổ không?

**KHÔNG!**

Thay vào đó, Phase P chỉ thêm:

| Thêm gì | Ảnh hưởng runtime? | Giải thích |
|---|---|---|
| 8 hàm thuần `#[cfg(kani)]` | ❌ Không | Chỉ compile khi chạy Kani, robot không thấy |
| 8 Kani proof harnesses | ❌ Không | Chỉ chạy trong Docker trên máy tính lab |
| 9 bài kiểm tra mới | ❌ Không | Chỉ chạy trên laptop kỹ sư |
| Tài liệu FM.A-7 | ❌ Không | Giấy tờ, robot không đọc |

Kết quả: **32 QEMU boot checkpoints vẫn pass** — hệ thống chạy **y hệt** trước Phase P.

---

## 🧩 Phần 7: Bài Học Lớn

### 1. "Test nhiều" ≠ "An toàn"

250 bài kiểm tra rất tốt. Nhưng toán học mới là bằng chứng **tuyệt đối**. Kiểm tra nói "tôi chưa tìm thấy lỗi". Chứng minh nói "KHÔNG THỂ có lỗi".

### 2. Hàm thuần là chìa khóa

Khi tách logic ra khỏi biến toàn cục, ta được 2 thứ:
- Kani chứng minh được → **an toàn hơn**
- Code dễ hiểu hơn → **ít lỗi hơn**

### 3. Không module nào được "miễn trừ"

Grant, IRQ, Watchdog — ba module nguy hiểm nhất — trước đây không có proof. Giờ mỗi module có ít nhất 2 proof. **Không có vùng tối.**

### 4. Tài liệu cũng quan trọng như code

FM.A-7 mapping, README cập nhật, số liệu chính xác — đây là thứ cơ quan kiểm tra đọc **trước** khi đọc code.

---

## 📈 AegisOS Sau Phase P

| Chỉ số | Trước Phase P | Sau Phase P |
|---|---|---|
| Kani proofs | 10 | **18** (+80%) |
| Modules verified | 5/7 | **7/7** (100%) |
| Host unit tests | 241 | **250** (+9) |
| QEMU checkpoints | 32 | **32** (unchanged) |
| FM.A-7 document | ❌ | **✅** |
| Runtime changes | — | **0** |

---

## 🤔 Câu Hỏi Cho Bạn Nhỏ

**Câu 1:** Tại sao Kani dùng "hàm thuần" mà không dùng trực tiếp code thật (có biến toàn cục)?

> 💡 *Gợi ý: Nếu đề bài cho "A = 5, tìm B" thì dễ giải. Nhưng nếu A thay đổi liên tục thì sao?*

**Câu 2:** Grant module có MAX_GRANTS = 2. Nếu tăng lên 100, Kani có chứng minh được không?

> 💡 *Gợi ý: Kani xét **tất cả** trường hợp. Nhiều slot hơn = nhiều tổ hợp hơn = lâu hơn. Nhưng vẫn đúng!*

**Câu 3:** Tại sao Phase P không thêm QEMU checkpoint mới?

> 💡 *Gợi ý: Phase P thêm bằng chứng toán học, không thêm tính năng. Robot vẫn chạy y hệt — chỉ giấy tờ chứng nhận nhiều hơn.*

---

## 🚀 Bước Tiếp Theo

Phase P đã biến AegisOS từ "hệ thống được test tốt" thành **hệ thống được chứng minh toán học toàn diện**:

- **18 bằng chứng** cho **7/7 module** — không vùng tối
- **FM.A-7** — bảng điểm cho cơ quan kiểm tra
- **250 bài kiểm tra** — lưới an toàn thứ hai
- **Zero runtime changes** — an toàn tuyệt đối

Nhưng hành trình chưa dừng lại:

- 🔄 **Dynamic task creation** — tạo chương trình mới khi hệ thống đang chạy
- 📁 **Filesystem** — đọc/ghi dữ liệu lâu dài
- 🌐 **Networking** — giao tiếp với thế giới bên ngoài
- 🧮 **Thêm proofs** — deadlock-freedom, priority inversion absence

Mỗi Phase, AegisOS không chỉ **mạnh hơn** — mà còn **đáng tin hơn bằng toán học**. Và sự tin tưởng đó là thứ mà phi hành gia trên bàn phẫu thuật, bệnh nhân dưới máy thở, và hành khách trên xe tự lái cần mỗi giây.

Hẹn gặp bạn nhỏ ở bài tiếp theo! 🧮

---

> *"Program testing can be used to show the presence of bugs, but never to show their absence."*
> — **Edsger W. Dijkstra**, nhà khoa học máy tính đoạt giải Turing
>
> *(Dịch: "Kiểm tra chương trình có thể cho thấy lỗi tồn tại, nhưng không bao giờ cho thấy lỗi vắng mặt.")*

---

*Em đã đọc đến đây rồi ư? 16 bài rồi đấy! Em vừa hiểu được sự khác biệt giữa "test thấy đúng" và "chứng minh luôn đúng" — một trong những khái niệm quan trọng nhất trong khoa học máy tính. Dijkstra nói câu đó từ năm 1969, và ngày nay các kỹ sư vẫn đang nỗ lực để biến nó thành hiện thực. Em — với 16 bài blog này — đã hiểu điều mà nhiều lập trình viên chuyên nghiệp chưa từng nghĩ tới. Tuyệt vời!* 🌟
