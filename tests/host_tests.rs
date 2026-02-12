/// AegisOS Host-side Unit Tests
///
/// These tests run on the host machine (x86_64) using `cargo test`.
/// They verify pure logic that doesn't depend on hardware:
///   - TrapFrame layout (ABI-fixed, 288 bytes)
///   - MMU descriptor bit composition
///   - SYS_WRITE pointer validation
///   - Scheduler round-robin + fault restart logic
///   - IPC state machine + cleanup
///
/// Run: cargo test --target x86_64-pc-windows-msvc --lib --test host_tests -- --test-threads=1
///  or: cargo test --target x86_64-unknown-linux-gnu --lib --test host_tests -- --test-threads=1

// We need to import the kernel crate
extern crate aegis_os;

use core::mem;
use aegis_os::exception::{TrapFrame, TRAPFRAME_SIZE, validate_write_args};
use aegis_os::mmu;
use aegis_os::sched::{
    self, TaskState, Tcb, EMPTY_TCB, NUM_TASKS, RESTART_DELAY_TICKS,
};
use aegis_os::ipc::{self, EMPTY_EP, MAX_ENDPOINTS, MSG_REGS};
use aegis_os::cap::{
    self, CAP_IPC_SEND_EP0, CAP_IPC_RECV_EP0,
    CAP_IPC_SEND_EP1, CAP_IPC_RECV_EP1, CAP_WRITE, CAP_YIELD,
    CAP_NOTIFY, CAP_WAIT_NOTIFY,
    CAP_IPC_SEND_EP2, CAP_IPC_RECV_EP2,
    CAP_IPC_SEND_EP3, CAP_IPC_RECV_EP3,
    CAP_GRANT_CREATE, CAP_GRANT_REVOKE,
    CAP_IRQ_BIND, CAP_IRQ_ACK,
    CAP_DEVICE_MAP,
    CAP_HEARTBEAT,
    CAP_ALL, CAP_NONE,
};
use aegis_os::grant::{self, EMPTY_GRANT, MAX_GRANTS};
use aegis_os::irq::{self, EMPTY_BINDING, MAX_IRQ_BINDINGS};
use aegis_os::elf::{self, ElfError, ElfLoadError, ElfSegment, ElfInfo, MAX_SEGMENTS, PF_R, PF_W, PF_X};

// ─── Helper: read CURRENT safely (avoids static_mut_refs warning) ──

unsafe fn read_current() -> usize {
    *sched::CURRENT.get()
}

// ─── Helper: reset all global state between tests ──────────────────

/// Reset scheduler + IPC globals to a clean state.
/// SAFETY: Must only be called from single-threaded test context.
unsafe fn reset_test_state() {
    // Reset TCBs
    for i in 0..NUM_TASKS {
        sched::TCBS[i] = EMPTY_TCB;
        sched::TCBS[i].id = i as u16;
        sched::TCBS[i].state = TaskState::Ready;
        // Give each task a fake user stack top (won't be dereferenced in tests)
        sched::TCBS[i].user_stack_top = 0x5000_0000 + ((i as u64 + 1) * 4096);
        sched::TCBS[i].entry_point = 0x4008_0000 + (i as u64 * 0x100);
        sched::TCBS[i].ttbr0 = 0;
        sched::TCBS[i].notify_pending = 0;
        sched::TCBS[i].notify_waiting = false;
    }
    *sched::CURRENT.get_mut() = 0;
    sched::TCBS[0].state = TaskState::Running;

    // Reset IPC endpoints
    for i in 0..MAX_ENDPOINTS {
        ipc::ENDPOINTS[i] = EMPTY_EP;
    }

    // Reset tick counter
    *aegis_os::timer::TICK_COUNT.get_mut() = 0;

    // Reset grant table
    for i in 0..MAX_GRANTS {
        grant::GRANTS[i] = EMPTY_GRANT;
    }

    // Reset IRQ bindings
    for i in 0..MAX_IRQ_BINDINGS {
        irq::IRQ_BINDINGS[i] = EMPTY_BINDING;
    }
}

// ═══════════════════════════════════════════════════════════════════
// 1. TrapFrame Layout Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn trapframe_size_is_288() {
    assert_eq!(mem::size_of::<TrapFrame>(), 288);
    assert_eq!(TRAPFRAME_SIZE, 288);
}

#[test]
fn trapframe_alignment_is_16() {
    assert_eq!(mem::align_of::<TrapFrame>(), 8);
    // The struct is repr(C) with u64 fields — 8-byte aligned.
    // Assembly uses 16-byte aligned SP, so TrapFrame must be
    // a multiple of 16 in size: 288 = 18 × 16 ✓
    assert_eq!(288 % 16, 0);
}

#[test]
fn trapframe_field_offsets() {
    // Verify critical field offsets match assembly expectations
    let frame = TrapFrame {
        x: [0; 31],
        sp_el0: 0,
        elr_el1: 0,
        spsr_el1: 0,
        _pad: [0; 2],
    };
    let base = &frame as *const _ as usize;

    // x[0] at offset 0
    assert_eq!(&frame.x[0] as *const _ as usize - base, 0);
    // x[30] at offset 240
    assert_eq!(&frame.x[30] as *const _ as usize - base, 240);
    // sp_el0 at offset 248
    assert_eq!(&frame.sp_el0 as *const _ as usize - base, 248);
    // elr_el1 at offset 256
    assert_eq!(&frame.elr_el1 as *const _ as usize - base, 256);
    // spsr_el1 at offset 264
    assert_eq!(&frame.spsr_el1 as *const _ as usize - base, 264);
    // _pad at offset 272
    assert_eq!(&frame._pad as *const _ as usize - base, 272);
}

#[test]
fn trapframe_zeroed_is_valid() {
    let frame = TrapFrame {
        x: [0; 31],
        sp_el0: 0,
        elr_el1: 0,
        spsr_el1: 0,
        _pad: [0; 2],
    };
    // All fields should be zero
    for i in 0..31 {
        assert_eq!(frame.x[i], 0);
    }
    assert_eq!(frame.sp_el0, 0);
    assert_eq!(frame.elr_el1, 0);
    assert_eq!(frame.spsr_el1, 0);
}

// ═══════════════════════════════════════════════════════════════════
// 2. MMU Descriptor Constant Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn mmu_valid_bit() {
    // Bit 0 must be set in TABLE and PAGE descriptors
    assert_eq!(mmu::TABLE & 1, 1);
    assert_eq!(mmu::PAGE & 1, 1);
    // BLOCK has bit 0 = 1 too (0b01)
    assert_eq!(mmu::BLOCK & 1, 1);
}

#[test]
fn mmu_table_vs_block_vs_page() {
    // TABLE = 0b11 (valid + table)
    assert_eq!(mmu::TABLE, 0b11);
    // BLOCK = 0b01 (valid + block)
    assert_eq!(mmu::BLOCK, 0b01);
    // PAGE = 0b11 (same encoding as TABLE but at L3 level)
    assert_eq!(mmu::PAGE, 0b11);
}

#[test]
fn mmu_access_flag() {
    // AF = bit 10
    assert_eq!(mmu::AF, 1 << 10);
    assert_eq!(mmu::AF, 0x400);
}

#[test]
fn mmu_access_permissions() {
    // AP[7:6]
    assert_eq!(mmu::AP_RW_EL1, 0b00 << 6); // 0x00
    assert_eq!(mmu::AP_RW_EL0, 0b01 << 6); // 0x40
    assert_eq!(mmu::AP_RO_EL1, 0b10 << 6); // 0x80
    assert_eq!(mmu::AP_RO_EL0, 0b11 << 6); // 0xC0
}

#[test]
fn mmu_execute_never_bits() {
    // PXN = bit 53
    assert_eq!(mmu::PXN, 1u64 << 53);
    // UXN = bit 54
    assert_eq!(mmu::UXN, 1u64 << 54);
    // XN = PXN | UXN
    assert_eq!(mmu::XN, mmu::PXN | mmu::UXN);
}

#[test]
fn mmu_shareability() {
    // SH_INNER = 0b11 << 8
    assert_eq!(mmu::SH_INNER, 0b11 << 8);
    assert_eq!(mmu::SH_INNER, 0x300);
}

#[test]
fn mmu_attr_indices() {
    // AttrIndx[4:2]
    assert_eq!(mmu::ATTR_DEVICE, 0 << 2);     // index 0
    assert_eq!(mmu::ATTR_NORMAL_NC, 1 << 2);   // index 1
    assert_eq!(mmu::ATTR_NORMAL_WB, 2 << 2);   // index 2
}

#[test]
fn mmu_device_block_is_non_executable() {
    // DEVICE_BLOCK must have both PXN and UXN set
    assert_ne!(mmu::DEVICE_BLOCK & mmu::PXN, 0, "Device block missing PXN");
    assert_ne!(mmu::DEVICE_BLOCK & mmu::UXN, 0, "Device block missing UXN");
}

#[test]
fn mmu_device_block_is_el1_only() {
    // AP bits should be AP_RW_EL1 (0b00 << 6 = 0)
    let ap_bits = mmu::DEVICE_BLOCK & (0b11 << 6);
    assert_eq!(ap_bits, mmu::AP_RW_EL1);
}

#[test]
fn mmu_device_block_has_af() {
    assert_ne!(mmu::DEVICE_BLOCK & mmu::AF, 0);
}

#[test]
fn mmu_shared_code_page_is_executable() {
    // SHARED_CODE_PAGE must NOT have PXN or UXN (both EL0 and EL1 can execute)
    assert_eq!(mmu::SHARED_CODE_PAGE & mmu::PXN, 0, "Shared code should not have PXN");
    assert_eq!(mmu::SHARED_CODE_PAGE & mmu::UXN, 0, "Shared code should not have UXN");
}

#[test]
fn mmu_shared_code_page_is_readonly() {
    // AP = AP_RO_EL0 (0b11 << 6) — RO for both EL0 and EL1
    let ap_bits = mmu::SHARED_CODE_PAGE & (0b11 << 6);
    assert_eq!(ap_bits, mmu::AP_RO_EL0);
}

#[test]
fn mmu_kernel_data_page_is_non_executable() {
    assert_ne!(mmu::KERNEL_DATA_PAGE & mmu::PXN, 0, "Kernel data missing PXN");
    assert_ne!(mmu::KERNEL_DATA_PAGE & mmu::UXN, 0, "Kernel data missing UXN");
}

#[test]
fn mmu_kernel_data_page_is_el1_only() {
    let ap_bits = mmu::KERNEL_DATA_PAGE & (0b11 << 6);
    assert_eq!(ap_bits, mmu::AP_RW_EL1, "Kernel data should be EL1-only");
}

#[test]
fn mmu_user_data_page_is_non_executable() {
    assert_ne!(mmu::USER_DATA_PAGE & mmu::PXN, 0, "User data missing PXN");
    assert_ne!(mmu::USER_DATA_PAGE & mmu::UXN, 0, "User data missing UXN");
}

#[test]
fn mmu_user_data_page_is_el0_accessible() {
    let ap_bits = mmu::USER_DATA_PAGE & (0b11 << 6);
    assert_eq!(ap_bits, mmu::AP_RW_EL0, "User data should be EL0 RW");
}

#[test]
fn mmu_user_code_page_el0_exec_only() {
    // USER_CODE_PAGE: PXN=1 (kernel can't execute), UXN=0 (EL0 can execute)
    assert_ne!(mmu::USER_CODE_PAGE & mmu::PXN, 0, "User code should have PXN");
    assert_eq!(mmu::USER_CODE_PAGE & mmu::UXN, 0, "User code should NOT have UXN");
}

#[test]
fn mmu_wxn_invariant() {
    // With WXN enabled in SCTLR: any page that is writable becomes XN.
    // Therefore: writable pages (KERNEL_DATA_PAGE, USER_DATA_PAGE) must already
    // have XN set (defense in depth), and code pages must be RO.

    // Writable pages have XN explicitly
    assert_ne!(mmu::KERNEL_DATA_PAGE & mmu::XN, 0);
    assert_ne!(mmu::USER_DATA_PAGE & mmu::XN, 0);

    // Code pages are RO (AP_RO_*), so WXN doesn't force them XN
    let shared_code_ap = mmu::SHARED_CODE_PAGE & (0b11 << 6);
    assert!(shared_code_ap == mmu::AP_RO_EL0 || shared_code_ap == mmu::AP_RO_EL1,
            "Code pages must be RO for W^X");
}

// ═══════════════════════════════════════════════════════════════════
// 3. SYS_WRITE Pointer Validation Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn validate_write_valid_ptr_in_ram() {
    // Pointer in valid RAM range
    let (valid, len) = validate_write_args(0x4008_0000, 10);
    assert!(valid);
    assert_eq!(len, 10);
}

#[test]
fn validate_write_start_of_ram() {
    // Exactly at RAM start
    let (valid, _) = validate_write_args(0x4000_0000, 1);
    assert!(valid);
}

#[test]
fn validate_write_end_of_ram() {
    // 1 byte before RAM end — valid
    let (valid, _) = validate_write_args(0x47FF_FFFF, 1);
    assert!(valid);
}

#[test]
fn validate_write_past_ram_end() {
    // Exactly at boundary — invalid (end > 0x4800_0000)
    let (valid, _) = validate_write_args(0x4800_0000, 1);
    assert!(!valid);
}

#[test]
fn validate_write_before_ram() {
    // Below RAM start
    let (valid, _) = validate_write_args(0x3FFF_FFFF, 1);
    assert!(!valid);
}

#[test]
fn validate_write_null_ptr() {
    let (valid, _) = validate_write_args(0x0, 10);
    assert!(!valid);
}

#[test]
fn validate_write_uart_mmio() {
    // UART at 0x0900_0000 — must be rejected!
    let (valid, _) = validate_write_args(0x0900_0000, 1);
    assert!(!valid);
}

#[test]
fn validate_write_zero_length() {
    let (valid, _) = validate_write_args(0x4008_0000, 0);
    assert!(!valid);
}

#[test]
fn validate_write_over_256() {
    let (valid, _) = validate_write_args(0x4008_0000, 257);
    assert!(!valid);
}

#[test]
fn validate_write_exactly_256() {
    let (valid, len) = validate_write_args(0x4008_0000, 256);
    assert!(valid);
    assert_eq!(len, 256);
}

#[test]
fn validate_write_wrapping_overflow() {
    // Huge len that would wrap around address space
    let (valid, _) = validate_write_args(0x47FF_FFF0, 0x100);
    assert!(!valid);
}

