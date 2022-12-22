// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_ipld_encoding::to_vec;
use fvm_shared::address::Address;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::crypto::signature::{
    Signature, SECP_PUB_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
};
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
    WindowPoStVerifyInfo,
};
use fvm_shared::MAX_CID_LEN;
use num_traits::FromPrimitive;

use crate::{status_code_to_bool, sys, SyscallResult};

/// Verifies that a signature is valid for an address and plaintext.
///
/// NOTE: This only supports f1 and f3 addresses.
pub fn verify_signature(
    signature: &Signature,
    signer: &Address,
    plaintext: &[u8],
) -> SyscallResult<bool> {
    let sig_type = signature.signature_type();
    let sig_bytes = signature.bytes();
    let signer = signer.to_bytes();
    unsafe {
        sys::crypto::verify_signature(
            sig_type as u32,
            sig_bytes.as_ptr(),
            sig_bytes.len() as u32,
            signer.as_ptr(),
            signer.len() as u32,
            plaintext.as_ptr(),
            plaintext.len() as u32,
        )
        .map(status_code_to_bool)
    }
}

/// Recovers the signer public key from the message hash and signature.
pub fn recover_secp_public_key(
    hash: &[u8; SECP_SIG_MESSAGE_HASH_SIZE],
    signature: &[u8; SECP_SIG_LEN],
) -> SyscallResult<[u8; SECP_PUB_LEN]> {
    unsafe { sys::crypto::recover_secp_public_key(hash.as_ptr(), signature.as_ptr()) }
}

/// Hashes input data using blake2b with 256 bit output.
pub fn hash_blake2b(data: &[u8]) -> [u8; 32] {
    const BLAKE2B_256: u64 = 0xb220;
    // This can only fail if we manage to pass in corrupted memory.
    let mut ret = [0u8; 32];
    unsafe {
        sys::crypto::hash(
            BLAKE2B_256,
            data.as_ptr(),
            data.len() as u32,
            ret.as_mut_ptr(),
            32,
        )
    }
    .expect("failed to compute blake2b hash");
    ret
}

/// Hashes input data using one of the supported functions.
/// hashes longer than 64 bytes will be truncated.
pub fn hash_owned(hasher: SupportedHashes, data: &[u8]) -> Vec<u8> {
    let mut ret = Vec::with_capacity(64);

    unsafe {
        let written = sys::crypto::hash(
            hasher as u64,
            data.as_ptr(),
            data.len() as u32,
            ret.as_mut_ptr(),
            64, // maximum the buffer will hold, but will likely be less
        )
        .unwrap_or_else(|_| panic!("failed compute hash using {:?}", hasher))
            as usize;
        assert!(written <= ret.capacity());
        // SAFETY: hash syscall should've written _exactly_ the number of bytes it wrote to the buffer
        ret.set_len(written as usize);
    }

    ret
}

/// Hashes input data using one of the supported functions into a buffer.
pub fn hash_into(hasher: SupportedHashes, data: &[u8], digest: &mut [u8]) -> usize {
    unsafe {
        sys::crypto::hash(
            hasher as u64,
            data.as_ptr(),
            data.len() as u32,
            digest.as_mut_ptr(),
            digest.len() as u32,
        )
        .unwrap_or_else(|_| panic!("failed compute hash using {:?}", hasher)) as usize
    }
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
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
pub fn verify_seal(info: &SealVerifyInfo) -> SyscallResult<bool> {
    let info = to_vec(info).expect("failed to marshal seal verification input");
    unsafe { sys::crypto::verify_seal(info.as_ptr(), info.len() as u32).map(status_code_to_bool) }
}

/// Verifies a window proof of spacetime.
pub fn verify_post(info: &WindowPoStVerifyInfo) -> SyscallResult<bool> {
    let info = to_vec(info).expect("failed to marshal PoSt verification input");
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

pub fn verify_aggregate_seals(info: &AggregateSealVerifyProofAndInfos) -> SyscallResult<bool> {
    let info = to_vec(info).expect("failed to marshal aggregate seal verification input");
    unsafe {
        sys::crypto::verify_aggregate_seals(info.as_ptr(), info.len() as u32)
            .map(status_code_to_bool)
    }
}

pub fn verify_replica_update(info: &ReplicaUpdateInfo) -> SyscallResult<bool> {
    let info = to_vec(info).expect("failed to marshal replica update verification input");
    unsafe {
        sys::crypto::verify_replica_update(info.as_ptr(), info.len() as u32)
            .map(status_code_to_bool)
    }
}

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
