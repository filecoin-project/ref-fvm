use anyhow::{anyhow, Result};
use cid::multihash::Code;
use fvm_ipld_encoding::{RawBytes, DAG_CBOR};
use fvm_sdk::message::params_raw;
use fvm_sdk::vm::abort;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::crypto::signature::{Signature, SignatureType};
use fvm_shared::error::ExitCode;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Just doing a few mutations in an array to make the hashes different.
const MUTATION_COUNT: usize = 10;

#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    /// Hash random data to measure `OnHashing`.
    OnHashing = 1,
    /// Put and get random data to measure `OnBlock*`.
    OnBlock,
    /// Try (and fail) to verify random data with a public key and signature.
    OnVerifySignature,
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
        Method::OnHashing => on_hashing(read_params::<OnHashingParams>(params_ptr)?),
        Method::OnBlock => on_block(read_params::<OnBlockParams>(params_ptr)?),
        Method::OnVerifySignature => {
            on_verify_signature(read_params::<OnVerifySignatureParams>(params_ptr)?)
        }
    }
}

fn on_hashing(p: OnHashingParams) -> Result<()> {
    let h = p.hasher().ok_or(anyhow!("unknown hasher"))?;
    let mut data = random_bytes(p.size, p.seed);
    for i in 0..p.iterations {
        random_mutations(&mut data, p.seed + i as u64, MUTATION_COUNT);
        fvm_sdk::crypto::hash(h, &data);
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
    for i in 0..p.iterations {
        let _ = fvm_sdk::ipld::get(&cids[i])?;
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

/// Knuth's quick and dirty random number generator.
/// https://en.wikipedia.org/wiki/Linear_congruential_generator
fn lcg64(mut seed: u64) -> impl Iterator<Item = u64> {
    let a = 6364136223846793005;
    let c = 1442695040888963407;
    std::iter::repeat_with(move || {
        seed = a * seed + c;
        seed
    })
}

fn lcg8(seed: u64) -> impl Iterator<Item = u8> {
    lcg64(seed).map(|x| (x % 256) as u8)
}

fn read_params<T: DeserializeOwned>(params_ptr: u32) -> Result<T> {
    let params = params_raw(params_ptr)?.1;
    let value = RawBytes::new(params).deserialize()?;
    Ok(value)
}
