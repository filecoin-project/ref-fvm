// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#[macro_use]
extern crate lazy_static;
// workaround for a compiler bug, see https://github.com/rust-lang/rust/issues/55779
extern crate serde;

use blockstore::Blockstore;
use cid::Cid;
use ipld_amt::Amt;
use serde::{de::DeserializeOwned, Serialize};
use unsigned_varint::decode::Error as UVarintError;

use crate::runtime::{ActorCode, Runtime};
use builtin::HAMT_BIT_WIDTH;
use fvm_shared::bigint::BigInt;
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ActorError;
use fvm_shared::MethodNum;
pub use ipld_amt;
pub use ipld_hamt;
use ipld_hamt::{BytesKey, Error as HamtError, Hamt};

pub use self::builtin::*;
pub use self::util::*;

pub use fvm_shared::BLOCKS_PER_EPOCH as EXPECTED_LEADERS_PER_EPOCH;

mod builtin;
pub mod runtime;
pub mod util;

/// Map type to be used within actors. The underlying type is a HAMT.
pub type Map<'bs, BS, V> = Hamt<&'bs BS, V, BytesKey>;

/// Array type used within actors. The underlying type is an AMT.
pub type Array<'bs, V, BS> = Amt<V, &'bs BS>;

/// Deal weight
pub type DealWeight = BigInt;

/// Create a hamt with a custom bitwidth.
#[inline]
pub fn make_empty_map<BS, V>(store: &'_ BS, bitwidth: u32) -> Map<'_, BS, V>
where
    BS: Blockstore,
    V: DeserializeOwned + Serialize,
{
    Map::<_, V>::new_with_bit_width(store, bitwidth)
}

/// Create a map with a root cid.
#[inline]
pub fn make_map_with_root<'bs, BS, V>(
    root: &Cid,
    store: &'bs BS,
) -> Result<Map<'bs, BS, V>, HamtError>
where
    BS: Blockstore,
    V: DeserializeOwned + Serialize,
{
    Map::<_, V>::load_with_bit_width(root, store, HAMT_BIT_WIDTH)
}

/// Create a map with a root cid.
#[inline]
pub fn make_map_with_root_and_bitwidth<'bs, BS, V>(
    root: &Cid,
    store: &'bs BS,
    bitwidth: u32,
) -> Result<Map<'bs, BS, V>, HamtError>
where
    BS: Blockstore,
    V: DeserializeOwned + Serialize,
{
    Map::<_, V>::load_with_bit_width(root, store, bitwidth)
}

pub fn u64_key(k: u64) -> BytesKey {
    let mut bz = unsigned_varint::encode::u64_buffer();
    let slice = unsigned_varint::encode::u64(k, &mut bz);
    slice.to_vec().into()
}

pub fn parse_uint_key(s: &[u8]) -> Result<u64, UVarintError> {
    let (v, _) = unsigned_varint::decode::u64(s)?;
    Ok(v)
}

pub fn invoke_code<RT, BS>(
    code: &Cid,
    rt: &mut RT,
    method_num: MethodNum,
    params: &RawBytes,
) -> Option<Result<RawBytes, ActorError>>
where
    BS: Blockstore,
    RT: Runtime<BS>,
{
    if code == &*SYSTEM_ACTOR_CODE_ID {
        Some(system::Actor::invoke_method(rt, method_num, params))
    } else if code == &*INIT_ACTOR_CODE_ID {
        Some(init::Actor::invoke_method(rt, method_num, params))
    } else if code == &*CRON_ACTOR_CODE_ID {
        Some(cron::Actor::invoke_method(rt, method_num, params))
    } else if code == &*ACCOUNT_ACTOR_CODE_ID {
        Some(account::Actor::invoke_method(rt, method_num, params))
    } else if code == &*POWER_ACTOR_CODE_ID {
        Some(power::Actor::invoke_method(rt, method_num, params))
    } else if code == &*MINER_ACTOR_CODE_ID {
        Some(miner::Actor::invoke_method(rt, method_num, params))
    } else if code == &*MARKET_ACTOR_CODE_ID {
        Some(market::Actor::invoke_method(rt, method_num, params))
    } else if code == &*PAYCH_ACTOR_CODE_ID {
        Some(paych::Actor::invoke_method(rt, method_num, params))
    } else if code == &*MULTISIG_ACTOR_CODE_ID {
        Some(multisig::Actor::invoke_method(rt, method_num, params))
    } else if code == &*REWARD_ACTOR_CODE_ID {
        Some(reward::Actor::invoke_method(rt, method_num, params))
    } else if code == &*VERIFREG_ACTOR_CODE_ID {
        Some(verifreg::Actor::invoke_method(rt, method_num, params))
    } else {
        None
    }
}
