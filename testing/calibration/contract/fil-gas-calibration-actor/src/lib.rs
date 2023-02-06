// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{anyhow, Result};
use cid::multihash::Code;
use fvm_ipld_encoding::{DAG_CBOR, IPLD_RAW};
use fvm_sdk::message::params_raw;
use fvm_sdk::vm::abort;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::crypto::signature::{Signature, SignatureType, SECP_SIG_LEN};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::{ActorEvent, Entry, Flags};
use fvm_shared::sys::SendFlags;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Just doing a few mutations in an array to make the hashes different.
const MUTATION_COUNT: usize = 10;
const NOP_ACTOR_ADDRESS: Address = Address::new_id(10001);

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

#[no_mangle]
pub fn invoke(params_ptr: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and run the scenario.
    // The test is expected to capture gas metrics. Other than that we're not
    // interested in any return value.
    let method = FromPrimitive::from_u64(fvm_sdk::message::method_number()).unwrap_or_else(|| {
        abort(
            ExitCode::USR_UNHANDLED_MESSAGE.value(),
            Some("unrecognized method"),
        )
    });

    if let Err(err) = dispatch(method, params_ptr) {
        abort(
            ExitCode::USR_UNHANDLED_MESSAGE.value(),
            Some(format!("error running method: {err}").as_ref()),
        )
    }

    0
}

fn dispatch(method: Method, params_ptr: u32) -> Result<()> {
    match method {
        Method::OnHashing => dispatch_to(on_hashing, params_ptr),
        Method::OnBlock => dispatch_to(on_block, params_ptr),
        Method::OnVerifySignature => dispatch_to(on_verify_signature, params_ptr),
        Method::OnRecoverSecpPublicKey => dispatch_to(on_recover_secp_public_key, params_ptr),
        Method::OnSend => dispatch_to(on_send, params_ptr),
        Method::OnEvent => dispatch_to(on_event, params_ptr),
    }
}

fn dispatch_to<F, P>(f: F, params_ptr: u32) -> Result<()>
where
    F: FnOnce(P) -> Result<()>,
    P: DeserializeOwned,
{
    f(read_params::<P>(params_ptr)?)
}

fn on_hashing(p: OnHashingParams) -> Result<()> {
    let h = p.hasher().ok_or_else(|| anyhow!("unknown hasher"))?;
    let mut data = random_bytes(p.size, p.seed);
    for i in 0..p.iterations {
        random_mutations(&mut data, p.seed + i as u64, MUTATION_COUNT);
        fvm_sdk::crypto::hash_owned(h, &data);
    }
    Ok(())
}

fn on_block(p: OnBlockParams) -> Result<()> {
    let mut data = random_bytes(p.size, p.seed);
    let mut cids = Vec::new();

    for i in 0..p.iterations {
        random_mutations(&mut data, p.seed + i as u64, MUTATION_COUNT);

        let cid = fvm_sdk::ipld::put(Code::Blake2b256.into(), 32, DAG_CBOR, data.as_slice())?;

        // First just put it to the side, because if we read it back now, then strangely the times of puts go down by 10x in the beginning
        // and only in later go up to where they are when they are the only thing we do. The distribution takes the shape of a sloping V.
        cids.push(cid);

        // TODO: Why does including the following line affect the runtime of the put in the next iteration?
        // let back = fvm_sdk::ipld::get(&cid)?;
        //assert_eq!(data, back);
    }

    // Read the data back so we have stats about that too.
    for k in cids.iter().take(p.iterations) {
        let _ = fvm_sdk::ipld::get(k)?;
    }

    Ok(())
}

fn on_verify_signature(p: OnVerifySignatureParams) -> Result<()> {
    let sig_type = match p.signer.protocol() {
        Protocol::BLS => SignatureType::BLS,
        Protocol::Secp256k1 => SignatureType::Secp256k1,
        other => return Err(anyhow!("unexpected protocol: {other}")),
    };
    let sig = Signature {
        sig_type,
        bytes: p.signature,
    };

    let mut data = random_bytes(p.size, p.seed);

    for i in 0..p.iterations {
        random_mutations(&mut data, p.seed + i as u64, MUTATION_COUNT);
        fvm_sdk::crypto::verify_signature(&sig, &p.signer, &data)?;
    }

    Ok(())
}

fn on_recover_secp_public_key(p: OnRecoverSecpPublicKeyParams) -> Result<()> {
    let mut data = random_bytes(p.size, p.seed);
    let sig: [u8; SECP_SIG_LEN] = p
        .signature
        .try_into()
        .map_err(|_| anyhow!("unexpected signature length"))?;

    for i in 0..p.iterations {
        random_mutations(&mut data, p.seed + i as u64, MUTATION_COUNT);
        let hash = fvm_sdk::crypto::hash_blake2b(&data);
        fvm_sdk::crypto::recover_secp_public_key(&hash, &sig)?;
    }

    Ok(())
}

