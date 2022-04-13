//! Syscalls for cryptographic operations.

#[doc(inline)]
pub use fvm_shared::sys::out::crypto::*;

super::fvm_syscalls! {
    module = "crypto";

    /// Verifies that a signature is valid for an address and plaintext.
    ///
    /// Returns 0 on success, or -1 if the signature fails to validate.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                               |
    /// |-------------------|------------------------------------------------------|
    /// | `IllegalArgument` | signature, address, or plaintext buffers are invalid |
    pub fn verify_signature(
        sig_off: *const u8,
        sig_len: u32,
        addr_off: *const u8,
        addr_len: u32,
        plaintext_off: *const u8,
        plaintext_len: u32,
    ) -> Result<i32>;

    /// Hashes input data using blake2b with 256 bit output.
    ///
    /// The output buffer must be sized to a minimum of 32 bytes.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                          |
    /// |-------------------|-------------------------------------------------|
    /// | `IllegalArgument` | the input buffer does not point to valid memory |
    pub fn hash_blake2b(
        data_off: *const u8,
        data_len: u32,
    ) -> Result<[u8; 32]>;

    /// Computes an unsealed sector CID (CommD) from its constituent piece CIDs
    /// (CommPs) and sizes.
    ///
    /// Writes the CID in the provided output buffer, and returns the length of
    /// the written CID.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                   |
    /// |-------------------|--------------------------|
    /// | `IllegalArgument` | an argument is malformed |
    pub fn compute_unsealed_sector_cid(
        proof_type: i64,
        pieces_off: *const u8,
        pieces_len: u32,
        cid_off: *mut u8,
        cid_len: u32,
    ) -> Result<u32>;

    /// Verifies a sector seal proof.
    ///
    /// Returns 0 to indicate that the proof was valid, -1 otherwise.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                   |
    /// |-------------------|--------------------------|
    /// | `IllegalArgument` | an argument is malformed |
    pub fn verify_seal(info_off: *const u8, info_len: u32) -> Result<i32>;

    /// Verifies a window proof of spacetime.
    ///
    /// Returns 0 to indicate that the proof was valid, -1 otherwise.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                   |
    /// |-------------------|--------------------------|
    /// | `IllegalArgument` | an argument is malformed |
    pub fn verify_post(info_off: *const u8, info_len: u32) -> Result<i32>;

    /// Verifies that two block headers provide proof of a consensus fault.
    ///
    /// Returns a 0 status if a consensus fault was recognized, along with the
    /// BlockId containing the fault details. Otherwise, a -1 status is returned,
    /// and the second result parameter must be ignored.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                |
    /// |-------------------|---------------------------------------|
    /// | `LimitExceeded`   | exceeded lookback limit finding block |
    /// | `IllegalArgument` | an argument is malformed              |
    pub fn verify_consensus_fault(
        h1_off: *const u8,
        h1_len: u32,
        h2_off: *const u8,
        h2_len: u32,
        extra_off: *const u8,
        extra_len: u32,
    ) -> Result<VerifyConsensusFault>;

    /// Verifies an aggregated batch of sector seal proofs.
    ///
    /// Returns 0 to indicate that the proof was valid, -1 otherwise.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                         |
    /// |-------------------|--------------------------------|
    /// | `LimitExceeded`   | exceeds seal aggregation limit |
    /// | `IllegalArgument` | an argument is malformed       |
    pub fn verify_aggregate_seals(agg_off: *const u8, agg_len: u32) -> Result<i32>;

    /// Verifies a replica update proof.
    ///
    /// Returns 0 to indicate that the proof was valid, -1 otherwise.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                        |
    /// |-------------------|-------------------------------|
    /// | `LimitExceeded`   | exceeds replica update limit  |
    /// | `IllegalArgument` | an argument is malformed      |
    pub fn verify_replica_update(rep_off: *const u8, rep_len: u32) -> Result<i32>;

    /// Verifies an aggregated batch of sector seal proofs.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                   |
    /// |-------------------|--------------------------|
    /// | `IllegalArgument` | an argument is malformed |
    pub fn batch_verify_seals(batch_off: *const u8, batch_len: u32, result_off: *const u8) -> Result<()>;
}
