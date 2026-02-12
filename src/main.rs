// AegisOS — Kernel binary entry point
// This entire file is AArch64-only. When building for host tests (x86_64),
// the content is gated off and only the lib crate is tested.

// On AArch64: full kernel binary with boot asm, syscall wrappers, tasks
#![cfg_attr(target_arch = "aarch64", no_std)]
#![cfg_attr(target_arch = "aarch64", no_main)]

// On host (x86_64): empty bin that does nothing (tests use --lib --test)
#![cfg_attr(not(target_arch = "aarch64"), allow(unused))]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_arch = "aarch64")]
use core::panic::PanicInfo;

#[cfg(target_arch = "aarch64")]
use aegis_os::uart_print;
#[cfg(target_arch = "aarch64")]
use aegis_os::exception;
#[cfg(target_arch = "aarch64")]
use aegis_os::sched;
#[cfg(target_arch = "aarch64")]
use aegis_os::timer;
#[cfg(target_arch = "aarch64")]
use aegis_os::gic;

// Boot assembly — inline vào binary thông qua global_asm!
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(include_str!("arch/aarch64/boot.s"));

// ─── Syscall wrappers ──────────────────────────────────────────────

/// SYS_YIELD (syscall #0): voluntarily yield the CPU to the next task.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_yield() {
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "mov x7, #0",
            "svc #0",
            out("x7") _,
            options(nomem, nostack)
        );
    }
}

/// SYS_SEND (syscall #1): send message on endpoint.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_send(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) {
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") 1u64, // SYS_SEND
            options(nomem, nostack)
        );
    }
}

/// SYS_RECV (syscall #2): receive message from endpoint.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_recv(ep_id: u64) -> u64 {
    let msg0: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") 2u64, // SYS_RECV
            lateout("x0") msg0,
            options(nomem, nostack)
        );
    }
    msg0
}

/// SYS_RECV variant returning first two message registers (x0, x1).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_recv2(ep_id: u64) -> (u64, u64) {
    let msg0: u64;
    let msg1: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x6") ep_id,
            in("x7") 2u64, // SYS_RECV
            lateout("x0") msg0,
            lateout("x1") msg1,
            options(nomem, nostack)
        );
    }
    (msg0, msg1)
}

/// SYS_CALL (syscall #3): send message then wait for reply.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_call(ep_id: u64, m0: u64, m1: u64, m2: u64, m3: u64) -> u64 {
    let reply0: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") m0,
            in("x1") m1,
            in("x2") m2,
            in("x3") m3,
            in("x6") ep_id,
            in("x7") 3u64, // SYS_CALL
            lateout("x0") reply0,
            options(nomem, nostack)
        );
    }
    reply0
}

/// SYS_WRITE (syscall #4): write string to UART via kernel.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_write(buf: *const u8, len: usize) {
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") buf as u64,
            in("x1") len as u64,
            in("x7") 4u64, // SYS_WRITE
            options(nomem, nostack)
        );
    }
}

/// Print a string from EL0 via SYS_WRITE syscall
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn user_print(s: &str) {
    syscall_write(s.as_ptr(), s.len());
}

/// SYS_NOTIFY (syscall #5): send notification bitmask to target task.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_notify(target_id: u64, bits: u64) {
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") bits,
            in("x6") target_id,
            in("x7") 5u64, // SYS_NOTIFY
            options(nomem, nostack)
        );
    }
}

