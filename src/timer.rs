/// Đồng hồ hệ thống (Timer)
/// AegisOS Timer — ARM Generic Timer (CNTP_EL0)
///
/// Uses the EL1 Physical Timer (CNTP) with PPI INTID 30.
/// QEMU virt timer frequency: 62,500,000 Hz (62.5 MHz).

#[cfg(target_arch = "aarch64")]
use crate::uart_print;

/// GIC INTID for EL1 Physical Timer (PPI 14)
pub const TIMER_INTID: u32 = 30;

/// Tick interval in ticks (computed at init)
#[cfg(target_arch = "aarch64")]
static mut TICK_INTERVAL: u64 = 0;

/// Monotonic tick counter
pub static mut TICK_COUNT: u64 = 0;

/// Initialize timer for periodic ticks
/// `tick_ms` = interval in milliseconds (e.g., 10 for 10ms)
#[cfg(target_arch = "aarch64")]
pub fn init(tick_ms: u32) {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, CNTFRQ_EL0", out(reg) freq, options(nomem, nostack));
    }

    let ticks = freq * (tick_ms as u64) / 1000;
    unsafe { TICK_INTERVAL = ticks; }

    // Set countdown value
    unsafe {
        core::arch::asm!(
            "msr CNTP_TVAL_EL0, {t}",
            t = in(reg) ticks,
            options(nomem, nostack)
        );
    }

    // Enable timer, unmask interrupt (ENABLE=1, IMASK=0)
    unsafe {
        core::arch::asm!(
            "mov x0, #1",
            "msr CNTP_CTL_EL0, x0",
            out("x0") _,
            options(nomem, nostack)
        );
    }

    uart_print("[AegisOS] timer started (");
    // Print tick_ms as simple decimal
    print_decimal(tick_ms);
    uart_print("ms, freq=");
    print_decimal(freq as u32 / 1_000_000);
    uart_print("MHz)\n");
}

/// Re-arm timer — call from IRQ handler
#[cfg(target_arch = "aarch64")]
pub fn rearm() {
    let ticks = unsafe { TICK_INTERVAL };
    unsafe {
        core::arch::asm!(
            "msr CNTP_TVAL_EL0, {t}",
            t = in(reg) ticks,
            options(nomem, nostack)
        );
    }
}

/// Timer tick handler — called from IRQ dispatch with TrapFrame
#[cfg(target_arch = "aarch64")]
pub fn tick_handler(frame: &mut crate::exception::TrapFrame) {
    unsafe { TICK_COUNT += 1; }

    // Re-arm for next tick
    rearm();

    // Phase K: Track budget for current running task
    unsafe {
        let current = crate::sched::CURRENT;
        crate::sched::TCBS[current].ticks_used += 1;

        // Phase K: Epoch management — reset budgets every EPOCH_LENGTH ticks
        crate::sched::EPOCH_TICKS += 1;
        if crate::sched::EPOCH_TICKS >= crate::sched::EPOCH_LENGTH {
            crate::sched::epoch_reset();
        }

        // Phase K: Watchdog scan at regular intervals
        if TICK_COUNT % crate::sched::WATCHDOG_SCAN_PERIOD == 0 {
            crate::sched::watchdog_scan();
        }
    }

    // Context switch via scheduler
    crate::sched::schedule(frame);
}

/// Get current tick count
#[allow(dead_code)]
pub fn tick_count() -> u64 {
    unsafe { TICK_COUNT }
}

/// Simple decimal printer for small numbers
#[cfg(target_arch = "aarch64")]
fn print_decimal(mut val: u32) {
    if val == 0 {
        crate::uart_write(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        crate::uart_write(buf[i]);
    }
}
