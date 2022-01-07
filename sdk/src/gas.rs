use crate::sys;
use crate::SyscallResult;

/// Charge gas for the operation identified by name.
pub fn charge(name: &str, compute: u64) -> SyscallResult<()> {
    unsafe { sys::gas::charge(name.as_ptr(), name.len() as u32, compute) }
}
