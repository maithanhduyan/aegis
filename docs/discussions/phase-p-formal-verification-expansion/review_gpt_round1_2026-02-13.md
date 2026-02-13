# 🔭 GPT-Visionary — Review Round 1 | Phase P

## Tổng quan nhận định

Phase P đánh dấu một bước chuyển **chiến lược cực kỳ quan trọng** trong vòng đời AegisOS: từ "xây dựng features" sang "chứng minh features đúng đắn". Sau 15 phases đã thiết lập một microkernel hoàn chỉnh với 8 tasks, 14 syscalls, 3 ELF user binaries, và IPC thực tế, câu hỏi không còn là "nó có chạy không?" mà là "làm sao chứng minh nó an toàn?". Đây chính xác là câu hỏi mà DO-178C DAL A và ISO 26262 ASIL D đòi hỏi trả lời — và Phase P là bước đầu tiên để trả lời nó một cách có hệ thống.

Nhìn xa hơn 5–10 năm: nếu AegisOS muốn chạy trên vệ tinh thật, thiết bị y tế thật, hoặc xe tự lái thật, **formal verification coverage** sẽ là yếu tố quyết định certification. Mỗi module kernel không có proof là một "compliance gap" — và gap đó sẽ **tốn kém exponentially** nếu để càng lâu. Grant (shared memory) + IRQ (interrupt routing) + Watchdog (liveness monitoring) là 3 module có **ảnh hưởng safety cao nhất** sau IPC — đây đúng là priority.

Tuy nhiên, tôi thấy plan hiện tại có thể **mạnh hơn** nếu chúng ta (1) extract pure functions dưới dạng always-available thay vì Kani-only, (2) tận dụng Miri qua KernelCell shim để complement Kani, và (3) tạo FM.A-7 document có tính "sống" — tự cập nhật khi thêm proofs. Đây là đầu tư hạ tầng sẽ trả lại giá trị trong 10+ phases tiếp theo.

---

## Câu hỏi 1: Pure function extraction scope

### Lựa chọn: Option B — Always-available refactor

### Lý do chiến lược:

**1. Single source of truth — nguyên tắc nền tảng cho safety-critical code.**

DO-178C §5.2 (Software Development Standards) yêu cầu "source code phải reflect requirements trực tiếp". Nếu pure functions chỉ tồn tại trong Kani build (`#[cfg(kani)]`), chúng ta tạo ra **hai bản sao logic**: hàm gốc (production) và hàm thuần (verification). Khi grant logic thay đổi (ví dụ Phase Q thêm grant delegation), developer phải nhớ cập nhật **CẢ HAI** — và nếu quên, Kani proof sẽ verify logic **cũ** trong khi production chạy logic **mới**. Đây là class of bug mà formal methods KHÔNG bắt được — vì chúng verify wrong function.

Option B loại bỏ vấn đề này: production code GỌI pure function → pure function LÀ source of truth → Kani verify chính xác code chạy trên QEMU.

**2. Nền tảng cho testing pyramid 10 năm.**

AegisOS hiện có 241 host tests — hầu hết test logic qua globals. Khi project scale (thêm modules, thêm developers), testing qua globals trở nên **fragile** (test A modify global → test B fail vì state leak). Pure functions là **naturally testable** — mỗi test tự tạo input, assert output, không state leakage. Option B biến 3 modules thành exemplar cho mô hình test mới, và dần dần toàn bộ codebase sẽ follow.

**3. Auditor DAL A sẽ hỏi: "Code bạn verify có phải code chạy production không?"**

Với Option A, câu trả lời là "không — chúng tôi verify bản sao dưới `#[cfg(kani)]`, nhưng production code logic tương đương". Auditor sẽ cần bằng chứng **code equivalence** giữa hai bản — đây là overhead lớn. Với Option B, câu trả lời là "có — production gọi pure function, Kani verify chính pure function đó". Đây là câu trả lời mạnh hơn cho **mọi certification path**.

**4. Risk regression — quản lý được.**

