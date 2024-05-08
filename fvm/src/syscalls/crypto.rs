// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::cmp;

use fvm_shared::crypto::signature::{
    BLS_PUB_LEN, BLS_SIG_LEN, SECP_PUB_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
};

use super::Context;
use crate::{
    kernel::{ClassifyResult, CryptoOps, Result},
    syscall_error,
};

#[cfg(feature = "verify-signature")]
/// Verifies that a signature is valid for an address and plaintext.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
#[allow(clippy::too_many_arguments)]
pub fn verify_signature(
    context: Context<'_, impl CryptoOps>,
    sig_type: u32,
    sig_off: u32,
    sig_len: u32,
    addr_off: u32,
    addr_len: u32,
    plaintext_off: u32,
    plaintext_len: u32,
) -> Result<i32> {
    use anyhow::Context as _;
    use fvm_shared::crypto::signature::SignatureType;
    use num_traits::FromPrimitive;

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

/// Verifies that a bls aggregate signature is valid for a list of public keys and plaintexts.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
#[allow(clippy::too_many_arguments)]
pub fn verify_bls_aggregate(
    context: Context<'_, impl CryptoOps>,
    num_signers: u32,
    sig_off: u32,
    pub_keys_off: u32,
    plaintexts_off: u32,
    plaintext_lens_off: u32,
) -> Result<i32> {
    // Check that the provided number of signatures aggregated does not cause `u32` overflow.
    let pub_keys_len = num_signers
        .checked_mul(BLS_PUB_LEN as u32)
        .ok_or(syscall_error!(
            IllegalArgument;
            "number of signatures aggregated ({num_signers}) exceeds limit"
        ))?;

    let sig: &[u8; BLS_SIG_LEN] = context
        .memory
        .try_slice(sig_off, BLS_SIG_LEN as u32)?
        .try_into()
        .expect("bls signature bytes slice-to-array conversion should not fail");

    let pub_keys: &[[u8; BLS_PUB_LEN]] = context.memory.try_chunks(pub_keys_off, pub_keys_len)?;

    let plaintext_lens: &[u32] = context
        .memory
        .try_slice(plaintext_lens_off, num_signers * 4)
        .map(|bytes| {
            let ptr = bytes.as_ptr() as *const u32;
            unsafe { std::slice::from_raw_parts(ptr, num_signers as usize) }
        })?;

    let plaintexts_concat = context
        .memory
        .try_slice(plaintexts_off, plaintext_lens.iter().sum())?;

    context
        .kernel
        .verify_bls_aggregate(sig, pub_keys, plaintexts_concat, plaintext_lens)
        .map(|v| if v { 0 } else { -1 })
}

pub fn recover_secp_public_key(
    context: Context<'_, impl CryptoOps>,
    hash_off: u32,
    sig_off: u32,
) -> Result<[u8; SECP_PUB_LEN]> {
    let hash_bytes = context
        .memory
        .try_slice(hash_off, SECP_SIG_MESSAGE_HASH_SIZE as u32)?
        .try_into()
        .or_illegal_argument()?;

    let sig_bytes = context
        .memory
        .try_slice(sig_off, SECP_SIG_LEN as u32)?
        .try_into()
        .or_illegal_argument()?;

    context
        .kernel
        .recover_secp_public_key(&hash_bytes, &sig_bytes)
}

/// Hashes input data using the specified hash function, writing the digest into the provided
/// buffer.
pub fn hash(
    context: Context<'_, impl CryptoOps>,
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
    let length = cmp::min(digest_out.len(), digest.digest().len());
    digest_out[..length].copy_from_slice(&digest.digest()[..length]);
    Ok(length as u32)
}
