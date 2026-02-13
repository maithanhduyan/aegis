/// AegisOS KernelCell<T> — Safe encapsulation for kernel global state
///
/// Wraps `UnsafeCell<T>` with documented safety invariants for single-core
/// kernel execution. Replaces `static mut` with a pattern that:
/// 1. Makes Sync impl explicit (via `unsafe impl Sync`)
/// 2. Requires `unsafe` at every access point
/// 3. Centralizes the safety argument: single-core + interrupts masked
///
/// # Safety Invariant
///
/// KernelCell is only safe to use when ALL of these hold:
/// - **Single-core execution**: QEMU virt with Cortex-A53 uniprocessor config
/// - **No preemption**: Kernel code runs with interrupts masked (DAIF.I=1)
///   during critical sections, or access is from a single execution context
/// - **No re-entrancy**: The same KernelCell is not accessed recursively
///
/// These invariants are enforced by the AegisOS execution model:
/// - Kernel runs at EL1, single-threaded
/// - IRQ handler runs to completion before returning
/// - `--test-threads=1` for host tests

use core::cell::UnsafeCell;

// ─── KernelCell<T> ─────────────────────────────────────────────────

/// A transparent wrapper around `UnsafeCell<T>` for kernel global state.
///
/// Unlike `static mut`, every access requires an explicit `unsafe` block
/// and a `// SAFETY:` comment — making the safety argument visible to
/// auditors and formal verification tools.
#[repr(transparent)]
pub struct KernelCell<T>(UnsafeCell<T>);

// SAFETY: KernelCell is only used in single-core kernel context.
// All access occurs either:
// - During boot (before interrupts enabled, no concurrency)
// - In IRQ handler (runs to completion, single-core, no preemption)
// - In host tests (--test-threads=1, sequential execution)
unsafe impl<T> Sync for KernelCell<T> {}

impl<T> KernelCell<T> {
    /// Create a new KernelCell with the given initial value.
    /// Usable in `static` declarations (const fn).
    pub const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    /// Get a shared reference to the inner value.
    ///
    /// # Safety
    ///
    /// Caller must ensure single-core execution with no concurrent
    /// mutable access (interrupts masked or single-threaded context).
    #[inline(always)]
    pub unsafe fn get(&self) -> &T {
        // SAFETY: Caller guarantees no concurrent mutable access.
        unsafe { &*self.0.get() }
    }

    /// Get a mutable reference to the inner value.
    ///
    /// # Safety
    ///
    /// Caller must ensure single-core execution with no concurrent
    /// access of any kind (interrupts masked or single-threaded context).
    #[inline(always)]
    pub unsafe fn get_mut(&self) -> &mut T {
        // SAFETY: Caller guarantees exclusive access.
        unsafe { &mut *self.0.get() }
    }

    /// Get raw pointer to the inner value. Does not require unsafe.
    /// Useful for passing to FFI or performing atomic-like operations.
    #[inline(always)]
    pub const fn as_ptr(&self) -> *mut T {
        self.0.get()
    }
}

// ─── kcell_index! macro ────────────────────────────────────────────

/// Convenience macro for indexed access into `KernelCell<[T; N]>`.
///
/// Expands to `&mut (*$cell.get_mut())[$idx]`, reducing boilerplate
/// when accessing elements of a KernelCell-wrapped array.
///
/// # Safety
///
/// Must be called inside an `unsafe` block — same invariants as
/// `KernelCell::get_mut()`: single-core, no concurrent access.
///
/// # Example
///
/// ```ignore
/// unsafe { kcell_index!(TCBS, i).state = TaskState::Ready; }
/// ```
#[macro_export]
macro_rules! kcell_index {
    ($cell:expr, $idx:expr) => {
        &mut (*$cell.get_mut())[$idx]
    };
}

// ─── Miri shim skeleton (Phase P) ────────────────────────────────────

/// When running under Miri, KernelCell could use RefCell<T> instead of
/// UnsafeCell<T> to get borrow-checking at runtime. However, RefCell
/// verifies different semantics than production UnsafeCell (it checks
/// Rust borrowing rules, not raw pointer aliasing). This shim is a
/// placeholder for future SMP/preemptive kernel work.
///
/// Backlog: "Miri CI integration — needed when AegisOS has SMP or
///          preemptive kernel (Phase Q+)"
///
/// Example future usage:
/// ```ignore
/// #[cfg(miri)]
/// pub struct KernelCell<T>(core::cell::RefCell<T>);
///
/// #[cfg(miri)]
/// impl<T> KernelCell<T> {
///     pub const fn new(val: T) -> Self { Self(RefCell::new(val)) }
///     pub unsafe fn get(&self) -> &T { /* borrow() */ }
///     pub unsafe fn get_mut(&self) -> &mut T { /* borrow_mut() */ }
/// }
/// ```
#[cfg(miri)]
compile_error!("Miri support is deferred — see docs/standard/05-proof-coverage-mapping.md §Proof Limitations");
