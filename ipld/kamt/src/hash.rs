use std::marker::PhantomData;

use cid::Cid;
use forest_hash_utils::Hash;
use fvm_ipld_blockstore::Blockstore;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use crate::{Error, HashedKey, Kamt, KamtLike};

/// Convenience trait for using the Kamt with an API a bit more similar to the Hamt.
pub trait KeyHasher<K, const N: usize> {
    fn hash(key: &K) -> HashedKey<N>;
}

/// Convenience wrapper around the `Kamt` that calls a hashing function on keys.
#[derive(Debug)]
pub struct HasherKamt<BS, K, V, const N: usize, H> {
    inner: Kamt<BS, V, N>,
    phantom_h: PhantomData<H>,
    phantom_k: PhantomData<K>,
}

impl<BS, K, V, const N: usize, H> HasherKamt<BS, K, V, N, H>
where
    H: KeyHasher<K, N>,
{
    pub fn new(kamt: Kamt<BS, V, N>) -> Self {
        Self {
            inner: kamt,
            phantom_h: PhantomData,
            phantom_k: PhantomData,
        }
    }
}

impl<V: PartialEq, S: Blockstore, const N: usize, K, H> PartialEq for HasherKamt<S, K, V, N, H> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<BS, K, V, const N: usize, H> Serialize for HasherKamt<BS, K, V, N, H>
where
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.serialize(serializer)
    }
}

impl<BS, K, V, const N: usize, H> KamtLike<BS, K, V, N> for HasherKamt<BS, K, V, N, H>
where
    H: KeyHasher<K, N>,
    V: Serialize + DeserializeOwned,
    BS: Blockstore,
{
    fn set_root(&mut self, cid: &Cid) -> Result<(), Error> {
        self.inner.set_root(cid)
    }

    fn store(&self) -> &BS {
        self.inner.store()
    }

    fn set(&mut self, key: K, value: V) -> Result<Option<V>, Error>
    where
        V: PartialEq,
    {
        self.inner.set(H::hash(&key), value)
    }

    fn set_if_absent(&mut self, key: K, value: V) -> Result<bool, Error>
    where
        V: PartialEq,
    {
        self.inner.set_if_absent(H::hash(&key), value)
    }

    fn get(&self, k: &K) -> Result<Option<&V>, Error>
    where
        V: DeserializeOwned,
    {
        self.inner.get(&H::hash(k))
    }

    fn contains_key(&self, k: &K) -> Result<bool, Error> {
        self.inner.contains_key(&H::hash(k))
    }

    fn delete(&mut self, k: &K) -> Result<Option<V>, Error> {
        self.inner.delete(&H::hash(k))
    }

    fn flush(&mut self) -> Result<Cid, Error> {
        self.inner.flush()
    }

    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn for_each<F>(&self, f: F) -> Result<(), Error>
    where
        V: DeserializeOwned,
        F: FnMut(&HashedKey<N>, &V) -> anyhow::Result<()>,
    {
        self.inner.for_each(f)
    }

    fn into_store(self) -> BS {
        self.inner.into_store()
    }
}

/// Convenience hasher for docstrings, where no key is longer than 32 bytes.
#[derive(Debug)]
pub struct Identity;

impl KeyHasher<u8, 32> for Identity {
    fn hash(key: &u8) -> HashedKey<32> {
        IdentityHasher::hash(key)
    }
}

impl KeyHasher<u32, 32> for Identity {
    fn hash(key: &u32) -> HashedKey<32> {
        IdentityHasher::hash(key)
    }
}

impl KeyHasher<i32, 32> for Identity {
    fn hash(key: &i32) -> HashedKey<32> {
        IdentityHasher::hash(key)
    }
}

impl KeyHasher<[u8; 32], 32> for Identity {
    fn hash(key: &[u8; 32]) -> HashedKey<32> {
        key.clone()
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
