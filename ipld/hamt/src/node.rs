// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Borrow;
use std::fmt::Debug;
use std::marker::PhantomData;

use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;
use once_cell::unsync::OnceCell;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::bitfield::Bitfield;
use super::hash_bits::HashBits;
use super::pointer::Pointer;
use super::{Error, Hash, HashAlgorithm, KeyValuePair};
use crate::Config;

/// Node in Hamt tree which contains bitfield of set indexes and pointers to nodes
#[derive(Debug)]
pub(crate) struct Node<K, V, H> {
    pub(crate) bitfield: Bitfield,
    pub(crate) pointers: Vec<Pointer<K, V, H>>,
    hash: PhantomData<H>,
}

impl<K: PartialEq, V: PartialEq, H> PartialEq for Node<K, V, H> {
    fn eq(&self, other: &Self) -> bool {
        (self.bitfield == other.bitfield) && (self.pointers == other.pointers)
    }
}

impl<K, V, H> Serialize for Node<K, V, H>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (&self.bitfield, &self.pointers).serialize(serializer)
    }
}

impl<'de, K, V, H> Deserialize<'de> for Node<K, V, H>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (bitfield, pointers) = Deserialize::deserialize(deserializer)?;
        Ok(Node {
            bitfield,
            pointers,
            hash: Default::default(),
        })
    }
}

impl<K, V, H> Default for Node<K, V, H> {
    fn default() -> Self {
        Node {
            bitfield: Bitfield::zero(),
            pointers: Vec::new(),
            hash: Default::default(),
        }
    }
}

