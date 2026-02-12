/// AegisOS Shared Memory Grant Module
///
/// Allows two tasks to share a specific physical memory page under
/// kernel-controlled access. The owner creates a grant, mapping the
/// page into both tasks' L3 page tables as AP_RW_EL0. Revoking
/// unmaps the peer's access (sets entry back to AP_RW_EL1).
///
/// Grant pages are statically allocated in the `.grant_pages` linker
/// section — no heap, no dynamic allocation.
///
/// Syscalls:
///   SYS_GRANT_CREATE = 7: owner grants a page to a peer task
///   SYS_GRANT_REVOKE = 8: owner revokes peer's access

use crate::kernel::cell::KernelCell;
use crate::sched;
use crate::uart_print;

// ─── Constants ─────────────────────────────────────────────────────

/// Maximum number of grant pages (statically allocated in linker.ld)
pub const MAX_GRANTS: usize = 2;

/// Grant page size (must match linker.ld allocation)
pub const GRANT_PAGE_SIZE: usize = 4096;

// ─── Grant struct ──────────────────────────────────────────────────

/// A shared memory grant — tracks who owns and shares a page.
#[derive(Clone, Copy)]
pub struct Grant {
    /// Task that created the grant (None = slot unused)
    pub owner: Option<usize>,
    /// Task that was granted access (None = not shared)
    pub peer: Option<usize>,
    /// Physical address of the grant page
    pub phys_addr: u64,
    /// Whether this grant is currently active
    pub active: bool,
}

pub const EMPTY_GRANT: Grant = Grant {
    owner: None,
    peer: None,
    phys_addr: 0,
    active: false,
};

// ─── Static grant table ────────────────────────────────────────────

pub static GRANTS: KernelCell<[Grant; MAX_GRANTS]> = KernelCell::new([EMPTY_GRANT; MAX_GRANTS]);

// ─── Grant page addresses (from linker) ────────────────────────────

/// Get the physical address of grant page `grant_id`.
/// Returns None if grant_id is out of range.
#[cfg(target_arch = "aarch64")]
pub fn grant_page_addr(grant_id: usize) -> Option<u64> {
    if grant_id >= MAX_GRANTS {
        return None;
    }
    extern "C" {
        static __grant_pages_start: u8;
    }
    // SAFETY: Linker-provided symbol, address taken for grant page calculation.
    let base = unsafe { &__grant_pages_start as *const u8 as u64 };
    Some(base + (grant_id as u64) * GRANT_PAGE_SIZE as u64)
}

/// Host-test stub: return a fake but distinct address per grant.
#[cfg(not(target_arch = "aarch64"))]
pub fn grant_page_addr(grant_id: usize) -> Option<u64> {
    if grant_id >= MAX_GRANTS {
        return None;
    }
    // Fake addresses within the first 2MiB (L3 range) for test purposes
    Some(0x4010_0000_u64 + (grant_id as u64) * GRANT_PAGE_SIZE as u64)
}

// ─── Core operations ───────────────────────────────────────────────