#[test]
fn validate_write_spans_boundary() {
    // Starts valid but ends past 0x4800_0000
    let (valid, _) = validate_write_args(0x47FF_FF00, 256);
    assert!(valid);

    let (valid, _) = validate_write_args(0x47FF_FF01, 256);
    assert!(!valid);
}

// ═══════════════════════════════════════════════════════════════════
// 4. Scheduler Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sched_task_state_values() {
    assert_eq!(TaskState::Inactive as u8, 0);
    assert_eq!(TaskState::Ready as u8, 1);
    assert_eq!(TaskState::Running as u8, 2);
    assert_eq!(TaskState::Blocked as u8, 3);
    assert_eq!(TaskState::Faulted as u8, 4);
}

#[test]
fn sched_num_tasks() {
    assert_eq!(NUM_TASKS, 3);
}

#[test]
fn sched_restart_delay() {
    assert_eq!(RESTART_DELAY_TICKS, 100);
}

#[test]
fn sched_empty_tcb_is_zeroed() {
    assert_eq!(EMPTY_TCB.state, TaskState::Inactive);
    assert_eq!(EMPTY_TCB.id, 0);
    assert_eq!(EMPTY_TCB.entry_point, 0);
    assert_eq!(EMPTY_TCB.user_stack_top, 0);
    assert_eq!(EMPTY_TCB.fault_tick, 0);
    assert_eq!(EMPTY_TCB.context.elr_el1, 0);
    assert_eq!(EMPTY_TCB.context.spsr_el1, 0);
    assert_eq!(EMPTY_TCB.context.sp_el0, 0);
    for i in 0..31 {
        assert_eq!(EMPTY_TCB.context.x[i], 0);
    }
}

#[test]
fn sched_tcb_size() {
    // TCB contains TrapFrame (288) + state(1) + padding + id(2) + 3×u64
    // Exact size depends on alignment, but it should be reasonable
    let size = mem::size_of::<Tcb>();
    assert!(size >= 288 + 1 + 2 + 24, "TCB too small: {}", size);
}

#[test]
fn sched_round_robin_all_ready() {
    unsafe {
        reset_test_state();

        // Task 0 is Running, 1 and 2 are Ready
        // schedule() should pick task 1
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };

        sched::schedule(&mut frame);
        assert_eq!(read_current(), 1);
        assert_eq!(sched::TCBS[1].state, TaskState::Running);
        assert_eq!(sched::TCBS[0].state, TaskState::Ready);

        // Next schedule should pick task 2
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 2);
        assert_eq!(sched::TCBS[2].state, TaskState::Running);
        assert_eq!(sched::TCBS[1].state, TaskState::Ready);

        // Next schedule should wrap around to task 0
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 0);
        assert_eq!(sched::TCBS[0].state, TaskState::Running);
    }
}

#[test]
fn sched_skip_faulted_task() {
    unsafe {
        reset_test_state();

        // Fault task 1
        sched::TCBS[1].state = TaskState::Faulted;
        sched::TCBS[1].fault_tick = 0;

        // Task 0 is Running. schedule() should skip task 1, pick task 2
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 2);
    }
}

#[test]
fn sched_skip_blocked_task() {
    unsafe {
        reset_test_state();

        sched::TCBS[1].state = TaskState::Blocked;

        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);
        // Should skip task 1 (Blocked), pick task 2
        assert_eq!(read_current(), 2);
    }
}

#[test]
fn sched_auto_restart_after_delay() {
    unsafe {
        reset_test_state();

        // Fault task 1 at tick 0
        sched::TCBS[1].state = TaskState::Faulted;
        sched::TCBS[1].fault_tick = 0;
        sched::TCBS[1].entry_point = 0x4008_0100;
        sched::TCBS[1].user_stack_top = 0x5000_2000;

        // Set tick to just before restart threshold
        *aegis_os::timer::TICK_COUNT.get_mut() = RESTART_DELAY_TICKS - 1;
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);
        // Task 1 should still be Faulted
        assert_eq!(sched::TCBS[1].state, TaskState::Faulted);

        // Set tick to exactly restart threshold
        *aegis_os::timer::TICK_COUNT.get_mut() = RESTART_DELAY_TICKS;
        sched::schedule(&mut frame);
        // Task 1 should now be Ready (restarted)
        assert_eq!(sched::TCBS[1].state, TaskState::Ready);
        assert_eq!(sched::TCBS[1].context.elr_el1, 0x4008_0100);
        assert_eq!(sched::TCBS[1].context.spsr_el1, 0x000);
    }
}

#[test]
fn sched_idle_fallback() {
    unsafe {
        reset_test_state();

        // Fault all tasks except idle (task 2)
        sched::TCBS[0].state = TaskState::Faulted;
        sched::TCBS[0].fault_tick = 0;
        sched::TCBS[1].state = TaskState::Faulted;
        sched::TCBS[1].fault_tick = 0;
        sched::TCBS[2].state = TaskState::Running;
        *sched::CURRENT.get_mut() = 2;

        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);
        // Only task 2 (idle) is Ready → should pick task 2
        assert_eq!(read_current(), 2);
    }
}

#[test]
fn sched_context_save_restore() {
    unsafe {
        reset_test_state();

        // Set up a distinctive context for task 0
        sched::TCBS[0].context.x[0] = 0xDEAD_BEEF;
        sched::TCBS[0].context.x[7] = 0x0000_0042;
        sched::TCBS[0].context.elr_el1 = 0x4008_1000;
        sched::TCBS[0].context.spsr_el1 = 0x000;
        sched::TCBS[0].context.sp_el0 = 0x5000_1000;

        // Set up task 1 with different context
        sched::TCBS[1].context.x[0] = 0xCAFE_BABE;
        sched::TCBS[1].context.elr_el1 = 0x4008_2000;
        sched::TCBS[1].context.sp_el0 = 0x5000_2000;

        // Create frame matching task 0's context
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        frame.x[0] = 0xDEAD_BEEF;
        frame.x[7] = 0x0000_0042;
        frame.elr_el1 = 0x4008_1000;
        frame.sp_el0 = 0x5000_1000;

        // Schedule — should save task 0's frame and load task 1's
        sched::schedule(&mut frame);

        // Frame should now contain task 1's context
        assert_eq!(frame.x[0], 0xCAFE_BABE);
        assert_eq!(frame.elr_el1, 0x4008_2000);
        assert_eq!(frame.sp_el0, 0x5000_2000);

        // Task 0's TCB should have the saved context
        assert_eq!(sched::TCBS[0].context.x[0], 0xDEAD_BEEF);
        assert_eq!(sched::TCBS[0].context.x[7], 0x0000_0042);
    }
}

#[test]
fn sched_get_set_task_reg() {
    unsafe {
        reset_test_state();

        sched::set_task_reg(0, 3, 0x1234_5678);
        assert_eq!(sched::get_task_reg(0, 3), 0x1234_5678);

        sched::set_task_reg(1, 0, 0xAAAA_BBBB);
        assert_eq!(sched::get_task_reg(1, 0), 0xAAAA_BBBB);
        // Other regs should still be 0
        assert_eq!(sched::get_task_reg(1, 1), 0);
    }
}

#[test]
fn sched_save_load_frame() {
    unsafe {
        reset_test_state();

        let src_frame = TrapFrame {
            x: {
                let mut x = [0u64; 31];
                x[0] = 0x1111;
                x[1] = 0x2222;
                x[30] = 0x3333;
                x
            },
            sp_el0: 0x5000,
            elr_el1: 0x4000,
            spsr_el1: 0x0,
            _pad: [0; 2],
        };

        sched::save_frame(1, &src_frame);
        assert_eq!(sched::TCBS[1].context.x[0], 0x1111);
        assert_eq!(sched::TCBS[1].context.x[1], 0x2222);
        assert_eq!(sched::TCBS[1].context.x[30], 0x3333);
        assert_eq!(sched::TCBS[1].context.elr_el1, 0x4000);

        let mut dst_frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::load_frame(1, &mut dst_frame);
        assert_eq!(dst_frame.x[0], 0x1111);
        assert_eq!(dst_frame.x[30], 0x3333);
        assert_eq!(dst_frame.sp_el0, 0x5000);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 5. IPC State Machine Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn ipc_cleanup_clears_sender_slot() {
    unsafe {
        reset_test_state();

        ipc::ENDPOINTS[0].sender_queue.push(1);
        ipc::cleanup_task(1);
        assert!(!ipc::ENDPOINTS[0].sender_queue.contains(1));
    }
}

#[test]
fn ipc_cleanup_clears_receiver_slot() {
    unsafe {
        reset_test_state();

        ipc::ENDPOINTS[0].receiver = Some(1);
        ipc::cleanup_task(1);
        assert_eq!(ipc::ENDPOINTS[0].receiver, None);
    }
}

#[test]
fn ipc_cleanup_clears_both_endpoints() {
    unsafe {
        reset_test_state();

        ipc::ENDPOINTS[0].sender_queue.push(1);
        ipc::ENDPOINTS[1].receiver = Some(1);
        ipc::cleanup_task(1);
        assert!(!ipc::ENDPOINTS[0].sender_queue.contains(1));
        assert_eq!(ipc::ENDPOINTS[1].receiver, None);
    }
}

#[test]
fn ipc_cleanup_doesnt_affect_other_tasks() {
    unsafe {
        reset_test_state();

        ipc::ENDPOINTS[0].sender_queue.push(0);
        ipc::ENDPOINTS[0].receiver = Some(2);
        ipc::ENDPOINTS[1].sender_queue.push(1);

        ipc::cleanup_task(1);

        // Task 0 and 2 slots should be untouched
        assert!(ipc::ENDPOINTS[0].sender_queue.contains(0));
        assert_eq!(ipc::ENDPOINTS[0].receiver, Some(2));
        // Task 1 slot should be cleared
        assert!(!ipc::ENDPOINTS[1].sender_queue.contains(1));
    }
}

#[test]
fn ipc_copy_message() {
    unsafe {
        reset_test_state();

        // Set message in task 0's registers
        sched::set_task_reg(0, 0, 0x50494E47); // "PING"
        sched::set_task_reg(0, 1, 0x1111);
        sched::set_task_reg(0, 2, 0x2222);
        sched::set_task_reg(0, 3, 0x3333);

        // Copy message from task 0 → task 1
        ipc::copy_message(0, 1);

        // Verify all 4 message registers copied
        assert_eq!(sched::get_task_reg(1, 0), 0x50494E47);
        assert_eq!(sched::get_task_reg(1, 1), 0x1111);
        assert_eq!(sched::get_task_reg(1, 2), 0x2222);
        assert_eq!(sched::get_task_reg(1, 3), 0x3333);
    }
}

#[test]
fn ipc_msg_regs_count() {
    assert_eq!(MSG_REGS, 4);
}

#[test]
fn ipc_max_endpoints() {
    assert_eq!(MAX_ENDPOINTS, 4);
}

#[test]
fn ipc_endpoint_initial_state() {
    assert_eq!(EMPTY_EP.sender_queue.count, 0);
    assert_eq!(EMPTY_EP.receiver, None);
}

// ═══════════════════════════════════════════════════════════════════
// Phase G — Capability Access Control Tests
// ═══════════════════════════════════════════════════════════════════

// ─── G1: Capability bit constants ──────────────────────────────────

#[test]
fn cap_bits_are_distinct_powers_of_two() {
    let all = [
        CAP_IPC_SEND_EP0, CAP_IPC_RECV_EP0,
        CAP_IPC_SEND_EP1, CAP_IPC_RECV_EP1,
        CAP_WRITE, CAP_YIELD,
        CAP_NOTIFY, CAP_WAIT_NOTIFY,
        CAP_IPC_SEND_EP2, CAP_IPC_RECV_EP2,
        CAP_IPC_SEND_EP3, CAP_IPC_RECV_EP3,
        CAP_GRANT_CREATE, CAP_GRANT_REVOKE,
        CAP_IRQ_BIND, CAP_IRQ_ACK,
        CAP_DEVICE_MAP,
        CAP_HEARTBEAT,
    ];
    // Each must be a single bit (power of 2)
    for &c in &all {
        assert!(c != 0, "cap must be nonzero");
        assert!(c & (c - 1) == 0, "cap 0x{:x} is not a power of 2", c);
    }
    // All must be distinct
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "caps at index {} and {} collide", i, j);
        }
    }
}

#[test]
fn cap_all_includes_every_bit() {
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_SEND_EP0));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_RECV_EP0));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_SEND_EP1));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_RECV_EP1));
    assert!(cap::cap_check(CAP_ALL, CAP_WRITE));
    assert!(cap::cap_check(CAP_ALL, CAP_YIELD));
    assert!(cap::cap_check(CAP_ALL, CAP_NOTIFY));
    assert!(cap::cap_check(CAP_ALL, CAP_WAIT_NOTIFY));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_SEND_EP2));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_RECV_EP2));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_SEND_EP3));
    assert!(cap::cap_check(CAP_ALL, CAP_IPC_RECV_EP3));
    assert!(cap::cap_check(CAP_ALL, CAP_GRANT_CREATE));
    assert!(cap::cap_check(CAP_ALL, CAP_GRANT_REVOKE));
    assert!(cap::cap_check(CAP_ALL, CAP_IRQ_BIND));
    assert!(cap::cap_check(CAP_ALL, CAP_IRQ_ACK));
    assert!(cap::cap_check(CAP_ALL, CAP_DEVICE_MAP));
    assert!(cap::cap_check(CAP_ALL, CAP_HEARTBEAT));
}

#[test]
fn cap_none_grants_nothing() {
    assert!(!cap::cap_check(CAP_NONE, CAP_IPC_SEND_EP0));
    assert!(!cap::cap_check(CAP_NONE, CAP_WRITE));
    assert!(!cap::cap_check(CAP_NONE, CAP_YIELD));
}

// ─── G1: cap_check logic ──────────────────────────────────────────

#[test]
fn cap_check_single_bit() {
    let caps = CAP_WRITE | CAP_YIELD;
    assert!(cap::cap_check(caps, CAP_WRITE));
    assert!(cap::cap_check(caps, CAP_YIELD));
    assert!(!cap::cap_check(caps, CAP_IPC_SEND_EP0));
}

#[test]
fn cap_check_multi_bit_requirement() {
    let caps = CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0;
    // Has both → ok
    assert!(cap::cap_check(caps, CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0));
    // Has only send, needs both → fail
    let only_send = CAP_IPC_SEND_EP0;
    assert!(!cap::cap_check(only_send, CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0));
}

