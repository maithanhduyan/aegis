/// AegisOS Structured Kernel Logging — `klog!` macro
///
/// Provides compile-time level filtering with automatic tick + task metadata.
/// Output format: `[TICK:XXXXXXXX] [TN] [LEVEL] message`
///
/// FP-safe: verified via `rust-objdump -d` that `core::fmt` does NOT emit
/// floating-point instructions (Phase M0 check).
///
/// Usage:
/// ```ignore
/// klog!(LogLevel::Info, "boot complete");
/// klog!(LogLevel::Warn, "budget exhausted for task {}", id);
/// ```

// ─── Log Levels ────────────────────────────────────────────────────

/// Log severity levels — lower value = higher severity.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum LogLevel {
    Error = 0,
    Warn  = 1,
    Info  = 2,
    Debug = 3,
}

/// Compile-time maximum log level. Messages above this are eliminated by
/// the compiler (dead-code elimination). Set to 3 for debug builds.
pub const LOG_LEVEL: u8 = 2; // INFO

// ─── UART Writer (core::fmt::Write) ───────────────────────────────

/// Zero-size UART writer implementing `core::fmt::Write`.
/// Forwards formatted output directly to PL011 UART (no buffering).
struct UartWriter;

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        crate::uart::uart_print(s);
        Ok(())
    }
}

// ─── Logging Internals ────────────────────────────────────────────

/// Print the structured log prefix: `[TICK:XXXXXXXX] [TN] [LEVEL] `
///
/// Reads `timer::tick_count()` for tick and `sched::CURRENT` for task ID.
/// During early boot (before scheduler init), tick=0 and task=T0.
#[inline(never)]
pub fn log_prefix(level: LogLevel) {
    use crate::uart::{uart_print, uart_write};

    // [TICK:XXXXXXXX]
    uart_print("[TICK:");
    let tick = crate::kernel::timer::tick_count();
    let hex = b"0123456789ABCDEF";
    for i in (0..8u32).rev() {
        let nibble = ((tick >> (i * 4)) & 0xF) as usize;
        uart_write(hex[nibble]);
    }
    uart_print("] ");

    // [TN] — task index (0, 1, 2)
    uart_print("[T");
    // SAFETY: Single-core kernel, reading CURRENT index for log metadata.
    let task = unsafe { *crate::kernel::sched::CURRENT.get() };
    uart_write(b'0' + task as u8);
    uart_print("] ");

    // [LEVEL]
    match level {
        LogLevel::Error => uart_print("[ERROR] "),
        LogLevel::Warn  => uart_print("[WARN ] "),
        LogLevel::Info  => uart_print("[INFO ] "),
        LogLevel::Debug => uart_print("[DEBUG] "),
    }
}

/// Print a formatted log message with prefix and trailing newline.
///
/// Called by the `klog!` macro — not intended for direct use.
#[inline(never)]
pub fn log_message(level: LogLevel, args: core::fmt::Arguments) {
    log_prefix(level);
    use core::fmt::Write;
    let _ = UartWriter.write_fmt(args);
    crate::uart::uart_print("\n");
}

// ─── Public Macro ──────────────────────────────────────────────────

/// Structured kernel log macro with compile-time level filtering.
///
/// # Examples
///
/// ```ignore
/// use aegis_os::kernel::log::LogLevel;
///
/// klog!(LogLevel::Info, "scheduler initialized");
/// klog!(LogLevel::Error, "task {} faulted at 0x{:X}", id, addr);
/// ```
///
/// Messages with level > `LOG_LEVEL` are eliminated at compile time.
#[macro_export]
macro_rules! klog {
    ($level:expr, $($arg:tt)*) => {
        if ($level as u8) <= $crate::kernel::log::LOG_LEVEL {
            $crate::kernel::log::log_message($level, format_args!($($arg)*));
        }
    };
}
