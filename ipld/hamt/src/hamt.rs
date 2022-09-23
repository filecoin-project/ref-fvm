// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Borrow;

use cid::Cid;
use forest_hash_utils::BytesKey;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use crate::node::Node;
use crate::{Error, Hash, HashAlgorithm, DEFAULT_BIT_WIDTH};

/// Implementation of the HAMT data structure for IPLD.
///
/// # Examples
///
/// ```
/// use fvm_ipld_hamt::Hamt;
///
/// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
///
/// let mut map: Hamt<_, _, usize> = Hamt::new(store);
/// map.set(1, "a".to_string()).unwrap();
/// assert_eq!(map.get(&1).unwrap(), Some(&"a".to_string()));
/// assert_eq!(map.delete(&1).unwrap(), Some((1, "a".to_string())));
/// assert_eq!(map.get::<_>(&1).unwrap(), None);
/// let cid = map.flush().unwrap();
/// ```
#[derive(Debug)]
pub struct Hamt<BS, V, K = BytesKey> {
    root: Node<K, V>,
    store: BS,
    bit_width: u32,
}

impl<BS, V, K> Serialize for Hamt<BS, V, K>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.root.serialize(serializer)
    }
}

impl<BS, V, K> PartialEq for Hamt<BS, V, K>
where
    BS: Blockstore,
    V: PartialEq,
    K: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl<BS, V, K> Hamt<BS, V, K>
where
    BS: Blockstore,
    V: Serialize + DeserializeOwned,
    K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
{
    pub fn new(store: BS) -> Self {
        Self::new_with_bit_width(store, DEFAULT_BIT_WIDTH)
    }

    /// Construct hamt with a bit width
    pub fn new_with_bit_width(store: BS, bit_width: u32) -> Self {
        Self {
            root: Default::default(),
            store,
            bit_width,
        }
    }

    /// Lazily instantiate a hamt from this root Cid.
    pub fn load(cid: &Cid, store: BS) -> Result<Self, Error> {
        Self::load_with_bit_width(cid, store, DEFAULT_BIT_WIDTH)
    }

    /// Lazily instantiate a hamt from this root Cid with a specified bit width.
    pub fn load_with_bit_width(cid: &Cid, store: BS, bit_width: u32) -> Result<Self, Error> {
        match store.get_cbor(cid)? {
            Some(root) => Ok(Self {
                root,
                store,
                bit_width,
            }),
            None => Err(Error::CidNotFound(cid.to_string())),
        }
    }

    /// Sets the root based on the Cid of the root node using the Hamt store
    pub fn set_root(&mut self, cid: &Cid) -> Result<(), Error> {
        match self.store.get_cbor(cid)? {
            Some(root) => self.root = root,
            None => return Err(Error::CidNotFound(cid.to_string())),
        }

        Ok(())
    }

    /// Returns a reference to the underlying store of the Hamt.
    pub fn store(&self) -> &BS {
        &self.store
    }

    /// Inserts a key-value pair into the HAMT.
    ///
    /// If the HAMT did not have this key present, `None` is returned.
    ///
    /// If the HAMT did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though;
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_hamt::Hamt;
    /// use std::rc::Rc;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Hamt<_, _, usize> = Hamt::new(Rc::new(store));
    /// map.set(37, "a".to_string()).unwrap();
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.set(37, "b".to_string()).unwrap();
    /// map.set(37, "c".to_string()).unwrap();
    /// ```
    pub fn set<H>(&mut self, key: K, value: V, hash_algo: &mut H) -> Result<Option<V>, Error>
    where
        V: PartialEq,
        H: HashAlgorithm,
    {
        self.root
            .set(
                key,
                value,
                self.store.borrow(),
                hash_algo,
                self.bit_width,
                true,
            )
            .map(|(r, _)| r)
    }

    /// Inserts a key-value pair into the HAMT only if that key does not already exist.
    ///
    /// If the HAMT did not have this key present, `true` is returned and the key/value is added.
    ///
    /// If the HAMT did have this key present, this function will return false
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_hamt::Hamt;
    /// use std::rc::Rc;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Hamt<_, _, usize> = Hamt::new(Rc::new(store));
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
    pub fn set_if_absent<H>(&mut self, key: K, value: V, hash_algo: &mut H) -> Result<bool, Error>
    where
        V: PartialEq,
        H: HashAlgorithm,
    {
        self.root
            .set(
                key,
                value,
                self.store.borrow(),
                hash_algo,
                self.bit_width,
                false,
            )
            .map(|(_, set)| set)
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_hamt::Hamt;
    /// use std::rc::Rc;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Hamt<_, _, usize> = Hamt::new(Rc::new(store));
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.get(&1).unwrap(), Some(&"a".to_string()));
    /// assert_eq!(map.get(&2).unwrap(), None);
    /// ```
    #[inline]
    pub fn get<'r, H, Q>(&self, k: &Q, hash_algo: &'r mut H) -> Result<Option<&V>, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        V: DeserializeOwned,
        H: HashAlgorithm,
    {
        match self
            .root
            .get(k, self.store.borrow(), hash_algo, self.bit_width)?
        {
            Some(v) => Ok(Some(v)),
            None => Ok(None),
        }
    }

    /// Returns `true` if a value exists for the given key in the HAMT.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_hamt::Hamt;
    /// use std::rc::Rc;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Hamt<_, _, usize> = Hamt::new(Rc::new(store));
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.contains_key(&1).unwrap(), true);
    /// assert_eq!(map.contains_key(&2).unwrap(), false);
    /// ```
    #[inline]
    pub fn contains_key<'r, H, Q>(&mut self, k: &Q, hash_algo: &'r mut H) -> Result<bool, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        H: HashAlgorithm,
    {
        Ok(self
            .root
            .get(k, self.store.borrow(), hash_algo, self.bit_width)?
            .is_some())
    }

    /// Removes a key from the HAMT, returning the value at the key if the key
    /// was previously in the HAMT.
    ///
    /// The key may be any borrowed form of the HAMT's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_hamt::Hamt;
    /// use std::rc::Rc;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Hamt<_, _, usize> = Hamt::new(Rc::new(store));
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.delete(&1).unwrap(), Some((1, "a".to_string())));
    /// assert_eq!(map.delete(&1).unwrap(), None);
    /// ```
    pub fn delete<'r, H, Q>(&mut self, k: &Q, hash_algo: &'r mut H) -> Result<Option<(K, V)>, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        H: HashAlgorithm,
    {
        self.root
            .remove_entry(k, self.store.borrow(), hash_algo, self.bit_width)
    }

    /// Flush root and return Cid for hamt
    pub fn flush(&mut self) -> Result<Cid, Error> {
        self.root.flush(self.store.borrow())?;
        Ok(self.store.put_cbor(&self.root, Code::Blake2b256)?)
    }

    /// Returns true if the HAMT has no entries
    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }

    /// Iterates over each KV in the Hamt and runs a function on the values.
    ///
    /// This function will constrain all values to be of the same type
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_hamt::Hamt;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Hamt<_, _, usize> = Hamt::new(store);
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
    #[inline]
    pub fn for_each<F>(&self, mut f: F) -> Result<(), Error>
    where
        V: DeserializeOwned,
        F: FnMut(&K, &V) -> anyhow::Result<()>,
    {
        self.root.for_each(self.store.borrow(), &mut f)
    }

    /// Consumes this HAMT and returns the Blockstore it owns.
    pub fn into_store(self) -> BS {
        self.store
    }
}