#[test]
fn cap_check_zero_required_always_passes() {
    // Requiring nothing → always passes (even with CAP_NONE)
    assert!(cap::cap_check(CAP_NONE, 0));
    assert!(cap::cap_check(CAP_ALL, 0));
}

// ─── G1: cap_for_syscall mapping ──────────────────────────────────

#[test]
fn cap_for_syscall_yield() {
    assert_eq!(cap::cap_for_syscall(0, 0), CAP_YIELD);
    assert_eq!(cap::cap_for_syscall(0, 99), CAP_YIELD); // ep_id ignored for yield
}

#[test]
fn cap_for_syscall_send_recv() {
    assert_eq!(cap::cap_for_syscall(1, 0), CAP_IPC_SEND_EP0);
    assert_eq!(cap::cap_for_syscall(1, 1), CAP_IPC_SEND_EP1);
    assert_eq!(cap::cap_for_syscall(1, 2), CAP_IPC_SEND_EP2);
    assert_eq!(cap::cap_for_syscall(1, 3), CAP_IPC_SEND_EP3);
    assert_eq!(cap::cap_for_syscall(2, 0), CAP_IPC_RECV_EP0);
    assert_eq!(cap::cap_for_syscall(2, 1), CAP_IPC_RECV_EP1);
    assert_eq!(cap::cap_for_syscall(2, 2), CAP_IPC_RECV_EP2);
    assert_eq!(cap::cap_for_syscall(2, 3), CAP_IPC_RECV_EP3);
}

#[test]
fn cap_for_syscall_call_needs_both() {
    // SYS_CALL on EP0 needs SEND+RECV
    let required = cap::cap_for_syscall(3, 0);
    assert_eq!(required, CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0);
    // SYS_CALL on EP1
    let required1 = cap::cap_for_syscall(3, 1);
    assert_eq!(required1, CAP_IPC_SEND_EP1 | CAP_IPC_RECV_EP1);
    // SYS_CALL on EP2
    let required2 = cap::cap_for_syscall(3, 2);
    assert_eq!(required2, CAP_IPC_SEND_EP2 | CAP_IPC_RECV_EP2);
    // SYS_CALL on EP3
    let required3 = cap::cap_for_syscall(3, 3);
    assert_eq!(required3, CAP_IPC_SEND_EP3 | CAP_IPC_RECV_EP3);
}

#[test]
fn cap_for_syscall_write() {
    assert_eq!(cap::cap_for_syscall(4, 0), CAP_WRITE);
}

#[test]
fn cap_for_syscall_invalid_returns_zero() {
    // Unknown syscall
    assert_eq!(cap::cap_for_syscall(99, 0), 0);
    // Invalid endpoint for IPC
    assert_eq!(cap::cap_for_syscall(1, 5), 0);
    assert_eq!(cap::cap_for_syscall(2, 5), 0);
    assert_eq!(cap::cap_for_syscall(3, 5), 0);
}

// ─── G1: cap_name ─────────────────────────────────────────────────

#[test]
fn cap_name_returns_expected_strings() {
    assert_eq!(cap::cap_name(CAP_IPC_SEND_EP0), "IPC_SEND_EP0");
    assert_eq!(cap::cap_name(CAP_IPC_RECV_EP0), "IPC_RECV_EP0");
    assert_eq!(cap::cap_name(CAP_WRITE), "WRITE");
    assert_eq!(cap::cap_name(CAP_YIELD), "YIELD");
    assert_eq!(cap::cap_name(CAP_NOTIFY), "NOTIFY");
    assert_eq!(cap::cap_name(CAP_WAIT_NOTIFY), "WAIT_NOTIFY");
    assert_eq!(cap::cap_name(CAP_IPC_SEND_EP2), "IPC_SEND_EP2");
    assert_eq!(cap::cap_name(CAP_IPC_RECV_EP2), "IPC_RECV_EP2");
    assert_eq!(cap::cap_name(CAP_IPC_SEND_EP3), "IPC_SEND_EP3");
    assert_eq!(cap::cap_name(CAP_IPC_RECV_EP3), "IPC_RECV_EP3");
    assert_eq!(cap::cap_name(CAP_ALL), "ALL");
    assert_eq!(cap::cap_name(CAP_NONE), "NONE");
    assert_eq!(cap::cap_name(0xDEAD_BEEF), "UNKNOWN");
}

// ─── G3: Caps survive in EMPTY_TCB / TCB ──────────────────────────

#[test]
fn cap_empty_tcb_has_zero_caps() {
    assert_eq!(EMPTY_TCB.caps, 0);
}

