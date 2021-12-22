use crate::error::{IntoSyscallResult, SyscallResult};
use crate::sys;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::randomness::{Randomness, RANDOMNESS_LENGTH};

/// Gets 32 bytes of randomness from the ticket chain.
/// The supplied output buffer must have at least 32 bytes of capacity.
/// If this syscall succeeds, exactly 32 bytes will be written starting at the
/// supplied offset.
pub fn get_chain_randomness(
    dst: DomainSeparationTag,
    round: ChainEpoch,
    entropy: &[u8],
) -> SyscallResult<Randomness> {
    let mut ret = [0u8; RANDOMNESS_LENGTH];
    unsafe {
        sys::rand::get_chain_randomness(
            dst as i64,
            round as i64,
            entropy.as_ptr(),
            entropy.len() as u32,
            ret.as_mut_ptr(),
        )
        .into_syscall_result()?
    }
    Ok(Randomness(ret.to_vec()))
}

/// Gets 32 bytes of randomness from the beacon system (currently Drand).
/// The supplied output buffer must have at least 32 bytes of capacity.
/// If this syscall succeeds, exactly 32 bytes will be written starting at the
/// supplied offset.
pub fn get_beacon_randomness(
    dst: DomainSeparationTag,
    round: ChainEpoch,
    entropy: &[u8],
) -> SyscallResult<Randomness> {
    let mut ret = [0u8; RANDOMNESS_LENGTH];
    unsafe {
        sys::rand::get_beacon_randomness(
            dst as i64,
            round as i64,
            entropy.as_ptr(),
            entropy.len() as u32,
            ret.as_mut_ptr(),
        )
        .into_syscall_result()?
    }
    Ok(Randomness(ret.to_vec()))
}
