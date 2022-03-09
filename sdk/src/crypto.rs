use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::encoding::{to_vec, Cbor};
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
    WindowPoStVerifyInfo,
};
use num_traits::FromPrimitive;

use crate::{status_code_to_bool, sys, SyscallResult, MAX_CID_LEN};

/// Verifies that a signature is valid for an address and plaintext.
#[allow(unused)]
pub fn verify_signature(
    signature: &Signature,
    signer: &Address,
    plaintext: &[u8],
) -> SyscallResult<bool> {
    let signature = signature
        .marshal_cbor()
        .expect("failed to marshal signature");
    let signer = signer.to_bytes();
    unsafe {
        sys::crypto::verify_signature(
            signature.as_ptr(),
            signature.len() as u32,
            signer.as_ptr(),
            signer.len() as u32,
            plaintext.as_ptr(),
            plaintext.len() as u32,
        )
        .map(status_code_to_bool)
    }
}

/// Hashes input data using blake2b with 256 bit output.
#[allow(unused)]
pub fn hash_blake2b(data: &[u8]) -> [u8; 32] {
    // This can only fail if we manage to pass in corrupted memory.
    unsafe { sys::crypto::hash_blake2b(data.as_ptr(), data.len() as u32) }
        .expect("failed to compute blake2b hash")
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
#[allow(unused)]
pub fn compute_unsealed_sector_cid(
    proof_type: RegisteredSealProof,
    pieces: &[PieceInfo],
) -> SyscallResult<Cid> {
    let pieces = to_vec(&pieces.to_vec()).expect("failed to marshal piece infos");
    let pieces = pieces.as_slice();
    let mut out = [0u8; MAX_CID_LEN];
    unsafe {
        let len = sys::crypto::compute_unsealed_sector_cid(
            i64::from(proof_type),
            pieces.as_ptr(),
            pieces.len() as u32,
            out.as_mut_ptr(),
            out.len() as u32,
        )?;
        assert!(
            len <= out.len() as u32,
            "CID too large: {} > {}",
            len,
            out.len()
        );
        Ok(Cid::read_bytes(&out[..len as usize]).expect("runtime returned an invalid CID"))
    }
}

/// Verifies a sector seal proof.
#[allow(unused)]
pub fn verify_seal(info: &SealVerifyInfo) -> SyscallResult<bool> {
    let info = info
        .marshal_cbor()
        .expect("failed to marshal seal verification input");
    unsafe { sys::crypto::verify_seal(info.as_ptr(), info.len() as u32).map(status_code_to_bool) }
}

/// Verifies a window proof of spacetime.
#[allow(unused)]
pub fn verify_post(info: &WindowPoStVerifyInfo) -> SyscallResult<bool> {
    let info = info
        .marshal_cbor()
        .expect("failed to marshal PoSt verification input");
    unsafe { sys::crypto::verify_post(info.as_ptr(), info.len() as u32).map(status_code_to_bool) }
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
/// Returns None and an error if the headers don't prove a fault.
#[allow(unused)]
pub fn verify_consensus_fault(
    h1: &[u8],
    h2: &[u8],
    extra: &[u8],
) -> SyscallResult<Option<ConsensusFault>> {
    let fvm_shared::sys::out::crypto::VerifyConsensusFault {
        fault,
        epoch,
        target,
    } = unsafe {
        sys::crypto::verify_consensus_fault(
            h1.as_ptr(),
            h1.len() as u32,
            h2.as_ptr(),
            h2.len() as u32,
            extra.as_ptr(),
            extra.len() as u32,
        )?
    };
    if fault == 0 {
        return Ok(None);
    }
    let fault_type =
        FromPrimitive::from_u32(fault).expect("received an invalid fault type from the runtime");
    Ok(Some(ConsensusFault {
        epoch,
        target: Address::new_id(target),
        fault_type,
    }))
}

#[allow(unused)]
pub fn verify_aggregate_seals(info: &AggregateSealVerifyProofAndInfos) -> SyscallResult<bool> {
    let info = info
        .marshal_cbor()
        .expect("failed to marshal aggregate seal verification input");
    unsafe {
        sys::crypto::verify_aggregate_seals(info.as_ptr(), info.len() as u32)
            .map(status_code_to_bool)
    }
}

#[allow(unused)]
pub fn verify_replica_update(info: &ReplicaUpdateInfo) -> SyscallResult<bool> {
    let info = info
        .marshal_cbor()
        .expect("failed to marshal replica update verification input");
    unsafe {
        sys::crypto::verify_replica_update(info.as_ptr(), info.len() as u32)
            .map(status_code_to_bool)
    }
}

#[allow(unused)]
pub fn batch_verify_seals(batch: &[SealVerifyInfo]) -> SyscallResult<Vec<bool>> {
    let encoded = to_vec(batch).expect("failed to marshal batch seal verification input");

    Ok(unsafe {
        let mut result: Vec<bool> = Vec::with_capacity(batch.len());
        sys::crypto::batch_verify_seals(
            encoded.as_ptr(),
            encoded.len() as u32,
            result.as_mut_ptr() as *mut u8,
        )?;
        result.set_len(batch.len());
        result
    })
}
