/// AegisOS — Portable kernel modules
///
/// Contains architecture-independent logic: IPC, capabilities, scheduler
/// policy, grant management, IRQ routing.
/// Phase L1: ipc.rs and cap.rs moved here.
/// Phase L2 will add: sched.rs, timer.rs, grant.rs, irq.rs, elf.rs.

pub mod ipc;
pub mod cap;
