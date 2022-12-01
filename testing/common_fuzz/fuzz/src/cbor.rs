// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use arbitrary::Arbitrary;
use cid::Cid;
use fvm_ipld_bitfield::{BitField, UnvalidatedBitField};
use fvm_ipld_encoding::strict_bytes;
#[allow(unused_imports)]
use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;
//use fvm_shared::bigint::{bigint_ser, BigInt};

#[derive(Deserialize_tuple, Serialize_tuple, Arbitrary, Debug)]
pub struct Payload {
    #[serde(with = "strict_bytes")]
    pub serde_bytes_bytes: Vec<u8>,
    pub integer: u64,
    pub address: Address,
    pub address_vec: Vec<Address>,
    pub bitfield: BitField,
    pub u_bitfield: UnvalidatedBitField,
    pub cid: Cid,
}