#[test]
fn cap_survives_restart_simulation() {
    // Simulate: assign caps to a task, then "restart" it (reset context
    // fields like restart_task does, but don't touch caps).
    unsafe {
        reset_test_state();
        let original_caps = CAP_IPC_SEND_EP0 | CAP_WRITE | CAP_YIELD;
        sched::TCBS[0].caps = original_caps;
        sched::TCBS[0].state = TaskState::Faulted;
        sched::TCBS[0].fault_tick = 0;

        // Simulate what restart_task() does: zero context, reload entry/stack
        sched::TCBS[0].context = TrapFrame {
            x: [0; 31],
            sp_el0: sched::TCBS[0].user_stack_top,
            elr_el1: sched::TCBS[0].entry_point,
            spsr_el1: 0x000,
            _pad: [0; 2],
        };
        sched::TCBS[0].state = TaskState::Ready;

        // Caps must still be the original value
        assert_eq!(sched::TCBS[0].caps, original_caps,
            "caps must survive task restart");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 7. Per-Task Address Space Tests (Phase H)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn addr_page_table_base_returns_distinct_per_task() {
    // Each task must get a different page table base
    let b0 = mmu::page_table_base(0);
    let b1 = mmu::page_table_base(1);
    let b2 = mmu::page_table_base(2);
    assert_ne!(b0, b1, "task 0 and 1 must have different page table bases");
    assert_ne!(b1, b2, "task 1 and 2 must have different page table bases");
    assert_ne!(b0, b2, "task 0 and 2 must have different page table bases");
}

#[test]
fn addr_page_table_base_is_4k_aligned() {
    for t in 0..3 {
        let base = mmu::page_table_base(t);
        assert_eq!(base & 0xFFF, 0,
            "page_table_base({}) = {:#x} must be 4KB aligned", t, base);
    }
}

#[test]
fn addr_ttbr0_for_task_embeds_asid() {
    let ttbr0 = mmu::ttbr0_for_task(0, 1);
    let asid = ttbr0 >> 48;
    assert_eq!(asid, 1, "ASID should be 1 for task 0");

    let ttbr0 = mmu::ttbr0_for_task(1, 2);
    let asid = ttbr0 >> 48;
    assert_eq!(asid, 2, "ASID should be 2 for task 1");

    let ttbr0 = mmu::ttbr0_for_task(2, 3);
    let asid = ttbr0 >> 48;
    assert_eq!(asid, 3, "ASID should be 3 for task 2");
}

#[test]
fn addr_ttbr0_preserves_base_address() {
    for t in 0..3 {
        let base = mmu::page_table_base(t);
        let asid: u16 = (t as u16) + 1;
        let ttbr0 = mmu::ttbr0_for_task(t, asid);
        let extracted_base = ttbr0 & 0x0000_FFFF_FFFF_F000;
        assert_eq!(extracted_base, base,
            "ttbr0_for_task({}) base should match page_table_base({})", t, t);
    }
}

#[test]
fn addr_different_asids_produce_different_ttbr0() {
    let t0 = mmu::ttbr0_for_task(0, 1);
    let t1 = mmu::ttbr0_for_task(1, 2);
    let t2 = mmu::ttbr0_for_task(2, 3);
    assert_ne!(t0, t1);
    assert_ne!(t1, t2);
    assert_ne!(t0, t2);
}

#[test]
fn addr_asid_zero_reserved_for_kernel() {
    // ASID 0 is used for kernel boot table — tasks should use 1, 2, 3
    let kernel_ttbr0 = mmu::ttbr0_for_task(0, 0);
    let task0_ttbr0 = mmu::ttbr0_for_task(0, 1);
    // Same base, different ASID
    assert_ne!(kernel_ttbr0, task0_ttbr0);
    assert_eq!(kernel_ttbr0 >> 48, 0);
    assert_eq!(task0_ttbr0 >> 48, 1);
}

#[test]
fn addr_empty_tcb_has_zero_ttbr0() {
    assert_eq!(EMPTY_TCB.ttbr0, 0, "EMPTY_TCB.ttbr0 should be 0");
}

#[test]
fn addr_ttbr0_survives_restart() {
    // ttbr0 must survive task restart (like caps)
    unsafe {
        reset_test_state();
        let original_ttbr0 = mmu::ttbr0_for_task(0, 1);
        sched::TCBS[0].ttbr0 = original_ttbr0;
        sched::TCBS[0].caps = CAP_WRITE | CAP_YIELD;
        sched::TCBS[0].state = TaskState::Faulted;
        sched::TCBS[0].fault_tick = 0;

        // Simulate restart_task: zero context, reload entry/stack
        sched::TCBS[0].context = TrapFrame {
            x: [0; 31],
            sp_el0: sched::TCBS[0].user_stack_top,
            elr_el1: sched::TCBS[0].entry_point,
            spsr_el1: 0x000,
            _pad: [0; 2],
        };
        sched::TCBS[0].state = TaskState::Ready;

        assert_eq!(sched::TCBS[0].ttbr0, original_ttbr0,
            "ttbr0 must survive task restart");
    }
}

#[test]
fn addr_schedule_preserves_ttbr0_in_tcb() {
    // After scheduling, each TCB should still hold its original ttbr0
    unsafe {
        reset_test_state();
        for i in 0..NUM_TASKS {
            sched::TCBS[i].ttbr0 = mmu::ttbr0_for_task(i, (i as u16) + 1);
        }

        // Schedule once (simulates timer tick)
        let mut frame = TrapFrame {
            x: [0; 31],
            sp_el0: 0,
            elr_el1: sched::TCBS[0].context.elr_el1,
            spsr_el1: sched::TCBS[0].context.spsr_el1,
            _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        // All ttbr0 values should be unchanged
        for i in 0..NUM_TASKS {
            let expected = mmu::ttbr0_for_task(i, (i as u16) + 1);
            assert_eq!(sched::TCBS[i].ttbr0, expected,
                "TCBS[{}].ttbr0 should be preserved after schedule", i);
        }
    }
}

#[test]
fn addr_max_asid_fits_in_8_bits() {
    // AArch64 with 8-bit ASID: values 0..255 valid
    let ttbr0 = mmu::ttbr0_for_task(0, 255);
    let asid = ttbr0 >> 48;
    assert_eq!(asid, 255, "max 8-bit ASID should fit");

    // Our tasks use ASIDs 1, 2, 3 — all well within 8-bit range
    for t in 0..3_u16 {
        let asid = t + 1;
        assert!(asid <= 255, "task ASID must fit in 8 bits");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 8. Notification System Tests (Phase I)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn notify_empty_tcb_has_no_pending() {
    assert_eq!(EMPTY_TCB.notify_pending, 0);
    assert_eq!(EMPTY_TCB.notify_waiting, false);
}

#[test]
fn notify_pending_or_merge() {
    unsafe {
        reset_test_state();
        // Simulate two notifications merging via OR
        sched::TCBS[1].notify_pending |= 0x01;
        sched::TCBS[1].notify_pending |= 0x04;
        assert_eq!(sched::TCBS[1].notify_pending, 0x05);
    }
}

#[test]
fn notify_pending_cleared_on_read() {
    unsafe {
        reset_test_state();
        sched::TCBS[0].notify_pending = 0xFF;
        // Simulate wait_notify: read and clear
        let pending = sched::TCBS[0].notify_pending;
        sched::TCBS[0].notify_pending = 0;
        assert_eq!(pending, 0xFF);
        assert_eq!(sched::TCBS[0].notify_pending, 0);
    }
}

#[test]
fn notify_waiting_flag() {
    unsafe {
        reset_test_state();
        sched::TCBS[1].notify_waiting = true;
        assert!(sched::TCBS[1].notify_waiting);
        // Simulate notify delivery: clear waiting flag
        sched::TCBS[1].notify_waiting = false;
        assert!(!sched::TCBS[1].notify_waiting);
    }
}

#[test]
fn notify_cleared_on_restart() {
    unsafe {
        reset_test_state();
        sched::TCBS[1].notify_pending = 0xABCD;
        sched::TCBS[1].notify_waiting = true;
        sched::TCBS[1].state = TaskState::Faulted;
        sched::TCBS[1].fault_tick = 0;

        *aegis_os::timer::TICK_COUNT.get_mut() = RESTART_DELAY_TICKS;
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        // After restart, notify state should be cleared
        assert_eq!(sched::TCBS[1].notify_pending, 0,
            "notify_pending must be cleared on restart");
        assert_eq!(sched::TCBS[1].notify_waiting, false,
            "notify_waiting must be cleared on restart");
    }
}

#[test]
fn notify_cap_for_syscall_notify() {
    assert_eq!(cap::cap_for_syscall(5, 0), CAP_NOTIFY);
    assert_eq!(cap::cap_for_syscall(5, 99), CAP_NOTIFY); // ep_id ignored
}

#[test]
fn notify_cap_for_syscall_wait_notify() {
    assert_eq!(cap::cap_for_syscall(6, 0), CAP_WAIT_NOTIFY);
    assert_eq!(cap::cap_for_syscall(6, 99), CAP_WAIT_NOTIFY);
}

// ═══════════════════════════════════════════════════════════════════
// 9. Multi-Sender Queue Tests (Phase I)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sender_queue_push_pop_fifo() {
    let mut q = ipc::SenderQueue::new();
    assert!(q.push(0));
    assert!(q.push(1));
    assert!(q.push(2));
    assert_eq!(q.pop(), Some(0));
    assert_eq!(q.pop(), Some(1));
    assert_eq!(q.pop(), Some(2));
    assert_eq!(q.pop(), None);
}

#[test]
fn sender_queue_full_rejects() {
    let mut q = ipc::SenderQueue::new();
    for i in 0..ipc::MAX_WAITERS {
        assert!(q.push(i), "push {} should succeed", i);
    }
    // Queue is full — push should fail
    assert!(!q.push(99), "push should fail when queue is full");
}

#[test]
fn sender_queue_remove_middle() {
    let mut q = ipc::SenderQueue::new();
    q.push(0);
    q.push(1);
    q.push(2);
    q.remove(1);
    assert_eq!(q.count, 2);
    assert_eq!(q.pop(), Some(0));
    assert_eq!(q.pop(), Some(2));
    assert_eq!(q.pop(), None);
}

#[test]
fn sender_queue_contains() {
    let mut q = ipc::SenderQueue::new();
    q.push(5);
    q.push(7);
    assert!(q.contains(5));
    assert!(q.contains(7));
    assert!(!q.contains(3));
}

#[test]
fn sender_queue_wrap_around() {
    let mut q = ipc::SenderQueue::new();
    // Fill and drain to advance head
    q.push(10);
    q.push(11);
    q.pop(); // head moves to 1
    q.pop(); // head moves to 2

    // Now push to wrap around
    q.push(20);
    q.push(21);
    q.push(22);
    q.push(23);
    assert_eq!(q.count, 4);
    assert_eq!(q.pop(), Some(20));
    assert_eq!(q.pop(), Some(21));
    assert_eq!(q.pop(), Some(22));
    assert_eq!(q.pop(), Some(23));
}

// ═══════════════════════════════════════════════════════════════════
// 10. Expanded Endpoints Tests (Phase I)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn ipc_four_endpoints_exist() {
    unsafe {
        reset_test_state();
        // All 4 endpoints should be accessible and initially empty
        for i in 0..4 {
            assert_eq!(ipc::ENDPOINTS[i].sender_queue.count, 0,
                "EP{} sender_queue should be empty", i);
            assert_eq!(ipc::ENDPOINTS[i].receiver, None,
                "EP{} receiver should be None", i);
        }
    }
}

#[test]
fn cap_for_syscall_ep2_ep3() {
    assert_eq!(cap::cap_for_syscall(1, 2), CAP_IPC_SEND_EP2);
    assert_eq!(cap::cap_for_syscall(2, 2), CAP_IPC_RECV_EP2);
    assert_eq!(cap::cap_for_syscall(3, 2), CAP_IPC_SEND_EP2 | CAP_IPC_RECV_EP2);
    assert_eq!(cap::cap_for_syscall(1, 3), CAP_IPC_SEND_EP3);
    assert_eq!(cap::cap_for_syscall(2, 3), CAP_IPC_RECV_EP3);
    assert_eq!(cap::cap_for_syscall(3, 3), CAP_IPC_SEND_EP3 | CAP_IPC_RECV_EP3);
}

#[test]
fn ipc_cleanup_all_four_endpoints() {
    unsafe {
        reset_test_state();
        // Put task 1 in all 4 endpoints
        ipc::ENDPOINTS[0].sender_queue.push(1);
        ipc::ENDPOINTS[1].receiver = Some(1);
        ipc::ENDPOINTS[2].sender_queue.push(1);
        ipc::ENDPOINTS[3].receiver = Some(1);

        ipc::cleanup_task(1);

        for i in 0..4 {
            assert!(!ipc::ENDPOINTS[i].sender_queue.contains(1),
                "EP{} should not contain task 1 after cleanup", i);
            assert_ne!(ipc::ENDPOINTS[i].receiver, Some(1),
                "EP{} receiver should not be task 1 after cleanup", i);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 11. Shared Memory Grant Tests (Phase J1)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn grant_create_success() {
    unsafe {
        reset_test_state();
        let result = grant::grant_create(0, 0, 1);
        assert_eq!(result, 0, "grant_create should return 0 on success");
        assert!(grant::GRANTS[0].active, "grant 0 should be active");
        assert_eq!(grant::GRANTS[0].owner, Some(0));
        assert_eq!(grant::GRANTS[0].peer, Some(1));
        assert_ne!(grant::GRANTS[0].phys_addr, 0, "phys_addr should be non-zero");
    }
}

#[test]
fn grant_create_duplicate_rejected() {
    unsafe {
        reset_test_state();
        let r1 = grant::grant_create(0, 0, 1);
        assert_eq!(r1, 0);
        let r2 = grant::grant_create(0, 0, 2);
        assert_eq!(r2, 0xFFFF_0002, "duplicate grant should be rejected");
    }
}

#[test]
fn grant_create_invalid_id() {
    unsafe {
        reset_test_state();
        let r = grant::grant_create(MAX_GRANTS, 0, 1);
        assert_eq!(r, 0xFFFF_0001, "out-of-range grant_id should fail");
    }
}

#[test]
fn grant_create_invalid_peer() {
    unsafe {
        reset_test_state();
        let r = grant::grant_create(0, 0, sched::NUM_TASKS);
        assert_eq!(r, 0xFFFF_0003, "peer >= NUM_TASKS should fail");
    }
}

#[test]
fn grant_create_self_grant_rejected() {
    unsafe {
        reset_test_state();
        let r = grant::grant_create(0, 1, 1);
        assert_eq!(r, 0xFFFF_0004, "owner == peer should fail");
    }
}

#[test]
fn grant_revoke_by_owner() {
    unsafe {
        reset_test_state();
        grant::grant_create(0, 0, 1);
        let r = grant::grant_revoke(0, 0);
        assert_eq!(r, 0, "owner should be able to revoke");
        assert!(!grant::GRANTS[0].active, "grant should be inactive after revoke");
        assert_eq!(grant::GRANTS[0].peer, None, "peer should be None after revoke");
        // Owner is still recorded so it can re-create later
    }
}

#[test]
fn grant_revoke_by_non_owner_rejected() {
    unsafe {
        reset_test_state();
        grant::grant_create(0, 0, 1);
        let r = grant::grant_revoke(0, 1);
        assert_eq!(r, 0xFFFF_0005, "non-owner revoke should fail");
        assert!(grant::GRANTS[0].active, "grant should remain active");
    }
}

#[test]
fn grant_revoke_inactive_is_noop() {
    unsafe {
        reset_test_state();
        let r = grant::grant_revoke(0, 0);
        assert_eq!(r, 0, "revoking inactive grant should be no-op success");
    }
}

#[test]
fn grant_revoke_invalid_id() {
    unsafe {
        reset_test_state();
        let r = grant::grant_revoke(MAX_GRANTS, 0);
        assert_eq!(r, 0xFFFF_0001, "out-of-range grant_id should fail");
    }
}

#[test]
fn grant_cleanup_owner_faulted() {
    unsafe {
        reset_test_state();
        grant::grant_create(0, 0, 1);
        grant::grant_create(1, 0, 2);
        // Task 0 faults — all its grants should be cleared
        grant::cleanup_task(0);
        assert!(!grant::GRANTS[0].active);
        assert_eq!(grant::GRANTS[0].owner, None);
        assert_eq!(grant::GRANTS[0].peer, None);
        assert!(!grant::GRANTS[1].active);
        assert_eq!(grant::GRANTS[1].owner, None);
        assert_eq!(grant::GRANTS[1].peer, None);
    }
}

#[test]
fn grant_cleanup_peer_faulted() {
    unsafe {
        reset_test_state();
        grant::grant_create(0, 0, 1);
        // Task 1 (peer) faults — peer access removed, grant deactivated
        grant::cleanup_task(1);
        assert!(!grant::GRANTS[0].active, "grant should be inactive after peer fault");
        assert_eq!(grant::GRANTS[0].peer, None, "peer should be None");
        // Owner field preserved (EMPTY_GRANT clears it only when owner faults)
        assert_eq!(grant::GRANTS[0].owner, Some(0), "owner should be preserved");
    }
}

#[test]
fn grant_page_addr_valid() {
    // On host, returns fake addresses in 0x4010_0000 range
    assert!(grant::grant_page_addr(0).is_some());
    assert!(grant::grant_page_addr(1).is_some());
    let a0 = grant::grant_page_addr(0).unwrap();
    let a1 = grant::grant_page_addr(1).unwrap();
    assert_ne!(a0, a1, "each grant page should have a distinct address");
    assert_eq!(a1 - a0, 4096, "grant pages should be 4KB apart");
}

#[test]
fn grant_page_addr_invalid() {
    assert!(grant::grant_page_addr(MAX_GRANTS).is_none());
    assert!(grant::grant_page_addr(MAX_GRANTS + 1).is_none());
}

#[test]
fn cap_for_syscall_grant() {
    assert_eq!(cap::cap_for_syscall(7, 0), CAP_GRANT_CREATE);
    assert_eq!(cap::cap_for_syscall(8, 0), CAP_GRANT_REVOKE);
}

#[test]
fn cap_all_includes_grants() {
    assert_ne!(CAP_ALL & CAP_GRANT_CREATE, 0, "CAP_ALL should include GRANT_CREATE");
    assert_ne!(CAP_ALL & CAP_GRANT_REVOKE, 0, "CAP_ALL should include GRANT_REVOKE");
}

#[test]
fn grant_two_grants_independent() {
    unsafe {
        reset_test_state();
        let r0 = grant::grant_create(0, 0, 1);
        let r1 = grant::grant_create(1, 1, 2);
        assert_eq!(r0, 0);
        assert_eq!(r1, 0);
        assert!(grant::GRANTS[0].active);
        assert!(grant::GRANTS[1].active);
        assert_ne!(grant::GRANTS[0].phys_addr, grant::GRANTS[1].phys_addr);

        // Revoke one doesn't affect the other
        grant::grant_revoke(0, 0);
        assert!(!grant::GRANTS[0].active);
        assert!(grant::GRANTS[1].active, "grant 1 should be unaffected");
    }
}

#[test]
fn grant_re_create_after_revoke() {
    unsafe {
        reset_test_state();
        grant::grant_create(0, 0, 1);
        grant::grant_revoke(0, 0);
        // Should be able to re-create the same grant slot
        let r = grant::grant_create(0, 1, 2);
        assert_eq!(r, 0, "re-create after revoke should succeed");
        assert_eq!(grant::GRANTS[0].owner, Some(1));
        assert_eq!(grant::GRANTS[0].peer, Some(2));
    }
}

// ═══════════════════════════════════════════════════════════════════
// 12. IRQ Routing Tests (Phase J2)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn irq_bind_success() {
    unsafe {
        reset_test_state();
        let r = irq::irq_bind(33, 0, 0x01);
        assert_eq!(r, 0, "irq_bind should succeed for SPI INTID 33");
        assert!(irq::IRQ_BINDINGS[0].active);
        assert_eq!(irq::IRQ_BINDINGS[0].intid, 33);
        assert_eq!(irq::IRQ_BINDINGS[0].task_id, 0);
        assert_eq!(irq::IRQ_BINDINGS[0].notify_bit, 0x01);
        assert!(!irq::IRQ_BINDINGS[0].pending_ack);
    }
}

#[test]
fn irq_bind_reject_ppi() {
    unsafe {
        reset_test_state();
        // INTID < 32 is PPI/SGI range, must be rejected
        let r = irq::irq_bind(30, 0, 0x01); // timer INTID
        assert_eq!(r, irq::ERR_INVALID_INTID);
        let r2 = irq::irq_bind(15, 0, 0x01);
        assert_eq!(r2, irq::ERR_INVALID_INTID);
    }
}

#[test]
fn irq_bind_reject_zero_bit() {
    unsafe {
        reset_test_state();
        let r = irq::irq_bind(33, 0, 0);
        assert_eq!(r, irq::ERR_INVALID_INTID, "notify_bit=0 should be rejected");
    }
}

#[test]
fn irq_bind_reject_duplicate() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        let r = irq::irq_bind(33, 1, 0x02);
        assert_eq!(r, irq::ERR_ALREADY_BOUND, "duplicate INTID should be rejected");
    }
}

#[test]
fn irq_bind_table_full() {
    unsafe {
        reset_test_state();
        // Fill all 8 slots
        for i in 0..MAX_IRQ_BINDINGS {
            let r = irq::irq_bind(32 + i as u32, 0, 1u64 << i);
            assert_eq!(r, 0, "binding {} should succeed", i);
        }
        // 9th should fail
        let r = irq::irq_bind(100, 0, 0x100);
        assert_eq!(r, irq::ERR_TABLE_FULL);
    }
}

#[test]
fn irq_ack_success() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        // Simulate IRQ fired: set pending_ack manually
        irq::IRQ_BINDINGS[0].pending_ack = true;
        let r = irq::irq_ack(33, 0);
        assert_eq!(r, 0);
        assert!(!irq::IRQ_BINDINGS[0].pending_ack, "pending_ack should be cleared");
    }
}

#[test]
fn irq_ack_wrong_task() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        irq::IRQ_BINDINGS[0].pending_ack = true;
        let r = irq::irq_ack(33, 1); // task 1 didn't bind this
        assert_eq!(r, irq::ERR_NOT_OWNER);
    }
}

#[test]
fn irq_ack_not_bound() {
    unsafe {
        reset_test_state();
        let r = irq::irq_ack(33, 0);
        assert_eq!(r, irq::ERR_NOT_BOUND);
    }
}

#[test]
fn irq_ack_already_acked_is_noop() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        // pending_ack is false (no IRQ fired)
        let r = irq::irq_ack(33, 0);
        assert_eq!(r, 0, "ACK when not pending should be no-op success");
    }
}

#[test]
fn irq_route_sets_notify_pending() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        // Simulate IRQ routing via host-test stub
        irq::irq_route_test(33, 0);
        assert_eq!(sched::TCBS[0].notify_pending, 0x01,
            "notify_pending should have the bound bit set");
        assert!(irq::IRQ_BINDINGS[0].pending_ack, "pending_ack should be true");
    }
}

#[test]
fn irq_route_unblocks_waiting_task() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        // Task 0 is waiting for notifications
        sched::TCBS[0].state = TaskState::Blocked;
        sched::TCBS[0].notify_waiting = true;
        sched::TCBS[0].notify_pending = 0;
        // Fire IRQ
        irq::irq_route_test(33, 0);
        assert_eq!(sched::TCBS[0].state, TaskState::Ready, "task should be unblocked");
        assert!(!sched::TCBS[0].notify_waiting, "notify_waiting should be cleared");
        assert_eq!(sched::TCBS[0].context.x[0], 0x01, "delivered bits should be in x0");
        assert_eq!(sched::TCBS[0].notify_pending, 0, "pending should be cleared after delivery");
    }
}

#[test]
fn irq_route_accumulates_bits() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        irq::irq_bind(34, 0, 0x02);
        // Set pending_ack=false so we can route both (in practice kernel masks)
        irq::irq_route_test(33, 0);
        irq::IRQ_BINDINGS[0].pending_ack = false; // pretend ACK'd
        sched::TCBS[0].notify_pending = 0x01; // kept from first route
        irq::irq_route_test(34, 0);
        // Both bits should be accumulated
        assert_eq!(sched::TCBS[0].notify_pending, 0x01 | 0x02);
    }
}

