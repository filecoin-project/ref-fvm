// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Borrow;

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use crate::iter::Iter;
use crate::node::Node;
use crate::{AsHashedKey, Config, Error};

/// Implementation of the KAMT data structure for IPLD.
///
/// # Examples
///
/// ```
/// use fvm_ipld_kamt::Kamt;
/// use fvm_ipld_kamt::id::Identity;
///
/// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
///
/// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
/// map.set(1, "a".to_string()).unwrap();
/// assert_eq!(map.get(&1).unwrap(), Some(&"a".to_string()));
/// assert_eq!(map.delete(&1).unwrap(), Some("a".to_string()));
/// assert_eq!(map.get(&1).unwrap(), None);
/// let cid = map.flush().unwrap();
/// ```
#[derive(Debug)]
pub struct Kamt<BS, K, V, H, const N: usize = 32> {
    root: Node<K, V, H, N>,
    store: BS,
    conf: Config,
    /// Remember the last flushed CID until it changes.
    flushed_cid: Option<Cid>,
}

impl<BS, K, V, H, const N: usize> Serialize for Kamt<BS, K, V, H, N>
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

impl<V: PartialEq, K: PartialEq, H, BS: Blockstore, const N: usize> PartialEq
    for Kamt<BS, K, V, H, N>
{
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl<BS, K, V, H, const N: usize> Kamt<BS, K, V, H, N>
where
    K: Serialize + DeserializeOwned + PartialOrd,
    V: Serialize + DeserializeOwned,
    BS: Blockstore,
{
    #[deprecated = "specify  config with an explicit bit-width"]
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

    /// Lazily instantiate a Kamt from this root Cid.
    #[deprecated = "specify  config with an explicit bit-width"]
    pub fn load(cid: &Cid, store: BS) -> Result<Self, Error> {
        Self::load_with_config(cid, store, Config::default())
    }

    /// Lazily instantiate a Kamt from this root Cid with a specified parameters.
    pub fn load_with_config(cid: &Cid, store: BS, conf: Config) -> Result<Self, Error> {
        Ok(Self {
            root: Node::load(&conf, &store, cid, 0)?,
            store,
            conf,
            flushed_cid: Some(*cid),
        })
    }

    /// Sets the root based on the Cid of the root node using the Kamt store
    pub fn set_root(&mut self, cid: &Cid) -> Result<(), Error> {
        self.root = Node::load(&self.conf, &self.store, cid, 0)?;

        Ok(())
    }

    /// Returns a reference to the underlying store of the Kamt.
    pub fn store(&self) -> &BS {
        &self.store
    }

    /// Consumes this KAMT and returns the Blockstore it owns.
    pub fn into_store(self) -> BS {
        self.store
    }

    /// Flush root and return Cid for Kamt
    pub fn flush(&mut self) -> Result<Cid, Error> {
        if let Some(cid) = self.flushed_cid {
            return Ok(cid);
        }
        self.root.flush(self.store.borrow())?;
        let cid = self.store.put_cbor(&self.root, Code::Blake2b256)?;
        self.flushed_cid = Some(cid);
        Ok(cid)
    }

    /// Returns true if the KAMT has no entries
    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }
}

impl<BS, K, V, H, const N: usize> Kamt<BS, K, V, H, N>
where
    K: Serialize + DeserializeOwned + PartialOrd,
    H: AsHashedKey<K, N>,
    V: Serialize + DeserializeOwned,
    BS: Blockstore,
{
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
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
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

    /// Inserts a key-value pair into the KAMT only if that key does not already exist.
    ///
    /// If the KAMT did not have this key present, `true` is returned and the key/value is added.
    ///
    /// If the KAMT did have this key present, this function will return false
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
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
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.get(&1).unwrap(), Some(&"a".to_string()));
    /// assert_eq!(map.get(&2).unwrap(), None);
    /// ```
    #[inline]
    pub fn get<Q>(&self, k: &Q) -> Result<Option<&V>, Error>
    where
        V: DeserializeOwned,
        K: Borrow<Q>,
        Q: PartialEq,
        H: AsHashedKey<Q, N>,
    {
        match self.root.get(k, self.store.borrow(), &self.conf)? {
            Some(v) => Ok(Some(v)),
            None => Ok(None),
        }
    }

    /// Returns `true` if a value exists for the given key in the KAMT.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// `Hash` and `Eq` on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.contains_key(&1).unwrap(), true);
    /// assert_eq!(map.contains_key(&2).unwrap(), false);
    /// ```
    #[inline]
    pub fn contains_key<Q>(&self, k: &Q) -> Result<bool, Error>
    where
        K: Borrow<Q>,
        Q: PartialEq,
        H: AsHashedKey<Q, N>,
    {
        Ok(self.root.get(k, self.store.borrow(), &self.conf)?.is_some())
    }

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
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
    /// map.set(1, "a".to_string()).unwrap();
    /// assert_eq!(map.delete(&1).unwrap(), Some("a".to_string()));
    /// assert_eq!(map.delete(&1).unwrap(), None);
    /// ```
    pub fn delete<Q>(&mut self, k: &Q) -> Result<Option<V>, Error>
    where
        K: Borrow<Q>,
        Q: PartialEq,
        H: AsHashedKey<Q, N>,
    {
        let deleted = self.root.remove_entry(k, self.store.borrow(), &self.conf)?;

        if deleted.is_some() {
            self.flushed_cid = None;
        }

        Ok(deleted)
    }

    /// Iterates over each KV in the Kamt and runs a function on the values.
    ///
    /// This function will constrain all values to be of the same type
    ///blah
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
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
    #[deprecated = "use `.iter()` instead"]
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
}