Concern chính của Option B: thay đổi call path có thể break regression. Nhưng nhìn cụ thể:
- `grant_create()`: gọi `grant_create_pure()` → nhận `Result<Grant, _>` → apply vào globals + MMU call. Error codes giữ nguyên. UART prints giữ nguyên vị trí. 32 QEMU checkpoints vẫn pass vì observable behavior (UART output) không đổi.
- Pattern đã chứng minh: Phase O IPC cũng có thể dùng Option B — chỉ chọn Option A vì thời gian. Phase P có cơ hội làm đúng từ đầu.

**Đề xuất bổ sung:** Backport IPC pure functions sang always-available (Option B) trong Phase P scope — thống nhất pattern toàn codebase. Effort: ~2h. Value: consistency + IPC proof giờ verify production code.

### Rủi ro dài hạn:

Nếu chọn Option A: trong 3–5 năm khi AegisOS scale lên 20+ modules, **mỗi module sẽ có 2 bản logic** — production và verification. Maintenance cost tăng linearly, drift risk tăng exponentially. Refactoring sau sẽ tốn kém hơn nhiều vì phải update proofs đang pass → risky.

---

## Câu hỏi 2: Kani proof granularity

### Lựa chọn: Option C — Tiered per module (với escalation plan)

### Lý do chiến lược:

**1. Grant: Full symbolic — trivially tractable.**

`MAX_GRANTS = 2` → mỗi Grant có 5 fields → full symbolic = ~10 biến. CBMC giải trong giây. Không có lý do constrain — chứng minh mạnh nhất có thể với chi phí gần zero.

**2. IRQ: Constrained — nhưng với documented escalation path.**

`MAX_IRQ_BINDINGS = 8` → full symbolic = 40+ biến × 2^32 INTID range = intractable. Constrain: `kani::assume(intid >= 32 && intid <= 127)` (SPIs thường dùng), `kani::assume(task_id < NUM_TASKS)`. Document từng assumption trong proof comment + FM.A-7. Khi compute budget cho CI tăng (hoặc CBMC improve), dỡ bỏ constraints → strength tăng tự động.

**3. Watchdog: Constrained budget, full logic.**

`watchdog_should_fault(enabled, interval, ticks_since)` → 3 scalars → full symbolic trivially. `budget_epoch_check_pure` → 8 tasks × budget/ticks_used → constrain: `budget ≤ 1000`, `ticks_used ≤ budget`. Đây là reasonable bounds cho thực tế.

**4. Escalation plan — mỗi proof có "strength level" trong FM.A-7.**

| Level | Mô tả | Áp dụng |
|---|---|---|
| **Full** | No constraints beyond type bounds | Grant proofs, watchdog_should_fault |
| **Bounded** | Constrained value ranges, documented | IRQ proofs, budget_epoch |
| **Partial** | Constrained + reduced array size | Fallback nếu timeout |

Ghi rõ trong FM.A-7 → auditor biết chính xác proof strength → roadmap để upgrade.

### Rủi ro dài hạn:

Nếu chọn Option A (full symbolic) cho tất cả: Kani timeout → proof fail → CI block → developer disable proof → worse than constrained proof. Tiered approach giữ mọi proof GREEN trong CI — đó là điều quan trọng nhất.

---

## Câu hỏi 3: Miri scope và KernelCell compatibility

### Lựa chọn: Option C — KernelCell shim

### Lý do chiến lược:

**1. Kani và Miri complement nhau, không thay thế nhau.**

Kani (model checking) verify **logic properties**: "sau cleanup, không còn active grant cho task_id". Miri (abstract interpretation) verify **memory safety**: "không đọc uninitialized memory, không out-of-bounds, không aliasing violation". Hai lớp verification khác nhau → defense in depth mạnh hơn.

**2. RefCell shim — pragmatic compromise.**

```rust
#[cfg(miri)]
pub struct KernelCell<T>(core::cell::RefCell<T>);

#[cfg(miri)]
impl<T> KernelCell<T> {
    pub const fn new(val: T) -> Self { Self(RefCell::new(val)) }
    pub fn get(&self) -> &T { /* borrow() */ }
    pub unsafe fn get_mut(&self) -> *mut T { /* borrow_mut() as *mut */ }
}
```

Miri verify: (1) Không đọc uninitialized, (2) Không double borrow (heuristic cho single-core correctness), (3) Bounds check trên array access. Shim semantics **khác** production — nhưng đó là OK vì mục tiêu khác nhau.

