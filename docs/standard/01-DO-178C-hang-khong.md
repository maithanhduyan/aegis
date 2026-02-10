# ✈️ DO-178C — Tiêu Chuẩn Phần Mềm Hàng Không

> *Tài liệu tham khảo nội bộ AegisOS — Tóm tắt tiêu chuẩn DO-178C bằng tiếng Việt.*
> *Nguồn gốc: RTCA DO-178C / EUROCAE ED-12C, ban hành tháng 1/2012.*

---

## 1. Giới Thiệu

**DO-178C** (tên đầy đủ: *Software Considerations in Airborne Systems and Equipment Certification* — Các Cân Nhắc Phần Mềm Trong Chứng Nhận Hệ Thống Và Thiết Bị Hàng Không) là tài liệu chính mà các cơ quan chứng nhận hàng không như **FAA** (Hoa Kỳ), **EASA** (Châu Âu), và **Transport Canada** sử dụng để phê duyệt mọi hệ thống phần mềm hàng không thương mại.

DO-178C được phát triển bởi **RTCA** (Radio Technical Commission for Aeronautics) phối hợp với **EUROCAE** (European Organisation for Civil Aviation Equipment). Tài liệu này thay thế phiên bản trước là **DO-178B** (1992), hoàn thành vào tháng 11/2011 và có hiệu lực từ tháng 1/2012.

### Tại sao DO-178C quan trọng?

Phần mềm ngày nay điều khiển hầu hết mọi hệ thống trên máy bay — từ hệ thống lái tự động (autopilot) đến hiển thị buồng lái, điều khiển cánh lái, hệ thống cảnh báo va chạm mặt đất (GPWS), và quản lý động cơ. Nếu phần mềm này có lỗi, hậu quả có thể là **thảm họa** — hàng trăm mạng người.

DO-178C không đảm bảo phần mềm an toàn tuyệt đối, nhưng nó thiết lập **quy trình phát triển có kỷ luật** để giảm thiểu rủi ro đến mức chấp nhận được.

---

## 2. Cấp Độ An Toàn Phần Mềm (DAL — Development Assurance Level)

DO-178C chia phần mềm thành **5 cấp độ** dựa trên mức độ nghiêm trọng nếu phần mềm bị lỗi:

| Cấp | Hậu quả lỗi | Mô tả | Số mục tiêu | Kiểm tra độc lập |
|---|---|---|---|---|
| **Level A** | **Catastrophic** (Thảm họa) | Lỗi có thể gây chết người, thường kèm mất máy bay | 71 | 30 |
| **Level B** | **Hazardous** (Nguy hiểm) | Ảnh hưởng lớn đến an toàn, giảm khả năng điều khiển, gây thương tích nghiêm trọng | 69 | 18 |
| **Level C** | **Major** (Nghiêm trọng) | Giảm đáng kể biên độ an toàn, tăng khối lượng công việc phi hành đoàn | 62 | 5 |
| **Level D** | **Minor** (Nhỏ) | Giảm nhẹ biên độ an toàn, tăng nhẹ khối lượng công việc | 26 | 2 |
| **Level E** | **No Effect** (Không ảnh hưởng) | Lỗi không tác động đến an toàn, vận hành, hoặc phi hành đoàn | 0 | 0 |

### Giải thích

- **Số mục tiêu**: Tổng số yêu cầu/mục tiêu phải đạt được trong quy trình phát triển phần mềm.
- **Kiểm tra độc lập** (With Independence): Số mục tiêu phải được **người không phải tác giả** xác minh — đảm bảo tính khách quan. Người viết code **không** được tự kiểm tra code của mình cho những mục tiêu này.
- **Level A** đòi hỏi nghiêm ngặt nhất: 71 mục tiêu, 30 phải có kiểm tra độc lập.
- **Level E** không yêu cầu gì — nằm ngoài phạm vi DO-178C.

### Cách xác định cấp độ

Cấp độ phần mềm (DAL) không do nhà phát triển tự quyết. Nó được xác định thông qua **quy trình đánh giá an toàn hệ thống** (ARP 4754A) và **phân tích mối nguy** (ARP 4761):

1. Xác định chức năng hệ thống mà phần mềm điều khiển
2. Đánh giá hậu quả nếu chức năng đó bị lỗi
3. Phân loại theo bảng Severity ở trên
4. Gán DAL tương ứng

