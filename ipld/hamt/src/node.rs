// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Borrow;
use std::fmt::Debug;
use std::marker::PhantomData;

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;
use once_cell::unsync::OnceCell;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::bitfield::Bitfield;
use super::hash_bits::HashBits;
use super::pointer::Pointer;
use super::{Error, Hash, HashAlgorithm, KeyValuePair, MAX_ARRAY_WIDTH};
use crate::ext::Extension;
use crate::{Config, HashedKey};

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
                Pointer::Link { cid, cache, .. } => {
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
                Pointer::Dirty { node, .. } => node.for_each(store, f)?,
                Pointer::Values(kvs) => {
                    for kv in kvs {
                        f(kv.0.borrow(), kv.1.borrow())?;
                    }
                }
            }
        }
        Ok(())
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

        let (node, ext) = match child {
            Pointer::Link { cid, cache, ext } => {
                let node = if let Some(cached_node) = cache.get() {
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
                };

                (node, ext)
            }
            Pointer::Dirty { node, ext } => (node, ext),
            Pointer::Values(vals) => {
                return Ok(vals.iter().find(|kv| key.eq(kv.key().borrow())));
            }
        };

        match match_extension(conf, hashed_key, ext)? {
            ExtensionMatch::Full { .. } => node.get_value(hashed_key, conf, key, store),
            ExtensionMatch::Partial { .. } => Ok(None),
        }
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
            if conf.push_data_to_leaves {
                // Consume all but the last level as an extension, then insert a node.
                let (is_leaf, ext, skipped) = extension_to_leaf(conf, hashed_key)?;
                if is_leaf {
                    self.insert_child(idx, key, value);
                } else {
                    // Add a leaf node an an extension pointing at it.
                    let mut sub = Node::<K, V, H>::default();
                    sub.modify_value(
                        hashed_key,
                        conf,
                        depth + 1 + skipped,
                        key,
                        value,
                        store,
                        overwrite,
                    )?;
                    self.insert_child_dirty(idx, Box::new(sub), ext);
                }
            } else if depth < conf.min_data_depth {
                // Need to insert some empty nodes reserved for links.
                let mut sub = Node::<K, V, H>::default();
                sub.modify_value(hashed_key, conf, depth + 1, key, value, store, overwrite)?;
                self.insert_child_dirty(idx, Box::new(sub), None);
            } else {
                self.insert_child(idx, key, value);
            }
            return Ok((None, true));
        }

        let cindex = self.index_for_bit_pos(idx);
        let child = self.get_child_mut(cindex);

        match child {
            Pointer::Link { cid, cache, ext } => match match_extension(conf, hashed_key, ext)? {
                ExtensionMatch::Full { skipped } => {
                    cache.get_or_try_init(|| {
                        store
                            .get_cbor(cid)?
                            .ok_or_else(|| Error::CidNotFound(cid.to_string()))
                    })?;
                    let child_node = cache.get_mut().expect("filled line above");

                    let (old, modified) = child_node.modify_value(
                        hashed_key,
                        conf,
                        depth + 1 + skipped,
                        key,
                        value,
                        store,
                        overwrite,
                    )?;
                    if modified {
                        *child = Pointer::Dirty {
                            node: std::mem::take(child_node),
                            ext: ext.take(),
                        };
                    }
                    Ok((old, modified))
                }
                ExtensionMatch::Partial(part) => {
                    *child = Self::split_extension(
                        conf,
                        hashed_key,
                        &part,
                        key,
                        value,
                        |midway, idx, tail| {
                            midway.insert_child_link(idx, *cid, tail, std::mem::take(cache));
                        },
                    )?;
                    Ok((None, true))
                }
            },
            Pointer::Dirty { node, ext } => match match_extension(conf, hashed_key, ext)? {
                ExtensionMatch::Full { skipped } => node.modify_value(
                    hashed_key,
                    conf,
                    depth + 1 + skipped,
                    key,
                    value,
                    store,
                    overwrite,
                ),
                ExtensionMatch::Partial(part) => {
                    *child = Self::split_extension(
                        conf,
                        hashed_key,
                        &part,
                        key,
                        value,
                        |midway, idx, tail| {
                            midway.insert_child_dirty(idx, std::mem::take(node), tail);
                        },
                    )?;
                    Ok((None, true))
                }
            },
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
                if vals.len() >= MAX_ARRAY_WIDTH {
                    let kvs = std::mem::take(vals);
                    let hashes = kvs.iter().map(|kv| H::hash(kv.key())).collect::<Vec<_>>();

                    // Find the longest common prefix between the new key and the existing keys that fall into the bucket.
                    let ext = find_longest_extension(conf, hashed_key, &hashes)?;
                    let skipped = ext
                        .as_ref()
                        .map(|e| e.consumed() as u32 / conf.bit_width)
                        .unwrap_or_default();

                    let consumed = hashed_key.consumed;
                    let mut sub = Node::<K, V, H>::default();
                    let modified = sub.modify_value(
                        hashed_key,
                        conf,
                        depth + 1 + skipped,
                        key,
                        value,
                        store,
                        overwrite,
                    )?;

                    for (p, hash) in kvs.into_iter().zip(hashes) {
                        sub.modify_value(
                            &mut HashBits::new_at_index(&hash, consumed),
                            conf,
                            depth + 1 + skipped,
                            p.0,
                            p.1,
                            store,
                            overwrite,
                        )?;
                    }

                    *child = Pointer::Dirty {
                        node: Box::new(sub),
                        ext,
                    };

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
            Pointer::Link { cid, cache, ext } => match match_extension(conf, hashed_key, ext)? {
                ExtensionMatch::Full { skipped } => {
                    cache.get_or_try_init(|| {
                        store
                            .get_cbor(cid)?
                            .ok_or_else(|| Error::CidNotFound(cid.to_string()))
                    })?;
                    let child_node = cache.get_mut().expect("filled line above");

                    let deleted =
                        child_node.rm_value(hashed_key, conf, depth + 1 + skipped, key, store)?;

                    if deleted.is_some() {
                        *child = Pointer::Dirty {
                            node: std::mem::take(child_node),
                            ext: ext.take(),
                        };
                        if Self::clean(child, conf, depth)? {
                            self.rm_child(cindex, idx);
                        }
                    }

                    Ok(deleted)
                }
                ExtensionMatch::Partial(_) => Ok(None),
            },
            Pointer::Dirty { node, ext } => {
                match match_extension(conf, hashed_key, ext)? {
                    ExtensionMatch::Full { skipped } => {
                        // Delete value and return deleted value
                        let deleted =
                            node.rm_value(hashed_key, conf, depth + 1 + skipped, key, store)?;

                        if deleted.is_some() && Self::clean(child, conf, depth)? {
                            self.rm_child(cindex, idx);
                        }

                        Ok(deleted)
                    }
                    ExtensionMatch::Partial(_) => Ok(None),
                }
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
            if let Pointer::Dirty { node, ext } = pointer {
                // Flush cached sub node to clear it's cache
                node.flush(store)?;

                // Put node in blockstore and retrieve Cid
                let cid = store.put_cbor(node, Code::Blake2b256)?;

                // Can keep the flushed node in link cache
                let cache = OnceCell::from(std::mem::take(node));

                // Replace cached node with Cid link
                *pointer = Pointer::Link {
                    cid,
                    ext: ext.take(),
                    cache,
                };
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

    fn insert_child_link(
        &mut self,
        idx: u32,
        cid: Cid,
        ext: Option<Extension>,
        cache: OnceCell<Box<Node<K, V, H>>>,
    ) {
        let i = self.index_for_bit_pos(idx);
        self.bitfield.set_bit(idx);
        self.pointers.insert(i, Pointer::Link { cid, ext, cache })
    }

    fn insert_child_dirty(&mut self, idx: u32, node: Box<Node<K, V, H>>, ext: Option<Extension>) {
        let i = self.index_for_bit_pos(idx);
        self.bitfield.set_bit(idx);
        self.pointers.insert(i, Pointer::Dirty { node, ext })
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

    /// We found a key that partially matched an extension. We have to insert a new node at the longest
    /// match and replace the existing link with one that points at this new node. The new node should
    /// in turn will have two children: a link to the original extension target, and the new key value pair.
    fn split_extension<'a, F>(
        conf: &Config,
        hashed_key: &'a mut HashBits,
        part: &PartialMatch,
        key: K,
        value: V,
        insert_pointer: F,
    ) -> Result<Pointer<K, V, H>, Error>
    where
        F: FnOnce(&mut Node<K, V, H>, u32, Option<Extension>),
    {
        // Need a new node at the split point.
        let mut midway = Node::<K, V, H>::default();

        // Point at the original node the link pointed at in the next nibble of the path after the split.
        let (head, idx, tail) = part.split(conf.bit_width)?;

        // Insert pointer to original.
        insert_pointer(&mut midway, idx, tail);

        // Insert the value at the next nibble of the hash.
        let idx = hashed_key.next(conf.bit_width)?;
        midway.insert_child(idx, key, value);

        // Replace the link in this node with one pointing at the midway node.
        Ok(Pointer::Dirty {
            node: Box::new(midway),
            ext: head,
        })
    }

    /// Clean after delete to retrieve canonical form.
    ///
    /// Returns true if the child pointer is completely empty and can be removed,
    /// which can happen if we artificially added nodes during insertion.
    fn clean(child: &mut Pointer<K, V, H>, conf: &Config, depth: u32) -> Result<bool, Error> {
        match child.clean(conf, depth) {
            Ok(()) => Ok(false),
            Err(Error::ZeroPointers) if depth < conf.min_data_depth || conf.push_data_to_leaves => {
                Ok(true)
            }
            Err(err) => Err(err),
        }
    }
}

/// Find the longest common non-empty prefix between the new key and the existing keys
/// that fell into the same bucket at some existing height.
fn find_longest_extension(
    conf: &Config,
    hashed_key: &mut HashBits,
    hashes: &[HashedKey],
) -> Result<Option<Extension>, Error> {
    if !conf.use_extensions {
        Ok(None)
    } else {
        let ext = Extension::longest_common_prefix(hashed_key, conf.bit_width, hashes)?;
        if ext.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ext))
        }
    }
}