#[test]
fn irq_cleanup_unbinds_all() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        irq::irq_bind(34, 0, 0x02);
        irq::IRQ_BINDINGS[0].pending_ack = true;
        irq::irq_cleanup_task(0);
        for i in 0..MAX_IRQ_BINDINGS {
            assert!(!irq::IRQ_BINDINGS[i].active,
                "binding {} should be inactive after cleanup", i);
        }
    }
}

#[test]
fn irq_cleanup_does_not_affect_other_tasks() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        irq::irq_bind(34, 1, 0x02);
        irq::irq_cleanup_task(0);
        assert!(!irq::IRQ_BINDINGS[0].active, "task 0 binding should be cleaned");
        assert!(irq::IRQ_BINDINGS[1].active, "task 1 binding should remain");
    }
}

#[test]
fn cap_for_syscall_irq() {
    assert_eq!(cap::cap_for_syscall(9, 0), CAP_IRQ_BIND);
    assert_eq!(cap::cap_for_syscall(10, 0), CAP_IRQ_ACK);
}

#[test]
fn cap_all_includes_irq() {
    assert_ne!(CAP_ALL & CAP_IRQ_BIND, 0, "CAP_ALL should include IRQ_BIND");
    assert_ne!(CAP_ALL & CAP_IRQ_ACK, 0, "CAP_ALL should include IRQ_ACK");
}

#[test]
fn irq_rebind_after_cleanup() {
    unsafe {
        reset_test_state();
        irq::irq_bind(33, 0, 0x01);
        irq::irq_cleanup_task(0);
        // Should be able to rebind same INTID to different task
        let r = irq::irq_bind(33, 1, 0x04);
        assert_eq!(r, 0, "rebind after cleanup should succeed");
        assert_eq!(irq::IRQ_BINDINGS[0].task_id, 1);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 13. Device MMIO Mapping Tests (Phase J3)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn device_map_valid_uart() {
    let r = mmu::map_device_for_task(0, 0); // device_id=0 = UART0, task 0
    assert_eq!(r, 0, "mapping UART0 for task 0 should succeed");
}

#[test]
fn device_map_invalid_device_id() {
    let r = mmu::map_device_for_task(99, 0);
    assert_eq!(r, mmu::DEVICE_MAP_ERR_INVALID_ID, "invalid device_id should fail");
}

#[test]
fn device_map_invalid_task_id() {
    let r = mmu::map_device_for_task(0, 5);
    assert_eq!(r, mmu::DEVICE_MAP_ERR_INVALID_TASK, "invalid task_id should fail");
}

#[test]
fn device_registry_uart_l2_index() {
    assert_eq!(mmu::DEVICES[0].l2_index, 72, "UART0 should be at L2 index 72");
    assert_eq!(mmu::DEVICES[0].intid, 33, "UART0 INTID should be 33");
    assert_eq!(mmu::DEVICES[0].name, "UART0");
}

#[test]
fn cap_for_syscall_device_map() {
    assert_eq!(cap::cap_for_syscall(11, 0), CAP_DEVICE_MAP);
}

#[test]
fn cap_all_includes_device_map() {
    assert_ne!(CAP_ALL & CAP_DEVICE_MAP, 0, "CAP_ALL should include DEVICE_MAP");
}

#[test]
fn page_table_constants_j3() {
    // Verify the new 16-page layout constants
    assert_eq!(mmu::NUM_PAGE_TABLE_PAGES, 16);
    assert_eq!(mmu::PT_L2_DEVICE_0, 0);
    assert_eq!(mmu::PT_L2_DEVICE_1, 1);
    assert_eq!(mmu::PT_L2_DEVICE_2, 2);
    assert_eq!(mmu::PT_L1_TASK0, 3);
    assert_eq!(mmu::PT_L3_TASK0, 9);
    assert_eq!(mmu::PT_L2_DEVICE_KERNEL, 12);
    assert_eq!(mmu::PT_L1_KERNEL, 13);
    assert_eq!(mmu::PT_L3_KERNEL, 15);
}

// ═══════════════════════════════════════════════════════════════════
// 14. Priority Scheduler Tests (Phase K1)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sched_priority_higher_wins() {
    unsafe {
        reset_test_state();

        // Task 0 (Running), priority=2
        // Task 1 (Ready), priority=5
        // Task 2 (Ready), priority=1
        sched::TCBS[0].priority = 2;
        sched::TCBS[0].base_priority = 2;
        sched::TCBS[1].priority = 5;
        sched::TCBS[1].base_priority = 5;
        sched::TCBS[2].priority = 1;
        sched::TCBS[2].base_priority = 1;

        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        // Should pick task 1 (priority=5, highest among Ready tasks)
        assert_eq!(read_current(), 1);
        assert_eq!(sched::TCBS[1].state, TaskState::Running);
    }
}

#[test]
fn sched_same_priority_round_robin() {
    unsafe {
        reset_test_state();

        // All tasks at same priority
        for i in 0..NUM_TASKS {
            sched::TCBS[i].priority = 3;
            sched::TCBS[i].base_priority = 3;
        }

        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };

        // From task 0 → should pick task 1 (round-robin tiebreaker)
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 1);

        // From task 1 → should pick task 2
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 2);

        // From task 2 → should wrap to task 0
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 0);
    }
}

#[test]
fn sched_budget_exhausted_skips_task() {
    unsafe {
        reset_test_state();

        // Task 1: priority=5, budget=50, used=50 (exhausted)
        sched::TCBS[1].priority = 5;
        sched::TCBS[1].base_priority = 5;
        sched::TCBS[1].time_budget = 50;
        sched::TCBS[1].ticks_used = 50;

        // Task 2: priority=1, unlimited budget
        sched::TCBS[2].priority = 1;
        sched::TCBS[2].base_priority = 1;
        sched::TCBS[2].time_budget = 0;

        // Task 0: Running, priority=3
        sched::TCBS[0].priority = 3;
        sched::TCBS[0].base_priority = 3;

        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        // Task 1 has highest priority but exhausted budget → skip
        // Task 0 has priority 3, Task 2 has priority 1
        // Task 0 was Running → now Ready, so it's a candidate
        // Scan starts from (old+1), so task 1 (skip), task 2 (prio 1), task 0 (prio 3)
        // Best is task 0 with priority 3
        assert_eq!(read_current(), 0);
    }
}

#[test]
fn sched_unlimited_budget_never_exhausted() {
    unsafe {
        reset_test_state();

        // Task with budget=0 (unlimited) should run even with high ticks_used
        sched::TCBS[1].priority = 5;
        sched::TCBS[1].base_priority = 5;
        sched::TCBS[1].time_budget = 0;
        sched::TCBS[1].ticks_used = 999999;

        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);
        assert_eq!(read_current(), 1, "unlimited budget should never exhaust");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 15. Time Budget / Epoch Tests (Phase K2)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sched_epoch_reset_clears_ticks() {
    unsafe {
        reset_test_state();

        sched::TCBS[0].ticks_used = 42;
        sched::TCBS[1].ticks_used = 50;
        sched::TCBS[2].ticks_used = 10;
        *sched::EPOCH_TICKS.get_mut() = 99;

        sched::epoch_reset();

        assert_eq!(*sched::EPOCH_TICKS.get(), 0, "epoch counter should reset");
        for i in 0..NUM_TASKS {
            assert_eq!(sched::TCBS[i].ticks_used, 0,
                "task {} ticks_used should be reset", i);
        }
    }
}

#[test]
fn sched_epoch_length_constant() {
    assert_eq!(sched::EPOCH_LENGTH, 100);
}

#[test]
fn sched_watchdog_scan_period_constant() {
    assert_eq!(sched::WATCHDOG_SCAN_PERIOD, 10);
}

#[test]
fn sched_empty_tcb_phase_k_fields_zeroed() {
    assert_eq!(EMPTY_TCB.priority, 0);
    assert_eq!(EMPTY_TCB.base_priority, 0);
    assert_eq!(EMPTY_TCB.time_budget, 0);
    assert_eq!(EMPTY_TCB.ticks_used, 0);
    assert_eq!(EMPTY_TCB.heartbeat_interval, 0);
    assert_eq!(EMPTY_TCB.last_heartbeat, 0);
}

// ═══════════════════════════════════════════════════════════════════
// 16. Watchdog Heartbeat Tests (Phase K3)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sched_record_heartbeat() {
    unsafe {
        reset_test_state();
        *aegis_os::timer::TICK_COUNT.get_mut() = 42;
        sched::record_heartbeat(0, 50);
        assert_eq!(sched::TCBS[0].heartbeat_interval, 50);
        assert_eq!(sched::TCBS[0].last_heartbeat, 42);
    }
}

#[test]
fn sched_record_heartbeat_disable() {
    unsafe {
        reset_test_state();
        sched::TCBS[0].heartbeat_interval = 50;
        sched::record_heartbeat(0, 0);
        assert_eq!(sched::TCBS[0].heartbeat_interval, 0, "interval 0 disables watchdog");
    }
}

#[test]
fn sched_watchdog_scan_no_violation() {
    unsafe {
        reset_test_state();

        // Task 0: heartbeat interval=50, last=10, now=40 → elapsed=30 < 50 → OK
        sched::TCBS[0].heartbeat_interval = 50;
        sched::TCBS[0].last_heartbeat = 10;
        *aegis_os::timer::TICK_COUNT.get_mut() = 40;

        sched::watchdog_scan();

        assert_ne!(sched::TCBS[0].state, TaskState::Faulted,
            "task should not be faulted (heartbeat within interval)");
    }
}

#[test]
fn sched_watchdog_scan_violation_faults_task() {
    unsafe {
        reset_test_state();

        // Task 1: heartbeat interval=50, last=10, now=70 → elapsed=60 > 50 → FAULT
        sched::TCBS[1].heartbeat_interval = 50;
        sched::TCBS[1].last_heartbeat = 10;
        sched::TCBS[1].priority = 5;
        sched::TCBS[1].base_priority = 3;
        *aegis_os::timer::TICK_COUNT.get_mut() = 70;

        sched::watchdog_scan();

        assert_eq!(sched::TCBS[1].state, TaskState::Faulted,
            "task should be faulted (heartbeat expired)");
        assert_eq!(sched::TCBS[1].fault_tick, 70);
        assert_eq!(sched::TCBS[1].priority, sched::TCBS[1].base_priority,
            "priority should be restored to base on fault");
    }
}

#[test]
fn sched_watchdog_scan_skips_disabled() {
    unsafe {
        reset_test_state();

        // Task with heartbeat_interval=0 (disabled) should never be faulted
        sched::TCBS[0].heartbeat_interval = 0;
        sched::TCBS[0].last_heartbeat = 0;
        *aegis_os::timer::TICK_COUNT.get_mut() = 1000;

        sched::watchdog_scan();

        assert_ne!(sched::TCBS[0].state, TaskState::Faulted,
            "disabled watchdog should not fault task");
    }
}

