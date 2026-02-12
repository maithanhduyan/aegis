# 📐 DO-333 — Các Phương Pháp Công Thức Toán Học (Formal Methods)

> *Tài liệu tham khảo nội bộ AegisOS — Tóm tắt tiêu chuẩn DO-333 bằng tiếng Việt.*
> *Nguồn gốc: RTCA DO-333 / EUROCAE ED-216, ban hành tháng 12/2011.*

---

## 1. Giới Thiệu

**DO-333** (tên đầy đủ: *Formal Methods Supplement to DO-178C and DO-278A* — Bổ sung Phương pháp Công Thức Toán Học cho DO-178C và DO-278A) là tài liệu hướng dẫn cách sử dụng các **phương pháp toán học** (formal methods) để phát triển và xác minh phần mềm hàng không.

Trước đây, trong DO-178B, Phương pháp Công Thức Toán Học chỉ được coi là "phương pháp thay thế". Với sự ra đời của DO-333, chúng trở thành một **phương pháp tuân thủ được công nhận** (recognized means of compliance), cho phép nhà phát triển sử dụng toán học để thay thế một phần hoặc toàn bộ quy trình kiểm thử truyền thống.

### Tại sao cần DO-333?

Kiểm thử truyền thống (Testing) chỉ có thể chứng minh sự hiện diện của lỗi, không thể chứng minh sự vắng mặt của lỗi. Với các hệ thống cực kỳ phức tạp hoặc yêu cầu độ tin cậy tuyệt đối (như hạt nhân hệ điều hành, hệ thống lái tự động), kiểm thử không bao giờ có thể bao phủ 100% các trạng thái.

Phương pháp Công Thức Toán Học sử dụng logic toán học để **chứng minh** tính đúng đắn của phần mềm, giống như chứng minh một định lý toán học, đảm bảo phần mềm hoạt động đúng trong **mọi trường hợp có thể**.

---

## 2. Các Kỹ Thuật Chính

DO-333 phân loại các Phương pháp Công Thức Toán Học thành 3 nhóm chính:

### 2.1. Kiểm Tra Mô Hình (Model Checking)

- **Nguyên lý:** Duyệt toàn bộ không gian trạng thái (exhaustive state-space exploration) của một mô hình hệ thống để kiểm tra xem một thuộc tính cụ thể có luôn đúng hay không.
- **Ứng dụng:** Kiểm tra các logic phức tạp, máy trạng thái (state machines), phát hiện deadlock (tắc nghẽn), race condition (tranh chấp).
- **Ưu điểm:** Tự động hóa cao. Nếu sai, công cụ sẽ chỉ ra ngay một "phản ví dụ" (counter-example) — chuỗi sự kiện dẫn đến lỗi.
- **Hạn chế:** Bùng nổ không gian trạng thái (state space explosion) với các hệ thống lớn.

### 2.2. Chứng Minh Định Lý (Theorem Proving)

- **Nguyên lý:** Sử dụng logic toán học (logic vị từ, logic bậc cao) để chứng minh rằng mã nguồn hoặc thiết kế tuân thủ các yêu cầu.
- **Ứng dụng:** Chứng minh tính đúng đắn của thuật toán, xác minh hạt nhân (như seL4), chứng minh các thuộc tính an toàn (security properties).
- **Ưu điểm:** Không giới hạn bởi không gian trạng thái; có thể chứng minh cho các hệ thống vô hạn.
- **Hạn chế:** Cần chuyên gia toán học trình độ cao; khó tự động hóa hoàn toàn (thường cần con người hỗ trợ công cụ).

### 2.3. Diễn Giải Trừu Tượng (Abstract Interpretation)

- **Nguyên lý:** Phân tích mã nguồn bằng cách ánh xạ các giá trị cụ thể sang các miền trừu tượng (ví dụ: thay vì tính `x = 5`, chỉ tính `x > 0`).
- **Ứng dụng:** Phân tích tĩnh (Static Analysis) để tìm lỗi runtime như: chia cho 0, tràn bộ nhớ (buffer overflow), tràn số (integer overflow), con trỏ null.
- **Ưu điểm:** Tự động hóa cao, có thể áp dụng trực tiếp lên mã nguồn lớn.
- **Hạn chế:** Có thể đưa ra cảnh báo giả (false positives) — báo lỗi ở chỗ thực tế không có lỗi.

---

## 3. Tác Động Đến Vòng Đời DO-178C

DO-333 không thay thế DO-178C mà **bổ sung** và **sửa đổi** các mục tiêu (objectives) khi áp dụng Phương pháp Công Thức Toán Học.

### Các Bảng Mục Tiêu (Objective Tables)

DO-333 định nghĩa các bảng `FM.A-x` tương ứng với các bảng `A-x` trong DO-178C:

| Bảng DO-333 | Tương ứng DO-178C | Nội dung chính |
|---|---|---|
| **FM.A-3** | Verification of Requirements | Sử dụng hình thức để chứng minh yêu cầu đầy đủ, nhất quán (thay vì review). |
| **FM.A-4** | Verification of Design | Chứng minh thiết kế thỏa mãn yêu cầu cấp cao (HLR). |
| **FM.A-5** | Verification of Coding | Chứng minh mã nguồn thỏa mãn thiết kế/yêu cầu (thay vì unit test). |
| **FM.A-6** | Verification of EOC | Chứng minh mã máy (Executable Object Code) tương đương mã nguồn. |
| **FM.A-7** | Verification of Verification Results | **Quan trọng:** Phân tích độ bao phủ của chứng minh hình thức (thay thế Structural Coverage). |