fn on_send(p: OnSendParams) -> Result<()> {
    let value = if p.value_transfer {
        TokenAmount::from_atto(1)
    } else {
        TokenAmount::default()
    };
    let method = p.invoke as u64;

    for _i in 0..p.iterations {
        fvm_sdk::send::send(
            &NOP_ACTOR_ADDRESS,
            method,
            None,
            value.clone(),
            None,
            SendFlags::default(),
        )
        .unwrap();
    }
    Ok(())
}

fn on_event(p: OnEventParams) -> Result<()> {
    match p.mode {
        EventCalibrationMode::Shape(_) => on_event_shape(p),
        EventCalibrationMode::TargetSize(_) => on_event_target_size(p),
    }
}

fn on_event_shape(p: OnEventParams) -> Result<()> {
    let EventCalibrationMode::Shape((key_size, value_size, last_value_size)) = p.mode else { panic!() };
    let mut value = vec![0; value_size];
    let mut last_value = vec![0; last_value_size];

    for i in 0..p.iterations {
        random_mutations(&mut value, p.seed + i as u64, MUTATION_COUNT);
        let key = random_ascii_string(key_size, p.seed + p.iterations as u64 + i as u64); // non-overlapping seed
        let mut entries: Vec<Entry> = std::iter::repeat_with(|| Entry {
            flags: p.flags,
            key: key.clone(),
            codec: IPLD_RAW,
            value: value.clone(),
        })
        .take(p.entries - 1)
        .collect();

        random_mutations(&mut last_value, p.seed + i as u64, MUTATION_COUNT);
        entries.push(Entry {
            flags: p.flags,
            key,
            codec: IPLD_RAW,
            value: last_value.clone(),
        });

        fvm_sdk::event::emit_event(&ActorEvent::from(entries))?;
    }

    Ok(())
}

fn on_event_target_size(p: OnEventParams) -> Result<()> {
    let EventCalibrationMode::TargetSize(target_size) = p.mode else { panic!() };

    // Deduct the approximate overhead of each entry (3 bytes) + flag (1 byte). This
    // is fuzzy because the size of the encoded CBOR depends on the length of fields, but it's good enough.
    let size_per_entry = ((target_size.checked_sub(p.entries * 4).unwrap_or(1)) / p.entries).max(1);
    let mut rand = lcg64(p.seed);
    for _ in 0..p.iterations {
        let mut entries = Vec::with_capacity(p.entries);
        for _ in 0..p.entries {
            let (r1, r2, r3) = (
                rand.next().unwrap(),
                rand.next().unwrap(),
                rand.next().unwrap(),
            );
            // Generate a random key of an arbitrary length that fits within the size per entry.
            // This will never be equal to size_per_entry, and it might be zero, which is fine
            // for gas calculation purposes.
            let key = random_ascii_string((r1 % size_per_entry as u64) as usize, r2);
            // Generate a value to fill up the remaining bytes.
            let value = random_bytes(size_per_entry - key.len(), r3);
            entries.push(Entry {
                flags: p.flags,
                codec: IPLD_RAW,
                key,
                value,
            })
        }
        fvm_sdk::event::emit_event(&ActorEvent::from(entries))?;
    }

    Ok(())
}

fn random_bytes(size: usize, seed: u64) -> Vec<u8> {
    lcg8(seed).take(size).collect()
}

fn random_mutations(data: &mut Vec<u8>, seed: u64, n: usize) {
    let size = data.len();
    if size > 0 {
        for (i, b) in lcg64(seed).zip(lcg8(seed + 1)).take(n) {
            data[i as usize % size] = b;
        }
    }
}

/// Generates a random string in the 0x20 - 0x7e ASCII character range
/// (alphanumeric + symbols, excluding the delete symbol).
fn random_ascii_string(n: usize, seed: u64) -> String {
    let bytes = lcg64(seed).map(|x| ((x % 95) + 32) as u8).take(n).collect();
    String::from_utf8(bytes).unwrap()
}

/// Knuth's quick and dirty random number generator.
/// https://en.wikipedia.org/wiki/Linear_congruential_generator
fn lcg64(initial_seed: u64) -> impl Iterator<Item = u64> {
    let a = 6364136223846793005_u64;
    let c = 1442695040888963407_u64;
    let mut seed = initial_seed;
    std::iter::repeat_with(move || {
        seed = a.wrapping_mul(seed).wrapping_add(c);
        seed
    })
}

fn lcg8(seed: u64) -> impl Iterator<Item = u8> {
    lcg64(seed).map(|x| (x % 256) as u8)
}

fn read_params<T: DeserializeOwned>(params_ptr: u32) -> Result<T> {
    let params = params_raw(params_ptr).unwrap().unwrap();
    let value = params.deserialize()?;
    Ok(value)
}
