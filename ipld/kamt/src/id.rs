// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::borrow::Cow;

use forest_hash_utils::Hash;

use crate::{AsHashedKey, HashedKey};

/// Convenience hasher for docstrings and tests,
/// where no key is longer than 32 bytes.
#[derive(Debug)]
pub struct Identity;

/// Arrays of identical length can be taken as-is.
macro_rules! identity_arr {
    ($($n:literal),*) => {
        $(
            impl AsHashedKey<[u8; $n], $n> for Identity {
                fn as_hashed_key(key: &[u8; $n]) -> Cow<HashedKey<$n>> {
                    Cow::Borrowed(key)
                }
            }
        )*
    };
}

/// Numbers can be fed as binary into a 32 byte array.
macro_rules! identity_hash {
    ($($t:ty),*) => {
        $(
            impl AsHashedKey<$t, 32> for Identity {
                fn as_hashed_key(key: &$t) -> Cow<HashedKey<32>> {
                    Cow::Owned(IdentityHasher::hash(key))
                }
            }
        )*
    };
}

identity_arr!(20, 32, 64);
identity_hash!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

/// Take the first 32 bytes as is.
#[derive(Default)]
struct IdentityHasher {
    bz: HashedKey<32>,
}

impl IdentityHasher {
    pub fn hash<K: Hash>(key: K) -> HashedKey<32> {
        let mut hasher = Self::default();
        key.hash(&mut hasher);
        hasher.bz
    }
}

impl std::hash::Hasher for IdentityHasher {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        for (i, byte) in bytes.iter().take(self.bz.len()).enumerate() {
            self.bz[i] = *byte;
        }
    }
}
