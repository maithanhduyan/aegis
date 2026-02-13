# 🔭 GPT-Visionary — Review Round 1 | Phase P

> **Ngày:** 2026-02-13
> **Reviewer:** GPT-Visionary-Agent
> **Tài liệu tham chiếu:** `docs/plan/16-plan-phase-p-formal-verification-expansion_2026-02-13_10-00.md`
> **Góc nhìn:** Chiến lược dài hạn 5–10–20 năm, hướng đến DAL A / ASIL D certification trên phần cứng thật

---

## Tổng quan nhận định

Phase P là **điểm uốn chiến lược** (strategic inflection point) của AegisOS. Sau 15 phases xây nền tảng chức năng — từ boot đầu tiên đến 3 ELF user tasks hoàn chỉnh — đây là lần đầu tiên dự án dành **toàn bộ một phase** cho formal verification mà không thêm feature mới nào. Đây là tín hiệu trưởng thành quan trọng: một dự án safety-critical phải chứng minh rằng nó **biết dừng lại để verify** trước khi tiến tiếp. Trong bối cảnh DO-178C DAL A — nơi mà mỗi dòng code phải có truy vết hai chiều đến yêu cầu — Phase P biến AegisOS từ "microkernel hoạt động đúng trên QEMU" thành "microkernel có bằng chứng toán học đi kèm".

Nhìn xa hơn, đây là phase thiết lập **verification infrastructure** cho 5–10 năm tới. Mỗi feature phase trong tương lai (SMP, virtualization, real-time networking, hardware security modules) sẽ cần pure function extraction + Kani proof + FM.A-7 mapping. Nếu Phase P thiết lập pattern đúng, mọi phase sau sẽ đi theo con đường đã mở. Nếu sai, technical debt sẽ tích lũy — pure functions drift khỏi production code, proofs trở thành theater (chứng minh properties không ai dùng), và FM.A-7 document trở thành artifact chết. Vì vậy, mỗi quyết định trong 6 câu hỏi dưới đây cần được đánh giá không chỉ trên "Phase P có pass không?" mà trên "10 năm nữa, khi AegisOS chạy trên phần cứng thật với 50+ modules, quyết định này có còn đúng không?"

Một nhận xét tổng thể: plan hiện tại **tốt về breadth** (cover đúng 3 module thiếu proof) nhưng **cần cải thiện về depth** ở một số quyết định thiết kế — đặc biệt là pure function extraction strategy (Q1) và Miri scope (Q3). Tôi sẽ phân tích chi tiết bên dưới.

---

## Câu hỏi 1: Pure function extraction scope

### Lựa chọn: Option B — Always-available refactor

### Lý do chiến lược:

**1. Single source of truth là nguyên tắc bất khả thương lượng cho safety-critical code.**

Nhìn vào code hiện tại của `kernel/ipc.rs`, pattern `#[cfg(kani)]` đã tạo ra **hai bản sao logic**: hàm gốc (`cleanup_task`) dùng globals trực tiếp, và `cleanup_pure` dưới `#[cfg(kani)]` duplicate logic đó dưới dạng immutable. Đây là khoản nợ kỹ thuật Phase O để lại — chấp nhận được cho IPC pilot, nhưng **không thể mở rộng** cho 3 module nữa trong Phase P mà không biến thành vấn đề nghiêm trọng.

Lý do: khi grant logic thay đổi (ví dụ Phase Q thêm grant delegation chain), developer phải nhớ update **cả** `grant_create` lẫn `grant_create_pure`. Với `#[cfg(kani)]`, compiler sẽ **không bắt lỗi** nếu logic drift — vì khi build production (`cargo build --release`), pure functions không được compile. Chỉ khi chạy Kani mới phát hiện sai lệch — và lúc đó developer có thể đã merge code sai vào main.

**2. DO-178C §6.3.4f yêu cầu "Software verification process activities shall verify the source code is accurate and consistent."**

Hai bản sao logic mâu thuẫn với "consistent". Auditor sẽ hỏi: "Kani chứng minh `grant_create_pure` đúng — nhưng `grant_create` (production code) có gọi hàm đó không? Làm sao biết chúng cùng logic?" Với Option A, câu trả lời là "chúng tôi duplicate bằng tay và tin rằng chúng giống nhau" — đây là **verification gap** mà auditor DAL A sẽ không chấp nhận.