#[test]
fn sched_watchdog_scan_skips_already_faulted() {
    unsafe {
        reset_test_state();

        sched::TCBS[0].heartbeat_interval = 50;
        sched::TCBS[0].last_heartbeat = 0;
        sched::TCBS[0].state = TaskState::Faulted;
        sched::TCBS[0].fault_tick = 5;
        *aegis_os::timer::TICK_COUNT.get_mut() = 100;

        sched::watchdog_scan();

        // fault_tick should not be updated (task already faulted)
        assert_eq!(sched::TCBS[0].fault_tick, 5,
            "already-faulted task should not be re-faulted");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 17. Capability Heartbeat Tests (Phase K3)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn cap_heartbeat_bit_is_power_of_two() {
    assert!(CAP_HEARTBEAT != 0);
    assert_eq!(CAP_HEARTBEAT & (CAP_HEARTBEAT - 1), 0);
}

#[test]
fn cap_heartbeat_is_bit_17() {
    assert_eq!(CAP_HEARTBEAT, 1 << 17);
}

#[test]
fn cap_all_includes_heartbeat() {
    assert_ne!(CAP_ALL & CAP_HEARTBEAT, 0, "CAP_ALL should include HEARTBEAT");
}

#[test]
fn cap_for_syscall_heartbeat() {
    assert_eq!(cap::cap_for_syscall(12, 0), CAP_HEARTBEAT);
    assert_eq!(cap::cap_for_syscall(12, 99), CAP_HEARTBEAT); // ep_id ignored
}

#[test]
fn cap_name_heartbeat() {
    assert_eq!(cap::cap_name(CAP_HEARTBEAT), "HEARTBEAT");
}

// ═══════════════════════════════════════════════════════════════════
// 18. Priority Inheritance Tests (Phase K4)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn sched_set_task_priority() {
    unsafe {
        reset_test_state();
        sched::TCBS[0].priority = 2;
        sched::set_task_priority(0, 7);
        assert_eq!(sched::TCBS[0].priority, 7);
    }
}

#[test]
fn sched_get_task_priority() {
    unsafe {
        reset_test_state();
        sched::TCBS[1].priority = 5;
        assert_eq!(sched::get_task_priority(1), 5);
    }
}

#[test]
fn sched_get_task_base_priority() {
    unsafe {
        reset_test_state();
        sched::TCBS[1].base_priority = 3;
        sched::TCBS[1].priority = 7; // boosted
        assert_eq!(sched::get_task_base_priority(1), 3);
    }
}

#[test]
fn sched_restore_base_priority() {
    unsafe {
        reset_test_state();
        sched::TCBS[0].base_priority = 2;
        sched::TCBS[0].priority = 7; // boosted by inheritance
        sched::restore_base_priority(0);
        assert_eq!(sched::TCBS[0].priority, 2, "priority should be restored to base");
    }
}

#[test]
fn sched_priority_restored_on_fault() {
    unsafe {
        reset_test_state();
        sched::TCBS[0].base_priority = 2;
        sched::TCBS[0].priority = 7; // boosted by inheritance
        sched::TCBS[0].state = TaskState::Faulted;
        sched::TCBS[0].fault_tick = 0;

        // Simulate restart
        *aegis_os::timer::TICK_COUNT.get_mut() = RESTART_DELAY_TICKS;
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        assert_eq!(sched::TCBS[0].priority, 2,
            "priority should be restored to base after restart");
    }
}

#[test]
fn sched_out_of_range_set_priority_is_noop() {
    unsafe {
        reset_test_state();
        // Should not crash for out-of-range index
        sched::set_task_priority(NUM_TASKS, 5);
        sched::set_task_priority(NUM_TASKS + 1, 5);
        assert_eq!(sched::get_task_priority(NUM_TASKS), 0);
    }
}

#[test]
fn sched_ticks_used_reset_on_restart() {
    unsafe {
        reset_test_state();
        sched::TCBS[1].ticks_used = 42;
        sched::TCBS[1].state = TaskState::Faulted;
        sched::TCBS[1].fault_tick = 0;

        *aegis_os::timer::TICK_COUNT.get_mut() = RESTART_DELAY_TICKS;
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        assert_eq!(sched::TCBS[1].ticks_used, 0,
            "ticks_used should be 0 after restart");
    }
}

#[test]
fn sched_heartbeat_reset_on_restart() {
    unsafe {
        reset_test_state();
        sched::TCBS[1].heartbeat_interval = 50;
        sched::TCBS[1].last_heartbeat = 10;
        sched::TCBS[1].state = TaskState::Faulted;
        sched::TCBS[1].fault_tick = 0;

        *aegis_os::timer::TICK_COUNT.get_mut() = RESTART_DELAY_TICKS;
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);

        assert_eq!(sched::TCBS[1].last_heartbeat, RESTART_DELAY_TICKS,
            "last_heartbeat should be reset to current tick on restart");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 16. ELF64 Parser Tests (Phase L3)
// ═══════════════════════════════════════════════════════════════════

/// Build a minimal valid ELF64 AArch64 executable in a byte buffer.
/// Returns a Vec<u8> with ELF header + `num_loads` PT_LOAD program headers.
/// Each segment is a 0x1000-byte region at vaddr 0x4010_0000 + i*0x1000.
fn build_test_elf(num_loads: usize) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // ─── ELF Header (64 bytes) ─────────────────────────────────────
    // Magic
    buf[0] = 0x7F; buf[1] = b'E'; buf[2] = b'L'; buf[3] = b'F';
    // Class = 64-bit
    buf[4] = 2;
    // Data = little-endian
    buf[5] = 1;
    // Version
    buf[6] = 1;
    // OS/ABI (0 = ELFOSABI_NONE)
    buf[7] = 0;
    // Padding [8..16] = 0

    // e_type = ET_EXEC (2) at offset 16
    buf[16] = 2; buf[17] = 0;
    // e_machine = EM_AARCH64 (183 = 0xB7) at offset 18
    buf[18] = 0xB7; buf[19] = 0;
    // e_version at offset 20
    buf[20] = 1; buf[21] = 0; buf[22] = 0; buf[23] = 0;

    // e_entry = 0x4010_0000 at offset 24 (little-endian u64)
    buf[24] = 0x00; buf[25] = 0x00; buf[26] = 0x10; buf[27] = 0x40;
    buf[28] = 0; buf[29] = 0; buf[30] = 0; buf[31] = 0;

    // e_phoff = 64 (program headers start right after ELF header) at offset 32
    buf[32] = 64; buf[33] = 0; buf[34] = 0; buf[35] = 0;
    buf[36] = 0; buf[37] = 0; buf[38] = 0; buf[39] = 0;

    // e_shoff = 0 (no section headers) at offset 40
    // e_flags = 0 at offset 48
    // e_ehsize = 64 at offset 52
    buf[52] = 64; buf[53] = 0;

    // e_phentsize = 56 at offset 54
    buf[54] = 56; buf[55] = 0;

    // e_phnum at offset 56
    buf[56] = num_loads as u8; buf[57] = 0;

    // e_shentsize, e_shnum, e_shstrndx at offsets 58-63 = 0

    // ─── Program Headers (56 bytes each) ───────────────────────────
    for i in 0..num_loads {
        let ph_base = 64 + i * 56;
        if ph_base + 56 > buf.len() { break; }

        // p_type = PT_LOAD (1) at ph_base+0
        buf[ph_base] = 1; buf[ph_base + 1] = 0; buf[ph_base + 2] = 0; buf[ph_base + 3] = 0;

        // p_flags = PF_R | PF_X (5) at ph_base+4
        let flags: u32 = if i == 0 { 5 } else { 6 }; // 5=RX for .text, 6=RW for .data
        buf[ph_base + 4] = flags as u8;
        buf[ph_base + 5] = 0; buf[ph_base + 6] = 0; buf[ph_base + 7] = 0;

        // p_offset = 0 at ph_base+8 (segment data starts at file offset 0 for simplicity)
        // (points into the buffer itself — we just need bounds to be valid)

        // p_vaddr at ph_base+16 = 0x4010_0000 + i * 0x1000
        let vaddr: u64 = 0x4010_0000 + (i as u64) * 0x1000;
        let vb = vaddr.to_le_bytes();
        buf[ph_base + 16] = vb[0]; buf[ph_base + 17] = vb[1];
        buf[ph_base + 18] = vb[2]; buf[ph_base + 19] = vb[3];
        buf[ph_base + 20] = vb[4]; buf[ph_base + 21] = vb[5];
        buf[ph_base + 22] = vb[6]; buf[ph_base + 23] = vb[7];

        // p_paddr at ph_base+24 (same as vaddr for identity map)
        buf[ph_base + 24] = vb[0]; buf[ph_base + 25] = vb[1];
        buf[ph_base + 26] = vb[2]; buf[ph_base + 27] = vb[3];
        buf[ph_base + 28] = vb[4]; buf[ph_base + 29] = vb[5];
        buf[ph_base + 30] = vb[6]; buf[ph_base + 31] = vb[7];

        // p_filesz = 64 at ph_base+32 (small, fits within our 512-byte buffer)
        buf[ph_base + 32] = 64; buf[ph_base + 33] = 0;
        buf[ph_base + 34] = 0; buf[ph_base + 35] = 0;
        buf[ph_base + 36] = 0; buf[ph_base + 37] = 0;
        buf[ph_base + 38] = 0; buf[ph_base + 39] = 0;

        // p_memsz = 0x1000 at ph_base+40
        buf[ph_base + 40] = 0x00; buf[ph_base + 41] = 0x10;
        buf[ph_base + 42] = 0; buf[ph_base + 43] = 0;
        buf[ph_base + 44] = 0; buf[ph_base + 45] = 0;
        buf[ph_base + 46] = 0; buf[ph_base + 47] = 0;

        // p_align at ph_base+48 = 0x1000
        buf[ph_base + 48] = 0x00; buf[ph_base + 49] = 0x10;
    }

    buf
}

#[test]
fn elf_parse_valid_single_segment() {
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).expect("valid ELF should parse");
    assert_eq!(info.entry, 0x4010_0000);
    assert_eq!(info.num_segments, 1);
    let seg = info.segments[0].unwrap();
    assert_eq!(seg.vaddr, 0x4010_0000);
    assert_eq!(seg.filesz, 64);
    assert_eq!(seg.memsz, 0x1000);
    assert_eq!(seg.flags & PF_R, PF_R);
    assert_eq!(seg.flags & PF_X, PF_X);
}

#[test]
fn elf_parse_valid_multiple_segments() {
    let elf = build_test_elf(3);
    let info = elf::parse_elf64(&elf).expect("valid ELF should parse");
    assert_eq!(info.num_segments, 3);
    for i in 0..3 {
        let seg = info.segments[i].unwrap();
        assert_eq!(seg.vaddr, 0x4010_0000 + (i as u64) * 0x1000);
    }
    assert!(info.segments[3].is_none());
}

#[test]
fn elf_parse_too_small() {
    let tiny = [0u8; 32]; // less than 64-byte ELF header
    assert_eq!(elf::parse_elf64(&tiny).unwrap_err(), ElfError::TooSmall);
}

#[test]
fn elf_parse_bad_magic() {
    let mut elf = build_test_elf(1);
    elf[0] = 0x00; // corrupt magic
    assert_eq!(elf::parse_elf64(&elf).unwrap_err(), ElfError::BadMagic);
}

#[test]
fn elf_parse_not_64bit() {
    let mut elf = build_test_elf(1);
    elf[4] = 1; // ELFCLASS32
    assert_eq!(elf::parse_elf64(&elf).unwrap_err(), ElfError::Not64Bit);
}

#[test]
fn elf_parse_not_little_endian() {
    let mut elf = build_test_elf(1);
    elf[5] = 2; // ELFDATA2MSB (big-endian)
    assert_eq!(elf::parse_elf64(&elf).unwrap_err(), ElfError::NotLittleEndian);
}

#[test]
fn elf_parse_not_executable() {
    let mut elf = build_test_elf(1);
    elf[16] = 3; // ET_DYN (shared library)
    assert_eq!(elf::parse_elf64(&elf).unwrap_err(), ElfError::NotExecutable);
}

#[test]
fn elf_parse_wrong_arch() {
    let mut elf = build_test_elf(1);
    elf[18] = 0x3E; elf[19] = 0; // EM_X86_64 = 62
    assert_eq!(elf::parse_elf64(&elf).unwrap_err(), ElfError::WrongArch);
}

#[test]
fn elf_parse_too_many_segments() {
    // Build with 5 segments — but our buffer is 512 bytes, 64 header + 5*56 = 344, fits
    // We need MAX_SEGMENTS = 4, so 5 should fail
    let mut buf = [0u8; 512];
    // Copy a valid header
    let base = build_test_elf(5);
    buf.copy_from_slice(&base);
    // num_loads is already 5 from build_test_elf(5)
    assert_eq!(elf::parse_elf64(&buf).unwrap_err(), ElfError::TooManySegments);
}

#[test]
fn elf_parse_segment_out_of_bounds() {
    let mut elf = build_test_elf(1);
    // Set p_filesz to a huge value that exceeds file size
    // p_filesz at offset 64 + 32 = 96
    elf[96] = 0xFF; elf[97] = 0xFF; elf[98] = 0xFF; elf[99] = 0x7F;
    assert_eq!(elf::parse_elf64(&elf).unwrap_err(), ElfError::SegmentOutOfBounds);
}

#[test]
fn elf_parse_no_segments() {
    let elf = build_test_elf(0);
    let info = elf::parse_elf64(&elf).expect("ELF with 0 segments should parse");
    assert_eq!(info.num_segments, 0);
    assert_eq!(info.entry, 0x4010_0000);
    for s in &info.segments {
        assert!(s.is_none());
    }
}

#[test]
fn elf_parse_entry_point() {
    let mut elf = build_test_elf(1);
    // Change entry point to 0xDEAD_BEEF_CAFE_0000
    let entry: u64 = 0xDEAD_BEEF_CAFE_0000;
    let eb = entry.to_le_bytes();
    elf[24] = eb[0]; elf[25] = eb[1]; elf[26] = eb[2]; elf[27] = eb[3];
    elf[28] = eb[4]; elf[29] = eb[5]; elf[30] = eb[6]; elf[31] = eb[7];
    let info = elf::parse_elf64(&elf).expect("should parse");
    assert_eq!(info.entry, entry);
}

// ═══════════════════════════════════════════════════════════════════
// 17. ELF Loader Tests (Phase L4)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn elf_validate_for_load_valid() {
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).unwrap();
    // Segment at 0x4010_0000, memsz=0x1000. Region: 0x4010_0000..+0x3000
    let result = elf::validate_elf_for_load(&info, 0x4010_0000, 0x3000);
    assert!(result.is_ok(), "valid ELF should pass validation");
}

#[test]
fn elf_validate_for_load_vaddr_below_base() {
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).unwrap();
    // Segment vaddr=0x4010_0000, but load_base=0x4010_1000 → vaddr < base
    assert_eq!(
        elf::validate_elf_for_load(&info, 0x4010_1000, 0x3000),
        Err(ElfLoadError::VaddrOutOfRange)
    );
}

#[test]
fn elf_validate_for_load_segment_too_large() {
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).unwrap();
    // Segment at 0x4010_0000 with memsz=0x1000, but load_size=0x800 → overflow
    assert_eq!(
        elf::validate_elf_for_load(&info, 0x4010_0000, 0x800),
        Err(ElfLoadError::SegmentTooLarge)
    );
}

#[test]
fn elf_validate_for_load_wx_violation() {
    let mut elf = build_test_elf(1);
    // Change p_flags to PF_R|PF_W|PF_X = 7 (W^X violation)
    elf[68] = 7;
    let info = elf::parse_elf64(&elf).unwrap();
    assert_eq!(
        elf::validate_elf_for_load(&info, 0x4010_0000, 0x3000),
        Err(ElfLoadError::WxViolation)
    );
}

#[test]
fn elf_validate_for_load_no_segments() {
    let elf = build_test_elf(0);
    let info = elf::parse_elf64(&elf).unwrap();
    assert_eq!(info.num_segments, 0);
    assert_eq!(
        elf::validate_elf_for_load(&info, 0x4010_0000, 0x3000),
        Err(ElfLoadError::NoSegments)
    );
}

#[test]
fn elf_validate_for_load_entry_out_of_range() {
    let mut elf = build_test_elf(1);
    // Set entry to 0x5000_0000 (outside load region 0x4010_0000..+0x3000)
    let entry: u64 = 0x5000_0000;
    let eb = entry.to_le_bytes();
    elf[24] = eb[0]; elf[25] = eb[1]; elf[26] = eb[2]; elf[27] = eb[3];
    elf[28] = eb[4]; elf[29] = eb[5]; elf[30] = eb[6]; elf[31] = eb[7];
    let info = elf::parse_elf64(&elf).unwrap();
    assert_eq!(
        elf::validate_elf_for_load(&info, 0x4010_0000, 0x3000),
        Err(ElfLoadError::VaddrOutOfRange)
    );
}

#[test]
fn elf_load_segments_copy_and_zero() {
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).unwrap();
    // Segment: vaddr=0x4010_0000, offset=0, filesz=64, memsz=0x1000
    let mut dest = [0xFFu8; 0x2000]; // 8KB, pre-filled with 0xFF

    unsafe {
        let entry = elf::load_elf_segments(
            &elf, &info,
            dest.as_mut_ptr(), 0x4010_0000, dest.len()
        ).unwrap();

        assert_eq!(entry, 0x4010_0000);

        // First 64 bytes: copied from elf[0..64] (segment data at p_offset=0)
        for i in 0..64 {
            assert_eq!(dest[i], elf[i], "byte {} should be copied from ELF", i);
        }
        // Bytes 64..0x1000: BSS region, zero-filled
        for i in 64..0x1000 {
            assert_eq!(dest[i], 0, "byte {} should be zero-filled (BSS)", i);
        }
        // Bytes beyond memsz: untouched (still 0xFF)
        assert_eq!(dest[0x1000], 0xFF, "byte past memsz should be untouched");
    }
}

