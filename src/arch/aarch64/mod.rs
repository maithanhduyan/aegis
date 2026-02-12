/// AegisOS — AArch64 architecture module
///
/// Contains all AArch64-specific code: GIC driver, boot assembly.
/// Phase L1: gic.rs moved here. boot.s lives here for include_str!.
/// Phase L2 will add: vectors.rs, trap.rs, context.rs, mmu.rs, timer.rs,
///                     syscall.rs, bootstrap.rs.

pub mod gic;
