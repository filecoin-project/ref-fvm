use std::collections::HashMap;
use fvm_shared::*;
use fvm_shared::encoding::{de::StdError, Cbor};

/// Verifies that a signature is valid for an address and plaintext.
pub fn verify_signature(
    signature: &crypto::signature::Signature,
    signer: &address::Address,
    plaintext: &[u8],
) -> Result<(), Box<dyn StdError>> {
    let signature = signature.marshal_cbor();
    let signer = signer.marshal_cbor();

}

/// Hashes input data using blake2b with 256 bit output.
pub fn hash_blake2b(data: &[u8]) -> Result<[u8; 32], Box<dyn StdError>> {
    Ok(blake2b_256(data))
}

/// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
fn compute_unsealed_sector_cid(
    proof_type: sector::RegisteredSealProof,
    pieces: &[sector::PieceInfo],
) -> Result<cid::Cid, Box<dyn StdError>> {
    compute_unsealed_sector_cid(proof_type, pieces);
}

/// Verifies a sector seal proof.
fn verify_seal(vi: &sector::SealVerifyInfo) -> Result<(), Box<dyn StdError>> {

}

/// Verifies a window proof of spacetime.
fn verify_post(verify_info: &sector::WindowPoStVerifyInfo) -> Result<(), Box<dyn StdError>> {
    serialize

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
    h1: &[u8],
    h2: &[u8],
    extra: &[u8],
) -> Result<Option<ConsensusFault>, Box<dyn StdError>> {

}

fn batch_verify_seals(
    vis: &[(&address::Address, &Vec<sector::SealVerifyInfo>)],
) -> Result<HashMap<address::Address, Vec<bool>>, Box<dyn StdError>> {
    let mut verified = HashMap::new();
    for (&addr, s) in vis.iter() {
        let vals = s.iter().map(|si| self.verify_seal(si).is_ok()).collect();
        verified.insert(addr, vals);
    }
    Ok(verified)
}

fn verify_aggregate_seals(
    aggregate: &sector::AggregateSealVerifyProofAndInfos,
) -> Result<(), Box<dyn StdError>> {

}

// TODO implement verify_replica_update
// fn verify_replica_update();