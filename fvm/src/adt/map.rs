// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use fvm_shared::bigint::BigInt;
use fvm_shared::encoding::de::DeserializeOwned;
use fvm_shared::encoding::ser::Serialize;
use fvm_shared::HAMT_BIT_WIDTH;
use ipld_blockstore::BlockStore;
use ipld_hamt::{BytesKey, Error as HamtError, Hamt};

/// Map type to be used within actors. The underlying type is a hamt.
pub type Map<'bs, BS, V> = Hamt<'bs, BS, V, BytesKey>;

/// Deal weight
pub type DealWeight = BigInt;

/// Create a hamt with a custom bitwidth.
#[inline]
pub fn make_empty_map<BS, V>(store: &'_ BS, bitwidth: u32) -> Map<'_, BS, V>
where
    BS: BlockStore,
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
    BS: BlockStore,
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
    BS: BlockStore,
    V: DeserializeOwned + Serialize,
{
    Map::<_, V>::load_with_bit_width(root, store, bitwidth)
}
