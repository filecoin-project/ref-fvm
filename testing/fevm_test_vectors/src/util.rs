use std::collections::BTreeMap;

use anyhow::{anyhow, Context};
use async_std::task::block_on;
use fil_actors_runtime::runtime::builtins::Type;
use fil_actors_runtime::test_utils::ACTOR_CODES;
use fvm_ipld_car::load_car_unchecked;
use fvm_ipld_encoding::CborStore;
use fvm_shared::version::NetworkVersion;
use num_traits::FromPrimitive;

use crate::*;

pub fn get_code_cid_map() -> anyhow::Result<BTreeMap<Type, Cid>> {
    let bs = MemoryBlockstore::new();
    let actor_v10_bundle = (NetworkVersion::V18, actors_v10::BUNDLE_CAR);
    let roots = block_on(async { load_car_unchecked(&bs, actor_v10_bundle.1).await.unwrap() });
    assert_eq!(roots.len(), 1);

    let manifest_cid = roots[0];
    let (_, builtin_actors_cid): (u32, Cid) = bs
        .get_cbor(&manifest_cid)?
        .context("failed to load actor manifest")?;

    let vec: Vec<(String, Cid)> = match bs.get_cbor(&builtin_actors_cid)? {
        Some(vec) => vec,
        None => {
            return Err(anyhow!("cannot find manifest root cid {}", manifest_cid));
        }
    };

    let mut by_id: BTreeMap<Type, Cid> = BTreeMap::new();
    for ((_, code_cid), id) in vec.into_iter().zip(1u32..) {
        let actor_type = Type::from_u32(id).unwrap();
        by_id.insert(actor_type, code_cid);
    }
    Ok(by_id)
}

pub fn get_test_code_cid_map() -> anyhow::Result<BTreeMap<Type, Cid>> {
    Ok(ACTOR_CODES.clone())
}

pub fn compute_address_create(from: &EthAddress, nonce: u64) -> EthAddress {
    let mut stream = rlp::RlpStream::new();
    stream.begin_list(2).append(&&from.0[..]).append(&nonce);
    EthAddress(hash_20(&stream.out()))
}

pub fn hash_20(data: &[u8]) -> [u8; 20] {
    hash(SupportedHashes::Keccak256, data)[12..32]
        .try_into()
        .unwrap()
}

pub fn hash(hasher: SupportedHashes, data: &[u8]) -> Vec<u8> {
    let hasher = Code::try_from(hasher as u64).unwrap();
    let (_, digest, written) = hasher.digest(data).into_inner();
    Vec::from(&digest[..written as usize])
}

pub fn hex_to_u256(str: &str) -> U256 {
    let v = hex_to_bytes(str);
    let mut r = [0u8; 32];
    r[32 - v.len()..32].copy_from_slice(&v);
    U256::from_big_endian(&r)
}

pub fn hex_to_eth_address(str: &str) -> EthAddress {
    let v = hex_to_bytes(str);
    let mut r = [0u8; 20];
    r[20 - v.len()..20].copy_from_slice(&v);
    EthAddress(r)
}

pub fn hex_to_bytes(str: &str) -> Vec<u8> {
    if str.starts_with("0x") {
        let str = &str[2..str.len()];
        hex::decode(if str.len().is_odd() {
            let mut s = String::from("0");
            s.push_str(str);
            s
        } else {
            str.to_string()
        })
        .unwrap()
    } else {
        hex::decode(if str.len().is_odd() {
            let mut s = String::from("0");
            s.push_str(&str);
            s
        } else {
            str.to_string()
        })
        .unwrap()
    }
}

pub fn u256_to_bytes(u: &U256) -> Vec<u8> {
    let mut v = vec![0u8; 32];
    (0..4).for_each(|i| {
        let e = hex::decode(hex::encode(u.0[3 - i].to_be_bytes())).unwrap();
        v[i * 8..(i + 1) * 8].copy_from_slice(&e);
    });
    v
}

#[test]
fn test_get_code_cid_map() {
    let map = get_code_cid_map().unwrap();
    println!("{:?}", map.get(&Type::Init).unwrap());
}
