// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use address::Address;
use cid::Cid;
use clock::ChainEpoch;

pub mod address;
pub mod bigint;
pub mod chainid;
pub mod clock;
pub mod commcid;
pub mod consensus;
pub mod crypto;
pub mod deal;
pub mod econ;
pub mod error;
pub mod event;
pub mod message;
pub mod piece;
pub mod randomness;
pub mod receipt;
pub mod sector;
pub mod state;
pub mod sys;
pub mod upgrade;
pub mod version;

use cid::multihash::Multihash;
use crypto::hash::SupportedHashes;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::DAG_CBOR;

use crate::error::ExitCode;

/// Codec for raw data.
pub const IPLD_RAW: u64 = 0x55;

/// Multihash code for the identity hash function.
pub const IDENTITY_HASH: u64 = 0x0;

/// The maximum supported CID size.
pub const MAX_CID_LEN: usize = 100;

/// Default bit width for the hamt in the filecoin protocol.
pub const HAMT_BIT_WIDTH: u32 = 5;

/// Identifier for Actors, includes builtin and initialized actors
pub type ActorID = u64;

/// Method number indicator for calling actor methods.
pub type MethodNum = u64;

/// Base actor send method.
pub const METHOD_SEND: MethodNum = 0;
/// Base actor constructor method.
pub const METHOD_CONSTRUCTOR: MethodNum = 1;

/// The outcome of a `Send`, covering its ExitCode and optional return data
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Response {
    pub exit_code: ExitCode,
    pub return_data: Option<IpldBlock>,
}

// This is a somewhat nasty hack that lets us unwrap in a const function.
const fn const_unwrap<T: Copy, E>(r: Result<T, E>) -> T {
    let v = match r {
        Ok(v) => v,
        Err(_) => panic!(), // aborts at compile time
    };
    // given the match above, this will _only_ drop `Ok(T)` where `T` is copy, so it won't actually
    // do anything. However, we need it to convince the compiler that we never drop `Err(E)` because
    // `E` likely isn't `Copy` (and therefore can't be "dropped" at compile time.
    std::mem::forget(r);
    v
}

// 45b0cfc220ceec5b7c1c62c4d4193d38e4eba48e8815729ce75f9c0ab0e4c1c0
const EMPTY_ARR_HASH_DIGEST: &[u8] = &[
    0x45, 0xb0, 0xcf, 0xc2, 0x20, 0xce, 0xec, 0x5b, 0x7c, 0x1c, 0x62, 0xc4, 0xd4, 0x19, 0x3d, 0x38,
    0xe4, 0xeb, 0xa4, 0x8e, 0x88, 0x15, 0x72, 0x9c, 0xe7, 0x5f, 0x9c, 0x0a, 0xb0, 0xe4, 0xc1, 0xc0,
];

// bafy2bzacebc3bt6cedhoyw34drrmjvazhu4oj25er2ebk4u445pzycvq4ta4a
pub const EMPTY_ARR_CID: Cid = Cid::new_v1(
    DAG_CBOR,
    const_unwrap(Multihash::wrap(
        SupportedHashes::Blake2b256 as u64,
        EMPTY_ARR_HASH_DIGEST,
    )),
);

#[test]
fn test_empty_arr_cid() {
    use fvm_ipld_encoding::to_vec;
    use multihash_codetable::{Code, MultihashDigest};

    let empty = to_vec::<[(); 0]>(&[]).unwrap();
    let expected = Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty));
    assert_eq!(EMPTY_ARR_CID, expected);
}
