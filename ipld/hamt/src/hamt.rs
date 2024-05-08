// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Borrow;
use std::marker::PhantomData;

use cid::Cid;
use forest_hash_utils::BytesKey;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use crate::iter::IterImpl;
use crate::node::Node;
use crate::pointer::version::Version;
use crate::{pointer::version, Config, Error, Hash, HashAlgorithm, Sha256};

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
pub type Hamt<BS, V, K = BytesKey, H = Sha256> = HamtImpl<BS, V, K, H, version::V3>;
/// Legacy amt V0
pub type Hamtv0<BS, V, K = BytesKey, H = Sha256> = HamtImpl<BS, V, K, H, version::V0>;

#[derive(Debug)]
#[doc(hidden)]
pub struct HamtImpl<BS, V, K = BytesKey, H = Sha256, Ver = version::V3> {
    root: Node<K, V, H, Ver>,
    store: BS,
    conf: Config,
    hash: PhantomData<H>,
    /// Remember the last flushed CID until it changes.
    flushed_cid: Option<Cid>,
}

impl<BS, V, K, H, Ver> Serialize for HamtImpl<BS, V, K, H, Ver>
where
    K: Serialize,
    V: Serialize,
    H: HashAlgorithm,
    Ver: Version,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.root.serialize(serializer)
    }
}

