// TODO: remove this when we hookup these syscalls.
#![allow(unused)]

use std::collections::HashMap;
use std::{cmp, iter};

use anyhow::{anyhow, Context as _};
use cid::Cid;
use fvm_ipld_encoding::{Cbor, DAG_CBOR};
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::signature::{Signature, SignatureType};
use fvm_shared::error::ErrorNumber::IllegalArgument;
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
    WindowPoStVerifyInfo,
};
use fvm_shared::{sys, ActorID};
use num_traits::FromPrimitive;

use super::Context;
use crate::kernel::{BlockId, ClassifyResult, ExecutionError, Result, SyscallError};
use crate::{syscall_error, Kernel};

/// Verifies that a signature is valid for an address and plaintext.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
#[allow(clippy::too_many_arguments)]
pub fn verify_signature(
    mut context: Context<'_, impl Kernel>,
    sig_type: u32,
    sig_off: u32,
    sig_len: u32,
    addr_off: u32,
    addr_len: u32,
    plaintext_off: u32,
    plaintext_len: u32,
) -> Result<i32> {
    let sig_type = SignatureType::from_u32(sig_type)
        .with_context(|| format!("unknown signature type {}", sig_type))
        .or_illegal_argument()?;
    let sig_bytes = context.memory.try_slice(sig_off, sig_len)?;
    let addr = context.memory.read_address(addr_off, addr_len)?;
    let plaintext = context.memory.try_slice(plaintext_off, plaintext_len)?;

    context
        .kernel
        .verify_signature(sig_type, sig_bytes, &addr, plaintext)
        .map(|v| if v { 0 } else { -1 })
}

/// Hashes input data using the specified hash function, writing the digest into the provided
/// buffer.
pub fn hash(
    mut context: Context<'_, impl Kernel>,
    hash_code: u64,
    data_off: u32, // input
    data_len: u32,
    digest_off: u32, // output
    digest_len: u32,
) -> Result<u32> {
    // Check the digest bounds first so we don't do any work if they're incorrect.
    context.memory.check_bounds(digest_off, digest_len)?;

    // Then hash.
    let digest = {
        let data = context.memory.try_slice(data_off, data_len)?;
        context.kernel.hash(hash_code, data)?
    };

    // Then copy the result.
    let digest_out = context.memory.try_slice_mut(digest_off, digest_len)?;
    let length = cmp::min(digest_out.len(), digest.len());
    digest_out[..length].copy_from_slice(&digest[..length]);
    Ok(length as u32)
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs
/// (CommPs) and sizes.
///
/// Writes the CID in the provided output buffer.
pub fn compute_unsealed_sector_cid(
    mut context: Context<'_, impl Kernel>,
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

/// Verifies a sector seal proof.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_seal(
    mut context: Context<'_, impl Kernel>,
    info_off: u32, // SealVerifyInfo
    info_len: u32,
) -> Result<i32> {
    let info = context
        .memory
        .read_cbor::<SealVerifyInfo>(info_off, info_len)?;
    context
        .kernel
        .verify_seal(&info)
        .map(|v| if v { 0 } else { -1 })
}

/// Verifies a window proof of spacetime.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_post(
    mut context: Context<'_, impl Kernel>,
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
    context: Context<'_, impl Kernel>,
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
    mut context: Context<'_, impl Kernel>,
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
    mut context: Context<'_, impl Kernel>,
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
    mut context: Context<'_, impl Kernel>,
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
    let mut result = context.kernel.batch_verify_seals(&batch)?;

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
