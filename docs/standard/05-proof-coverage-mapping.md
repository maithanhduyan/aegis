# Proof Coverage Mapping — DO-333 FM.A-7

> **AegisOS Formal Verification Coverage** — Maps each Kani proof to the module, property, and safety standard requirement it satisfies. Fulfills DO-333 objective FM.A-7 ("Verification of Verification Results").
>
> **Total proofs: 18** (10 existing + 8 Phase P) | **Modules covered: 7/7 kernel modules**

---

## Bảng Mapping — Kani Proofs → Safety Properties

| # | Module | Proof Harness | Property Verified | Constraint | DO-333 | ISO 26262 | Phase |
|---|---|---|---|---|---|---|---|
| 1 | `kernel/cap.rs` | `cap_check_bitwise_correctness` | Capability bitmask logic correct for all 19 bits | Full symbolic | FM.A-5 | — | N |
| 2 | `kernel/cap.rs` | `cap_for_syscall_no_panic_and_bounded` | No panic for syscall 0–13, result ≤ 18 bits | Full symbolic | FM.A-5 | — | N |
| 3 | `kernel/sched.rs` | `schedule_idle_guarantee` | IDLE task always selected when no Ready tasks; picked task is eligible | Full symbolic (8 tasks) | FM.A-4 | Part 6 §8 | N |
| 4 | `kernel/sched.rs` | `restart_task_state_machine` | Only Faulted→Ready; Exited stays Exited; context restored correctly | Full symbolic | FM.A-4 | Part 6 §8 | N |
| 5 | `kernel/ipc.rs` | `ipc_queue_no_overflow` | push full→false, pop empty→None, count∈[0, MAX_WAITERS] | Full symbolic (4 slots) | FM.A-5 | Part 9 DFA | O |
| 6 | `kernel/ipc.rs` | `ipc_message_integrity` | Payload x[0..3] preserved across copy_message_pure | Full symbolic (4×u64) | FM.A-5 | Part 9 DFA | O |
| 7 | `kernel/ipc.rs` | `ipc_cleanup_completeness` | Cleanup removes task from ALL endpoint sender queues and receiver slots | Full symbolic (4 EP × 4 waiters) | FM.A-5 | Part 9 DFA | O |
| 8 | `mmu.rs` | `pt_index_in_bounds` | Page table index within valid range for all task IDs | Full symbolic | FM.A-5 | — | N |
| 9 | `mmu.rs` | `pt_index_no_task_aliasing` | No two tasks share page table indices | Full symbolic (8 tasks) | FM.A-5 | Part 9 FFI | N |
| 10 | `platform/qemu_virt.rs` | `elf_load_addr_no_overlap` | No ELF slot overlap, all within bounds | Full symbolic (6 slots) | FM.A-5 | — | O |
| 11 | `kernel/grant.rs` | `grant_no_overlap` | After create, grant has correct owner/peer; slot was inactive | **Full symbolic** (MAX_GRANTS=2) | FM.A-5 | Part 9 FFI | **P** |
| 12 | `kernel/grant.rs` | `grant_cleanup_completeness` | After cleanup, task NOT in any active grant (owner or peer) | **Full symbolic** (MAX_GRANTS=2) | FM.A-5 | Part 9 DFA | **P** |
| 13 | `kernel/grant.rs` | `grant_slot_exhaustion_safe` | Create on full slots → error, original state unmodified | **Full symbolic** (MAX_GRANTS=2) | FM.A-5 | Part 9 DFA | **P** |
| 14 | `kernel/irq.rs` | `irq_route_correctness` | Route delivers correct (task_id, notify_bit) for bound INTID | **Constrained** (intid 32–127) | FM.A-5 | Part 6 §8 | **P** |
| 15 | `kernel/irq.rs` | `irq_no_orphaned_binding` | After cleanup, no active binding references the cleaned task | **Constrained** (intid 32–127, task_id < 8) | FM.A-5 | Part 9 DFA | **P** |
| 16 | `kernel/irq.rs` | `irq_bind_no_duplicate_intid` | Cannot bind same INTID twice — returns ERR_ALREADY_BOUND | **Constrained** (intid 32–127) | FM.A-5 | Part 6 §8 | **P** |
| 17 | `kernel/sched.rs` | `watchdog_violation_detection` | interval>0 ∧ elapsed>interval → fault; interval=0 → never fault | Full symbolic (u64) | FM.A-5 | Part 6 §8 | **P** |
| 18 | `kernel/sched.rs` | `budget_epoch_reset_fairness` | All non-Inactive/Exited tasks get ticks_used=0; Inactive/Exited preserved | Full symbolic (8 tasks × 6 states) | FM.A-5 | Part 6 §8 | **P** |

### Constraint Strength Legend

