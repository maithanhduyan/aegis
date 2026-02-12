/// AegisOS Architecture Abstraction Layer
///
/// Phase L: All arch-specific code lives under `arch/<target>/`.
/// Portable kernel code references `arch::current::*` which resolves
/// to the active architecture module at compile time.

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64 as current;