impl<K, V, H> Node<K, V, H>
where
    K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
    H: HashAlgorithm,
    V: Serialize + DeserializeOwned,
{
    pub fn set<S: Blockstore>(
        &mut self,
        key: K,
        value: V,
        store: &S,
        conf: &Config,
        overwrite: bool,
    ) -> Result<(Option<V>, bool), Error>
    where
        V: PartialEq,
    {
        let hash = H::hash(&key);
        self.modify_value(
            &mut HashBits::new(&hash),
            conf,
            0,
            key,
            value,
            store,
            overwrite,
        )
    }

    #[inline]
    pub fn get<Q: ?Sized, S: Blockstore>(
        &self,
        k: &Q,
        store: &S,
        conf: &Config,
    ) -> Result<Option<&V>, Error>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        Ok(self.search(k, store, conf)?.map(|kv| kv.value()))
    }

    #[inline]
    pub fn remove_entry<Q: ?Sized, S>(
        &mut self,
        k: &Q,
        store: &S,
        conf: &Config,
    ) -> Result<Option<(K, V)>, Error>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
        S: Blockstore,
    {
        let hash = H::hash(k);
        self.rm_value(&mut HashBits::new(&hash), conf, 0, k, store)
    }

    pub fn is_empty(&self) -> bool {
        self.pointers.is_empty()
    }

    pub(crate) fn for_each<S, F>(&self, store: &S, f: &mut F) -> Result<(), Error>
    where
        F: FnMut(&K, &V) -> anyhow::Result<()>,
        S: Blockstore,
    {
        for p in &self.pointers {
            match p {
                Pointer::Link { cid, cache } => {
                    if let Some(cached_node) = cache.get() {
                        cached_node.for_each(store, f)?
                    } else {
                        let node = if let Some(node) = store.get_cbor(cid)? {
                            node
                        } else {
                            #[cfg(not(feature = "ignore-dead-links"))]
                            return Err(Error::CidNotFound(cid.to_string()));

                            #[cfg(feature = "ignore-dead-links")]
                            continue;
                        };

                        // Ignore error intentionally, the cache value will always be the same
                        let cache_node = cache.get_or_init(|| node);
                        cache_node.for_each(store, f)?
                    }
                }
                Pointer::Dirty(node) => node.for_each(store, f)?,
                Pointer::Values(kvs) => {
                    for kv in kvs {
                        f(kv.0.borrow(), kv.1.borrow())?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn for_each_ranged<Q: ?Sized, S, F>(
        &self,
        store: &S,
        conf: &Config,
        mut starting_cursor: Option<(HashBits, &Q)>,
        limit: Option<usize>,
        f: &mut F,
    ) -> Result<(usize, Option<K>), Error>
    where
        K: Borrow<Q> + Clone,
        Q: Eq + Hash,
        F: FnMut(&K, &V) -> anyhow::Result<()>,
        S: Blockstore,
    {
        // determine which subtree the starting_cursor is in
        let cindex = match starting_cursor {
            Some((ref mut bits, _)) => {
                let idx = bits.next(conf.bit_width)?;
                self.index_for_bit_pos(idx)
            }
            None => 0,
        };

        let mut traversed_count = 0;

        // skip exploration of subtrees that are before the subtree which contains the cursor
        for p in &self.pointers[cindex..] {
            match p {
                Pointer::Link { cid, cache } => {
                    if let Some(cached_node) = cache.get() {
                        let (traversed, key) = cached_node.for_each_ranged(
                            store,
                            conf,
                            starting_cursor.take(),
                            limit.map(|l| l.checked_sub(traversed_count).unwrap()),
                            f,
                        )?;
                        traversed_count += traversed;
                        if limit.map_or(false, |l| traversed_count >= l) && key.is_some() {
                            return Ok((traversed_count, key));
                        }
                    } else {
                        let node = if let Some(node) = store.get_cbor(cid)? {
                            node
                        } else {
                            #[cfg(not(feature = "ignore-dead-links"))]
                            return Err(Error::CidNotFound(cid.to_string()));

                            #[cfg(feature = "ignore-dead-links")]
                            continue;
                        };

                        // Ignore error intentionally, the cache value will always be the same
                        let cache_node = cache.get_or_init(|| node);
                        let (traversed, key) = cache_node.for_each_ranged(
                            store,
                            conf,
                            starting_cursor.take(),
                            limit.map(|l| l.checked_sub(traversed_count).unwrap()),
                            f,
                        )?;
                        traversed_count += traversed;
                        if limit.map_or(false, |l| traversed_count >= l) && key.is_some() {
                            return Ok((traversed_count, key));
                        }
                    }
                }
                Pointer::Dirty(node) => {
                    let (traversed, key) = node.for_each_ranged(
                        store,
                        conf,
                        starting_cursor.take(),
                        limit.map(|l| l.checked_sub(traversed_count).unwrap()),
                        f,
                    )?;
                    traversed_count += traversed;
                    if limit.map_or(false, |l| traversed_count >= l) && key.is_some() {
                        return Ok((traversed_count, key));
                    }
                }
                Pointer::Values(kvs) => {
                    for kv in kvs {
                        if limit.map_or(false, |l| traversed_count == l) {
                            // we have already found all requested items, return the key of the next item
                            return Ok((traversed_count, Some(kv.0.clone())));
                        } else if starting_cursor.map_or(false, |(_, key)| key.eq(kv.0.borrow())) {
                            // mark that we have arrived at the starting cursor
                            starting_cursor = None
                        }

                        if starting_cursor.is_none() {
                            // have already passed the start cursor
                            f(&kv.0, kv.1.borrow())?;
                            traversed_count += 1;
                        }
                    }
                }
            }
        }

        Ok((traversed_count, None))
    }

    /// Search for a key.
    fn search<Q: ?Sized, S: Blockstore>(
        &self,
        q: &Q,
        store: &S,
        conf: &Config,
    ) -> Result<Option<&KeyValuePair<K, V>>, Error>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let hash = H::hash(q);
        self.get_value(&mut HashBits::new(&hash), conf, q, store)
    }

    fn get_value<Q: ?Sized, S: Blockstore>(
        &self,
        hashed_key: &mut HashBits,
        conf: &Config,
        key: &Q,
        store: &S,
    ) -> Result<Option<&KeyValuePair<K, V>>, Error>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let idx = hashed_key.next(conf.bit_width)?;

        if !self.bitfield.test_bit(idx) {
            return Ok(None);
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child(cindex);

        let node = match child {
            Pointer::Link { cid, cache } => {
                if let Some(cached_node) = cache.get() {
                    // Link node is cached
                    cached_node
                } else {
                    let node: Box<Node<K, V, H>> = if let Some(node) = store.get_cbor(cid)? {
                        node
                    } else {
                        #[cfg(not(feature = "ignore-dead-links"))]
                        return Err(Error::CidNotFound(cid.to_string()));

                        #[cfg(feature = "ignore-dead-links")]
                        return Ok(None);
                    };
                    // Intentionally ignoring error, cache will always be the same.
                    cache.get_or_init(|| node)
                }
            }
            Pointer::Dirty(node) => node,
            Pointer::Values(vals) => {
                return Ok(vals.iter().find(|kv| key.eq(kv.key().borrow())));
            }
        };

        node.get_value(hashed_key, conf, key, store)
    }

    /// Internal method to modify values.
    ///
    /// Returns the a tuple with:
    /// * the old data at this key, if any
    /// * whether the data has been modified
    #[allow(clippy::too_many_arguments)]
    fn modify_value<S: Blockstore>(
        &mut self,
        hashed_key: &mut HashBits,
        conf: &Config,
        depth: u32,
        key: K,
        value: V,
        store: &S,
        overwrite: bool,
    ) -> Result<(Option<V>, bool), Error>
    where
        V: PartialEq,
    {
        let idx = hashed_key.next(conf.bit_width)?;

        // No existing values at this point.
        if !self.bitfield.test_bit(idx) {
            if depth >= conf.min_data_depth {
                self.insert_child(idx, key, value);
            } else {
                // Need to insert some empty nodes reserved for links.
                let mut sub = Node::<K, V, H>::default();
                sub.modify_value(hashed_key, conf, depth + 1, key, value, store, overwrite)?;
                self.insert_child_dirty(idx, Box::new(sub));
            }
            return Ok((None, true));
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child_mut(cindex);

        match child {
            Pointer::Link { cid, cache } => {
                cache.get_or_try_init(|| {
                    store
                        .get_cbor(cid)?
                        .ok_or_else(|| Error::CidNotFound(cid.to_string()))
                })?;
                let child_node = cache.get_mut().expect("filled line above");

                let (old, modified) = child_node.modify_value(
                    hashed_key,
                    conf,
                    depth + 1,
                    key,
                    value,
                    store,
                    overwrite,
                )?;
                if modified {
                    *child = Pointer::Dirty(std::mem::take(child_node));
                }
                Ok((old, modified))
            }
            Pointer::Dirty(node) => {
                node.modify_value(hashed_key, conf, depth + 1, key, value, store, overwrite)
            }
            Pointer::Values(vals) => {
                // Update, if the key already exists.
                if let Some(i) = vals.iter().position(|p| p.key() == &key) {
                    if overwrite {
                        // If value changed, the parent nodes need to be marked as dirty.
                        // ! The assumption here is that `PartialEq` is implemented correctly,
                        // ! and that if that is true, the serialized bytes are equal.
                        // ! To be absolutely sure, can serialize each value and compare or
                        // ! refactor the Hamt to not be type safe and serialize on entry and
                        // ! exit. These both come at costs, and this isn't a concern.
                        let value_changed = vals[i].value() != &value;
                        return Ok((
                            Some(std::mem::replace(&mut vals[i].1, value)),
                            value_changed,
                        ));
                    } else {
                        // Can't overwrite, return None and false that the Node was not modified.
                        return Ok((None, false));
                    }
                }

                // If the array is full, create a subshard and insert everything
                if vals.len() >= conf.max_array_width {
                    let kvs = std::mem::take(vals);
                    let hashed_kvs = kvs.into_iter().map(|KeyValuePair(k, v)| {
                        let hash = H::hash(&k);
                        (k, v, hash)
                    });

                    let consumed = hashed_key.consumed;
                    let mut sub = Node::<K, V, H>::default();
                    let modified = sub.modify_value(
                        hashed_key,
                        conf,
                        depth + 1,
                        key,
                        value,
                        store,
                        overwrite,
                    )?;

                    for (k, v, hash) in hashed_kvs {
                        sub.modify_value(
                            &mut HashBits::new_at_index(&hash, consumed),
                            conf,
                            depth + 1,
                            k,
                            v,
                            store,
                            overwrite,
                        )?;
                    }

                    *child = Pointer::Dirty(Box::new(sub));

                    return Ok(modified);
                }

                // Otherwise insert the element into the array in order.
                let max = vals.len();
                let idx = vals.iter().position(|c| c.key() > &key).unwrap_or(max);

                let np = KeyValuePair::new(key, value);
                vals.insert(idx, np);

                Ok((None, true))
            }
        }
    }

    /// Internal method to delete entries.
    fn rm_value<Q: ?Sized, S: Blockstore>(
        &mut self,
        hashed_key: &mut HashBits,
        conf: &Config,
        depth: u32,
        key: &Q,
        store: &S,
    ) -> Result<Option<(K, V)>, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let idx = hashed_key.next(conf.bit_width)?;

        // No existing values at this point.
        if !self.bitfield.test_bit(idx) {
            return Ok(None);
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child_mut(cindex);

        match child {
            Pointer::Link { cid, cache } => {
                cache.get_or_try_init(|| {
                    store
                        .get_cbor(cid)?
                        .ok_or_else(|| Error::CidNotFound(cid.to_string()))
                })?;
                let child_node = cache.get_mut().expect("filled line above");

                let deleted = child_node.rm_value(hashed_key, conf, depth + 1, key, store)?;

                if deleted.is_some() {
                    *child = Pointer::Dirty(std::mem::take(child_node));
                    if Self::clean(child, conf, depth)? {
                        self.rm_child(cindex, idx);
                    }
                }

                Ok(deleted)
            }
            Pointer::Dirty(node) => {
                // Delete value and return deleted value
                let deleted = node.rm_value(hashed_key, conf, depth + 1, key, store)?;

                if deleted.is_some() && Self::clean(child, conf, depth)? {
                    self.rm_child(cindex, idx);
                }

                Ok(deleted)
            }
            Pointer::Values(vals) => {
                // Delete value
                for (i, p) in vals.iter().enumerate() {
                    if key.eq(p.key().borrow()) {
                        let old = if vals.len() == 1 {
                            if let Pointer::Values(new_v) = self.rm_child(cindex, idx) {
                                new_v.into_iter().next().unwrap()
                            } else {
                                unreachable!()
                            }
                        } else {
                            vals.remove(i)
                        };
                        return Ok(Some((old.0, old.1)));
                    }
                }

                Ok(None)
            }
        }
    }

    pub fn flush<S: Blockstore>(&mut self, store: &S) -> Result<(), Error> {
        for pointer in &mut self.pointers {
            if let Pointer::Dirty(node) = pointer {
                // Flush cached sub node to clear it's cache
                node.flush(store)?;

                // Put node in blockstore and retrieve Cid
                let cid = store.put_cbor(node, Code::Blake2b256)?;

                // Can keep the flushed node in link cache
                let cache = OnceCell::from(std::mem::take(node));

                // Replace cached node with Cid link
                *pointer = Pointer::Link { cid, cache };
            }
        }

        Ok(())
    }

    fn rm_child(&mut self, i: usize, idx: u32) -> Pointer<K, V, H> {
        self.bitfield.clear_bit(idx);
        self.pointers.remove(i)
    }

    fn insert_child(&mut self, idx: u32, key: K, value: V) {
        let i = self.index_for_bit_pos(idx);
        self.bitfield.set_bit(idx);
        self.pointers.insert(i, Pointer::from_key_value(key, value))
    }

    fn insert_child_dirty(&mut self, idx: u32, node: Box<Node<K, V, H>>) {
        let i = self.index_for_bit_pos(idx);
        self.bitfield.set_bit(idx);
        self.pointers.insert(i, Pointer::Dirty(node))
    }

    fn index_for_bit_pos(&self, bp: u32) -> usize {
        let mask = Bitfield::zero().set_bits_le(bp);
        assert_eq!(mask.count_ones(), bp as usize);
        mask.and(&self.bitfield).count_ones()
    }

    fn get_child_mut(&mut self, i: usize) -> &mut Pointer<K, V, H> {
        &mut self.pointers[i]
    }

    fn get_child(&self, i: usize) -> &Pointer<K, V, H> {
        &self.pointers[i]
    }

    /// Clean after delete to retrieve canonical form.
    ///
    /// Returns true if the child pointer is completely empty and can be removed,
    /// which can happen if we artificially inserted nodes during insertion.
    fn clean(child: &mut Pointer<K, V, H>, conf: &Config, depth: u32) -> Result<bool, Error> {
        match child.clean(conf, depth) {
            Ok(()) => Ok(false),
            Err(Error::ZeroPointers) if depth < conf.min_data_depth => Ok(true),
            Err(err) => Err(err),
        }
    }
}
