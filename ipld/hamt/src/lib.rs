// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

//! HAMT crate for use as rust IPLD data structure
//!
//! [Data structure reference](https://github.com/ipld/specs/blob/51fab05b4fe4930d3d851d50cc1e5f1a02092deb/data-structures/hashmap.md)
//!
//! Implementation based off the work @dignifiedquire started [here](https://github.com/dignifiedquire/rust-hamt-ipld). This implementation matched the rust HashMap interface very closely, but came at the cost of saving excess values to the database and requiring unsafe code to update the cache from the underlying store as well as discarding any errors that came in any operations. The function signatures that exist are based on this, but refactored to match the spec more closely and match the necessary implementation.
//!
//! The Hamt is a data structure that mimmics a HashMap which has the features of being sharded, persisted, and indexable by a Cid. The Hamt supports a variable bit width to adjust the amount of possible pointers that can exist at each height of the tree. Hamt can be modified at any point, but the underlying values are only persisted to the store when the [flush](struct.Hamt.html#method.flush) is called.

mod bitfield;
mod error;
mod ext;
mod hamt;
mod hash;
mod hash_algorithm;
mod hash_bits;
mod node;
mod pointer;

pub use forest_hash_utils::{BytesKey, Hash};
use serde::{Deserialize, Serialize};

pub use self::error::Error;
pub use self::hamt::Hamt;
pub use self::hash::*;
pub use self::hash_algorithm::*;

const MAX_ARRAY_WIDTH: usize = 3;

/// Default bit width for indexing a hash at each depth level
const DEFAULT_BIT_WIDTH: u32 = 8;

/// Configuration options for a HAMT instance.
#[derive(Debug, Clone)]
pub struct Config {
    /// The `bit_width` drives how wide and high the tree is going to be.
    /// Each node in the tree will have `2^bit_width` number of slots for child nodes,
    /// and consume `bit_width` number of bits from the hashed keys at each level.
    pub bit_width: u32,
    /// Enabling extensions can help when the `HashAlgorithm` used by the HAMT is `Identity`,
    /// which allows the user of the HAMT to set the keys arbitrarily. This can result in
    /// parts of the tree being very deep, if there are keys that share a long common prefix.
    /// Extensions allow the HAMT to eschew storing almost empty nodes all along the way to
    /// the bottom by skipping levels which would only have a single pointer to the next
    /// node in the chain, the length of which is dictated by `bit_width`.
    ///
    /// It is safe to enable this on a HAMT which has been built without extensions,
    /// but everyone has to agree whether to use them or not for the CIDs to match.
    ///
    /// It's also safe to disable it, the code will still handle extensions that already exist.
    pub use_extensions: bool,
    /// The minimum depth at which the HAMT can store key-value pairs in a `Node`.
    ///
    /// Storing values in the nodes means we have to read and write larger chunks of data
    /// whenever we're accessing something (be it a link or values) in any other bucket.
    /// This is particularly costly in the root node, which is always retrieved as soon
    /// as the HAMT is instantiated.
    ///
    /// This setting allows us to keep the root, and possibly a few more levels, free of
    /// data, reserved for links. A sufficiently saturated tree will tend to contain only
    /// links in the first levels anyway, once all the buckets have been filled and pushed
    /// further down.
    ///
    /// A value of 0 means data can be put in the root node, which is the default behaviour.
    pub min_data_depth: u32,
    /// With this setting data is never stored in intermediary nodes, only in the lowest level
    /// of the tree, a.k.a. leaves. Due to hashing, only the first few levels of the tree are
    /// likely to see overlapping keys, after which an extension is used to point straight at
    /// the leaf. This will result in entries being stored in their own nodes and not weighing
    /// down on independent lookups, the intermediary nodes are reserved for routing.
    ///
    /// With hybrid hashing strategies it is still possible to co-locate related entries,
    /// such as array items, which can speed up iteration over them.
    pub push_data_to_leaves: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bit_width: DEFAULT_BIT_WIDTH,
            use_extensions: false,
            min_data_depth: 0,
            push_data_to_leaves: false,
        }
    }
}

type HashedKey = [u8; 32];

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct KeyValuePair<K, V>(K, V);

impl<K, V> KeyValuePair<K, V> {
    pub fn key(&self) -> &K {
        &self.0
    }
    pub fn value(&self) -> &V {
        &self.1
    }
}

impl<K, V> KeyValuePair<K, V> {
    pub fn new(key: K, value: V) -> Self {
        KeyValuePair(key, value)
    }
}