**3. DO-333 §6.3 compliance — differentiate AegisOS from competitors.**

Hầu hết embedded OS projects chỉ có testing (DO-178C §6.4). Một số có model checking (Kani). Rất ít có **cả model checking + abstract interpretation**. AegisOS có cả hai → positioning mạnh cho certification.

**4. Effort: ~4h nhưng reusable.**

KernelCell shim viết 1 lần, dùng cho mọi phase sau. Miri CI job setup 1 lần. Annotation `#[cfg(not(miri))]` cho asm tests — cũng chỉ 1 lần. Investment trả lại mỗi phase.

### Rủi ro dài hạn:

Nếu defer Miri: khi AegisOS thêm SMP (multi-core), memory safety bugs sẽ **xuất hiện** — và không có infrastructure Miri sẵn để catch chúng. Setup Miri bây giờ (khi code đơn giản) dễ hơn nhiều so với setup khi code phức tạp.

---

## Câu hỏi 4: Grant cleanup asymmetry

### Lựa chọn: Option A + minor fix (zero phys_addr on peer fault)

### Lý do chiến lược:

**1. Deep analysis cho thấy asymmetry có lý do kỹ thuật.**

```
Owner fault:  Owner lifecycle kết thúc → toàn bộ grant vô nghĩa → EMPTY_GRANT
Peer fault:   Peer chết NHƯNG owner vẫn sống → owner's MMU mapping vẫn valid
              → không thể zero owner field (owner đang dùng page!)
              → chỉ unmap peer + deactivate
```

Option B (zero toàn bộ khi peer fault) sẽ **unmap owner's page** — gây crash cho owner nếu owner đang access grant page. Đây KHÔNG phải cleanup — đây là **tạo fault mới**.

**2. Minor fix: zero `phys_addr` khi peer fault.**

Hiện tại peer fault giữ `phys_addr` stale trong inactive grant. Cosmetic nhưng nên clean: set `phys_addr = 0` khi deactivate. Reason: defense-in-depth — nếu có bug đọc phys_addr từ inactive grant, giá trị 0 gây fault rõ ràng hơn stale address.

**3. Document trong FM.A-7 + code comment.**

```rust
// DESIGN DECISION: Peer fault → deactivate + clear peer (owner alive, MMU retained)
// Owner fault → EMPTY_GRANT (owner lifecycle ends, full cleanup)
// See docs/standard/05-proof-coverage-mapping.md "Design Decisions"
```

Kani proof `grant_cleanup_completeness` verify: "sau cleanup, không có active grant reference faulted task as owner OR peer". Behavior AS-IS, documented.

### Rủi ro dài hạn:

Nếu chọn Option C (notification): scope creep lớn, cần thêm syscall/notification mechanism, vượt xa Phase P scope. Defer to Phase Q nếu grant delegation trở thành requirement.

---

## Câu hỏi 5: FM.A-7 document depth

### Lựa chọn: Option C — Living document + automation

### Lý do chiến lược:

**1. Proof count sẽ tăng exponentially.**

Phase N: 6 proofs. Phase O: +4 = 10. Phase P: +8 = 18. Pattern: ~8 proofs/phase. Phase Q+R+S: 18 → 34 → 50. Ở 50 proofs, manual table maintenance là **painful**. Invest automation bây giờ (khi table nhỏ) → reap benefits khi table lớn.

**2. Script rất đơn giản — ~15 dòng.**

```bash
#!/bin/bash
echo "# Auto-generated Kani Proof Inventory"
echo "| # | File | Harness | Unwind |"
grep -rn '#\[kani::proof\]' src/ | while read line; do
    file=$(echo $line | cut -d: -f1)
    # extract function name from next line...
done
```

Script extract proof list từ source → so sánh với FM.A-7 table → CI fail nếu mismatch. Effort: 1–2h. Maintenance: near-zero (grep is stable).

**3. FM.A-7 yêu cầu "evidence of completeness" — automation cung cấp.**

Auditor hỏi: "Bạn có chắc table liệt kê tất cả proofs?" Với manual table: "Chúng tôi tin vậy." Với automated check: "CI verify mỗi commit." Câu trả lời thứ hai mạnh hơn **qualitatively**.