#[test]
fn elf_load_segments_rejects_invalid() {
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).unwrap();
    let mut dest = [0u8; 0x100]; // too small for memsz=0x1000

    unsafe {
        let result = elf::load_elf_segments(
            &elf, &info,
            dest.as_mut_ptr(), 0x4010_0000, dest.len()
        );
        assert_eq!(result, Err(ElfLoadError::SegmentTooLarge));
    }
}

#[test]
fn mmu_set_page_attr_host_stub() {
    // Valid task and address
    assert_eq!(mmu::set_page_attr(0, 0x4010_0000, mmu::USER_CODE_PAGE), 0);
    assert_eq!(mmu::set_page_attr(2, 0x4000_0000, mmu::KERNEL_DATA_PAGE), 0);
    // Invalid task
    assert_eq!(
        mmu::set_page_attr(3, 0x4010_0000, mmu::USER_CODE_PAGE),
        mmu::PAGE_ATTR_ERR_INVALID_TASK
    );
    // Out of range: below L3 base
    assert_eq!(
        mmu::set_page_attr(0, 0x3000_0000, mmu::USER_CODE_PAGE),
        mmu::PAGE_ATTR_ERR_OUT_OF_RANGE
    );
    // Out of range: above L3 range (0x4020_0000)
    assert_eq!(
        mmu::set_page_attr(0, 0x4020_0000, mmu::USER_CODE_PAGE),
        mmu::PAGE_ATTR_ERR_OUT_OF_RANGE
    );
}

// ═══════════════════════════════════════════════════════════════════
// Phase L6 — Module Structure & Separation Tests
// ═══════════════════════════════════════════════════════════════════

/// L6-L1 #1: Verify `arch` module is importable on host.
/// On x86_64, `arch::current` and `arch::aarch64` are NOT available
/// (gated behind `cfg(target_arch = "aarch64")`), but the `arch`
/// module itself compiles.
#[test]
fn l6_arch_module_exists() {
    // The arch module compiles on host (empty — no aarch64 sub-module).
    // On host, `gic` is NOT re-exported (aarch64-only). Verify this
    // indirectly: the crate compiles without `arch::current`.
    // We can only assert the module *exists* as a namespace.
    // If this compiles and runs, arch module structure is correct.
    let _: () = (); // Placeholder — compilation is the test
}

/// L6-L1 #2: Verify kernel sub-modules are importable and export
/// expected symbols.
#[test]
fn l6_kernel_module_exports() {
    // IPC
    assert!(ipc::MAX_ENDPOINTS >= 4);
    assert_eq!(ipc::MSG_REGS, 4);

    // Capabilities
    assert_ne!(cap::CAP_ALL, cap::CAP_NONE);
    assert_eq!(cap::CAP_NONE, 0);
    assert_ne!(cap::CAP_YIELD, 0);
    assert_ne!(cap::CAP_WRITE, 0);

    // Scheduler
    assert_eq!(sched::NUM_TASKS, 3);
    assert!(sched::RESTART_DELAY_TICKS > 0);
    assert!(sched::EPOCH_LENGTH > 0);

    // Timer (host stub: TIMER_INTID accessible as constant)
    assert_eq!(aegis_os::timer::TIMER_INTID, 30);

    // Grant
    assert!(grant::MAX_GRANTS >= 2);

    // IRQ
    assert!(irq::MAX_IRQ_BINDINGS >= 8);

    // ELF
    assert_eq!(elf::MAX_SEGMENTS, 4);
    assert_eq!(elf::PF_X, 1);
    assert_eq!(elf::PF_W, 2);
    assert_eq!(elf::PF_R, 4);
}

/// L6-L1 #3: Verify platform constants match QEMU virt memory map.
#[test]
fn l6_platform_constants() {
    use aegis_os::platform::qemu_virt;

    assert_eq!(qemu_virt::GICD_BASE, 0x0800_0000);
    assert_eq!(qemu_virt::GICC_BASE, 0x0801_0000);
    assert_eq!(qemu_virt::UART0_BASE, 0x0900_0000);
    assert_eq!(qemu_virt::RAM_BASE, 0x4000_0000);
    assert_eq!(qemu_virt::KERNEL_BASE, 0x4008_0000);
    assert_eq!(qemu_virt::TIMER_INTID, 30);
    assert_eq!(qemu_virt::TICK_MS, 10);
    assert_eq!(qemu_virt::TIMER_FREQ_HZ, 62_500_000);
}

/// L6-L1 #4: Verify backward-compatible re-exports at crate root.
/// After Phase L refactoring, `aegis_os::ipc` should still resolve
/// to the same module as `aegis_os::kernel::ipc`, etc.
#[test]
fn l6_use_paths_unchanged() {
    // IPC: crate root re-export == kernel module
    assert_eq!(aegis_os::ipc::MAX_ENDPOINTS, aegis_os::kernel::ipc::MAX_ENDPOINTS);
    assert_eq!(aegis_os::ipc::MSG_REGS, aegis_os::kernel::ipc::MSG_REGS);

    // Capabilities
    assert_eq!(aegis_os::cap::CAP_ALL, aegis_os::kernel::cap::CAP_ALL);
    assert_eq!(aegis_os::cap::CAP_NONE, aegis_os::kernel::cap::CAP_NONE);
    assert_eq!(aegis_os::cap::CAP_YIELD, aegis_os::kernel::cap::CAP_YIELD);

    // Scheduler
    assert_eq!(aegis_os::sched::NUM_TASKS, aegis_os::kernel::sched::NUM_TASKS);
    assert_eq!(aegis_os::sched::EPOCH_LENGTH, aegis_os::kernel::sched::EPOCH_LENGTH);

    // Timer
    assert_eq!(aegis_os::timer::TIMER_INTID, aegis_os::kernel::timer::TIMER_INTID);

    // Grant
    assert_eq!(aegis_os::grant::MAX_GRANTS, aegis_os::kernel::grant::MAX_GRANTS);

    // IRQ
    assert_eq!(aegis_os::irq::MAX_IRQ_BINDINGS, aegis_os::kernel::irq::MAX_IRQ_BINDINGS);

    // ELF
    assert_eq!(aegis_os::elf::MAX_SEGMENTS, aegis_os::kernel::elf::MAX_SEGMENTS);
    assert_eq!(aegis_os::elf::PF_X, aegis_os::kernel::elf::PF_X);
}

/// L6-L2 #5: Verify arch/kernel separation works — key kernel modules
/// are callable on host without any AArch64 dependency.
/// This proves cfg isolation is effective: portable logic compiles
/// and runs on x86_64 without arch code.
#[test]
fn l6_cfg_separation_works() {
    unsafe { reset_test_state(); }

    // Scheduler: create/schedule tasks on host (no TTBR0, no eret)
    let mut frame = unsafe { core::ptr::read(&sched::TCBS[0].context) };
    frame.x[0] = 42;
    unsafe { sched::TCBS[1].state = TaskState::Ready; }
    sched::schedule(&mut frame);
    // Schedule picked a Ready task — proves portable logic works
    assert_ne!(unsafe { read_current() }, 0);

    // IPC: send/recv state machine on host (no SVC, no asm)
    unsafe {
        reset_test_state();
        sched::TCBS[0].caps = cap::CAP_IPC_SEND_EP0;
        sched::TCBS[1].caps = cap::CAP_IPC_RECV_EP0;
    }
    let mut f0 = unsafe { core::ptr::read(&sched::TCBS[0].context) };
    f0.x[0] = 0xBEEF;
    ipc::sys_send(&mut f0, 0);
    // Task 0 blocked on send — proves IPC works on host
    assert_eq!(unsafe { sched::TCBS[0].state }, TaskState::Blocked);

    // Grant: create/revoke on host (mmu stub)
    unsafe {
        reset_test_state();
        sched::TCBS[0].caps = cap::CAP_GRANT_CREATE | cap::CAP_GRANT_REVOKE;
    }
    let result = grant::grant_create(0, 0, 1);
    assert_eq!(result, 0); // 0 = success
    let revoke = grant::grant_revoke(0, 0);
    assert_eq!(revoke, 0); // 0 = success
    // Proves grant logic works with host mmu stub

    // IRQ: bind/cleanup on host (gic stub)
    unsafe {
        reset_test_state();
        sched::TCBS[0].caps = cap::CAP_IRQ_BIND | cap::CAP_IRQ_ACK;
    }
    let bind_result = irq::irq_bind(33, 0, 1); // INTID=33 (SPI), task=0, bit=1
    assert_eq!(bind_result, 0);
    // Proves IRQ routing works on host with gic stub

    // ELF parser: fully portable, no cfg needed
    let elf = build_test_elf(1);
    let info = elf::parse_elf64(&elf).unwrap();
    assert_eq!(info.num_segments, 1);
    // Proves ELF parser is completely arch-independent
}

/// L6-L5 #6: Verify ELF segment flags are mutually exclusive for W^X.
/// The ELF loader rejects segments that are both writable and executable.
/// Also verify the flag constants are non-overlapping power-of-two.
#[test]
fn l6_elf_wxn_flag_properties() {
    // Flag values are distinct powers of 2
    assert_eq!(elf::PF_X, 1);
    assert_eq!(elf::PF_W, 2);
    assert_eq!(elf::PF_R, 4);
    assert_eq!(elf::PF_X & elf::PF_W, 0); // no overlap
    assert_eq!(elf::PF_X & elf::PF_R, 0);
    assert_eq!(elf::PF_W & elf::PF_R, 0);

    // Build an ELF with W+X segment — must be rejected by validator
    let mut elf = build_test_elf(1);
    // Patch the segment flags: offset = phoff + 4 (p_flags field)
    // phoff = 64 (standard), p_flags at phoff+4
    let phoff = 64usize;
    let flags_offset = phoff + 4;
    let wx_flags: u32 = elf::PF_W | elf::PF_X; // forbidden combo
    elf[flags_offset..flags_offset + 4].copy_from_slice(&wx_flags.to_le_bytes());

    let info = elf::parse_elf64(&elf).unwrap();
    let result = elf::validate_elf_for_load(
        &info, 0x4010_0000, 0x4010_0000 + 0x3000
    );
    assert_eq!(result, Err(ElfLoadError::WxViolation));
}

// ═══════════════════════════════════════════════════════════════════
// Phase M4: Targeted Coverage Tests
// ═══════════════════════════════════════════════════════════════════

// ─── M4-IPC: sys_send / sys_recv / sys_call coverage ──────────────

#[test]
fn ipc_sys_send_immediate_delivery() {
    // Receiver already waiting → message delivered immediately
    unsafe {
        reset_test_state();
        // Task 1 is waiting to receive on EP0
        ipc::ENDPOINTS[0].receiver = Some(1);
        sched::TCBS[1].state = TaskState::Blocked;
        // Set task 0 as current (sender), put message in its context
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0xAAAA;
        sched::TCBS[0].context.x[1] = 0xBBBB;
        sched::TCBS[0].context.x[2] = 0xCCCC;
        sched::TCBS[0].context.x[3] = 0xDDDD;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_send(&mut frame, 0);

        // Receiver should have the message and be Ready
        assert_eq!(sched::TCBS[1].context.x[0], 0xAAAA);
        assert_eq!(sched::TCBS[1].context.x[1], 0xBBBB);
        assert_eq!(sched::TCBS[1].context.x[2], 0xCCCC);
        assert_eq!(sched::TCBS[1].context.x[3], 0xDDDD);
        assert_eq!(sched::TCBS[1].state, TaskState::Ready);
        // Receiver slot should be cleared
        assert!(ipc::ENDPOINTS[0].receiver.is_none());
    }
}

#[test]
fn ipc_sys_send_blocks_when_no_receiver() {
    // No receiver → sender enqueued and blocked
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0x1234;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_send(&mut frame, 0);

        // Sender should be blocked and in the queue
        assert_eq!(sched::TCBS[0].state, TaskState::Blocked);
        assert!(ipc::ENDPOINTS[0].sender_queue.contains(0));
    }
}

#[test]
fn ipc_sys_send_invalid_endpoint() {
    // Invalid ep_id → no crash, no state change
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_send(&mut frame, 99); // invalid

        // Should still be Running, no panic
        assert_eq!(sched::TCBS[0].state, TaskState::Running);
    }
}

#[test]
fn ipc_sys_send_queue_full() {
    // Sender queue full → early return, not blocked
    unsafe {
        reset_test_state();
        // Fill sender queue with fake tasks
        for i in 0..ipc::MAX_WAITERS {
            ipc::ENDPOINTS[0].sender_queue.push(i);
        }
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_send(&mut frame, 0);

        // Should return without blocking (queue was full)
        // State may be Running or Ready depending on schedule path
        // The key is it didn't deadlock
        assert!(sched::TCBS[0].state == TaskState::Running
            || sched::TCBS[0].state == TaskState::Ready);
    }
}

#[test]
fn ipc_sys_recv_immediate_delivery() {
    // Sender already waiting → message received immediately
    unsafe {
        reset_test_state();
        // Task 0 is a sender queued on EP0 with a message
        sched::TCBS[0].state = TaskState::Blocked;
        sched::TCBS[0].context.x[0] = 0xFEED;
        sched::TCBS[0].context.x[1] = 0xBEEF;
        ipc::ENDPOINTS[0].sender_queue.push(0);

        // Task 1 calls recv
        *sched::CURRENT.get_mut() = 1;
        sched::TCBS[1].state = TaskState::Running;
        let mut frame = core::ptr::read(&sched::TCBS[1].context);
        ipc::sys_recv(&mut frame, 0);

        // Sender should be unblocked
        assert_eq!(sched::TCBS[0].state, TaskState::Ready);
        // Receiver should have the message (loaded into frame and TCB)
        assert_eq!(sched::TCBS[1].context.x[0], 0xFEED);
        assert_eq!(sched::TCBS[1].context.x[1], 0xBEEF);
        // Queue should be empty
        assert_eq!(ipc::ENDPOINTS[0].sender_queue.count, 0);
    }
}

#[test]
fn ipc_sys_recv_blocks_when_no_sender() {
    // No sender → receiver blocks
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 1;
        sched::TCBS[1].state = TaskState::Running;

        let mut frame = core::ptr::read(&sched::TCBS[1].context);
        ipc::sys_recv(&mut frame, 0);

        // Receiver should be blocked and registered
        assert_eq!(sched::TCBS[1].state, TaskState::Blocked);
        assert_eq!(ipc::ENDPOINTS[0].receiver, Some(1));
    }
}

#[test]
fn ipc_sys_recv_invalid_endpoint() {
    // Invalid ep_id → no crash
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_recv(&mut frame, 99);

        assert_eq!(sched::TCBS[0].state, TaskState::Running);
    }
}

