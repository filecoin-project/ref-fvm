// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

/// Export the wasm binary
#[cfg(not(feature = "runtime-wasm"))]
pub mod wasm {
    include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_wasm_binaries() {
            assert!(!WASM_BINARY.unwrap().is_empty());
            assert!(!WASM_BINARY_BLOATY.unwrap().is_empty());
        }
    }
}

// TODO: disable everything else when not using runtime-wasm

#[macro_use]
extern crate lazy_static;
// workaround for a compiler bug, see https://github.com/rust-lang/rust/issues/55779
extern crate serde;

use blockstore::Blockstore;
use cid::Cid;
use ipld_amt::Amt;
use serde::{de::DeserializeOwned, Serialize};
use unsigned_varint::decode::Error as UVarintError;

use crate::runtime::Runtime;
use builtin::HAMT_BIT_WIDTH;
use fvm_shared::bigint::BigInt;
pub use ipld_amt;
pub use ipld_hamt;
use ipld_hamt::{BytesKey, Error as HamtError, Hamt};

pub use self::builtin::*;
pub use self::util::*;

pub use fvm_shared::BLOCKS_PER_EPOCH as EXPECTED_LEADERS_PER_EPOCH;

pub mod builtin;
pub mod runtime;
pub mod util;

#[cfg(feature = "test_utils")]
pub mod test_utils;

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