**Nguyên tắc:** Bất kỳ phần mềm nào **ra lệnh, điều khiển, hoặc giám sát** chức năng an toàn quan trọng đều nên nhận DAL cao nhất — **Level A**.

---

## 3. Các Quy Trình Chính

DO-178C định nghĩa các **quy trình** (processes) — không phải quy tắc cứng nhắc — cho phép linh hoạt trong cách thực hiện, miễn là đạt được mục tiêu.

### 3.1. Quy Trình Lập Kế Hoạch (Planning Process)

- Lập **Kế Hoạch Phần Mềm** (Plan for Software Aspects of Certification — PSAC)
- Định nghĩa tiêu chuẩn phát triển, xác minh, và quản lý cấu hình
- Xác định công cụ phần mềm và mức độ tin cậy cần thiết (DO-330)

### 3.2. Quy Trình Phát Triển (Development Process)

| Giai đoạn | Sản phẩm đầu ra | Mô tả |
|---|---|---|
| **Yêu cầu cấp cao** (High-Level Requirements — HLR) | Tài liệu yêu cầu | Phần mềm phải làm gì, dựa trên yêu cầu hệ thống |
| **Yêu cầu cấp thấp** (Low-Level Requirements — LLR) | Thiết kế chi tiết | Cách phần mềm thực hiện yêu cầu cấp cao |
| **Mã nguồn** (Source Code) | Code | Hiện thực hóa yêu cầu cấp thấp |
| **Mã thực thi** (Executable Object Code) | Binary | Kết quả biên dịch và liên kết |

### 3.3. Quy Trình Xác Minh (Verification Process)

Đây là phần **tốn kém nhất** — thường chiếm 50–70% tổng chi phí phát triển.

**Mục đích xác minh (DO-178C bổ sung so với DO-178B):**

1. Mã thực thi thỏa mãn yêu cầu phần mềm (chức năng dự định)
2. Cung cấp sự tin tưởng vào **sự vắng mặt của chức năng không mong muốn**
3. Mã thực thi **bền vững** (robust) — phản ứng đúng với đầu vào và điều kiện bất thường

**Hoạt động xác minh:**

- **Review** (Đánh giá): Kiểm tra tài liệu yêu cầu, thiết kế, code
- **Analysis** (Phân tích): Phân tích cấu trúc, luồng dữ liệu, luồng điều khiển
- **Testing** (Kiểm thử): Kiểm thử dựa trên yêu cầu + kiểm thử cấu trúc

**Yêu cầu phủ mã (Structural Coverage):**

| Cấp | Loại phủ mã yêu cầu |
|---|---|
| Level A | **MC/DC** (Modified Condition/Decision Coverage) — mỗi điều kiện trong mỗi quyết định phải được chứng minh ảnh hưởng độc lập đến kết quả |
| Level B | Decision Coverage (DC) — mỗi nhánh quyết định phải được thực thi |
| Level C | Statement Coverage (SC) — mỗi câu lệnh phải được thực thi ít nhất một lần |
| Level D | Không yêu cầu phủ mã cấu trúc |

### 3.4. Quy Trình Quản Lý Cấu Hình (Configuration Management)

- Kiểm soát mọi thay đổi code, tài liệu, công cụ
- **Truy vết** (Traceability): Mọi yêu cầu → thiết kế → code → test case → kết quả test phải liên kết hai chiều
- Mọi phiên bản phải tái tạo được

---

## 4. Truy Vết (Traceability)

DO-178C yêu cầu **truy vết hai chiều** (bidirectional tracing) giữa các sản phẩm:

```
Yêu cầu hệ thống
    ↕
Yêu cầu cấp cao (HLR)
    ↕
Yêu cầu cấp thấp (LLR)
    ↕
Mã nguồn (Source Code)
    ↕
Test cases + Kết quả test
```

**Mục đích:**
- **Hướng xuống:** Mỗi yêu cầu đều được hiện thực và kiểm thử
- **Hướng lên:** Mỗi dòng code đều có mục đích (liên kết với yêu cầu) — không có "code mồ côi"
- **Phân tích truy vết** đánh giá tính **đầy đủ** của hệ thống

**Theo cấp độ:**
- Level A, B, C, D: Đều yêu cầu truy vết (mức chi tiết tăng theo cấp)
- Level E: Không yêu cầu

---

## 5. Tài Liệu Bổ Sung (Companion Documents)