Với Option B, câu trả lời là: "Production code gọi trực tiếp pure function → logic đã được verify **chính là** logic chạy trên phần cứng." Đây là **strongest claim** có thể có.

**3. Tầm nhìn 5–10 năm: refactor pipeline sẽ phải xảy ra — làm sớm rẻ hơn.**

Khi AegisOS scale lên 20+ modules, pattern "hàm gốc touch globals → pure function duplicate cho Kani" sẽ tạo ra hàng trăm hàm duplicate. Tại một thời điểm nào đó, team sẽ buộc phải refactor sang Option B. Chi phí refactor tăng theo kích thước codebase — refactor 3 module (grant/irq/watchdog) bây giờ rẻ hơn refactor 20 module 3 năm sau.

**4. Xử lý concern về watchdog_scan() dependency trên tick_count().**

Concern hợp lệ: `watchdog_scan()` gọi `crate::timer::tick_count()` — external state. Giải pháp đơn giản: pure function nhận `now: u64` as parameter:

```rust
// Pure function — always available
pub fn watchdog_should_fault(enabled: bool, interval: u64, last_heartbeat: u64, now: u64) -> bool {
    if !enabled || interval == 0 { return false; }
    now.wrapping_sub(last_heartbeat) > interval
}

// Production wrapper
pub fn watchdog_scan() {
    let now = crate::timer::tick_count();
    // ... gọi watchdog_should_fault(hb > 0, hb, last_hb, now) cho mỗi task
}
```

Pattern `inject external dependency as parameter` là standard practice trong testable design — không phải innovation, mà là basic software engineering. Đây cũng chính xác là cách `irq_route_pure` nên hoạt động: nhận `table: &[IrqBinding; 8]` thay vì đọc globals.

**5. Concern về 241 existing tests.**

Option B **không yêu cầu sửa test signatures**. Hàm gốc (`grant_create`, `irq_bind`, v.v.) vẫn giữ nguyên public API. Chúng chỉ thay đổi internal implementation: thay vì inline logic, chúng gọi pure function rồi apply kết quả. 241 tests gọi hàm gốc → vẫn pass. Rủi ro regression rất thấp nếu thực hiện đúng.

**6. Cân nhắc IPC backport.**

Phase P nên bao gồm **backport IPC pure functions** từ `#[cfg(kani)]` sang always-available. Điều này đảm bảo consistency: **tất cả** modules dùng cùng pattern, không có ngoại lệ. Nợ kỹ thuật Phase O nên trả ngay trong Phase P.

### Rủi ro dài hạn:

| Rủi ro | Xác suất | Giảm thiểu |
|---|---|---|
| Pure function extraction thay đổi error path subtlety (ví dụ: thứ tự UART print thay đổi) | Trung bình | UART print giữ trong wrapper, không trong pure function. Pure function chỉ trả `Result`. |
| Developer quên gọi pure function, viết logic trực tiếp trong wrapper | Thấp | CI lint: `grep` cho `GRANTS.get_mut()` trong logic blocks (ngoài apply step) |
| Performance regression do thêm function call + copy array | Rất thấp | Arrays nhỏ (2–8 entries). Compiler inline `#[inline]`. Thực tế không đo được trên Cortex-A53. |
| Nếu Option B fail giữa chừng, phải rollback → mất thời gian | Thấp | Implement theo từng module: grant → verify → irq → verify → watchdog → verify. Mỗi module là atomic. |

---

## Câu hỏi 2: Kani proof granularity

### Lựa chọn: Option C — Tiered per module, nhưng với lộ trình escalation rõ ràng

### Lý do chiến lược:

**1. Thực tế: state space không bình đẳng giữa các module.**

Grant có `MAX_GRANTS = 2` — tổng cộng 2 × 5 fields = 10 symbolic variables. Full symbolic exploration hoàn toàn tractable — Kani/CBMC sẽ complete trong vài giây. Không có lý do gì để constrain.

IRQ có `MAX_IRQ_BINDINGS = 8` — tổng cộng 8 × 5 fields = 40 symbolic variables. Full symbolic sẽ tạo $2^{40+}$ states (thực tế nhỏ hơn do constraints, nhưng vẫn rất lớn). Constrained proofs (assume valid task_id, unique INTIDs) giảm state space đáng kể và vẫn có giá trị chứng minh cao.

