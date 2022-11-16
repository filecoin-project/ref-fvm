// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Borrow;

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use crate::node::Node;
use crate::{Config, Error, HashedKey};

/// Implementation of the KAMT data structure for IPLD.
///
/// # Examples
///
/// ```
/// use fvm_ipld_kamt::{Kamt, KamtLike};
/// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
///
/// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
///
/// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
/// map.set(1, "a".to_string()).unwrap();
/// assert_eq!(map.get(&1).unwrap(), Some(&"a".to_string()));
/// assert_eq!(map.delete(&1).unwrap(), Some("a".to_string()));
/// assert_eq!(map.get(&1).unwrap(), None);
/// let cid = map.flush().unwrap();
/// ```
#[derive(Debug)]
pub struct Kamt<BS, V, const N: usize = 32> {
    root: Node<N, V>,
    store: BS,
    conf: Config,
    /// Remember the last flushed CID until it changes.
    flushed_cid: Option<Cid>,
}

impl<BS, V, const N: usize> Serialize for Kamt<BS, V, N>
where
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.root.serialize(serializer)
    }
}

impl<V: PartialEq, S: Blockstore, const N: usize> PartialEq for Kamt<S, V, N> {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl<BS, V, const N: usize> Kamt<BS, V, N>
where
    V: Serialize + DeserializeOwned,
    BS: Blockstore,
{
    pub fn new(store: BS) -> Self {
        Self::new_with_config(store, Config::default())
    }

    pub fn new_with_config(store: BS, conf: Config) -> Self {
        Self {
            root: Node::default(),
            store,
            conf,
            flushed_cid: None,
        }
    }

    /// Construct hamt with a bit width
    pub fn new_with_bit_width(store: BS, bit_width: u32) -> Self {
        Self::new_with_config(
            store,
            Config {
                bit_width,
                ..Default::default()
            },
        )
    }

    /// Lazily instantiate a hamt from this root Cid.
    pub fn load(cid: &Cid, store: BS) -> Result<Self, Error> {
        Self::load_with_config(cid, store, Config::default())
    }

    /// Lazily instantiate a hamt from this root Cid with a specified parameters.
    pub fn load_with_config(cid: &Cid, store: BS, conf: Config) -> Result<Self, Error> {
        match store.get_cbor(cid)? {
            Some(root) => Ok(Self {
                root,
                store,
                conf,
                flushed_cid: Some(*cid),
            }),
            None => Err(Error::CidNotFound(cid.to_string())),
        }
    }
    /// Lazily instantiate a hamt from this root Cid with a specified bit width.
    pub fn load_with_bit_width(cid: &Cid, store: BS, bit_width: u32) -> Result<Self, Error> {
        Self::load_with_config(
            cid,
            store,
            Config {
                bit_width,
                ..Default::default()
            },
        )
    }
}

/// Common operations on KAMTs that use keys that can be turned into fixed size keys.
///
/// This exists mostly as a convenience trait so that tests are not that painful to write.
/// The hashing aspect is not part of the `Kamt` itself purely to empahasize that the user
/// is expected to do their own hashing. Otherwise we could just add extensions/skipping
/// to the HAMT, like we did originally.
pub trait KamtLike<BS, K, V, const N: usize> {
    /// Sets the root based on the Cid of the root node using the Kamt store
    fn set_root(&mut self, cid: &Cid) -> Result<(), Error>;

    /// Returns a reference to the underlying store of the Kamt.
    fn store(&self) -> &BS;

    /// Inserts a key-value pair into the KAMT.
    ///
    /// If the KAMT did not have this key present, `None` is returned.
    ///
    /// If the KAMT did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though;
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::{Kamt, KamtLike};
    /// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
    /// map.set(37, "a".to_string()).unwrap();
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.set(37, "b".to_string()).unwrap();
    /// map.set(37, "c".to_string()).unwrap();
    /// ```
    fn set(&mut self, key: K, value: V) -> Result<Option<V>, Error>
    where
        V: PartialEq;

    /// Inserts a key-value pair into the KAMT only if that key does not already exist.
    ///
    /// If the KAMT did not have this key present, `true` is returned and the key/value is added.
    ///
    /// If the KAMT did have this key present, this function will return false
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::{Kamt, KamtLike};
    /// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
    /// let a = map.set_if_absent(37, "a".to_string()).unwrap();
    /// assert_eq!(map.is_empty(), false);
    /// assert_eq!(a, true);
    ///
    /// let b = map.set_if_absent(37, "b".to_string()).unwrap();
    /// assert_eq!(b, false);
    /// assert_eq!(map.get(&37).unwrap(), Some(&"a".to_string()));
    ///
    /// let c = map.set_if_absent(30, "c".to_string()).unwrap();
    /// assert_eq!(c, true);
    /// ```
    fn set_if_absent(&mut self, key: K, value: V) -> Result<bool, Error>
    where
        V: PartialEq;

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::{Kamt, KamtLike};
    /// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.get(&1).unwrap(), Some(&"a".to_string()));
    /// assert_eq!(map.get(&2).unwrap(), None);
    /// ```
    fn get(&self, k: &K) -> Result<Option<&V>, Error>
    where
        V: DeserializeOwned;

