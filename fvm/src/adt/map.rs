// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use blockstore::Blockstore;
use fvm_shared::bigint::BigInt;
use fvm_shared::encoding::de::DeserializeOwned;
use fvm_shared::encoding::ser::Serialize;
use ipld_hamt::{BytesKey, Hamt};

/// Map type to be used within actors. The underlying type is a hamt.
pub type Map<'bs, BS, V> = Hamt<'bs, BS, V, BytesKey>;

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