Watchdog: 8 tasks × 3 fields (enabled, interval, last_heartbeat) = 24 variables. Nhưng `watchdog_should_fault` chỉ xét **1 task tại một thời điểm** — pure function nhận 4 scalars → full symbolic trivially tractable. `budget_epoch_check_pure` xét 8 tasks nhưng mỗi task chỉ 2 fields → 16 variables → tractable nếu constrained hợp lý.

**2. Tiered approach NHƯNG phải có escalation plan.**

Vấn đề của Option C gốc ("inconsistent proof strength") được giải quyết bằng **explicit documentation**: trong FM.A-7 mapping, ghi rõ cho mỗi proof là "full symbolic" hay "constrained", liệt kê assumptions, và đánh dấu "escalation target" cho constrained proofs. Ví dụ:

```
| irq_route_correctness | Constrained | Assumes: unique INTIDs, task_id < 8 | Escalation: Phase R — full symbolic khi CBMC v6 hỗ trợ array slicing tốt hơn |
```

Điều này biến "inconsistent" thành "deliberately tiered with documented rationale" — auditor thích điều này vì nó cho thấy team **biết** giới hạn và có kế hoạch.

**3. Về concern Kani timeout budget (≤5 phút).**

5 phút per proof trong CI là hợp lý cho Phase P. Nhưng nhìn xa 5 năm: khi có 50+ proofs, serial execution sẽ mất 250+ phút. Nên bắt đầu thiết kế cho **parallel Kani execution** (mỗi proof là independent). CI job nên chạy `cargo kani --harness <name>` song song, không sequential. Phase P là lúc tốt để thiết lập pattern này.

**4. Đề xuất cụ thể cho IRQ proofs.**

`irq_route_pure` chỉ nhận `table: &[IrqBinding; 8]` + `intid: u32` → trả `Option<(usize, u64)>`. Proof property: "nếu có binding active với intid X, trả đúng (task_id, notify_bit)". Symbolic state: 8 bindings symbolic, intid symbolic. Constrain: `intid >= 32`, `task_id < 8`. Kani sẽ enumerate ~$8 \times 2^{32}$ states cho intid — **quá lớn**. Cần constrain intid range: `kani::assume(intid < 256)` hoặc dùng `kani::any_where(|x| *x >= 32 && *x < 256)`. Document assumption: "proofs cover INTID 32–255 (first 224 SPIs, sufficient for QEMU virt)."

### Rủi ro dài hạn:

| Rủi ro | Xác suất | Giảm thiểu |
|---|---|---|
| Constrained proofs miss bug nằm ngoài assumed invariants | Trung bình | Document assumptions rõ ràng. Khi invariant enforcement code thay đổi, re-evaluate proof assumptions. |
| Kani version upgrade thay đổi performance characteristics → proofs break | Thấp | Pin Kani version trong `rust-toolchain.toml` hoặc Dockerfile. Test Kani upgrade trong staging trước. |
| Tiered approach tạo false sense of security ("chúng ta có proof rồi") cho constrained modules | Trung bình | FM.A-7 document phải ghi rõ **proof strength level** (full/constrained/partial). Never say "verified" without qualification. |
| Kani tổng thời gian chạy tăng khi thêm proofs → CI chậm | Cao (dài hạn) | Parallel execution từ Phase P. Cache CBMC artifacts. |

---

## Câu hỏi 3: Miri scope và KernelCell compatibility

### Lựa chọn: Option C — Miri + KernelCell shim, nhưng với phạm vi rõ ràng và expectation management

### Lý do chiến lược:

**1. Loại bỏ Option D (defer) — DO-333 §6.3 không phải optional cho DAL A.**

DO-333 §6.3 khuyến nghị abstract interpretation nhưng không bắt buộc — **nếu** đã có model checking (Kani) ở mức đủ mạnh. Tuy nhiên, Kani verify **pure functions** (logic correctness), trong khi Miri verify **memory safety** (UB absence). Chúng bổ sung cho nhau, không thay thế. Một auditor DAL A sẽ hỏi: "Các bạn có bao nhiêu `unsafe` block? Làm sao verify chúng?" — Kani proofs trên pure functions **không trả lời câu hỏi này**. Miri là câu trả lời tự nhiên nhất.

Defer Miri nghĩa là **mọi `unsafe` block** trong grant/irq/sched/ipc vẫn chỉ được verify bằng host tests (runtime assertions, not formal). Đó là gap trong verification story.