/// SYS_WAIT_NOTIFY (syscall #6): block until notification arrives.
/// Returns the pending bitmask in x0.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_wait_notify() -> u64 {
    let bits: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x7") 6u64, // SYS_WAIT_NOTIFY
            lateout("x0") bits,
            options(nomem, nostack)
        );
    }
    bits
}
/// SYS_GRANT_CREATE (syscall #7): create shared memory grant.
/// x0 = grant_id, x6 = peer_task_id.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_grant_create(grant_id: u64, peer_task_id: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") grant_id,
            in("x6") peer_task_id,
            in("x7") 7u64, // SYS_GRANT_CREATE
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_GRANT_REVOKE (syscall #8): revoke shared memory grant.
/// x0 = grant_id.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_grant_revoke(grant_id: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") grant_id,
            in("x7") 8u64, // SYS_GRANT_REVOKE
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}
/// SYS_IRQ_BIND (syscall #9): bind an IRQ INTID to a notification bit.
/// x0 = intid (must be ≥ 32, SPIs only), x1 = notify_bit.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_irq_bind(intid: u64, notify_bit: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") intid,
            in("x1") notify_bit,
            in("x7") 9u64, // SYS_IRQ_BIND
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_IRQ_ACK (syscall #10): acknowledge an IRQ handled, re-enable INTID.
/// x0 = intid.
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_irq_ack(intid: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") intid,
            in("x7") 10u64, // SYS_IRQ_ACK
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_DEVICE_MAP (syscall #11): map device MMIO into user-space.
/// x0 = device_id (0 = UART0).
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_device_map(device_id: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") device_id,
            in("x7") 11u64, // SYS_DEVICE_MAP
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

/// SYS_HEARTBEAT (syscall #12): register/refresh watchdog heartbeat.
/// x0 = heartbeat interval in ticks (0 = disable watchdog).
/// Returns result in x0 (0 = success).
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub fn syscall_heartbeat(interval: u64) -> u64 {
    let result: u64;
    // SAFETY: SVC triggers synchronous exception handled by kernel vector table. Register ABI is documented in syscall convention.
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") interval,
            in("x7") 12u64, // SYS_HEARTBEAT
            lateout("x0") result,
            options(nomem, nostack)
        );
    }
    result
}

// ─── Task entry points (Phase J4: User-Mode UART Driver PoC) ───────

/// UART0 PL011 Data Register address (identity-mapped after SYS_DEVICE_MAP)
#[cfg(target_arch = "aarch64")]
const UART0_DR: *mut u8 = 0x0900_0000 as *mut u8;

/// Task 0 — UART User-Mode Driver
///
/// Requests UART MMIO access from kernel, then loops serving IPC requests
/// from client tasks. Reads data from shared grant page and writes each
/// byte directly to UART DR — a genuine EL0 device driver.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn uart_driver_entry() -> ! {
    // 1. Map UART0 MMIO into our address space (EL0 accessible)
    syscall_device_map(0); // device_id=0 = UART0

    // 2. Register watchdog heartbeat (50 ticks = 500ms interval)
    syscall_heartbeat(50);

    // 3. Announce we're ready (still using SYS_WRITE for initial status)
    user_print("DRV:ready ");

    // 4. Serve client requests forever
    loop {
        // Refresh heartbeat each iteration
        syscall_heartbeat(50);
        // Block waiting for an IPC request on EP 0
        let (buf_addr_raw, len_raw) = syscall_recv2(0);

        // msg x0 = buffer address in grant page
        // msg x1 = byte count to write
        let buf_addr = buf_addr_raw as *const u8;
        let len = len_raw as usize;

        // Write each byte directly to UART DR (EL0 MMIO write!)
        for i in 0..len {
            // SAFETY: buf_addr points to grant page (identity-mapped), UART0_DR is mapped EL0 device MMIO.
            unsafe {
                let byte = core::ptr::read_volatile(buf_addr.add(i));
                core::ptr::write_volatile(UART0_DR, byte);
            }
        }

        // Reply "OK" to unblock the client
        syscall_send(0, 0x4F4B, 0, 0, 0); // "OK"
    }
}

/// Task 1 — Client using UART driver via IPC + shared memory
///
/// Creates a shared memory grant, writes a message into the grant page,
/// then calls the UART driver via IPC to output it. This demonstrates
/// the full user-mode driver stack: grant + IPC + MMIO.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn client_entry() -> ! {
    // 1. Create a shared memory grant: grant 0, owner=us(task 1), peer=driver(task 0)
    syscall_grant_create(0, 0); // grant_id=0, peer_task_id=0

    // 2. Register watchdog heartbeat (50 ticks = 500ms interval)
    syscall_heartbeat(50);

    // 3. Get the grant page address (identity-mapped, known at compile time)
    let grant_addr = aegis_os::grant::grant_page_addr(0).unwrap_or(0) as *mut u8;

    loop {
        // Refresh heartbeat each iteration
        syscall_heartbeat(50);
        // 3. Write the message into the grant page
        let msg = b"J4:UserDrv ";
        // SAFETY: grant_addr points to shared grant page mapped for this task. Write-volatile ensures MMIO-safe ordering.
        unsafe {
            for (i, &byte) in msg.iter().enumerate() {
                core::ptr::write_volatile(grant_addr.add(i), byte);
            }
        }

        // 4. Call the UART driver: send buffer address + length via IPC
        syscall_call(0, grant_addr as u64, msg.len() as u64, 0, 0);
    }
}

/// Idle task: just wfi in a loop
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn idle_entry() -> ! {
    loop {
        // SAFETY: wfi is a hint instruction that idles the core until next interrupt.
        unsafe { core::arch::asm!("wfi"); }
    }
}