/// Consume all but the very last level as an extension and return if non-empty.
/// The rest of the key will be used as the index in the leaf node.
///
/// Returns a flag telling if we're in the leaf node already, any extensions to
/// reach the leaf, and the number of levels skipped by the extension.
fn extension_to_leaf(
    conf: &Config,
    hashed_key: &mut HashBits,
) -> Result<(bool, Option<Extension>, u32), Error> {
    let bits_left = hashed_key.len() as u32 - hashed_key.consumed;
    if bits_left == 0 {
        // We are already in the leaf node.
        return Ok((true, None, 0));
    }

    // There is at least one more level to go.
    let consume = match bits_left {
        x if x <= conf.bit_width => 0,
        x if x % conf.bit_width == 0 => x - conf.bit_width,
        x => x - x % conf.bit_width,
    };

    let ext = Extension::from_bits(hashed_key, consume as u8)?;
    let ext = if ext.is_empty() { None } else { Some(ext) };

    Ok((false, ext, consume / conf.bit_width))
}

/// Helper method to check if a key matches an extension (if there is one)
/// and return the number of levels skipped. If the key doesn't match,
/// this will be the number of levels where the extension has to be split.
fn match_extension<'a, 'b>(
    conf: &Config,
    hashed_key: &'a mut HashBits,
    ext: &'b Option<Extension>,
) -> Result<ExtensionMatch<'b>, Error> {
    match ext {
        None => Ok(ExtensionMatch::Full { skipped: 0 }),
        Some(ext) => {
            let matched = ext.longest_match(hashed_key, conf.bit_width)?;
            let skipped = matched as u32 / conf.bit_width;

            if matched == ext.consumed() {
                Ok(ExtensionMatch::Full { skipped })
            } else {
                Ok(ExtensionMatch::Partial(PartialMatch { ext, matched }))
            }
        }
    }
}

/// Result of matching a `HashedKey` to an `Extension`.
enum ExtensionMatch<'a> {
    /// The hash fully matched the extension, which is also the case if there was no extension at all.
    Full { skipped: u32 },
    /// The hash matched some (potentially empty) prefix of the extension.
    Partial(PartialMatch<'a>),
}

struct PartialMatch<'a> {
    /// The original extension.
    ext: &'a Extension,
    /// Number of bits matched.
    matched: u8,
}

impl<'a> PartialMatch<'a> {
    /// Split the extension into the part before the match (which could be empty)
    /// the next nibble where the link pointing to the tail needs to be inserted
    /// into the new midway node, and the part after (which again could be empty).
    pub fn split(
        &self,
        bit_width: u32,
    ) -> Result<(Option<Extension>, u32, Option<Extension>), Error> {
        let (head, idx, tail) = self.ext.split(self.matched, bit_width)?;

        // Drop an empty head so we don't store it.
        let head = Some(head).filter(|e| !e.is_empty());
        let idx = idx.path_bits().next(bit_width)?;
        let tail = Some(tail).filter(|e| !e.is_empty());

        Ok((head, idx, tail))
    }
}
