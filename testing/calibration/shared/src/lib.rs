// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_shared::address::Address;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::event::Flags;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    /// Hash random data to measure `OnHashing`.
    OnHashing = 1,
    /// Put and get random data to measure `OnBlock*`.
    OnBlock,
    /// Try (and fail) to verify random data with a public key and signature.
    OnVerifySignature,
    /// Try (and fail) to recovery a public key from a signature, using random data.
    OnRecoverSecpPublicKey,
    /// Measure sends
    OnSend,
    /// Emit events, driven by the selected mode. See EventCalibrationMode for more info.
    OnEvent,
}

#[derive(Serialize, Deserialize)]
pub struct OnHashingParams {
    pub hasher: u64,
    pub iterations: usize,
    pub size: usize,
    pub seed: u64,
}

#[derive(Serialize, Deserialize)]
pub struct OnBlockParams {
    pub iterations: usize,
    pub size: usize,
    pub seed: u64,
}

#[derive(Serialize, Deserialize)]
pub struct OnVerifySignatureParams {
    pub iterations: usize,
    pub size: usize,
    pub signer: Address,
    /// A _valid_ signature over *something*, corresponding to the signature scheme
    /// of the address. A completely random sequence of bytes for signature would be
    /// immediately rejected by BLS, although not by Secp256k1. And we cannot generate
    /// valid signatures inside the contract because the libs we use don't compile to Wasm.
    pub signature: Vec<u8>,
    pub seed: u64,
}

#[derive(Serialize, Deserialize)]
pub struct OnRecoverSecpPublicKeyParams {
    pub iterations: usize,
    /// Size doesn't play a role with the SDK call because it works on hashes,
    /// but in theory it could, if the API asked for plain text. Let's pass
    /// it in just to show on the charts that the time doesn't depend on the input size.
    pub size: usize,
    pub signature: Vec<u8>,
    pub seed: u64,
}

#[derive(Serialize, Deserialize)]
pub enum EventCalibrationMode {
    /// Produce events with the specified shape.
    Shape((usize, usize, usize)),
    /// Attempt to reach a target size for the CBOR event.
    TargetSize(usize),
}

#[derive(Serialize, Deserialize)]
pub struct OnEventParams {
    pub iterations: usize,
    pub mode: EventCalibrationMode,
    /// Number of entries in the event.
    pub entries: usize,
    /// Flags to apply to all entries.
    pub flags: Flags,
    pub seed: u64,
}

#[derive(Serialize, Deserialize)]
pub struct OnSendParams {
    pub iterations: usize,
    pub value_transfer: bool,
    pub invoke: bool,
}

impl OnHashingParams {
    pub fn hasher(&self) -> Option<SupportedHashes> {
        match self.hasher {
            h if h == SupportedHashes::Sha2_256 as u64 => Some(SupportedHashes::Sha2_256),
            h if h == SupportedHashes::Blake2b256 as u64 => Some(SupportedHashes::Blake2b256),
            h if h == SupportedHashes::Blake2b512 as u64 => Some(SupportedHashes::Blake2b512),
            h if h == SupportedHashes::Keccak256 as u64 => Some(SupportedHashes::Keccak256),
            h if h == SupportedHashes::Ripemd160 as u64 => Some(SupportedHashes::Ripemd160),
            _ => None,
        }
    }
}
