// Copyright 2021-2023 Protocol Labs
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

/// Default bit width for indexing a hash at each depth level
const DEFAULT_BIT_WIDTH: u32 = 8;

/// Configuration options for a HAMT instance.
#[derive(Debug, Clone)]
pub struct Config {
    /// The `bit_width` drives how wide and high the tree is going to be.
    /// Each node in the tree will have `2^bit_width` number of slots for child nodes,
    /// and consume `bit_width` number of bits from the hashed keys at each level.
    pub bit_width: u32,

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
    ///
    /// The setting makes most sense when the size of values outweigh the size of the link
    /// pointing at them. When storing small, hash-sized values, it might not matter.
    pub min_data_depth: u32,

    /// Maximum number of key-value pairs in a bucket before it's pushed down.
    pub max_array_width: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bit_width: DEFAULT_BIT_WIDTH,
            min_data_depth: 0,
            max_array_width: 3,
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
