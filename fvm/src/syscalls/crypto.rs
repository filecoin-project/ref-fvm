// TODO: remove this when we hookup these syscalls.
#![allow(unused)]

use crate::kernel::{BlockId, ClassifyResult, ExecutionError, Result};
use crate::Kernel;
use anyhow::Context as _;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::encoding::{Cbor, DAG_CBOR};
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use std::collections::HashMap;
use wasmtime::{Caller, Trap};

use super::Context;

/// Verifies that a signature is valid for an address and plaintext.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_signature(
    caller: &mut Caller<'_, impl Kernel>,
    sig_off: u32, // Signature
    sig_len: u32,
    addr_off: u32, // Address
    addr_len: u32,
    plaintext_off: u32,
    plaintext_len: u32,
) -> Result<i32> {
    let (kernel, memory) = caller.kernel_and_memory()?;
    let sig: Signature = memory.read_cbor(sig_off, sig_len)?;
    let addr: Address = memory.read_address(addr_off, addr_len)?;
    // plaintext doesn't need to be a mutable borrow, but otherwise we would be
    // borrowing the ctx both immutably and mutably.
    let plaintext = memory.try_slice(plaintext_len, plaintext_off)?;
    kernel
        .verify_signature(&sig, &addr, plaintext)
        .map(|v| if v { 0 } else { -1 })
}

/// Hashes input data using blake2b with 256 bit output.
///
/// The output buffer must be sized to 32 bytes.
pub fn hash_blake2b(
    caller: &mut Caller<'_, impl Kernel>,
    data_off: u32,
    data_len: u32,
    obuf_off: u32,
) -> Result<()> {
    const HASH_LEN: usize = 32;

    let (kernel, mut memory) = caller.kernel_and_memory()?;
    let hash = {
        let data = memory.try_slice(data_len, data_off)?;
        kernel.hash_blake2b(data)?
    };
    assert_eq!(hash.len(), 32);
    let obuf = memory.try_slice_mut(obuf_off, HASH_LEN as u32)?;
    obuf.copy_from_slice(&hash[..HASH_LEN]);
    Ok(())
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs
/// (CommPs) and sizes.
///
/// Writes the CID in the provided output buffer.
pub fn compute_unsealed_sector_cid(
    caller: &mut Caller<'_, impl Kernel>,
    proof_type: i64, // RegisteredSealProof,
    pieces_off: u32, // [PieceInfo]
    pieces_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<()> {
    let (kernel, mut memory) = caller.kernel_and_memory()?;
    let pieces: Vec<PieceInfo> = memory.read_cbor(pieces_off, pieces_len)?;
    let typ = RegisteredSealProof::from(proof_type); // TODO handle Invalid?
    let cid = kernel.compute_unsealed_sector_cid(typ, pieces.as_slice())?;
    let mut out_slice = memory.try_slice_mut(cid_off, cid_len)?;

    cid.write_bytes(&mut out_slice).or_fatal()
}

/// Verifies a sector seal proof.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_seal(
    caller: &mut Caller<'_, impl Kernel>,
    info_off: u32, // SealVerifyInfo
    info_len: u32,
) -> Result<i32> {
    let (kernel, memory) = caller.kernel_and_memory()?;
    let info = memory.read_cbor::<SealVerifyInfo>(info_off, info_len)?;
    kernel.verify_seal(&info).map(|v| if v { 0 } else { -1 })
}

/// Verifies a window proof of spacetime.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_post(
    caller: &mut Caller<'_, impl Kernel>,
    info_off: u32, // WindowPoStVerifyInfo,
    info_len: u32,
) -> Result<i32> {
    let (kernel, memory) = caller.kernel_and_memory()?;
    let info = memory.read_cbor::<WindowPoStVerifyInfo>(info_off, info_len)?;
    kernel.verify_post(&info).map(|v| if v { 0 } else { -1 })
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
/// Returns nil and an error if the headers don't prove a fault.
///
/// The return i32 indicates the status code of the verification:
///  - 0: consensus fault recognized, along with the block id where to find the
///       ConsensusFault report.
///  - -1: no consensus fault recognized.
pub fn verify_consensus_fault(
    caller: &mut Caller<'_, impl Kernel>,
    h1_off: u32,
    h1_len: u32,
    h2_off: u32,
    h2_len: u32,
    extra_off: u32,
    extra_len: u32,
) -> Result<(i32, BlockId)> {
    let (kernel, memory) = caller.kernel_and_memory()?;

    let h1 = memory.try_slice(h1_off, h1_len)?;
    let h2 = memory.try_slice(h2_off, h2_len)?;
    let extra = memory.try_slice(extra_off, extra_len)?;

    // TODO the extern should only signal an error in case there was an internal
    //  interrupting error evaluating the consensus fault. If the outcome is
    //  "no consensus fault was found", the extern should not error, as doing so
    //  would interrupt execution via the Trap (at least currently).
    let ret = kernel.verify_consensus_fault(h1, h2, extra)?;

    match ret {
        // Consensus fault detected; return the actor as a block.
        Some(fault) => {
            let ser = fault
                .marshal_cbor()
                .context("failed to marshal consensus fault result")
                .or_fatal()?;
            kernel
                .block_create(DAG_CBOR, ser.as_slice())
                .map(|bid| (1, bid))
                .or_illegal_argument() // TODO: not quite the correct error...
        }
        // No consensus fault.
        None => Ok((0, 0)),
    }
}

/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_aggregate_seals(
    caller: &mut Caller<'_, impl Kernel>,
    agg_off: u32, // AggregateSealVerifyProofAndInfos
    agg_len: u32,
) -> Result<i32> {
    let (kernel, memory) = caller.kernel_and_memory()?;
    let info = memory.read_cbor::<AggregateSealVerifyProofAndInfos>(agg_off, agg_len)?;
    kernel
        .verify_aggregate_seals(&info)
        .map(|v| if v { 0 } else { -1 })
}

// TODO implement
fn batch_verify_seals(
    caller: Caller<'_, impl Kernel>,
    vis: &[(&Address, &[SealVerifyInfo])],
) -> Result<HashMap<Address, Vec<i32>>> {
    todo!()
}
