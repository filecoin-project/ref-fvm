// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::borrow::Cow;

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
                    const BYTES: usize = <$t>::BITS as usize / 8;
                    let mut output = [0u8; 32];
                    output[..BYTES].copy_from_slice(&<$t>::to_ne_bytes(*key));
                    Cow::Owned(output)
                }
            }
        )*
    };
}

identity_arr!(20, 32, 64);
identity_hash!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);