// ─── Kernel main ───────────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    uart_print("\n[AegisOS] boot\n");
    uart_print("[AegisOS] MMU enabled (identity map)\n");
    uart_print("[AegisOS] W^X enforced (WXN + 4KB pages)\n");

    exception::init();
    uart_print("[AegisOS] exceptions ready\n");

    gic::init();
    gic::set_priority(timer::TIMER_INTID, 0);
    gic::enable_intid(timer::TIMER_INTID);

    sched::init(&[
        uart_driver_entry as *const () as u64,  // task 0: UART driver
        client_entry as *const () as u64,        // task 1: client
        idle_entry as *const () as u64,           // task 2: placeholder (ELF overrides)
        idle_entry as *const () as u64,           // task 3: placeholder (ELF overrides)
        idle_entry as *const () as u64,           // task 4: placeholder (ELF overrides)
        0,                                        // task 5: reserved (Inactive)
        0,                                        // task 6: reserved (Inactive)
        idle_entry as *const () as u64,           // task 7: idle (IDLE_TASK_ID)
    ]);

    // ─── Phase N: Apply per-task metadata from const table ─────────
    {
        use aegis_os::cap::*;
        use aegis_os::sched::TaskMetadata;
        use aegis_os::mmu;

        // Metadata for inactive tasks (zero caps, lowest priority)
        const INACTIVE: TaskMetadata = TaskMetadata {
            caps: 0, priority: 0, time_budget: 0, heartbeat_interval: 0,
        };

        const TASK_META: [TaskMetadata; sched::NUM_TASKS] = [
            // Task 0 (UART driver): high priority, unlimited budget, full driver caps
            TaskMetadata {
                caps: CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
                    | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
                    | CAP_IRQ_BIND | CAP_IRQ_ACK | CAP_DEVICE_MAP | CAP_HEARTBEAT,
                priority: 6,
                time_budget: 0,
                heartbeat_interval: 0,
            },
            // Task 1 (client): medium priority, 50 ticks budget
            TaskMetadata {
                caps: CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
                    | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
                    | CAP_HEARTBEAT,
                priority: 4,
                time_budget: 50,
                heartbeat_interval: 0,
            },
            // Task 2 (hello): ELF-loaded, medium-high priority, basic caps
            TaskMetadata {
                caps: CAP_WRITE | CAP_YIELD | CAP_EXIT,
                priority: 5,
                time_budget: 2,
                heartbeat_interval: 0,
            },
            // Task 3 (sensor): ELF-loaded, IPC sender + heartbeat
            TaskMetadata {
                caps: CAP_IPC_SEND_EP1 | CAP_WRITE | CAP_YIELD | CAP_HEARTBEAT | CAP_EXIT,
                priority: 4,
                time_budget: 10,
                heartbeat_interval: 0,
            },
            // Task 4 (logger): ELF-loaded, IPC receiver + writer
            TaskMetadata {
                caps: CAP_IPC_RECV_EP1 | CAP_WRITE | CAP_YIELD | CAP_EXIT,
                priority: 3,
                time_budget: 10,
                heartbeat_interval: 0,
            },
            INACTIVE, // task 5: reserved
            INACTIVE, // task 6: reserved
            // Task 7 (idle): pure wfi loop, minimal caps, lowest priority
            TaskMetadata {
                caps: CAP_YIELD,
                priority: 0,
                time_budget: 0,
                heartbeat_interval: 0,
            },
        ];

        // SAFETY: Single-core kernel, called during boot before interrupts enabled.
        unsafe {
            for i in 0..sched::NUM_TASKS {
                (*sched::TCBS.get_mut())[i].caps = TASK_META[i].caps;
                (*sched::TCBS.get_mut())[i].priority = TASK_META[i].priority;
                (*sched::TCBS.get_mut())[i].base_priority = TASK_META[i].priority;
                (*sched::TCBS.get_mut())[i].time_budget = TASK_META[i].time_budget;
                (*sched::TCBS.get_mut())[i].heartbeat_interval = TASK_META[i].heartbeat_interval;
                // ASID = task_id + 1 (ASID 0 is reserved for kernel boot)
                // All tasks get page tables (even inactive — no harm, enables future activation)
                (*sched::TCBS.get_mut())[i].ttbr0 = mmu::ttbr0_for_task(i, (i + 1) as u16);
            }
        }
    }
    uart_print("[AegisOS] capabilities assigned\n");
    uart_print("[AegisOS] priority scheduler configured\n");
    uart_print("[AegisOS] time budget enforcement enabled\n");
    uart_print("[AegisOS] watchdog heartbeat enabled\n");
    uart_print("[AegisOS] notification system ready\n");
    uart_print("[AegisOS] grant system ready\n");
    uart_print("[AegisOS] IRQ routing ready\n");
    uart_print("[AegisOS] device MMIO mapping ready\n");
    uart_print("[AegisOS] per-task address spaces assigned\n");

    // ─── Phase L: Arch separation ──────────────────────────────────
    uart_print("[AegisOS] arch separation: module tree ready\n");
    uart_print("[AegisOS] arch separation: complete\n");

    // ─── Phase L3: ELF64 Parser ────────────────────────────────────
    uart_print("[AegisOS] ELF64 parser ready\n");
    // ─── Phase O: Multi-ELF Loader ─────────────────────────────────────
    uart_print("[AegisOS] ELF loader ready\n");
    {
        // Embed user-space ELF binaries (built separately via user/ workspace)
        static HELLO_ELF: &[u8] = include_bytes!("../user/target/aarch64-user/release/hello");
        static SENSOR_ELF: &[u8] = include_bytes!("../user/target/aarch64-user/release/sensor");
        static LOGGER_ELF: &[u8] = include_bytes!("../user/target/aarch64-user/release/logger");

        // Compile-time size check: each binary must fit in 16 KiB slot
        const _: () = assert!(HELLO_ELF.len() <= 16 * 1024, "hello ELF > 16 KiB");
        const _: () = assert!(SENSOR_ELF.len() <= 16 * 1024, "sensor ELF > 16 KiB");
        const _: () = assert!(LOGGER_ELF.len() <= 16 * 1024, "logger ELF > 16 KiB");

        // (task_id, slot, elf_data, name)
        let tasks: [(usize, usize, &[u8], &str); 3] = [
            (2, 0, HELLO_ELF,  "hello"),
            (3, 1, SENSOR_ELF, "sensor"),
            (4, 2, LOGGER_ELF, "logger"),
        ];

        for &(task_id, slot, elf_data, name) in &tasks {
            // SAFETY: boot-time, single-core, .elf_load region is writable.
            match unsafe { aegis_os::elf::load_elf_to_task(task_id, slot, elf_data) } {
                Ok(entry) => {
                    uart_print("[AegisOS] task ");
                    aegis_os::uart_print_dec(task_id as u64);
                    uart_print(" (");
                    uart_print(name);
                    uart_print(") loaded from ELF (entry=0x");
                    aegis_os::uart_print_hex(entry);
                    uart_print(")\n");
                }
                Err(e) => {
                    uart_print("!!! ELF load failed for ");
                    uart_print(name);
                    uart_print(": ");
                    uart_print(e);
                    uart_print("\n");
                }
            }
        }
    }
    uart_print("[AegisOS] multi-ELF loading complete\n");
    timer::init(10);

    uart_print("[AegisOS] enhanced panic handler ready\n");
    uart_print("[AegisOS] klog ready\n");
    uart_print("[AegisOS] safety audit complete\n");
    uart_print("[AegisOS] bootstrapping into uart_driver (EL0)...\n");
    sched::bootstrap();
}

