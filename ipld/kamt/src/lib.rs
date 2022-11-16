// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

//! KAMT crate for use as rust IPLD data structure, which stands for "fixed size Keyed AMT" and is basically a copy of the HAMT with some extra features
//! that were deemed to be too complex to add there.
//!
//! The original purpose of the features that gave birth to the KAMT was to optimize the HAMT for the EVM/Solidity storage layout,
//! which uses hashing+offset for keys to co-locate array items in a contiguous address space. While the HAMT allowed the hashing
//! strategy to work this way, it resulted in very deep parts of the tree where only the leaves contained key-value pairs. The
//! main feature of this data structure then is to skip the empty levels and point straight to the next data bearing node.
//!
//! The other difference is that to emphasize this the KAMT doesn't do any hashing on its own, it works with fixed size byte arrays as keys.
//!
//! [Data structure reference](https://github.com/ipld/specs/blob/51fab05b4fe4930d3d851d50cc1e5f1a02092deb/data-structures/hashmap.md)

mod bitfield;
mod error;
mod ext;
pub mod hash;
mod hash_bits;
mod kamt;
mod node;
mod pointer;

use forest_hash_utils::BytesKey;
use fvm_ipld_encoding::de::{self, DeserializeOwned};
use serde::{Deserialize, Serialize};

pub use self::error::Error;
pub use self::kamt::{Kamt, KamtLike};

/// Default bit width for indexing a hash at each depth level
const DEFAULT_BIT_WIDTH: u32 = 8;

/// Configuration options for a KAMT instance.
#[derive(Debug, Clone)]
pub struct Config {
    /// The `bit_width` drives how wide and high the tree is going to be.
    /// Each node in the tree will have `2^bit_width` number of slots for child nodes,
    /// and consume `bit_width` number of bits from the hashed keys at each level.
    pub bit_width: u32,
    /// Enabling extensions can help when the `HashAlgorithm` used by the KAMT is `Identity`,
    /// which allows the user of the KAMT to set the keys arbitrarily. This can result in
    /// parts of the tree being very deep, if there are keys that share a long common prefix.
    /// Extensions allow the KAMT to eschew storing almost empty nodes all along the way to
    /// the bottom by skipping levels which would only have a single pointer to the next
    /// node in the chain, the length of which is dictated by `bit_width`.
    ///
    /// It is safe to enable this on a KAMT which has been built without extensions,
    /// but everyone has to agree whether to use them or not for the CIDs to match.
    ///
    /// It's also safe to disable it, the code will still handle extensions that already exist.
    pub use_extensions: bool,
    /// The minimum depth at which the KAMT can store key-value pairs in a `Node`.
    ///
    /// Storing values in the nodes means we have to read and write larger chunks of data
    /// whenever we're accessing something (be it a link or values) in any other bucket.
    /// This is particularly costly in the root node, which is always retrieved as soon
    /// as the KAMT is instantiated.
    ///
    /// This setting allows us to keep the root, and possibly a few more levels, free of
    /// data, reserved for links. A sufficiently saturated tree will tend to contain only
    /// links in the first levels anyway, once all the buckets have been filled and pushed
    /// further down.
    ///
    /// A value of 0 means data can be put in the root node, which is the default behaviour.
    pub min_data_depth: u32,

    /// Maximum number of key-value pairs in a bucket before it's pushed down.
    pub max_array_width: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bit_width: DEFAULT_BIT_WIDTH,
            use_extensions: false,
            min_data_depth: 0,
            max_array_width: 3,
        }
    }
}

pub type HashedKey<const N: usize> = [u8; N];

#[derive(Debug, PartialEq)]
struct KeyValuePair<const N: usize, V>(HashedKey<N>, V);

impl<const N: usize, V> KeyValuePair<N, V> {
    pub fn key(&self) -> &HashedKey<N> {
        &self.0
    }

    pub fn value(&self) -> &V {
        &self.1
    }
}

impl<const N: usize, V> KeyValuePair<N, V> {
    pub fn new(key: HashedKey<N>, value: V) -> Self {
        KeyValuePair(key, value)
    }
}

impl<const N: usize, V: Serialize> Serialize for KeyValuePair<N, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (BytesKey::from(self.key().as_ref()), self.value()).serialize(serializer)
    }
}

impl<'de, const N: usize, V: DeserializeOwned> Deserialize<'de> for KeyValuePair<N, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data: (BytesKey, V) = Deserialize::deserialize(deserializer)?;
        if data.0.len() != N {
            Err(de::Error::custom(format!(
                "Expected hashed key to be {N} long; got {}",
                data.0.len()
            )))
        } else {
            let mut key = [0u8; N];
            key.copy_from_slice(&data.0);

            Ok(KeyValuePair(key, data.1))
        }
    }
}