**4. Living document = comprehensive (Option B) + automation = best of both.**

Bảng mapping đầy đủ + Uncovered Properties + Proof Limitations + **auto-verified proof inventory**. Tổng effort: 3h (2h doc + 1h script).

### Rủi ro dài hạn:

Nếu chọn Option A (minimal): document sẽ outdated sau 2 phases → FM.A-7 non-compliance → remediation cost khi certification.

---

## Câu hỏi 6: README refresh scope

### Lựa chọn: Option B — Full rewrite

### Lý do chiến lược:

**1. README là "front door" — first impression matters.**

GitHub visitors (potential contributors, evaluators, safety engineers) đọc README đầu tiên. Nếu README nói "3 tasks, 189 tests, 13 syscalls" mà code có 8 tasks, 241+ tests, 14 syscalls — credibility gap **ngay lập tức**. Đây không phải cosmetic issue — đây là trust issue.

**2. `.github/copilot-instructions.md` đã là source of truth — just adapt it.**

Copilot instructions đã được cập nhật Phase O: module table, memory map, syscall ABI, test counts, capability bits — tất cả chính xác. README rewrite = adapt content từ copilot-instructions.md sang public-facing format. Effort: 2–3h.

**3. Safety engineer cần standalone README.**

Trong certification context, README là "Software Description Document" (SDD) lite. Safety engineers **không** clone repo — họ nhận PDF/archive. README phải self-contained: architecture, memory map, build, test, verification.

**4. Phase P closing Phase O item #12 "Cập nhật README" — nên làm đúng, không half-measure.**

Closing a debt item with a partial fix is worse than not closing it — because then everyone thinks it's done.

### Rủi ro dài hạn:

Nếu chọn Option A (fix numbers): README vẫn missing architecture diagram, source layout, user workspace docs, KernelCell, klog!, Kani proofs, Miri, TaskState::Exited, cleanup_task_resources, grant/irq/watchdog docs. Sẽ phải rewrite anyway — and next time there will be EVEN MORE to update.

---

## Đề xuất bổ sung

### 1. Backport IPC pure functions

Phase O IPC pure functions (`copy_message_pure`, `cleanup_pure`) hiện `#[cfg(kani)]` — nên chuyển sang always-available (consistent với Option B ở Q1). Effort: ~1h. Value: IPC proofs giờ verify production code path.

### 2. Thêm proof: `grant_no_self_grant`

Mặc dù `grant_create` đã check `owner == peer`, Kani proof chứng minh pure function **reject** self-grant mọi lúc → strengthen property coverage. Trivial proof, ~15 phút.

### 3. IRQ `notify_bit` collision detection — document as known limitation

Không fix trong Phase P (zero runtime changes), nhưng ghi vào FM.A-7 "Uncovered Properties": "Two INTIDs bound to same task with same notify_bit will merge — IRQ identity lost. No proof or runtime check currently exists."

### 4. Kani CI caching

10 proofs hiện chạy ~2 phút. 18 proofs có thể 5–8 phút. Suggestion: cache CBMC artifacts trong CI → incremental verification khi chỉ 1 module thay đổi. Effort: ~1h CI config. Value: CI time stays manageable as proofs scale.

---

## Tóm tắt lựa chọn

| Câu hỏi | Lựa chọn | Lý do 1 dòng |
|---|---|---|
| **Q1: Pure function extraction** | **Option B** (always-available) | Single source of truth — auditors verify production code, not copy |
| **Q2: Kani granularity** | **Option C** (tiered) + escalation plan | Full cho grant (trivial), constrained cho irq/watchdog (documented upgrade path) |
| **Q3: Miri scope** | **Option C** (KernelCell shim) | Complement Kani with memory safety; shim reusable cho mọi phase sau |
| **Q4: Grant cleanup** | **Option A** + zero phys_addr | Asymmetry có lý do kỹ thuật; document + defense-in-depth cosmetic fix |
| **Q5: FM.A-7 depth** | **Option C** (living doc + automation) | 15-line script ensures proof table stays in sync — audit-grade evidence |
| **Q6: README refresh** | **Option B** (full rewrite) | copilot-instructions.md already provides source of truth to adapt |
