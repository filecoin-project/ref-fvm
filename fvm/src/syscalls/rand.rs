use crate::{
    kernel::{ClassifyResult, Result},
    Kernel,
};
use anyhow::Context as _;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use num_traits::FromPrimitive;
use wasmtime::Caller;

use super::Context;

const RAND_LEN: usize = 32;

/// Gets 32 bytes of randomness from the ticket chain.
/// The supplied output buffer must have at least 32 bytes of capacity.
/// If this syscall succeeds, exactly 32 bytes will be written starting at the
/// supplied offset.
pub fn get_chain_randomness(
    caller: &mut Caller<'_, impl Kernel>,
    pers: i64,  // DomainSeparationTag
    round: i64, // ChainEpoch
    entropy_off: u32,
    entropy_len: u32,
    obuf_off: u32,
) -> Result<()> {
    let (k, mut mem) = caller.kernel_and_memory()?;
    let entropy = mem.try_slice(entropy_off, entropy_len)?;
    // TODO determine if this error should lead to an abort.
    let pers = DomainSeparationTag::from_i64(pers)
        .context("invalid domain separation tag")
        .or_illegal_argument()?;
    let randomness = k.get_randomness_from_tickets(pers, round, entropy)?;
    assert_eq!(randomness.0.len(), RAND_LEN);

    let obuf = mem.try_slice_mut(obuf_off, RAND_LEN as u32)?;
    obuf.copy_from_slice(randomness.0.as_slice());
    Ok(())
}

/// Gets 32 bytes of randomness from the beacon system (currently Drand).
/// The supplied output buffer must have at least 32 bytes of capacity.
/// If this syscall succeeds, exactly 32 bytes will be written starting at the
/// supplied offset.
pub fn get_beacon_randomness(
    caller: &mut Caller<'_, impl Kernel>,
    pers: i64,  // DomainSeparationTag
    round: i64, // ChainEpoch
    entropy_off: u32,
    entropy_len: u32,
    obuf_off: u32,
) -> Result<()> {
    let (k, mut mem) = caller.kernel_and_memory()?;
    let entropy = mem.try_slice(entropy_off, entropy_len)?;
    // TODO determine if this error should lead to an abort.
    let pers = DomainSeparationTag::from_i64(pers)
        .context("invalid domain separation tag")
        .or_illegal_argument()?;
    let randomness = k.get_randomness_from_beacon(pers, round, entropy)?;
    assert_eq!(randomness.0.len(), RAND_LEN);

    let obuf = mem.try_slice_mut(obuf_off, RAND_LEN as u32)?;
    obuf.copy_from_slice(randomness.0.as_slice());
    Ok(())
}
