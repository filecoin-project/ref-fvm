// TODO: remove this when we hookup these syscalls.
#![allow(unused)]

use std::collections::HashMap;
use std::iter;

use anyhow::Context as _;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::encoding::{Cbor, DAG_CBOR};
use fvm_shared::error::ErrorNumber::IllegalArgument;
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::{sys, ActorID};
use wasmtime::{Caller, Trap};

use super::Context;
use crate::kernel::{BlockId, ClassifyResult, ExecutionError, Result, SyscallError};
use crate::{syscall_error, Kernel};

/// Verifies that a signature is valid for an address and plaintext.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_signature(
    mut context: Context<'_, impl Kernel>,
    sig_off: u32, // Signature
    sig_len: u32,
    addr_off: u32, // Address
    addr_len: u32,
    plaintext_off: u32,
    plaintext_len: u32,
) -> Result<i32> {
    let sig: Signature = context.memory.read_cbor(sig_off, sig_len)?;
    let addr: Address = context.memory.read_address(addr_off, addr_len)?;
    // plaintext doesn't need to be a mutable borrow, but otherwise we would be
    // borrowing the ctx both immutably and mutably.
    let plaintext = context.memory.try_slice(plaintext_off, plaintext_len)?;
    context
        .kernel
        .verify_signature(&sig, &addr, plaintext)
        .map(|v| if v { 0 } else { -1 })
}

/// Hashes input data using blake2b with 256 bit output.
///
/// The output buffer must be sized to 32 bytes.
pub fn hash_blake2b(
    mut context: Context<'_, impl Kernel>,
    data_off: u32,
    data_len: u32,
) -> Result<[u8; 32]> {
    let data = context.memory.try_slice(data_off, data_len)?;
    context.kernel.hash_blake2b(data)
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
    let pieces: Vec<PieceInfo> = context.memory.read_cbor(pieces_off, pieces_len)?;
    let typ = RegisteredSealProof::from(proof_type); // TODO handle Invalid?
    let cid = context
        .kernel
        .compute_unsealed_sector_cid(typ, pieces.as_slice())?;
    let mut out = context.memory.try_slice_mut(cid_off, cid_len)?;

    // The CID lib should really return the number of bytes written...
    // cid.write_bytes(&mut out).or_fatal()
    let bytes = cid.to_bytes();
    let len = bytes.len();
    if len > out.len() {
        return Err(syscall_error!(
            IllegalArgument;
            "output buffer too small; CID length: {}, buffer length: {}", len, out.len())
        .into());
    }
    out[..bytes.len()].copy_from_slice(bytes.as_slice());
    Ok(bytes.len() as u32)
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
    let batch = context
        .memory
        .read_cbor::<Vec<SealVerifyInfo>>(batch_off, batch_len)?;

    let mut result = context.kernel.batch_verify_seals(&batch)?;
    let output = context
        .memory
        .try_slice_mut(result_off, result.len() as u32)?;
    unsafe {
        output.copy_from_slice(&*(&*result as *const [bool] as *const [u8]));
    }
    Ok(())
}
