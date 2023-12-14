// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::panic;

use fvm_ipld_encoding::de::DeserializeOwned;
use fvm_shared::error::ErrorNumber;
use fvm_shared::sector::WindowPoStVerifyInfo;

use super::context::Memory;
use super::Context;
use crate::kernel::ClassifyResult;
use crate::kernel::{filecoin::FilecoinKernel, Result};
use crate::syscall_error;
use anyhow::anyhow;
use anyhow::Context as _;
use fvm_ipld_encoding::from_slice;
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
};
use fvm_shared::sys;

/// Private extension trait for reading CBOR. This operation is not safe to call on untrusted
/// (user-controlled) memory.
trait ReadCbor {
    fn read_cbor<T: DeserializeOwned>(&self, offset: u32, len: u32) -> Result<T>;
}

impl ReadCbor for Memory {
    /// Read a CBOR object from actor memory.
    ///
    /// **WARNING:** CBOR decoding is complex and this function offers no way to perform gas
    /// accounting. Only call this on data from _trusted_ (built-in) actors.
    ///
    /// On failure, this method returns an [`ErrorNumber::IllegalArgument`] error.
    fn read_cbor<T: DeserializeOwned>(&self, offset: u32, len: u32) -> Result<T> {
        let bytes = self.try_slice(offset, len)?;
        // Catch panics when decoding cbor from actors, _just_ in case.
        match panic::catch_unwind(|| from_slice(bytes).or_error(ErrorNumber::IllegalArgument)) {
            Ok(v) => v,
            Err(e) => {
                log::error!("panic when decoding cbor from actor: {:?}", e);
                Err(syscall_error!(IllegalArgument; "panic when decoding cbor from actor").into())
            }
        }
    }
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs
/// (CommPs) and sizes.
///
/// Writes the CID in the provided output buffer.
pub fn compute_unsealed_sector_cid(
    context: Context<'_, impl FilecoinKernel>,
    proof_type: i64, // RegisteredSealProof,
    pieces_off: u32, // [PieceInfo]
    pieces_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<u32> {
    // Check/read all arguments.
    let typ = RegisteredSealProof::from(proof_type);
    if let RegisteredSealProof::Invalid(invalid) = typ {
        return Err(syscall_error!(IllegalArgument; "invalid proof type {}", invalid).into());
    }
    let pieces: Vec<PieceInfo> = context.memory.read_cbor(pieces_off, pieces_len)?;
    context.memory.check_bounds(cid_off, cid_len)?;

    // Compute
    let cid = context
        .kernel
        .compute_unsealed_sector_cid(typ, pieces.as_slice())?;

    // REturn
    context.memory.write_cid(&cid, cid_off, cid_len)
}

/// Verifies a window proof of spacetime.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_post(
    context: Context<'_, impl FilecoinKernel>,
    info_off: u32, // WindowPoStVerifyInfo,
    info_len: u32,
) -> Result<i32> {
    let info = context
        .memory
        .read_cbor::<WindowPoStVerifyInfo>(info_off, info_len)?;
    context
        .kernel
        .verify_post(&info)
        .map(|v| if v { 0 } else { -1 })
}

/// Verifies that two block headers provide proof of a consensus fault:
/// - both headers mined by the same actor
/// - headers are different
/// - first header is of the same or lower epoch as the second
/// - at least one of the headers appears in the current chain at or after epoch `earliest`
/// - the headers provide evidence of a fault (see the spec for the different fault types).
/// The parameters are all serialized block headers. The third "extra" parameter is consulted only for
/// the "parent grinding fault", in which case it must be the sibling of h1 (same parent tipset) and one of the
/// blocks in the parent of h2 (i.e. h2's grandparent).
///
pub fn verify_consensus_fault(
    context: Context<'_, impl FilecoinKernel>,
    h1_off: u32,
    h1_len: u32,
    h2_off: u32,
    h2_len: u32,
    extra_off: u32,
    extra_len: u32,
) -> Result<sys::out::crypto::VerifyConsensusFault> {
    let h1 = context.memory.try_slice(h1_off, h1_len)?;
    let h2 = context.memory.try_slice(h2_off, h2_len)?;
    let extra = context.memory.try_slice(extra_off, extra_len)?;

    let ret = context.kernel.verify_consensus_fault(h1, h2, extra)?;

    match ret {
        // Consensus fault detected
        Some(fault) => Ok(sys::out::crypto::VerifyConsensusFault {
            fault: fault.fault_type as u32,
            epoch: fault.epoch,
            target: fault
                .target
                .id()
                .context("kernel returned non-id target address")
                .or_fatal()?,
        }),
        // No consensus fault.
        None => Ok(sys::out::crypto::VerifyConsensusFault {
            fault: 0,
            epoch: 0,
            target: 0,
        }),
    }
}

/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_aggregate_seals(
    context: Context<'_, impl FilecoinKernel>,
    agg_off: u32, // AggregateSealVerifyProofAndInfos
    agg_len: u32,
) -> Result<i32> {
    let info = context
        .memory
        .read_cbor::<AggregateSealVerifyProofAndInfos>(agg_off, agg_len)?;
    context
        .kernel
        .verify_aggregate_seals(&info)
        .map(|v| if v { 0 } else { -1 })
}

/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_replica_update(
    context: Context<'_, impl FilecoinKernel>,
    rep_off: u32, // ReplicaUpdateInfo
    rep_len: u32,
) -> Result<i32> {
    let info = context
        .memory
        .read_cbor::<ReplicaUpdateInfo>(rep_off, rep_len)?;
    context
        .kernel
        .verify_replica_update(&info)
        .map(|v| if v { 0 } else { -1 })
}

/// Verify a batch of seals encoded as a CBOR array of `SealVerifyInfo`.
///
/// When successful, this method will write a single byte back into the array at `result_off` for
/// each result: 0 for failed, 1 for success.
pub fn batch_verify_seals(
    context: Context<'_, impl FilecoinKernel>,
    batch_off: u32,
    batch_len: u32,
    result_off: u32,
) -> Result<()> {
    // Check and decode params.
    let batch = context
        .memory
        .read_cbor::<Vec<SealVerifyInfo>>(batch_off, batch_len)?;
    let output = context
        .memory
        .try_slice_mut(result_off, batch.len() as u32)?;

    // Execute.
    let result = context.kernel.batch_verify_seals(&batch)?;

    // Sanity check that we got the correct number of results.
    if result.len() != batch.len() {
        return Err(anyhow!(
            "expected one result per input: {} != {}",
            batch.len(),
            result.len()
        ))
        .or_fatal();
    }

    // Return.
    unsafe {
        output.copy_from_slice(&*(&*result as *const [bool] as *const [u8]));
    }
    Ok(())
}

/// Returns the network circulating supply.
pub fn total_fil_circ_supply(
    context: Context<'_, impl FilecoinKernel>,
) -> Result<sys::TokenAmount> {
    context
        .kernel
        .total_fil_circ_supply()?
        .try_into()
        .context("circulating supply exceeds u128 limit")
        .or_fatal()
}
