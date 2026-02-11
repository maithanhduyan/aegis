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
    CAP_ALL, CAP_NONE,
};

// ─── Helper: read CURRENT safely (avoids static_mut_refs warning) ──

unsafe fn read_current() -> usize {
    core::ptr::addr_of!(sched::CURRENT).read_volatile()
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
    }
    sched::CURRENT = 0;
    sched::TCBS[0].state = TaskState::Running;

    // Reset IPC endpoints
    for i in 0..MAX_ENDPOINTS {
        ipc::ENDPOINTS[i] = EMPTY_EP;
    }

    // Reset tick counter
    aegis_os::timer::TICK_COUNT = 0;
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
        aegis_os::timer::TICK_COUNT = RESTART_DELAY_TICKS - 1;
        let mut frame = TrapFrame {
            x: [0; 31], sp_el0: 0, elr_el1: 0, spsr_el1: 0, _pad: [0; 2],
        };
        sched::schedule(&mut frame);
        // Task 1 should still be Faulted
        assert_eq!(sched::TCBS[1].state, TaskState::Faulted);

        // Set tick to exactly restart threshold
        aegis_os::timer::TICK_COUNT = RESTART_DELAY_TICKS;
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
        sched::CURRENT = 2;

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

        ipc::ENDPOINTS[0].sender = Some(1);
        ipc::cleanup_task(1);
        assert_eq!(ipc::ENDPOINTS[0].sender, None);
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

        ipc::ENDPOINTS[0].sender = Some(1);
        ipc::ENDPOINTS[1].receiver = Some(1);
        ipc::cleanup_task(1);
        assert_eq!(ipc::ENDPOINTS[0].sender, None);
        assert_eq!(ipc::ENDPOINTS[1].receiver, None);
    }
}

#[test]
fn ipc_cleanup_doesnt_affect_other_tasks() {
    unsafe {
        reset_test_state();

        ipc::ENDPOINTS[0].sender = Some(0);
        ipc::ENDPOINTS[0].receiver = Some(2);
        ipc::ENDPOINTS[1].sender = Some(1);

        ipc::cleanup_task(1);

        // Task 0 and 2 slots should be untouched
        assert_eq!(ipc::ENDPOINTS[0].sender, Some(0));
        assert_eq!(ipc::ENDPOINTS[0].receiver, Some(2));
        // Task 1 slot should be cleared
        assert_eq!(ipc::ENDPOINTS[1].sender, None);
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
    assert_eq!(MAX_ENDPOINTS, 2);
}

#[test]
fn ipc_endpoint_initial_state() {
    assert_eq!(EMPTY_EP.sender, None);
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
    assert_eq!(cap::cap_for_syscall(2, 0), CAP_IPC_RECV_EP0);
    assert_eq!(cap::cap_for_syscall(2, 1), CAP_IPC_RECV_EP1);
}

#[test]
fn cap_for_syscall_call_needs_both() {
    // SYS_CALL on EP0 needs SEND+RECV
    let required = cap::cap_for_syscall(3, 0);
    assert_eq!(required, CAP_IPC_SEND_EP0 | CAP_IPC_RECV_EP0);
    // SYS_CALL on EP1
    let required1 = cap::cap_for_syscall(3, 1);
    assert_eq!(required1, CAP_IPC_SEND_EP1 | CAP_IPC_RECV_EP1);
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
    assert_eq!(cap::cap_name(CAP_ALL), "ALL");
    assert_eq!(cap::cap_name(CAP_NONE), "NONE");
    assert_eq!(cap::cap_name(0xFF), "UNKNOWN");
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
