use crate::sys;

/// Charge gas for the operation identified by name.
///
/// Panics inside validate context
pub fn charge(name: &str, compute: u64) {
    unsafe { sys::gas::charge(name.as_ptr(), name.len() as u32, compute) }
        // can only happen if name isn't utf8, memory corruption, etc.
        .expect("failed to charge gas")
}
