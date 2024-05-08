// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{anyhow, Result};
use cid::{multihash::Code, Cid};
use fvm_gas_calibration_shared::*;
use fvm_ipld_encoding::{DAG_CBOR, IPLD_RAW};
use fvm_sdk::message::params_raw;
use fvm_sdk::vm::abort;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::crypto::signature::{Signature, SignatureType, SECP_SIG_LEN};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::{ActorEvent, Entry};
use fvm_shared::sys::SendFlags;
use libipld::Ipld;
use num_traits::FromPrimitive;
use serde::de::DeserializeOwned;

/// Just doing a few mutations in an array to make the hashes different.
const MUTATION_COUNT: usize = 10;
const NOP_ACTOR_ADDRESS: Address = Address::new_id(10001);

#[no_mangle]
pub fn invoke(params_ptr: u32) -> u32 {
    fvm_sdk::initialize(); // helps with debugging

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
        Method::OnVerifyBlsAggregate => dispatch_to(on_verify_bls_aggregate, params_ptr),
        Method::OnRecoverSecpPublicKey => dispatch_to(on_recover_secp_public_key, params_ptr),
        Method::OnSend => dispatch_to(on_send, params_ptr),
        Method::OnEvent => dispatch_to(on_event, params_ptr),
        Method::OnScanIpldLinks => dispatch_to(on_scan_ipld_links, params_ptr),
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

        let cid = fvm_sdk::ipld::put(Code::Blake2b256.into(), 32, IPLD_RAW, data.as_slice())?;

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

fn on_verify_bls_aggregate(p: OnVerifyBlsAggregateParams) -> Result<()> {
    let sig = p.signature.try_into().unwrap();
    let keys: Vec<_> = p.keys.iter().map(|k| (&**k).try_into().unwrap()).collect();
    let messages: Vec<_> = p.messages.iter().map(|m| &**m).collect();
    for _ in 0..p.iterations {
        fvm_sdk::crypto::verify_bls_aggregate(&sig, &keys, &messages)?;
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
    let mut value = vec![0; p.total_value_size];

    let iterations = p.iterations as u64;
    for i in 0..iterations {
        random_mutations(&mut value, p.seed + i, MUTATION_COUNT);

        let entries: Vec<_> = random_chunk(&value, p.entries, p.seed + i)
            .into_iter()
            .map(|d| Entry {
                flags: p.flags,
                // We use a constant key size to avoid introducing too many variables. Instead, we:
                // 1. Benchmark utf8 validation separately.
                // 2. Assume that all other "key" related costs will behave the same as "value"
                //    costs.
                key: char::MAX.to_string(),
                codec: IPLD_RAW,
                value: d.into(),
            })
            .collect();

        fvm_sdk::event::emit_event(&ActorEvent::from(entries))?;
    }

    Ok(())
}

// Makes approximately fixed-sized test objects with the specified number of fields & links.
fn make_test_object(
    test_cids: &[Cid],
    seed: u64,
    field_count: usize,
    link_count: usize,
) -> Vec<u8> {
    // one field is always used by padding, one for the outer vector.
    assert!(field_count >= 2, "field count must be at least 2");
    assert!(
        field_count > link_count,
        "field count must be strictly greater than the field count"
    );
    const CID_SIZE: usize = 1 + 1 + 2 + 1 + 32; // cidv1+dagcbor+blake2b+len+digest
    const CID_FIELD_SIZE: usize = CID_SIZE + 2 + 2 + 1; // cid field + tag + that random 0 byte.
    const BUF_SIZE: usize = 42 + 2;
    const MAX_OVERHEAD: usize = 10;
    const MAX_SIZE: usize = 512 * 1024;

    let estimated_size =
        CID_FIELD_SIZE * link_count + BUF_SIZE * (field_count - link_count - 2) + MAX_OVERHEAD;
    assert!(estimated_size <= MAX_SIZE, "block too large");
    let padding = MAX_SIZE - estimated_size;

    let cids = std::iter::repeat_with(|| test_cids.iter().rev().copied()).flatten();

    let items = std::iter::once(Ipld::Bytes(random_bytes(padding, seed - 1)))
        .chain(cids.take(link_count).map(Ipld::Link))
        .chain(
            (link_count..(field_count - 2))
                .map(|i| random_bytes(42, seed + i as u64))
                .map(Ipld::Bytes),
        )
        .collect::<Vec<_>>();
    fvm_ipld_encoding::to_vec(&items).expect("failed to encode block")
}

fn on_scan_ipld_links(p: OnScanIpldLinksParams) -> Result<()> {
    let mut test_cids = vec![fvm_sdk::sself::root().unwrap()];
    for i in 0..p.iterations {
        let obj = make_test_object(
            &test_cids,
            p.seed + i as u64,
            p.cbor_field_count,
            p.cbor_link_count,
        );
        let cid = fvm_sdk::ipld::put(Code::Blake2b256.into(), 32, DAG_CBOR, &obj).unwrap();
        test_cids.push(cid);
        let res = fvm_sdk::ipld::get(&cid).unwrap();
        assert_eq!(obj, res);
    }
    Ok(())
}

fn random_bytes(size: usize, seed: u64) -> Vec<u8> {
    lcg8(seed).take(size).collect()
}

fn random_mutations(data: &mut [u8], seed: u64, n: usize) {
    let size = data.len();
    if size > 0 {
        for (i, b) in lcg64(seed).zip(lcg8(seed + 1)).take(n) {
            data[i as usize % size] = b;
        }
    }
}

// Based on the seed, this function chunks the input into one of:
//
// 1. Evenly sized (approximately) chunks.
// 2. A single large chunk followed by small and/or empty chunks.
//
// Rather than produce uniformly random chunks, we attempt to cover the "worst case" scenarios as
// much as possible.
fn random_chunk(inp: &[u8], count: usize, seed: u64) -> Vec<&[u8]> {
    if count == 0 {
        Vec::new()
    } else if seed % 2 == 0 {
        inp.chunks((inp.len() / count).max(1))
            .chain(std::iter::repeat(&[][..]))
            .take(count)
            .collect()
    } else if inp.len() >= count {
        let (prefix, rest) = inp.split_at(inp.len() - (count - 1));
        std::iter::once(prefix).chain(rest.chunks(1)).collect()
    } else {
        let mut res = vec![&[][..]; count];
        res[0] = inp;
        res
    }
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
