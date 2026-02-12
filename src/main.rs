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

    sched::init(
        uart_driver_entry as *const () as u64,
        client_entry as *const () as u64,
        idle_entry as *const () as u64,
    );

    // ─── Phase G: Assign capabilities ──────────────────────────────
    unsafe {
        use aegis_os::cap::*;
        // Task 0 (UART driver): needs RECV/SEND on EP0 + WRITE + YIELD + notifications + grants + IRQ + device map + heartbeat
        sched::TCBS[0].caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
            | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
            | CAP_IRQ_BIND | CAP_IRQ_ACK | CAP_DEVICE_MAP | CAP_HEARTBEAT;
        // Task 1 (client): needs CALL on EP0 + WRITE + YIELD + notifications + grants + heartbeat
        sched::TCBS[1].caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0 | CAP_WRITE | CAP_YIELD
            | CAP_NOTIFY | CAP_WAIT_NOTIFY | CAP_GRANT_CREATE | CAP_GRANT_REVOKE
            | CAP_HEARTBEAT;
        // Task 2 (idle / ELF-loaded): needs YIELD + WRITE (L5 ELF task uses SYS_WRITE)
        sched::TCBS[2].caps = CAP_YIELD | CAP_WRITE;
    }
    uart_print("[AegisOS] capabilities assigned\n");

    // ─── Phase K: Assign priorities and time budgets ────────────────
    unsafe {
        // Task 0 (UART driver): high priority, unlimited budget
        sched::TCBS[0].priority = 6;
        sched::TCBS[0].base_priority = 6;
        sched::TCBS[0].time_budget = 0; // unlimited

        // Task 1 (client): medium priority, 50 ticks budget per epoch
        sched::TCBS[1].priority = 4;
        sched::TCBS[1].base_priority = 4;
        sched::TCBS[1].time_budget = 50;

        // Task 2 (ELF demo): runs briefly at boot to prove ELF loading,
        // then defers to task 1 once its 2-tick budget is exhausted.
        sched::TCBS[2].priority = 5;
        sched::TCBS[2].base_priority = 5;
        sched::TCBS[2].time_budget = 2; // 2 ticks, then defer
    }
    uart_print("[AegisOS] priority scheduler configured\n");
    uart_print("[AegisOS] time budget enforcement enabled\n");
    uart_print("[AegisOS] watchdog heartbeat enabled\n");
    uart_print("[AegisOS] notification system ready\n");
    uart_print("[AegisOS] grant system ready\n");
    uart_print("[AegisOS] IRQ routing ready\n");
    uart_print("[AegisOS] device MMIO mapping ready\n");

    // ─── Phase H: Assign per-task address spaces ───────────────────
    unsafe {
        use aegis_os::mmu;
        // ASID = task_id + 1 (ASID 0 is reserved for kernel boot)
        sched::TCBS[0].ttbr0 = mmu::ttbr0_for_task(0, 1);
        sched::TCBS[1].ttbr0 = mmu::ttbr0_for_task(1, 2);
        sched::TCBS[2].ttbr0 = mmu::ttbr0_for_task(2, 3);
    }
    uart_print("[AegisOS] per-task address spaces assigned\n");

    // ─── Phase L: Arch separation ──────────────────────────────────
    uart_print("[AegisOS] arch separation: module tree ready\n");
    uart_print("[AegisOS] arch separation: complete\n");

    // ─── Phase L3: ELF64 Parser ────────────────────────────────────
    uart_print("[AegisOS] ELF64 parser ready\n");
    // ─── Phase L4/L5: ELF Loader + Demo Binary ─────────────────────────
    uart_print("[AegisOS] ELF loader ready\n");
    {
        extern "C" {
            static __elf_load_start: u8;
            static __elf_load_end: u8;
        }
        let load_base = unsafe { &__elf_load_start as *const u8 as u64 };
        let load_size = unsafe {
            (&__elf_load_end as *const u8 as u64) - load_base
        } as usize;

        // Phase L5: embed real user-space ELF binary (built separately)
        static USER_ELF: &[u8] = include_bytes!("../user/hello/target/aarch64-user/release/hello");

        // Parse the embedded ELF
        if let Ok(info) = aegis_os::elf::parse_elf64(USER_ELF) {
            let dest = unsafe { &__elf_load_start as *const u8 as *mut u8 };
            let result = unsafe {
                aegis_os::elf::load_elf_segments(USER_ELF, &info, dest, load_base, load_size)
            };
            if let Ok(entry) = result {
                // Cache maintenance: flush D-cache, invalidate I-cache
                unsafe {
                    let mut addr = load_base;
                    let end = load_base + load_size as u64;
                    while addr < end {
                        core::arch::asm!(
                            "dc cvau, {addr}",
                            addr = in(reg) addr,
                            options(nomem, nostack)
                        );
                        addr += 64;
                    }
                    core::arch::asm!(
                        "dsb ish",
                        "ic iallu",
                        "dsb ish",
                        "isb",
                        options(nomem, nostack)
                    );
                }
                // Set page permissions per segment
                unsafe {
                    use aegis_os::mmu;
                    for i in 0..info.num_segments {
                        if let Some(seg) = info.segments[i] {
                            let template = if seg.flags & aegis_os::elf::PF_X != 0 {
                                mmu::USER_CODE_PAGE
                            } else if seg.flags & aegis_os::elf::PF_W != 0 {
                                mmu::USER_DATA_PAGE
                            } else {
                                mmu::KERNEL_RODATA_PAGE
                            };
                            let seg_start = seg.vaddr & !0xFFF;
                            let seg_end = (seg.vaddr + seg.memsz + 0xFFF) & !0xFFF;
                            let mut page = seg_start;
                            while page < seg_end {
                                mmu::set_page_attr(2, page, template);
                                page += 4096;
                            }
                        }
                    }
                }
                // Override task 2 with ELF-loaded entry point
                unsafe {
                    sched::TCBS[2].entry_point = entry;
                    sched::TCBS[2].context.elr_el1 = entry;
                }
                uart_print("[AegisOS] task 2 loaded from ELF (entry=0x");
                aegis_os::uart_print_hex(entry);
                uart_print(")\n");
                uart_print("[AegisOS] client task loaded from ELF binary\n");
            }
        }
    }
    timer::init(10);

    uart_print("[AegisOS] enhanced panic handler ready\n");
    uart_print("[AegisOS] klog ready\n");
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