| Level | Meaning | Completeness |
|---|---|---|
| **Full symbolic** | All inputs are unconstrained `kani::any()` (bounded by type/unwind) | Exhaustive within bounds |
| **Constrained** | Some inputs restricted (e.g., `intid 32–127`) to avoid solver timeout | Exhaustive within constraints; upgrade path documented |

---

## Design Decisions

### 1. Grant Cleanup Asymmetry (Intentional)

**Observation**: `cleanup_task()` handles owner vs. peer differently:
- **Owner fault**: Grant → `EMPTY_GRANT` (full zero)
- **Peer fault**: `peer = None`, `active = false` (owner field preserved)

**Rationale**: When peer faults, the owner task may still be alive with an active MMU mapping to the grant page. Zeroing the owner field would leave a dangling mapping. Setting `active = false` gates all future access paths while preserving owner information for debugging.

**Kani verification**: `grant_cleanup_completeness` proves that after cleanup, the faulted task is NOT referenced in any **active** grant — which is the safety-critical property.

### 2. IRQ Constrained Proofs

**Observation**: IRQ proofs use `intid 32–127` instead of full `u32` range.

**Rationale**: AArch64 GICv2 SPIs are INTID 32–1019. User-bindable INTIDs on QEMU virt are a small subset. Full `u32` symbolic space would cause CBMC solver timeout (>30 min). Constraining to 32–127 covers all realistic QEMU virt interrupts while keeping proof time ≤5 min.

**Upgrade path**: When GICv3 or more peripherals are added, extend constraint range and increase `--cbmc-args --unwind`.

### 3. Pure Functions Under `#[cfg(kani)]`

**Observation**: Pure functions exist as Kani-only duplicates of production logic.

**Rationale**: Production code is stable across 6+ phases (J→O). Refactoring production functions to call pure functions risks regression in 250 host tests + 32 QEMU checkpoints. The `#[cfg(kani)]` approach provides formal verification with zero runtime risk.

**Migration trigger**: Move to always-available when module count > 6 or pre-certification audit requires it. Each function has a `TODO(Phase-Q+)` comment.

---

## Uncovered Properties (Backlog)

These properties are NOT yet formally verified. They represent future work:

| # | Property | Module | Difficulty | Priority |
|---|---|---|---|---|
| 1 | Scheduler deadlock-freedom | `sched.rs` | High | Medium |
| 2 | Priority inversion absence | `sched.rs` | High | Medium |
| 3 | IPC timeout correctness | `ipc.rs` | Medium | Low (no timeouts yet) |
| 4 | Grant delegation chains | `grant.rs` | Medium | Low (no delegation yet) |
| 5 | Notify bit collision detection | `irq.rs` | Low | Medium |
| 6 | MMIO mapping correctness | `mmu.rs` | High | Low (HW-dependent) |
| 7 | Context switch register preservation | `exception.rs` | High | High (ABI-critical) |

---

## Proof Limitations & Assumptions

1. **Single-core assumption**: All proofs assume uniprocessor execution (no data races). Invalid if AegisOS adds SMP support.
2. **Unwinding bounds**: `MAX_GRANTS=2`, `MAX_IRQ_BINDINGS=8`, `NUM_TASKS=8`. Proofs are exhaustive within these bounds but do not cover larger configurations.
3. **Pure functions only**: Proofs verify logic, NOT side effects (MMIO writes, GIC register access, TLB flushes). Side effects are tested by QEMU integration tests.
4. **No floating point**: All proofs operate on integer types. FP is trapped at EL0 (`CPACR_EL1.FPEN=0b01`).
5. **Kani version**: cargo-kani 0.67.0 (Docker `aegis-dev` container).

---

## Verification Command

```bash
# Run all 18 Kani proofs (requires aegis-dev Docker container)
docker exec -w /workspaces/aegis aegis-dev cargo kani --tests

# Expected output: 18 harnesses, 18 passed, 0 failed
```

---

## Traceability to Safety Standards

| Standard | Clause | How This Document Satisfies |
|---|---|---|
| **DO-333** FM.A-5 | Formal Analysis of Source Code | Bảng mapping (18 proofs) demonstrates source-level formal analysis |
| **DO-333** FM.A-7 | Verification of Verification Results | This document IS the FM.A-7 artifact — maps proofs ↔ requirements |
| **DO-333** §6.3 | Abstract Interpretation | Deferred (Miri skeleton only). Kani model checking satisfies formal analysis. |
| **DO-178C** §4 | Bidirectional traceability | Bảng mapping provides proof → requirement → module traceability |
| **DO-178C** §6.1 | Robustness | Watchdog violation detection proof (exhaustive) |
| **ISO 26262** Part 9 §7 | Dependent Failure Analysis | Grant cleanup + IRQ cleanup proofs verify freedom from interference |
| **ISO 26262** Part 6 §8 | Software Unit Verification | 18 Kani proofs extend unit-level formal verification |
| **IEC 62304** §5.5.5 | Software Unit Verification | Formal methods complement 250 host unit tests |