### Thay Thế Kiểm Thử (Credit for Testing)

Đây là giá trị lớn nhất của DO-333. Nếu bạn chứng minh được một đơn vị phần mềm (Unit) đúng đắn về mặt toán học:
1. **Không cần Unit Test:** Bạn có thể bỏ qua việc viết unit test cases cho đơn vị đó.
2. **Không cần Structural Coverage:** Bạn không cần đo MC/DC (cho Level A) nếu chứng minh hình thức đã bao phủ logic đó.

**Tuy nhiên:**
- Bạn vẫn phải thực hiện **Kiểm thử tích hợp phần cứng/phần mềm** để đảm bảo phần mềm chạy đúng trên chip thực tế (trừ khi bạn mô hình hóa được cả phần cứng chính xác tuyệt đối).
- Bạn phải chứng minh **bảo toàn thuộc tính** (Property Preservation): Những gì đúng trên mô hình/source code phải đảm bảo vẫn đúng trên mã máy (Executable Object Code).

---

## 4. Lợi Ích và Thách Thức

### Lợi Ích
1. **Độ an toàn tối đa:** Tìm ra các lỗi cực hiếm (corner cases) mà testing truyền thống thường bỏ sót (ví dụ: lỗi xảy ra sau 10 năm vận hành liên tục).
2. **Phát hiện lỗi sớm:** Có thể verify ngay từ khi mới có Requirements hoặc Design, không cần đợi viết code.
3. **Giảm chi phí Unit Test:** Viết test case cho MC/DC rất tốn kém; chứng minh toán học có thể hiệu quả hơn về lâu dài.

### Thách Thức
1. **Chi phí đầu vào:** Cần đội ngũ kỹ sư giỏi toán học và công cụ chuyên dụng đắt tiền.
2. **Khó áp dụng cho toàn bộ hệ thống:** Thường chỉ áp dụng cho các phần lõi quan trọng nhất (như Kernel, Scheduler) vì quá tốn kém để làm cho toàn bộ ứng dụng.
3. **Giả định (Assumptions):** Chứng minh chỉ đúng nếu các giả định (về compiler, hardware) là đúng.

---

## 5. Liên Hệ Với AegisOS

AegisOS, với mục tiêu là microkernel an toàn, là ứng cử viên hoàn hảo để áp dụng DO-333, tương tự như cách **seL4** đã làm (seL4 được chứng minh hình thức hoàn toàn).

| Hoạt động DO-333 | Áp dụng trong AegisOS |
|---|---|
| **Abstract Interpretation** | Sử dụng các công cụ Static Analysis (như Frama-C, Polyspace hoặc công cụ Rust/C hiện đại) để đảm bảo không có Runtime Errors (tràn stack, chia cho 0) trong Phase E. |
| **Model Checking** | Dùng cho **Scheduler**: Mô hình hóa máy trạng thái của scheduler để chứng minh không bao giờ xảy ra Deadlock hoặc Priority Inversion. |
| **Theorem Proving** | Mục tiêu dài hạn (Phase F+): Chứng minh tính đúng đắn của IPC (Inter-Process Communication) — đảm bảo message không bao giờ bị gửi sai địa chỉ hoặc bị mất. |
| **Quy trình thay thế** | Thay vì viết hàng nghìn unit test cho các hàm toán học/logic trong kernel, sử dụng chứng minh hình thức để đạt chuẩn DO-178C Level A mà không cần MC/DC coverage thủ công. |

### Bài học từ seL4 (The Gold Standard)
Microkernel **seL4** đã chứng minh được:
- **Functional Correctness:** Code C thực hiện chính xác những gì đặc tả yêu cầu.
- **Security:** Chứng minh được tính bảo mật, cô lập (isolation) là tuyệt đối.

AegisOS lấy cảm hứng từ đây: sử dụng kiến trúc đơn giản, cấp phát tĩnh (static allocation) để làm cho việc áp dụng DO-333 trở nên khả thi.

---

## 6. Thuật Ngữ Chính

| Tiếng Anh | Tiếng Việt | Giải thích |
|---|---|---|
| **Formal Methods** | Phương pháp Công Thức Toán Học | Kỹ thuật dựa trên toán học để đặc tả và xác minh hệ thống. |
| **Model Checking** | Kiểm tra mô hình | Duyệt toàn bộ trạng thái để tìm lỗi. |
| **Theorem Proving** | Chứng minh định lý | Dùng logic để suy diễn tính đúng đắn. |
| **Abstract Interpretation** | Diễn giải trừu tượng | Phân tích mã nguồn bằng các miền giá trị trừu tượng. |
| **Soundness** | Tính đúng đắn/vững chắc | Một phương pháp "sound" là phương pháp không bao giờ bỏ sót lỗi (nếu nó nói an toàn là an toàn tuyệt đối). |
| **Counter-example** | Phản ví dụ | Một kịch bản cụ thể (input + trạng thái) chứng minh hệ thống bị lỗi. |
| **Structural Coverage** | Phủ mã cấu trúc | (Trong DO-178C) Đo lường xem bao nhiêu code đã được chạy. DO-333 cho phép thay thế cái này bằng phân tích toán học. |

---

> *"Testing shows the presence, not the absence of bugs. Formal verification proves the absence of bugs."*
> — Edsger W. Dijkstra

---

*Tài liệu này được biên soạn cho mục đích tham khảo nội bộ dự án AegisOS. Để có thông tin đầy đủ và chính xác pháp lý, vui lòng tham khảo bản gốc DO-333 từ RTCA.*