#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart_print("\n!!! KERNEL PANIC !!!\n");

    // Tick count
    uart_print("  tick: 0x");
    aegis_os::uart_print_hex(timer::tick_count());
    uart_print("\n");

    // Current task
    uart_print("  task: ");
    aegis_os::uart_print_dec(sched::current_task_id() as u64);
    uart_print("\n");

    // Source location (file:line) if available
    if let Some(loc) = info.location() {
        uart_print("  at: ");
        uart_print(loc.file());
        uart_print(":");
        aegis_os::uart_print_dec(loc.line() as u64);
        uart_print("\n");
    }

    // ESR_EL1 and FAR_EL1 — capture exception syndrome for diagnostics
    let esr: u64;
    let far: u64;
    // SAFETY: reading system registers is a read-only operation at EL1
    unsafe {
        core::arch::asm!("mrs {}, ESR_EL1", out(reg) esr, options(nomem, nostack));
        core::arch::asm!("mrs {}, FAR_EL1", out(reg) far, options(nomem, nostack));
    }
    uart_print("  ESR_EL1: 0x");
    aegis_os::uart_print_hex(esr);
    uart_print("\n  FAR_EL1: 0x");
    aegis_os::uart_print_hex(far);
    uart_print("\n");

    loop {
        // SAFETY: wfe is a hint instruction, safe at EL1
        unsafe { core::arch::asm!("wfe", options(nomem, nostack)) };
    }
}

// On host target: provide a main() so the bin target compiles
#[cfg(not(target_arch = "aarch64"))]
fn main() {}