**2. Loại bỏ Option A (full Miri) — cost/benefit không hợp lý ngay bây giờ.**

241 tests × 50x slowdown = hours trong CI. `KernelCell` sẽ gây hàng chục false positives. Effort để annotate `#[cfg(not(miri))]` cho >50% tests là lớn. Không xứng đáng cho Phase P.

**3. Option B (pure functions only) quá hẹp — bỏ lỡ giá trị chính của Miri.**

Pure functions không có `unsafe` — Miri verify chúng nhưng tìm thấy gì? Gần như nothing. Giá trị của Miri nằm ở việc verify `unsafe` code paths: `KernelCell::get_mut()`, pointer arithmetic, array access. Option B tránh đúng chỗ Miri cần verify nhất.

**4. Option C — KernelCell shim — là cầu nối thực tế.**

Thiết kế: `#[cfg(miri)]` implementation của `KernelCell` dùng `RefCell<T>`:

```rust
#[cfg(miri)]
pub struct KernelCell<T>(core::cell::RefCell<T>);

#[cfg(miri)]
impl<T> KernelCell<T> {
    pub const fn new(val: T) -> Self { KernelCell(RefCell::new(val)) }
    pub fn get(&self) -> &T { /* borrow() — panics on aliasing violation */ }
    pub unsafe fn get_mut(&self) -> *mut T { self.0.as_ptr() }
}
```

Wait — `RefCell` cần `alloc` crate? Không, `core::cell::RefCell` nằm trong `core`. Nhưng `RefCell::new()` là `const fn` chỉ từ Rust 1.70+. Kiểm tra `rust-toolchain.toml` — AegisOS dùng nightly → OK.

Quan trọng: shim dùng `RefCell` sẽ **panic** nếu code tạo `&mut T` và `&T` đồng thời — đây chính xác là UB mà production `KernelCell` dựa vào single-core assumption để tránh. Miri + shim sẽ verify rằng **trong host test execution paths**, không có re-entrant access pattern nào. Điều này có giá trị — nó xác nhận rằng test scenarios không trigger aliasing.

**5. Scope rõ ràng cho Phase P Miri.**

- Chạy Miri trên **pure function tests mới** (8 tests) + **logic-only tests** (tests không dùng asm). Ước tính ~60–80 tests.
- `KernelCell` shim cho phép Miri chạy tests dùng globals **mà không false positive**.
- Tests dùng inline asm: `#[cfg(not(miri))]`.
- CI timeout: 15 phút cho Miri job.
- Document: "Miri verifies host test paths under RefCell shim — does NOT verify production KernelCell::get_mut() (which relies on single-core invariant)."

**6. Tầm nhìn dài hạn: Miri + Tree Borrows.**

Miri đang chuyển sang **Tree Borrows** model (thay thế Stacked Borrows) — permissive hơn với interior mutability patterns. Trong 2–3 năm, `UnsafeCell`-based `KernelCell` có thể chạy trực tiếp qua Miri mà không cần shim. Đầu tư vào Miri infra ngay bây giờ sẽ trả dividend khi Tree Borrows mature.

### Rủi ro dài hạn:

| Rủi ro | Xác suất | Giảm thiểu |
|---|---|---|
| KernelCell shim hides production bugs (shim semantics khác production) | Trung bình | Document rõ ràng shim scope. Shim tests verify **logic paths**, not memory model. Label in FM.A-7. |
| RefCell overhead khiến Miri timeout trên complex tests | Thấp | Start với pure function tests. Gradually expand. |
| Developer confuse "Miri pass" với "no UB in production" | Cao | Training + documentation: "Miri + shim verifies logic correctness under safe aliasing model. Production relies on single-core invariant (not verified by Miri)." |
| Miri version upgrade breaks shim | Thấp | Pin nightly version. Shim rất đơn giản (~10 dòng). |
| `RefCell::new()` const fn stability | Rất thấp | Stabilized trong Rust 1.70. AegisOS dùng nightly. |

---

## Câu hỏi 4: Grant cleanup asymmetry

### Lựa chọn: Option B — Fix: peer fault cũng zero toàn bộ, nhưng cần thêm documentation rationale

### Lý do chiến lược:

**1. Phân tích code thực tế.**