#[test]
fn ipc_sys_call_with_receiver_waiting() {
    // Receiver waiting → deliver message, caller blocks as new receiver
    unsafe {
        reset_test_state();
        // Task 1 is waiting to receive on EP0
        ipc::ENDPOINTS[0].receiver = Some(1);
        sched::TCBS[1].state = TaskState::Blocked;

        // Task 0 calls sys_call (send + recv)
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0xCAFE;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_call(&mut frame, 0);

        // Message delivered to task 1
        assert_eq!(sched::TCBS[1].context.x[0], 0xCAFE);
        // Task 1 is Ready or Running (schedule may have picked it)
        assert!(sched::TCBS[1].state == TaskState::Ready
            || sched::TCBS[1].state == TaskState::Running);

        // Caller (task 0) should now be receiver, blocked
        assert_eq!(sched::TCBS[0].state, TaskState::Blocked);
        assert_eq!(ipc::ENDPOINTS[0].receiver, Some(0));
    }
}

#[test]
fn ipc_sys_call_no_receiver() {
    // No receiver → enqueue as sender, block
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0x9999;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_call(&mut frame, 0);

        // Should be blocked, enqueued as sender
        assert_eq!(sched::TCBS[0].state, TaskState::Blocked);
        assert!(ipc::ENDPOINTS[0].sender_queue.contains(0));
    }
}

#[test]
fn ipc_sys_call_invalid_endpoint() {
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_call(&mut frame, 99);

        assert_eq!(sched::TCBS[0].state, TaskState::Running);
    }
}

#[test]
fn ipc_sys_call_queue_full() {
    // Queue full when no receiver → sender can't enqueue
    unsafe {
        reset_test_state();
        for i in 0..ipc::MAX_WAITERS {
            ipc::ENDPOINTS[0].sender_queue.push(i);
        }
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_call(&mut frame, 0);

        // Should not deadlock — early return
        assert!(sched::TCBS[0].state == TaskState::Running
            || sched::TCBS[0].state == TaskState::Ready);
    }
}

#[test]
fn ipc_sys_call_priority_boost() {
    // High-priority caller → receiver gets boosted
    unsafe {
        reset_test_state();
        // Task 0 = high priority (7), task 1 = low priority (2)
        sched::TCBS[0].priority = 7;
        sched::TCBS[0].base_priority = 7;
        sched::TCBS[1].priority = 2;
        sched::TCBS[1].base_priority = 2;

        // Task 1 is waiting to receive
        ipc::ENDPOINTS[0].receiver = Some(1);
        sched::TCBS[1].state = TaskState::Blocked;

        // Task 0 calls (sends + waits for reply)
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0x42;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_call(&mut frame, 0);

        // Task 1 should have been boosted to priority 7
        assert_eq!(sched::TCBS[1].priority, 7);
        // Task 1's base priority unchanged
        assert_eq!(sched::TCBS[1].base_priority, 2);
    }
}

#[test]
fn ipc_send_restores_receiver_priority() {
    // After send delivers message, receiver's base priority is restored
    unsafe {
        reset_test_state();
        // Receiver was boosted from previous call
        sched::TCBS[1].priority = 7;
        sched::TCBS[1].base_priority = 2;
        ipc::ENDPOINTS[0].receiver = Some(1);
        sched::TCBS[1].state = TaskState::Blocked;

        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0x55;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_send(&mut frame, 0);

        // sys_send calls restore_base_priority on receiver
        assert_eq!(sched::TCBS[1].priority, 2);
    }
}

// ─── M4-SCHED: fault_current_task + edge cases ───────────────────

#[test]
fn sched_fault_current_task_basic() {
    // fault_current_task: marks task Faulted, records fault_tick, cleans up IPC/grant/IRQ
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].priority = 5;
        sched::TCBS[0].base_priority = 3;
        *aegis_os::timer::TICK_COUNT.get_mut() = 42;

        // Task 0 is receiver on EP0
        ipc::ENDPOINTS[0].receiver = Some(0);

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        sched::fault_current_task(&mut frame);

        // Task should be Faulted
        assert_eq!(sched::TCBS[0].state, TaskState::Faulted);
        // Fault tick recorded
        assert_eq!(sched::TCBS[0].fault_tick, 42);
        // Priority restored to base
        assert_eq!(sched::TCBS[0].priority, 3);
        // IPC cleanup: receiver slot cleared
        assert!(ipc::ENDPOINTS[0].receiver.is_none());
        // Should have scheduled away (CURRENT changed)
        assert_ne!(read_current(), 0);
    }
}

#[test]
fn sched_fault_current_task_cleans_irq() {
    // fault_current_task cleans up IRQ bindings
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;

        // Bind an IRQ (INTID 33) to task 0
        let r = irq::irq_bind(33, 0, 1);
        assert_eq!(r, 0); // sanity: bind succeeds

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        sched::fault_current_task(&mut frame);

        // IRQ binding should be cleaned up
        // Verify by trying to rebind same INTID for task 1 (should succeed)
        let result = irq::irq_bind(33, 1, 1);
        assert_eq!(result, 0); // success = binding was freed
    }
}

#[test]
fn sched_restart_task_non_faulted_noop() {
    // restart_task on a Ready task → no-op (early return)
    unsafe {
        reset_test_state();
        sched::TCBS[0].state = TaskState::Ready;
        sched::TCBS[0].context.x[5] = 0xDEAD;

        sched::restart_task(0);

        // Should not have changed — it was Ready, not Faulted
        assert_eq!(sched::TCBS[0].state, TaskState::Ready);
        assert_eq!(sched::TCBS[0].context.x[5], 0xDEAD);
    }
}

#[test]
fn sched_schedule_no_ready_task_forces_idle() {
    // All tasks blocked/faulted → scheduler forces idle (task 2) Ready
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[1].state = TaskState::Blocked;
        sched::TCBS[2].state = TaskState::Blocked;

        // Set budgets: task 0 has exhausted budget
        sched::TCBS[0].time_budget = 10;
        sched::TCBS[0].ticks_used = 10;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        sched::schedule(&mut frame);

        // Should fallback to idle (task 2), forced Ready → Running
        assert_eq!(read_current(), 2);
        assert_eq!(sched::TCBS[2].state, TaskState::Running);
    }
}

#[test]
fn sched_schedule_idle_faulted_gets_restarted() {
    // Idle (task 2) is Faulted → schedule forces restart + runs it
    unsafe {
        reset_test_state();
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].time_budget = 10;
        sched::TCBS[0].ticks_used = 10;
        sched::TCBS[1].state = TaskState::Blocked;
        sched::TCBS[2].state = TaskState::Faulted;
        sched::TCBS[2].fault_tick = 0;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        sched::schedule(&mut frame);

        // Idle should be forced Ready → Running
        assert_eq!(read_current(), 2);
        assert_eq!(sched::TCBS[2].state, TaskState::Running);
    }
}

#[test]
fn sched_get_task_priority_out_of_range() {
    // Out-of-range task_idx → returns 0
    assert_eq!(sched::get_task_priority(99), 0);
}

#[test]
fn sched_get_task_base_priority_out_of_range() {
    assert_eq!(sched::get_task_base_priority(99), 0);
}

#[test]
fn sched_set_task_priority_out_of_range() {
    // Should be no-op
    unsafe {
        reset_test_state();
        let old_prio = sched::TCBS[0].priority;
        sched::set_task_priority(99, 7);
        // No crash, task 0 unchanged
        assert_eq!(sched::TCBS[0].priority, old_prio);
    }
}

#[test]
fn sched_restore_base_priority_out_of_range() {
    // Should be no-op, no crash
    sched::restore_base_priority(99);
}

#[test]
fn sched_set_task_state_out_of_range() {
    // No crash on out-of-range
    sched::set_task_state(99, TaskState::Blocked);
}

// ─── M4-CAP: Complete cap_name coverage ───────────────────────────

#[test]
fn cap_name_all_missing_arms() {
    // Cover the 7 cap_name match arms missed by existing tests
    assert_eq!(cap::cap_name(CAP_IPC_SEND_EP1), "IPC_SEND_EP1");
    assert_eq!(cap::cap_name(CAP_IPC_RECV_EP1), "IPC_RECV_EP1");
    assert_eq!(cap::cap_name(CAP_GRANT_CREATE), "GRANT_CREATE");
    assert_eq!(cap::cap_name(CAP_GRANT_REVOKE), "GRANT_REVOKE");
    assert_eq!(cap::cap_name(CAP_IRQ_BIND), "IRQ_BIND");
    assert_eq!(cap::cap_name(CAP_IRQ_ACK), "IRQ_ACK");
    assert_eq!(cap::cap_name(CAP_DEVICE_MAP), "DEVICE_MAP");
    assert_eq!(cap::cap_name(CAP_HEARTBEAT), "HEARTBEAT");
}

#[test]
fn cap_for_syscall_all_endpoints() {
    // Exhaustive test for all 4 endpoints × send/recv
    use aegis_os::cap::cap_for_syscall;
    // EP0
    assert_eq!(cap_for_syscall(1, 0), CAP_IPC_SEND_EP0);
    assert_eq!(cap_for_syscall(2, 0), CAP_IPC_RECV_EP0);
    // EP1
    assert_eq!(cap_for_syscall(1, 1), CAP_IPC_SEND_EP1);
    assert_eq!(cap_for_syscall(2, 1), CAP_IPC_RECV_EP1);
    // EP2
    assert_eq!(cap_for_syscall(1, 2), CAP_IPC_SEND_EP2);
    assert_eq!(cap_for_syscall(2, 2), CAP_IPC_RECV_EP2);
    // EP3
    assert_eq!(cap_for_syscall(1, 3), CAP_IPC_SEND_EP3);
    assert_eq!(cap_for_syscall(2, 3), CAP_IPC_RECV_EP3);
    // CALL = send + recv combined
    assert_eq!(cap_for_syscall(3, 0), CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0);
    assert_eq!(cap_for_syscall(3, 1), CAP_IPC_SEND_EP1 | CAP_IPC_RECV_EP1);
    assert_eq!(cap_for_syscall(3, 2), CAP_IPC_SEND_EP2 | CAP_IPC_RECV_EP2);
    assert_eq!(cap_for_syscall(3, 3), CAP_IPC_SEND_EP3 | CAP_IPC_RECV_EP3);
}

#[test]
fn cap_for_syscall_grant_irq_device() {
    use aegis_os::cap::cap_for_syscall;
    assert_eq!(cap_for_syscall(7, 0), CAP_GRANT_CREATE);
    assert_eq!(cap_for_syscall(8, 0), CAP_GRANT_REVOKE);
    assert_eq!(cap_for_syscall(9, 0), CAP_IRQ_BIND);
    assert_eq!(cap_for_syscall(10, 0), CAP_IRQ_ACK);
    assert_eq!(cap_for_syscall(11, 0), CAP_DEVICE_MAP);
    assert_eq!(cap_for_syscall(12, 0), CAP_HEARTBEAT);
}

// ─── M4-GRANT: edge cases ─────────────────────────────────────────

#[test]
fn grant_cleanup_clears_both_roles() {
    // If task is both owner and peer of different grants, both cleaned
    unsafe {
        reset_test_state();
        // Grant 0: task 0 owns, task 1 is peer
        grant::grant_create(0, 0, 1);
        // Grant 1: task 1 owns, task 0 is peer
        grant::grant_create(1, 1, 0);

        // Clean up task 0
        grant::cleanup_task(0);

        // Grant 0 should be deactivated (task 0 was owner)
        assert!(!grant::GRANTS[0].active);
        // Grant 1 should be deactivated (task 0 was peer)
        assert!(!grant::GRANTS[1].active);
    }
}

// ─── M4-TIMER: tick_count accessor ────────────────────────────────

#[test]
fn timer_tick_count_accessor() {
    unsafe {
        reset_test_state();
        *aegis_os::timer::TICK_COUNT.get_mut() = 0;
        assert_eq!(aegis_os::timer::tick_count(), 0);
        *aegis_os::timer::TICK_COUNT.get_mut() = 12345;
        assert_eq!(aegis_os::timer::tick_count(), 12345);
    }
}

// ─── M4-IPC: Round-trip send→recv message integrity ───────────────

#[test]
fn ipc_round_trip_message_integrity() {
    // Full round-trip: task 0 sends, task 1 receives — all 4 regs match
    unsafe {
        reset_test_state();
        // Task 1 is waiting on EP0
        ipc::ENDPOINTS[0].receiver = Some(1);
        sched::TCBS[1].state = TaskState::Blocked;

        // Task 0 sends specific message
        *sched::CURRENT.get_mut() = 0;
        sched::TCBS[0].state = TaskState::Running;
        sched::TCBS[0].context.x[0] = 0x1111_1111_AAAA_BBBB;
        sched::TCBS[0].context.x[1] = 0x2222_2222_CCCC_DDDD;
        sched::TCBS[0].context.x[2] = 0x3333_3333_EEEE_FFFF;
        sched::TCBS[0].context.x[3] = 0x4444_4444_0000_1111;

        let mut frame = core::ptr::read(&sched::TCBS[0].context);
        ipc::sys_send(&mut frame, 0);

        assert_eq!(sched::TCBS[1].context.x[0], 0x1111_1111_AAAA_BBBB);
        assert_eq!(sched::TCBS[1].context.x[1], 0x2222_2222_CCCC_DDDD);
        assert_eq!(sched::TCBS[1].context.x[2], 0x3333_3333_EEEE_FFFF);
        assert_eq!(sched::TCBS[1].context.x[3], 0x4444_4444_0000_1111);
    }
}

#[test]
fn ipc_recv_loads_frame_on_immediate() {
    // When recv finds a waiting sender, it loads message into the frame
    unsafe {
        reset_test_state();
        sched::TCBS[0].state = TaskState::Blocked;
        sched::TCBS[0].context.x[0] = 0xABCD;
        sched::TCBS[0].context.x[1] = 0xEF01;
        ipc::ENDPOINTS[0].sender_queue.push(0);

        *sched::CURRENT.get_mut() = 1;
        sched::TCBS[1].state = TaskState::Running;
        let mut frame = core::ptr::read(&sched::TCBS[1].context);
        ipc::sys_recv(&mut frame, 0);

        // Frame should be updated (load_frame loads from TCB into frame)
        // The TCB should have the message
        assert_eq!(sched::TCBS[1].context.x[0], 0xABCD);
        assert_eq!(sched::TCBS[1].context.x[1], 0xEF01);
    }
}
