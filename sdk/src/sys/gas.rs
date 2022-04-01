super::fvm_syscalls! {
    module = "gas";

    /// Charge gas for submitting a seal proof for bulk verify.
    pub fn on_submit_verify_seal() -> Result<()>;
}