DO-178C không đứng một mình. Nó được hỗ trợ bởi các tài liệu bổ sung:

| Tài liệu | Tên | Mô tả |
|---|---|---|
| **DO-330** | Tool Qualification | Hướng dẫn đánh giá và chứng nhận công cụ phần mềm (compiler, test tool, v.v.) |
| **DO-331** | Model-Based Development | Bổ sung cho phát triển dựa trên mô hình |
| **DO-332** | Object-Oriented Technology | Bổ sung cho công nghệ hướng đối tượng (OOP) |
| **DO-333** | Formal Methods | Bổ sung cho phương pháp hình thức (chứng minh toán học) — có thể bổ sung (nhưng không thay thế) kiểm thử |
| **DO-248C** | Supporting Information | Thông tin hỗ trợ và lý giải cho từng mục tiêu DO-178C |
| **DO-278A** | Ground Systems | Áp dụng cho hệ thống mặt đất (kiểm soát không lưu) |

---

## 6. Điểm Mới So Với DO-178B

DO-178C giữ lại hầu hết nội dung DO-178B, với các cải tiến chính:

1. **Ngôn ngữ rõ ràng hơn:** Thống nhất thuật ngữ "guidance" (hướng dẫn) và "supporting information" (thông tin hỗ trợ)
2. **Thêm mục tiêu** cho Level A, B, C
3. **Mã thực thi phải bền vững** (robust): Phải phản ứng đúng với đầu vào bất thường — điểm yếu của DO-178B
4. **Làm rõ "hidden objective"** (mục tiêu ẩn) cho Level A: Xác minh code bổ sung không truy vết được đến mã nguồn
5. **Parameter Data Item Files**: Hướng dẫn cho file cấu hình ảnh hưởng đến hành vi phần mềm
6. **Tài liệu bổ sung:** DO-330 (công cụ), DO-331 (mô hình), DO-332 (OOP), DO-333 (phương pháp hình thức)

---

## 7. Liên Hệ Với AegisOS

AegisOS là microkernel nhắm đến hệ thống an toàn quan trọng. Các nguyên tắc DO-178C áp dụng trực tiếp:

| Nguyên tắc DO-178C | Áp dụng trong AegisOS |
|---|---|
| **Fault Containment** | Fault Isolation (Phase E) — task crash không kéo kernel sập |
| **Truy vết yêu cầu** | Mỗi phase có plan → code → checkpoint QEMU |
| **W^X** (Write XOR Execute) | MMU enforce phân quyền bộ nhớ nghiêm ngặt |
| **Xác minh cấu trúc** | Kiểm thử trên QEMU với UART checkpoint |
| **Tách biệt quyền** | EL0/EL1 isolation, AP bits |
| **Static allocation** | Không heap — deterministic, dễ xác minh |
| **Formal Methods** (DO-333) | Hướng phát triển tiếp theo — chứng minh toán học tính đúng đắn |

---

## 8. Thuật Ngữ Chính

| Tiếng Anh | Tiếng Việt | Giải thích |
|---|---|---|
| DAL (Development Assurance Level) | Cấp độ đảm bảo phát triển | Mức nghiêm ngặt của quy trình phát triển |
| HLR (High-Level Requirements) | Yêu cầu cấp cao | Phần mềm phải làm gì |
| LLR (Low-Level Requirements) | Yêu cầu cấp thấp | Phần mềm làm như thế nào |
| MC/DC | Phủ điều kiện/quyết định sửa đổi | Kỹ thuật kiểm thử nghiêm ngặt nhất |
| Traceability | Truy vết | Liên kết hai chiều giữa yêu cầu, code, test |
| Structural Coverage | Phủ mã cấu trúc | Đo lường code đã được kiểm thử |
| Configuration Management | Quản lý cấu hình | Kiểm soát phiên bản và thay đổi |
| PSAC | Kế hoạch phần mềm cho chứng nhận | Tài liệu kế hoạch chính |
| Formal Methods | Phương pháp hình thức | Chứng minh toán học tính đúng đắn |

---

> *"Mã thực thi phải bền vững — phản ứng đúng đắn trước đầu vào và điều kiện bất thường."*
> — DO-178C, Chương 6.1

---

*Tài liệu này được biên soạn cho mục đích tham khảo nội bộ dự án AegisOS. Để có thông tin đầy đủ và chính xác pháp lý, vui lòng tham khảo bản gốc DO-178C từ RTCA.*