impl<BS, V, K, H, const N: usize> Kamt<BS, K, V, H, N>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
    BS: Blockstore,
{
    /// Returns an iterator over the entries of the map.
    ///
    /// The iterator element type is `Result<(&'a K, &'a V), Error>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
    /// map.set(1, 1).unwrap();
    /// map.set(4, 2).unwrap();
    ///
    /// let mut x: u32 = 0;
    /// for res in map.iter() {
    ///     let (key, value) = res.unwrap();
    ///     println!("key: {}, value: {}", key, value);
    ///     x = x+1;
    /// }
    /// assert_eq!(x,2)
    /// ```
    pub fn iter(&self) -> Iter<BS, V, K, H, N> {
        Iter::new(&self.store, &self.root, &self.conf)
    }

    /// Iterate over the KAMT starting at the given key.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_kamt::id::Identity;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Kamt<_, u32, _, Identity> = Kamt::new(store);
    /// map.set(1, 1).unwrap();
    /// map.set(2, 4).unwrap();
    /// map.set(3, 3).unwrap();
    /// map.set(4, 2).unwrap();
    ///
    /// let mut results = map.iter().take(2).collect::<Result<Vec<_>, _>>().unwrap();
    ///
    /// let last_key = results.last().unwrap().0;
    ///
    /// for res in map.iter_from(last_key).unwrap().skip(1) {
    ///     results.push(res.unwrap());
    /// }
    ///
    /// println!("{:?}", results);
    /// assert_eq!(results.len(), 4);
    /// ```

    /// Iterate over the KAMT starting at the given key. This can be used to implement "ranged" iteration:
    ///
    /// ```rust
    /// use fvm_ipld_kamt::Kamt;
    /// use fvm_ipld_blockstore::MemoryBlockstore;
    /// use fvm_ipld_kamt::id::Identity;
    /// use fvm_ipld_kamt::Config;
    /// let store = MemoryBlockstore::default();
    ///
    /// // Create a Kamt with 5 keys, a-e.
    /// let mut kamt: Kamt<_, u32, String, Identity> = Kamt::new_with_config(store,  Config {
    ///        bit_width: 5,
    ///        ..Default::default()
    ///    });
    /// let kvs: Vec<(u32, String)> = ["a", "b", "c", "d", "e"]
    ///     .iter()
    ///     .enumerate()
    ///    .map(|(index, &k)| (index as u32, k.to_owned()))
    ///    .collect();
    /// kvs.iter()
    ///     .map(|(k, v)|kamt.set(k.clone(), v.clone())
    ///     .map(|_|()))
    ///     .collect::<Result<(), _>>()?;
    ///
    /// // Read 2 elements.
    /// let mut results = kamt.iter().take(2).collect::<Result<Vec<(_,_)>, _>>()?;
    /// assert_eq!(results.len(), 2);
    /// // Read the rest then sort.
    /// for res in kamt.iter_from(results.last().unwrap().0)?.skip(1) {
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

    pub fn iter_from<Q>(&self, key: &Q) -> Result<Iter<BS, V, K, H, N>, Error>
    where
        K: Borrow<Q>,
        Q: PartialEq,
        H: AsHashedKey<Q, N>,
    {
        Iter::new_from(&self.store, &self.root, key, &self.conf)
    }
}

impl<'a, BS, V, K, H, const N: usize> IntoIterator for &'a Kamt<BS, K, V, H, N>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,

    BS: Blockstore,
{
    type Item = Result<(&'a K, &'a V), Error>;
    type IntoIter = Iter<'a, BS, V, K, H, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Identity;
    use fvm_ipld_blockstore::MemoryBlockstore;

    #[test]

    fn test_iter_from() -> anyhow::Result<()> {
        let store = MemoryBlockstore::default();

        // Create a Kamt with 5 keys, a-e.
        let mut kamt: Kamt<_, [u8; 32], String, Identity> =
            Kamt::new_with_config(store, Config::default());

        let keys: Vec<[u8; 32]> = vec![
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("0000000000000000000000000000000000000000000000000000000000000002")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("0100000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("0100000000000000000000000000000000000000000000000000000000000002")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("0100000000000000000000000000000000000000000000000000000000000003")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("0200000000000000000000000000000000000000000000000000000000000001")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("0200000000000000000000000000000000000000000000000000000000000002")
                .unwrap()
                .try_into()
                .unwrap(),
        ];

        for (i, key) in keys.iter().enumerate() {
            let value = format!("value{}", i + 1);

            kamt.set(*key, value)?;
        }
        let kvs: Vec<(&[u8; 32], &String)> = keys
            .iter()
            .map(|k| (k, kamt.get(k).unwrap().unwrap()))
            .collect();

        // Read 2 elements.
        let mut results = kamt.iter().take(2).collect::<Result<Vec<_>, _>>()?;
        assert_eq!(results.len(), 2);

        // Read the rest. Don't bother sorting because the KAMT stores keys in sorted order.
        for res in kamt.iter_from(results.last().unwrap().0)?.skip(1) {
            results.push(res?);
        }

        // Assert that we got out what we put in.
        assert_eq!(kvs, results);

        Ok(())
    }
}
