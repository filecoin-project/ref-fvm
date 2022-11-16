use std::borrow::Cow;

use forest_hash_utils::Hash;

use crate::{AsHashedKey, HashedKey};

/// Convenience hasher for docstrings and tests,
/// where no key is longer than 32 bytes.
#[derive(Debug)]
pub struct Identity;

impl AsHashedKey<u8, 32> for Identity {
    fn as_hashed_key(key: &u8) -> Cow<HashedKey<32>> {
        Cow::Owned(IdentityHasher::hash(key))
    }
}

impl AsHashedKey<u16, 32> for Identity {
    fn as_hashed_key(key: &u16) -> Cow<HashedKey<32>> {
        Cow::Owned(IdentityHasher::hash(key))
    }
}

impl AsHashedKey<u32, 32> for Identity {
    fn as_hashed_key(key: &u32) -> Cow<HashedKey<32>> {
        Cow::Owned(IdentityHasher::hash(key))
    }
}

impl AsHashedKey<i32, 32> for Identity {
    fn as_hashed_key(key: &i32) -> Cow<HashedKey<32>> {
        Cow::Owned(IdentityHasher::hash(key))
    }
}

impl AsHashedKey<[u8; 32], 32> for Identity {
    fn as_hashed_key(key: &[u8; 32]) -> Cow<HashedKey<32>> {
        Cow::Borrowed(key)
    }
}

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
