use crate::sys;

/// Charge gas for the operation identified by name.
pub fn on_submit_verify_seal() {
    unsafe { sys::gas::on_submit_verify_seal() }.expect("failed to charge gas for bulk verify")
}
