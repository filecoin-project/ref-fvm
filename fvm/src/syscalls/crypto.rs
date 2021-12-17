use crate::kernel::ExecutionError;
use crate::syscalls::context::Context;
use crate::Kernel;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use std::collections::HashMap;
use wasmtime::{Caller, Trap};

/// Verifies that a signature is valid for an address and plaintext.
fn verify_signature(
    caller: Caller<'_, impl Kernel>,
    sig_off: u32, // Signature
    sig_len: u32,
    addr_off: u32, // Address
    addr_len: u32,
    plaintext_off: u32,
    plaintext_len: u32,
) -> Result<bool, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let sig: Signature = ctx.read_cbor(sig_off, sig_len)?;
    let addr: Address = ctx.read_address(addr_off, addr_len)?;
    // plaintext doesn't need to be a mutable borrow, but otherwise we would be
    // borrowing the ctx both immutably and mutably.
    let (plaintext, k) = ctx.try_slice_and_runtime(plaintext_len, plaintext_off)?;
    k.verify_signature(&sig, &addr, plaintext)
        .map_err(ExecutionError::from)
        .map_err(Trap::from)
}

/// Hashes input data using blake2b with 256 bit output.
///
/// The output buffer must be sized to 32 bytes.
fn hash_blake2b(
    caller: Caller<'_, impl Kernel>,
    data_off: u32,
    data_len: u32,
    obuf_off: u32,
) -> Result<(), Trap> {
    const HASH_LEN: usize = 32;
    let mut ctx = Context::new(caller).with_memory()?;
    // data doesn't need to be a mutable borrow, but otherwise we would be
    // borrowing the ctx both immutably and mutably.
    let (data, k) = ctx.try_slice_and_runtime(data_len, data_off)?;
    let hash = k.hash_blake2b(data)?;
    assert_eq!(hash.len(), 32);
    let mut obuf = ctx.try_slice_mut(obuf_off, HASH_LEN as u32)?;
    obuf.copy_from_slice(&hash[..HASH_LEN]);
    Ok(())
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs
/// (CommPs) and sizes.
fn compute_unsealed_sector_cid(
    caller: Caller<'_, impl Kernel>,
    proof_type: i64, // RegisteredSealProof,
    pieces_off: u32, // &[PieceInfo]
    pieces_len: u32,
) -> Result<Cid, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let pieces: Vec<PieceInfo> = ctx.read_cbor(pieces_off, pieces_len)?;
    let typ = RegisteredSealProof::from(proof_type); // TODO handle Invalid?
    Ok(ctx
        .data_mut()
        .compute_unsealed_sector_cid(typ, pieces.as_slice())?)
}

/// Verifies a sector seal proof.
fn verify_seal(
    caller: Caller<'_, impl Kernel>,
    info_off: u32, // &SealVerifyInfo
    info_len: u32,
) -> Result<bool, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let info = ctx.read_cbor::<SealVerifyInfo>(info_off, info_len)?;
    ctx.data_mut()
        .verify_seal(&info)
        .map_err(ExecutionError::from)
        .map_err(Trap::from)
}

/// Verifies a window proof of spacetime.
fn verify_post(
    caller: Caller<'_, impl Kernel>,
    info_off: u32, // &WindowPoStVerifyInfo,
    info_len: u32,
) -> Result<bool, Trap> {
    todo!()
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
fn verify_consensus_fault(
    caller: Caller<'_, impl Kernel>,
    h1_off: u32,
    h1_len: u32,
    h2_off: u32,
    h2_len: u32,
    extra_off: u32,
    extra_len: u32,
) -> Result<Option<ConsensusFault>, Trap> {
    todo!()
}

fn batch_verify_seals(
    caller: Caller<'_, impl Kernel>,
    vis: &[(&Address, &[SealVerifyInfo])],
) -> Result<HashMap<Address, Vec<bool>>, Trap> {
    todo!()
}

fn verify_aggregate_seals(
    caller: Caller<'_, impl Kernel>,
    agg_off: u32, // &AggregateSealVerifyProofAndInfos
    agg_len: u32,
) -> Result<bool, Trap> {
    todo!()
}