impl<K: PartialEq, V: PartialEq, S: Blockstore, H: HashAlgorithm, Ver> PartialEq
    for HamtImpl<S, V, K, H, Ver>
{
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl<BS, V, K, H, Ver> HamtImpl<BS, V, K, H, Ver>
where
    K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
    BS: Blockstore,
    Ver: Version,
    H: HashAlgorithm,
{
    #[deprecated = "specify a bit-width explicitly"]
    pub fn new(store: BS) -> Self {
        Self::new_with_config(store, Config::default())
    }

    pub fn new_with_config(store: BS, conf: Config) -> Self {
        Self {
            root: Node::default(),
            store,
            conf,
            hash: Default::default(),
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
    #[deprecated = "specify a bit-width explicitly"]
    pub fn load(cid: &Cid, store: BS) -> Result<Self, Error> {
        Self::load_with_config(cid, store, Config::default())
    }

    /// Lazily instantiate a hamt from this root Cid with a specified parameters.
    pub fn load_with_config(cid: &Cid, store: BS, conf: Config) -> Result<Self, Error> {
        Ok(Self {
            root: Node::load(&conf, &store, cid, 0)?,
            store,
            conf,
            hash: Default::default(),
            flushed_cid: Some(*cid),
        })
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

    /// Sets the root based on the Cid of the root node using the Hamt store
    pub fn set_root(&mut self, cid: &Cid) -> Result<(), Error> {
        self.root = Node::load(&self.conf, &self.store, cid, 0)?;
        self.flushed_cid = Some(*cid);

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
    pub fn set(&mut self, key: K, value: V) -> Result<Option<V>, Error>
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
    pub fn set_if_absent(&mut self, key: K, value: V) -> Result<bool, Error>
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
    pub fn get<Q>(&self, k: &Q) -> Result<Option<&V>, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        V: DeserializeOwned,
    {
        match self.root.get(k, self.store.borrow(), &self.conf)? {
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
    pub fn contains_key<Q>(&self, k: &Q) -> Result<bool, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        Ok(self.root.get(k, self.store.borrow(), &self.conf)?.is_some())
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
    pub fn delete<Q>(&mut self, k: &Q) -> Result<Option<(K, V)>, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let deleted = self.root.remove_entry(k, self.store.borrow(), &self.conf)?;

        if deleted.is_some() {
            self.flushed_cid = None;
        }

        Ok(deleted)
    }

    /// Flush root and return Cid for hamt
    pub fn flush(&mut self) -> Result<Cid, Error> {
        if let Some(cid) = self.flushed_cid {
            return Ok(cid);
        }
        self.root.flush(self.store.borrow())?;
        let cid = self.store.put_cbor(&self.root, Code::Blake2b256)?;
        self.flushed_cid = Some(cid);
        Ok(cid)
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
        for res in self {
            let (k, v) = res?;
            (f)(k, v)?;
        }
        Ok(())
    }

    /// Iterates over each KV in the Hamt and runs a function on the values. If starting key is
    /// provided, iteration will start from that key. If max is provided, iteration will stop after
    /// max number of items have been traversed. The number of items that were traversed is
    /// returned. If there are more items in the Hamt after max items have been traversed, the key
    /// of the next item will be returned.
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
    /// let mut map: Hamt<_, _, u64> = Hamt::new(store);
    /// map.set(1, 1).unwrap();
    /// map.set(2, 2).unwrap();
    /// map.set(3, 3).unwrap();
    /// map.set(4, 4).unwrap();
    ///
    /// let mut numbers = vec![];
    ///
    /// map.for_each_ranged(None, None, |_, v: &u64| {
    ///     numbers.push(*v);
    ///     Ok(())
    /// }).unwrap();
    ///
    /// let mut subset = vec![];
    ///
    /// let (_, next_key) = map.for_each_ranged(Some(&numbers[0]), Some(2), |_, v: &u64| {
    ///     subset.push(*v);
    ///     Ok(())
    /// }).unwrap();
    ///
    /// assert_eq!(subset, numbers[..2]);
    /// assert_eq!(next_key.unwrap(), numbers[2]);
    /// ```
    #[inline]
    pub fn for_each_ranged<Q, F>(
        &self,
        starting_key: Option<&Q>,
        max: Option<usize>,
        mut f: F,
    ) -> Result<(usize, Option<K>), Error>
    where
        K: Borrow<Q> + Clone,
        Q: Eq + Hash + ?Sized,
        V: DeserializeOwned,
        F: FnMut(&K, &V) -> anyhow::Result<()>,
    {
        let mut iter = match &starting_key {
            Some(key) => self.iter_from(key)?,
            None => self.iter(),
        }
        .fuse();
        let mut traversed = 0usize;
        for res in iter.by_ref().take(max.unwrap_or(usize::MAX)) {
            let (k, v) = res?;
            (f)(k, v)?;
            traversed += 1;
        }
        let next = iter.next().transpose()?.map(|kv| kv.0).cloned();
        Ok((traversed, next))
    }

    /// Consumes this HAMT and returns the Blockstore it owns.
    pub fn into_store(self) -> BS {
        self.store
    }
}

impl<BS, V, K, H, Ver> HamtImpl<BS, V, K, H, Ver>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
    Ver: Version,
    BS: Blockstore,
{
    /// Iterate over the HAMT. Alternatively, you can directly iterate over the HAMT without calling
    /// this method:
    ///
    /// ```rust
    /// use fvm_ipld_hamt::Hamt;
    /// use fvm_ipld_blockstore::MemoryBlockstore;
    ///
    /// let store = MemoryBlockstore::default();
    ///
    /// let hamt: Hamt<_, String> = Hamt::new_with_bit_width(store, 5);
    ///
    /// // ...
    ///
    /// for kv in &hamt {
    ///     let (k, v) = kv?;
    ///     println!("{k:?}: {v}");
    /// }
    ///
    /// # anyhow::Ok(())
    /// ```
    pub fn iter(&self) -> IterImpl<BS, V, K, H, Ver> {
        IterImpl::new(&self.store, &self.root, &self.conf)
    }

    /// Iterate over the HAMT starting at the given key. This can be used to implement "ranged"
    /// iteration:
    ///
    /// ```rust
    /// use fvm_ipld_hamt::{Hamt, BytesKey};
    /// use fvm_ipld_blockstore::MemoryBlockstore;
    ///
    /// let store = MemoryBlockstore::default();
    ///
    /// // Create a HAMT with 5 keys, a-e.
    /// let mut hamt: Hamt<_, String> = Hamt::new_with_bit_width(store, 5);
    /// let kvs: Vec<(BytesKey, String)> = ["a", "b", "c", "d", "e"]
    ///     .into_iter()
    ///     .map(|k|(BytesKey(k.as_bytes().to_owned()), k.to_owned()))
    ///     .collect();
    /// kvs.iter()
    ///     .map(|(k, v)| hamt.set(k.clone(), v.clone())
    ///     .map(|_|()))
    ///     .collect::<Result<(), _>>()?;
    ///
    /// // Read 2 elements.
    /// let mut results = hamt.iter().take(2).collect::<Result<Vec<_>, _>>()?;
    /// assert_eq!(results.len(), 2);
    ///
    /// // Read the rest then sort.
    /// for res in hamt.iter_from(results.last().unwrap().0)?.skip(1) {
    ///     results.push((res?));
    /// }
    /// results.sort_by_key(|kv| kv.1);
    ///
    /// // Assert that we got out what we put in.
    /// let results: Vec<_> = results.into_iter().map(|(k, v)|(k.clone(), v.clone())).collect();
    /// assert_eq!(kvs, results);
    ///
    /// # anyhow::Ok(())
    /// ```
    pub fn iter_from<Q>(&self, key: &Q) -> Result<IterImpl<BS, V, K, H, Ver>, Error>
    where
        H: HashAlgorithm,
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        IterImpl::new_from(&self.store, &self.root, key, &self.conf)
    }
}

impl<'a, BS, V, K, H, Ver> IntoIterator for &'a HamtImpl<BS, V, K, H, Ver>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
    Ver: Version,
    BS: Blockstore,
{
    type Item = Result<(&'a K, &'a V), Error>;
    type IntoIter = IterImpl<'a, BS, V, K, H, Ver>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
