// AegisOS — Kernel binary entry point
// This entire file is AArch64-only. When building for host tests (x86_64),
// the content is gated off and only the lib crate is tested.

// On AArch64: full kernel binary with boot asm, syscall wrappers, tasks
#![cfg_attr(target_arch = "aarch64", no_std)]
#![cfg_attr(target_arch = "aarch64", no_main)]

// On host (x86_64): empty bin that does nothing (tests use --lib --test)
#![cfg_attr(not(target_arch = "aarch64"), allow(unused))]

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
        // Task 2 (idle): only needs YIELD (WFI loop)
        sched::TCBS[2].caps = CAP_YIELD;
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

        // Task 2 (idle): lowest priority, unlimited budget
        sched::TCBS[2].priority = 0;
        sched::TCBS[2].base_priority = 0;
        sched::TCBS[2].time_budget = 0; // unlimited
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
    // ─── Phase L4: ELF Loader ──────────────────────────────────────────
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

        // AArch64 yield loop: mov x7,#0 / svc #0 / b -8
        static YIELD_CODE: [u8; 12] = [
            0x07, 0x00, 0x80, 0xd2, // mov x7, #0 (SYS_YIELD)
            0x01, 0x00, 0x00, 0xd4, // svc #0
            0xfe, 0xff, 0xff, 0x17, // b -8
        ];

        // Build synthetic ELF64: header (64B) + 1 PT_LOAD phdr (56B) + code (12B)
        let mut elf = [0u8; 256];
        // ELF header
        elf[0] = 0x7f; elf[1] = b'E'; elf[2] = b'L'; elf[3] = b'F';
        elf[4] = 2;     // ELFCLASS64
        elf[5] = 1;     // ELFDATA2LSB
        elf[6] = 1;     // EV_CURRENT
        elf[16] = 2;    // e_type = ET_EXEC
        elf[18] = 0xB7; // e_machine = EM_AARCH64 (183)
        elf[20] = 1;    // e_version
        // e_entry = load_base
        let vb = load_base.to_le_bytes();
        elf[24] = vb[0]; elf[25] = vb[1]; elf[26] = vb[2]; elf[27] = vb[3];
        elf[28] = vb[4]; elf[29] = vb[5]; elf[30] = vb[6]; elf[31] = vb[7];
        elf[32] = 64;   // e_phoff = 64
        elf[52] = 64;   // e_ehsize = 64
        elf[54] = 56;   // e_phentsize = 56
        elf[56] = 1;    // e_phnum = 1
        // Program header (56 bytes at offset 64)
        elf[64] = 1;    // p_type = PT_LOAD
        elf[68] = 5;    // p_flags = PF_R | PF_X
        elf[72] = 120;  // p_offset = 120 (code at byte 120)
        // p_vaddr = load_base
        elf[80] = vb[0]; elf[81] = vb[1]; elf[82] = vb[2]; elf[83] = vb[3];
        elf[84] = vb[4]; elf[85] = vb[5]; elf[86] = vb[6]; elf[87] = vb[7];
        // p_paddr = load_base
        elf[88] = vb[0]; elf[89] = vb[1]; elf[90] = vb[2]; elf[91] = vb[3];
        elf[92] = vb[4]; elf[93] = vb[5]; elf[94] = vb[6]; elf[95] = vb[7];
        elf[96] = 12;   // p_filesz = 12
        // p_memsz = 4096 (full page, BSS-zeroed)
        elf[104] = 0x00; elf[105] = 0x10;
        // Code bytes at offset 120
        let mut ci = 0;
        while ci < YIELD_CODE.len() {
            elf[120 + ci] = YIELD_CODE[ci];
            ci += 1;
        }

        // Parse the synthetic ELF
        if let Ok(info) = aegis_os::elf::parse_elf64(&elf) {
            let dest = unsafe { &__elf_load_start as *const u8 as *mut u8 };
            let result = unsafe {
                aegis_os::elf::load_elf_segments(&elf, &info, dest, load_base, load_size)
            };
            if result.is_ok() {
                // Cache maintenance: flush D-cache, invalidate I-cache
                unsafe {
                    let mut addr = load_base;
                    let end = load_base + 4096;
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
                // Set page permissions: USER_CODE_PAGE for task 2
                unsafe {
                    use aegis_os::mmu;
                    mmu::set_page_attr(2, load_base, mmu::USER_CODE_PAGE);
                }
                // Override idle task (task 2) with ELF-loaded entry point
                unsafe {
                    sched::TCBS[2].entry_point = load_base;
                    sched::TCBS[2].context.elr_el1 = load_base;
                }
                uart_print("[AegisOS] task 2 loaded from ELF (entry=0x");
                aegis_os::uart_print_hex(load_base);
                uart_print(")\n");
            }
        }
    }
    timer::init(10);

    uart_print("[AegisOS] bootstrapping into uart_driver (EL0)...\n");
    sched::bootstrap();
}

#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    uart_print("PANIC\n");
    loop {}
}

// On host target: provide a main() so the bin target compiles
#[cfg(not(target_arch = "aarch64"))]
fn main() {}