    /// Returns `true` if a value exists for the given key in the KAMT.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::{Kamt, KamtLike};
    /// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.contains_key(&1).unwrap(), true);
    /// assert_eq!(map.contains_key(&2).unwrap(), false);
    /// ```
    fn contains_key(&self, k: &K) -> Result<bool, Error>;

    /// Removes a key from the KAMT, returning the value at the key if the key
    /// was previously in the KAMT.
    ///
    /// The key may be any borrowed form of the KAMT's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::{Kamt, KamtLike};
    /// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.delete(&1).unwrap(), Some("a".to_string()));
    /// assert_eq!(map.delete(&1).unwrap(), None);
    /// ```
    fn delete(&mut self, k: &K) -> Result<Option<V>, Error>;

    /// Flush root and return Cid for hamt
    fn flush(&mut self) -> Result<Cid, Error>;

    /// Returns true if the KAMT has no entries
    fn is_empty(&self) -> bool;

    /// Iterates over each KV in the Kamt and runs a function on the values.
    ///
    /// This function will constrain all values to be of the same type
    ///blah
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::{Kamt, KamtLike};
    /// use fvm_ipld_kamt::hash::{HasherKamt, Identity};
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: HasherKamt<_, u32, _, 32, Identity> = HasherKamt::new(Kamt::new(store));
    /// map.set(1, 1).unwrap();
    /// map.set(4, 2).unwrap();
    ///
    /// let mut total = 0;
    /// map.for_each(|_, v: &u64| {
    ///    total += v;
    ///    Ok(())
    /// }).unwrap();
    /// assert_eq!(total, 3);
    /// ```
    fn for_each<F>(&self, f: F) -> Result<(), Error>
    where
        V: DeserializeOwned,
        F: FnMut(&HashedKey<N>, &V) -> anyhow::Result<()>;

    /// Consumes this KAMT and returns the Blockstore it owns.
    fn into_store(self) -> BS;
}

impl<BS, V, const N: usize> KamtLike<BS, HashedKey<N>, V, N> for Kamt<BS, V, N>
where
    V: Serialize + DeserializeOwned,
    BS: Blockstore,
{
    fn set_root(&mut self, cid: &Cid) -> Result<(), Error> {
        match self.store.get_cbor(cid)? {
            Some(root) => {
                self.root = root;
                self.flushed_cid = Some(*cid);
            }
            None => return Err(Error::CidNotFound(cid.to_string())),
        }

        Ok(())
    }

    fn store(&self) -> &BS {
        &self.store
    }

    fn set(&mut self, key: HashedKey<N>, value: V) -> Result<Option<V>, Error>
    where
        V: PartialEq,
    {
        let (old, modified) = self
            .root
            .set(key, value, self.store.borrow(), &self.conf, true)?;

        if modified {
            self.flushed_cid = None;
        }

        Ok(old)
    }

    fn set_if_absent(&mut self, key: HashedKey<N>, value: V) -> Result<bool, Error>
    where
        V: PartialEq,
    {
        let set = self
            .root
            .set(key, value, self.store.borrow(), &self.conf, false)
            .map(|(_, set)| set)?;

        if set {
            self.flushed_cid = None;
        }

        Ok(set)
    }

    #[inline]
    fn get(&self, k: &HashedKey<N>) -> Result<Option<&V>, Error>
    where
        V: DeserializeOwned,
    {
        match self.root.get(k, self.store.borrow(), &self.conf)? {
            Some(v) => Ok(Some(v)),
            None => Ok(None),
        }
    }

    #[inline]
    fn contains_key(&self, k: &HashedKey<N>) -> Result<bool, Error> {
        Ok(self.root.get(k, self.store.borrow(), &self.conf)?.is_some())
    }

    fn delete(&mut self, k: &HashedKey<N>) -> Result<Option<V>, Error> {
        let deleted = self.root.remove_entry(k, self.store.borrow(), &self.conf)?;

        if deleted.is_some() {
            self.flushed_cid = None;
        }

        Ok(deleted)
    }

    fn flush(&mut self) -> Result<Cid, Error> {
        if let Some(cid) = self.flushed_cid {
            return Ok(cid);
        }
        self.root.flush(self.store.borrow())?;
        let cid = self.store.put_cbor(&self.root, Code::Blake2b256)?;
        self.flushed_cid = Some(cid);
        Ok(cid)
    }

    fn is_empty(&self) -> bool {
        self.root.is_empty()
    }

    #[inline]
    fn for_each<F>(&self, mut f: F) -> Result<(), Error>
    where
        V: DeserializeOwned,
        F: FnMut(&HashedKey<N>, &V) -> anyhow::Result<()>,
    {
        self.root.for_each(self.store.borrow(), &mut f)
    }

    fn into_store(self) -> BS {
        self.store
    }
}