Nhìn vào [cleanup_task trong grant.rs](src/kernel/grant.rs#L193-L227):

```
Owner fault:  (*GRANTS.get_mut())[i] = EMPTY_GRANT;     // ← full zero
Peer fault:   (*GRANTS.get_mut())[i].peer = None;        // ← partial
              (*GRANTS.get_mut())[i].active = false;      // ← deactivate
```

Sau peer fault: `owner` vẫn là `Some(owner_id)`, `phys_addr` vẫn có giá trị. Grant `active = false` ngăn access — nhưng **data residue** tồn tại trong kernel memory.

**2. Data residue là vấn đề cho security certification.**

ISO 26262 Part 9 §7 (DFA — freedom from interference) yêu cầu: lỗi ở component A không gây lỗi ở component B. Với cleanup asymmetry: nếu grant slot được reuse (future `grant_create` gán slot này cho task khác), và code mới **quên kiểm tra `active`** (bug), nó sẽ thấy stale `owner` field → potential confusion. Full zero loại bỏ class of bugs này.

Trong DO-178C DAL A, auditor sẽ hỏi: "Tại sao cleanup paths khác nhau? Có analysis chứng minh partial cleanup không gây interference?" — câu trả lời dễ nhất là "chúng tôi đã fix để consistent."

**3. Đánh giá Option A (document as intentional).**

Option A hợp lệ **nếu** có rationale mạnh: "Owner fault zeros vì owner controls grant lifecycle; peer fault chỉ removes peer access, giữ grant metadata cho owner reference." Nhưng nhìn code: `active = false` nghĩa là **không ai dùng được grant này nữa**. Owner cũng không thể. Vậy giữ `owner` field để làm gì? Không có API nào cho owner "reclaim" deactivated grant. Metadata bị giữ lại **không phục vụ mục đích nào** → đây là accidental asymmetry, không phải design decision.

**4. Đánh giá Option C (owner notification).**

Option C hay về concept — nhưng **scope creep** nghiêm trọng. Thêm notification khi peer fault nghĩa là: (1) grant cần notification bit, (2) owner phải xử lý notification, (3) grant có thể ở trạng thái "active but no peer" → phức tạp state machine. Đây là **feature mới**, không phải verification fix. Defer sang Phase Q/R nếu cần.

**5. Fix thực tế: 2 dòng code.**

```rust
// TRƯỚC:
} else if (*GRANTS.get_mut())[i].peer == Some(task_idx) {
    #[cfg(target_arch = "aarch64")]
    { crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, task_idx); }
    (*GRANTS.get_mut())[i].peer = None;
    (*GRANTS.get_mut())[i].active = false;
}

// SAU:
} else if (*GRANTS.get_mut())[i].peer == Some(task_idx) {
    #[cfg(target_arch = "aarch64")]
    { crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, task_idx); }
    // Also unmap from owner (grant is now dead — both sides cleaned up)
    if let Some(owner) = (*GRANTS.get_mut())[i].owner {
        #[cfg(target_arch = "aarch64")]
        { crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, owner); }
    }
    (*GRANTS.get_mut())[i] = EMPTY_GRANT;
}
```

Wait — có nên unmap owner's page khi peer fault? Owner còn sống, có thể đang dùng page. Hmm.

**Đánh giá lại:** Owner vẫn muốn truy cập grant page cho mục đích riêng (nó là owner's page). Unmap owner sẽ gây data loss cho owner — **BAD**. Vậy:

- Zero toàn bộ **metadata** (owner/peer/phys_addr/active) — **YES**, loại bỏ data residue
- Unmap owner's MMU mapping — **NO**, owner vẫn sống và có thể đang dùng page
- Unmap peer's MMU mapping — **YES**, đã có

Sửa lại:

```rust
} else if (*GRANTS.get_mut())[i].peer == Some(task_idx) {
    #[cfg(target_arch = "aarch64")]
    { crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, task_idx); }
    (*GRANTS.get_mut())[i] = EMPTY_GRANT;  // full zero metadata, nhưng KHÔNG unmap owner
}
```

Nhưng đây tạo ra vấn đề mới: owner's MMU mapping vẫn tồn tại nhưng grant metadata bị zero → **inconsistency**. Owner vẫn access được page nhưng kernel không biết.

**Kết luận sau phân tích sâu:** Cleanup asymmetry hiện tại có lý do kỹ thuật phức tạp hơn tưởng tượng. Sửa đổi tôi đề xuất:

- **Metadata:** peer fault → zero `peer`, set `active = false`. **Giữ `owner` và `phys_addr`** — vì owner's MMU mapping vẫn tồn tại (owner còn sống).
- **Thêm:** zero `phys_addr` cùng lúc deactivate (vì không ai cần nó nữa khi `active = false`).
- **FM.A-7:** Document rationale: "Peer fault deactivates grant and clears peer+phys_addr. Owner field retained because owner task is alive with active MMU mapping; owner field cleared when owner faults or grant_create overwrites slot."

**Thay đổi lựa chọn:** Chuyển sang **Option A** (document as intentional) + **minor fix** (zero `phys_addr` khi peer fault). Đây là balanced approach.

### Lựa chọn chính thức: Option A + minor fix (zero phys_addr)

### Rủi ro dài hạn:

| Rủi ro | Xác suất | Giảm thiểu |
|---|---|---|
| Stale `owner` field trong inactive grant gây confusion khi reuse | Thấp | `grant_create` đã overwrite toàn bộ slot khi create mới → stale field bị ghi đè. |
| Auditor đặt câu hỏi về asymmetry | Trung bình | FM.A-7 document rationale cụ thể (như phân tích trên). |
| Future developer assume symmetry, viết code sai | Thấp | Comment trong cleanup_task: `// INTENTIONAL: owner field kept — see FM.A-7 §Grant Cleanup Rationale` |
| Owner không biết grant bị revoke khi peer fault | Trung bình | Backlog cho Phase Q: thêm notification cho owner khi peer fault. Không làm trong Phase P. |

---

## Câu hỏi 5: FM.A-7 document depth

### Lựa chọn: Option C — Living document + automation

### Lý do chiến lược:

**1. FM.A-7 là tài liệu SỐNG — không phải artifact viết một lần.**

DO-333 FM.A-7 yêu cầu "verification of verification results" — tức là phải chứng minh rằng bộ proofs **cover đủ** properties. Điều này nghĩa là mỗi khi thêm module/feature mới, FM.A-7 phải update. Nếu document là static markdown, nó sẽ **chắc chắn** outdated sau 2–3 phases.

**2. Automation script cực kỳ đơn giản.**

```bash
#!/bin/bash
# extract-proofs.sh — list all Kani proof harnesses
echo "# Auto-extracted Kani Proofs ($(date))"
echo ""
echo "| # | File | Harness |"
echo "|---|---|---|"
grep -rn '#\[kani::proof\]' src/ | \
  awk -F: '{
    file=$1; line=$2;
    getline; harness=$0;
    gsub(/.*fn /, "", harness);
    gsub(/\(.*/, "", harness);
    printf "| %d | %s:%s | %s |\n", NR, file, line, harness
  }'
```

~15 dòng. Chạy trong CI hoặc pre-commit. Output so sánh với FM.A-7 document → CI fail nếu proof list trong document không match source. **Đây là enforcement**, không chỉ documentation.

**3. Tầm nhìn dài hạn: FM.A-7 là core artifact cho certification.**

Khi AegisOS apply cho DO-178C certification (5–10 năm), DER (Designated Engineering Representative) sẽ review FM.A-7 **đầu tiên** — nó là "map" của toàn bộ verification effort. Living document + automation đảm bảo map luôn chính xác.

**4. Đề xuất cấu trúc FM.A-7 document.**

```markdown
# Proof Coverage Mapping — DO-333 FM.A-7

## 1. Proof Inventory (auto-generated section)
(Script output: file, harness name, line number)

## 2. Property Mapping (manual)
| Proof | Property | Strength | Assumptions | Standard |

## 3. Uncovered Properties (manual)
| Module | Property | Priority | Target Phase |

## 4. Proof Limitations (manual)
| Proof | Limitation | Impact |

## 5. Automation
- CI job: `scripts/verify-fma7.sh` — fails if proof count mismatch
- Last verified: (date)
```

Section 1 auto-generated. Sections 2–4 manual but cross-referenced against Section 1. Section 5 = meta.

### Rủi ro dài hạn:

| Rủi ro | Xác suất | Giảm thiểu |
|---|---|---|
| Automation script breaks khi source structure thay đổi | Thấp | Script dùng `grep` — robust. Pin expected format in CI. |
| Manual sections (2–4) outdated | Trung bình | CI script: nếu proof count in source > proof count in Section 2 mapping → warning. |
| Over-engineering cho 18 proofs | Thấp | Script là 15 dòng. Overhead minimal. Value compounds over time. |
| Developer ignore CI warning cho FM.A-7 mismatch | Thấp | Make it CI **failure**, not warning. Block merge. |

---

## Câu hỏi 6: README refresh scope

### Lựa chọn: Option B — Full rewrite, đồng bộ với `.github/copilot-instructions.md`

### Lý do chiến lược:

**1. README là cửa sổ duy nhất cho outside world.**

AegisOS sẽ cần thu hút: contributors, auditors, academic reviewers, hardware partners, potential customers (nếu commercialize). Tất cả đều nhìn README **trước tiên**. README nói "3 tasks, 189 tests" khi thực tế là "8 tasks, 249 tests, 18 Kani proofs" — đây không chỉ là outdated, mà **undermines credibility**. Auditor sẽ hỏi: "Nếu README sai, tài liệu khác có đáng tin không?"

**2. `.github/copilot-instructions.md` đã là source of truth — leverage nó.**

Copilot instructions đã được cập nhật đến Phase O — có đầy đủ memory map, module table, syscall list, test counts. README full rewrite chỉ cần **adapt format** từ copilot-instructions sang user-facing README. Effort: ~2–3 giờ, không phải viết từ zero.

**3. Option C (lite + links) nghịch lý cho bare-metal OS.**

AegisOS target audience (safety engineers, hardware teams, certifiers) thường làm việc offline hoặc trong classified environments — họ clone repo và đọc locally, không browse VitePress. README phải standalone — mọi thông tin essential phải ở đây.

**4. Structure đề xuất cho README mới.**

```markdown
# AegisOS — Safety-Critical AArch64 Microkernel

## Overview (2 paragraphs)
## Architecture (table: modules, roles)
## Memory Map (table: addresses)
## Syscalls (table: 14 syscalls)
## Capabilities (19 bits, table)
## Build & Run (3-step: user → kernel → QEMU)
## Test Infrastructure
  - 249 host tests
  - 32 QEMU checkpoints
  - 18 Kani proofs (with FM.A-7 link)
  - Miri CI
## Source Layout (tree, including user/)
## Safety Standards Alignment (DO-178C, ISO 26262, IEC 62304)
## Contributing
## License
```

### Rủi ro dài hạn:

| Rủi ro | Xác suất | Giảm thiểu |
|---|---|---|
| README outdated lại sau 3 phases | Cao | Add "README freshness" check to phase completion checklist. Every phase that adds features/tests must update README. |
| Large diff gây merge conflict | Thấp | Phase P là "pure verification" — no parallel feature work. Clean merge window. |
| Information duplication giữa README và copilot-instructions | Trung bình | Acceptable. README = human-facing. Copilot-instructions = AI-facing. Content overlaps but format differs. |
| README quá dài cho casual browsers | Thấp | Table of Contents + collapsible sections (GitHub supports `<details>` tags). |

---

## Đề xuất bổ sung

### 1. Backport IPC pure functions sang always-available (nếu chọn Option B cho Q1)

IPC `copy_message_pure` và `cleanup_pure` hiện là `#[cfg(kani)]`. Nếu Phase P establishes Option B pattern, IPC nên được backport để **tất cả modules consistent**. Effort: ~1 giờ. Benefit: eliminate last duplication.

### 2. Notify_bit collision detection cho IRQ module

Plan hiện tại ghi nhận vấn đề nhưng không fix. Đề xuất: thêm validation trong `irq_bind_pure`:

```rust
// Reject if same task already has a binding with same notify_bit
for binding in table.iter() {
    if binding.active && binding.task_id == task_id && binding.notify_bit == notify_bit {
        return Err(IrqError::NotifyBitCollision);
    }
}
```

Thêm Kani proof `irq_no_notify_bit_collision`. Effort nhỏ, value cao — prevents silent notification merge.

### 3. Property-based testing (proptest) cho pure functions

Kani verify **bounded** state spaces. `proptest` (hoặc hand-rolled random testing) có thể complement Kani bằng cách chạy millions of random inputs. Đặc biệt hữu ích cho `budget_epoch_check_pure` (8 tasks × various budgets × various tick values). Nếu proptest quá heavy (external dep), hand-rolled random tests trong host_tests:

```rust
#[test]
fn fuzz_watchdog_should_fault() {
    // Pseudo-random but deterministic inputs
    for interval in [0, 1, 50, 100, u64::MAX] {
        for elapsed in [0, 1, 50, 100, 101, u64::MAX] {
            let result = watchdog_should_fault(interval > 0, interval, 0, elapsed);
            if interval > 0 && elapsed > interval { assert!(result); }
            if interval == 0 { assert!(!result); }
        }
    }
}
```

Zero dependencies. Runs in host tests. Complements Kani.

### 4. Kani proof cho `grant_revoke_pure` — ownership violation

Plan có `grant_no_overlap`, `grant_cleanup_completeness`, `grant_slot_exhaustion_safe`. Missing: **revoke authorization** — proof rằng `grant_revoke_pure` chỉ cho phép owner revoke, **never** peer hoặc third-party. Đây là security property quan trọng cho capability-based access control.

### 5. CI caching cho Kani artifacts

18 proofs × ~30 giây mỗi proof = ~9 phút. Tương lai 50+ proofs → ~25 phút. CBMC generates intermediate artifacts (SAT formulas, counterexample traces). Cache chúng giữa CI runs khi source file không thay đổi:

```yaml
- uses: actions/cache@v4
  with:
    path: ~/.kani/cache
    key: kani-${{ hashFiles('src/**/*.rs') }}
```

### 6. Xem xét thêm `grant_no_self_grant` proof

Plan hiện tại check `owner == peer` trong `grant_create` code — nhưng không có Kani proof rằng self-grant luôn bị reject. Thêm proof nhỏ:

```rust
#[kani::proof]
fn grant_no_self_grant() {
    let grants = [EMPTY_GRANT; MAX_GRANTS];
    let task: usize = kani::any();
    kani::assume(task < NUM_TASKS);
    let id: usize = kani::any();
    kani::assume(id < MAX_GRANTS);
    let result = grant_create_pure(&grants, id, task, task, 0, 0);
    assert!(result.is_err()); // self-grant always fails
}
```

---

## Tóm tắt lựa chọn

| Câu hỏi | Lựa chọn | Lý do 1 dòng |
|---|---|---|
| **Q1: Pure function extraction** | **Option B** (always-available) | Single source of truth — Kani proves the exact code that runs in production; eliminates drift risk across 5+ year codebase evolution |
| **Q2: Kani proof granularity** | **Option C** (tiered) + escalation plan | Full symbolic cho grant (tractable), constrained cho irq/watchdog (documented assumptions) — pragmatic without pretending uniform strength |
| **Q3: Miri scope** | **Option C** (KernelCell shim) | RefCell shim cho phép Miri verify logic paths qua globals; complements Kani (logic correctness vs memory safety); positions for Tree Borrows future |
| **Q4: Grant cleanup asymmetry** | **Option A + minor fix** (document + zero phys_addr) | Full analysis cho thấy asymmetry có lý do kỹ thuật (owner alive + MMU mapped); zero residual phys_addr; document rationale in FM.A-7 |
| **Q5: FM.A-7 depth** | **Option C** (living doc + automation) | 15-dòng script + CI enforcement đảm bảo proof inventory luôn match source; essential cho certification traceability |
| **Q6: README refresh** | **Option B** (full rewrite) | README = credibility; outdated README undermines trust in all other documentation; copilot-instructions.md là sẵn có source of truth |

---

## Phụ lục: Risk-adjusted Phase P Timeline

Với các lựa chọn trên, ước tính effort:

| Sub-phase | Effort | Dependency | Risk |
|---|---|---|---|
| P1: Pure function extraction (Option B — 3 modules + IPC backport) | 6–8 giờ | — | Medium (refactor production code) |
| P2: Kani proofs (8 new + tiered strategy) | 4–6 giờ | P1 | Medium (IRQ timeout) |
| P3: Miri + KernelCell shim | 3–4 giờ | P1 | Low-Medium (shim complexity) |
| P4: FM.A-7 living doc + automation + README rewrite | 4–5 giờ | P2, P3 | Low |
| **Total** | **17–23 giờ** | | |

Bottleneck: P1. Nếu pure function extraction gây regression, toàn bộ timeline bị delay. Mitigation: implement module-by-module, verify 241 tests pass sau mỗi module.
