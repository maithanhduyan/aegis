/// AegisOS — Portable kernel modules
///
/// Contains architecture-independent logic: IPC, capabilities, scheduler
/// policy, grant management, IRQ routing, timer tick logic.
/// Phase L1: ipc.rs and cap.rs moved here.
/// Phase L2: sched.rs, timer.rs, grant.rs, irq.rs moved here.

pub mod ipc;
pub mod cap;
pub mod sched;
pub mod timer;
pub mod grant;
pub mod irq;