/// Create a shared memory grant.
/// `grant_id`: which grant page (0..MAX_GRANTS)
/// `owner`: task creating the grant (current task)
/// `peer`: task receiving shared access
///
/// Returns 0 on success, error code on failure:
///   0xFFFF_0001 = invalid grant_id
///   0xFFFF_0002 = grant already active
///   0xFFFF_0003 = invalid peer
///   0xFFFF_0004 = owner == peer
pub fn grant_create(grant_id: usize, owner: usize, peer: usize) -> u64 {
    if grant_id >= MAX_GRANTS {
        uart_print("!!! GRANT: invalid grant_id\n");
        return 0xFFFF_0001;
    }

    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        if (*GRANTS.get_mut())[grant_id].active {
            uart_print("!!! GRANT: already active\n");
            return 0xFFFF_0002;
        }

        if peer >= sched::NUM_TASKS {
            uart_print("!!! GRANT: invalid peer\n");
            return 0xFFFF_0003;
        }

        if owner == peer {
            uart_print("!!! GRANT: owner == peer\n");
            return 0xFFFF_0004;
        }

        let phys = match grant_page_addr(grant_id) {
            Some(addr) => addr,
            None => return 0xFFFF_0001,
        };

        // Map grant page into both tasks' L3 page tables
        #[cfg(target_arch = "aarch64")]
        {
            crate::mmu::map_grant_for_task(phys, owner);
            crate::mmu::map_grant_for_task(phys, peer);
        }

        (*GRANTS.get_mut())[grant_id] = Grant {
            owner: Some(owner),
            peer: Some(peer),
            phys_addr: phys,
            active: true,
        };

        uart_print("[AegisOS] GRANT: task ");
        crate::uart_print_hex(owner as u64);
        uart_print(" -> task ");
        crate::uart_print_hex(peer as u64);
        uart_print(" (grant ");
        crate::uart_print_hex(grant_id as u64);
        uart_print(")\n");
    }

    0 // success
}

/// Revoke a shared memory grant.
/// `grant_id`: which grant to revoke
/// `caller`: task requesting revoke (must be owner)
///
/// Returns 0 on success, error code on failure.
pub fn grant_revoke(grant_id: usize, caller: usize) -> u64 {
    if grant_id >= MAX_GRANTS {
        uart_print("!!! GRANT: invalid grant_id\n");
        return 0xFFFF_0001;
    }

    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        if !(*GRANTS.get_mut())[grant_id].active {
            return 0; // no-op: already inactive
        }

        if (*GRANTS.get_mut())[grant_id].owner != Some(caller) {
            uart_print("!!! GRANT: caller is not owner\n");
            return 0xFFFF_0005;
        }

        // Unmap from peer's page table
        if let Some(peer) = (*GRANTS.get_mut())[grant_id].peer {
            #[cfg(target_arch = "aarch64")]
            {
                crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[grant_id].phys_addr, peer);
            }
        }

        (*GRANTS.get_mut())[grant_id].active = false;
        (*GRANTS.get_mut())[grant_id].peer = None;

        uart_print("[AegisOS] GRANT REVOKED: grant ");
        crate::uart_print_hex(grant_id as u64);
        uart_print("\n");
    }

    0 // success
}

// ─── Fault cleanup ─────────────────────────────────────────────────

/// Clean up all grants involving a faulted task.
/// If the task is owner: revoke grant (unmap peer).
/// If the task is peer: unmap peer's access.
/// Called from sched::fault_current_task() and sched::restart_task().
pub fn cleanup_task(task_idx: usize) {
    // SAFETY: Single-core kernel, interrupts masked during kernel execution. No concurrent access on uniprocessor QEMU virt.
    unsafe {
        for i in 0..MAX_GRANTS {
            if !(*GRANTS.get_mut())[i].active {
                continue;
            }

            if (*GRANTS.get_mut())[i].owner == Some(task_idx) {
                // Task is owner — unmap peer and deactivate
                if let Some(peer) = (*GRANTS.get_mut())[i].peer {
                    #[cfg(target_arch = "aarch64")]
                    {
                        crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, peer);
                    }
                }
                // Also unmap from owner (faulted task gets fresh state on restart)
                #[cfg(target_arch = "aarch64")]
                {
                    crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, task_idx);
                }
                (*GRANTS.get_mut())[i] = EMPTY_GRANT;
            } else if (*GRANTS.get_mut())[i].peer == Some(task_idx) {
                // Task is peer — unmap peer's access, keep owner's grant active but no peer
                #[cfg(target_arch = "aarch64")]
                {
                    crate::mmu::unmap_grant_for_task((*GRANTS.get_mut())[i].phys_addr, task_idx);
                }
                (*GRANTS.get_mut())[i].peer = None;
                (*GRANTS.get_mut())[i].active = false;
            }
        }
    }
}
