// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::borrow::Borrow;
use std::iter::FusedIterator;

use forest_hash_utils::BytesKey;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::de::DeserializeOwned;

use crate::hash_bits::HashBits;
use crate::node::Node;
use crate::pointer::version::Version;
use crate::pointer::{version, Pointer};
use crate::{Config, Error, Hash, HashAlgorithm, KeyValuePair, Sha256};

#[doc(hidden)]
pub struct IterImpl<'a, BS, V, K = BytesKey, H = Sha256, Ver = version::V3> {
    store: &'a BS,
    conf: &'a Config,
    stack: Vec<std::slice::Iter<'a, Pointer<K, V, H, Ver>>>,
    current: std::slice::Iter<'a, KeyValuePair<K, V>>,
}

/// Iterator over HAMT Key/Value tuples (hamt v0).
pub type Iterv0<'a, BS, V, K = BytesKey, H = Sha256> = IterImpl<'a, BS, V, K, H, version::V0>;

/// Iterator over HAMT Key/Value tuples.
pub type Iter<'a, BS, V, K = BytesKey, H = Sha256> = IterImpl<'a, BS, V, K, H, version::V3>;

impl<'a, K, V, BS, H, Ver> IterImpl<'a, BS, V, K, H, Ver>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
    Ver: Version,
    BS: Blockstore,
{
    pub(crate) fn new(store: &'a BS, root: &'a Node<K, V, H, Ver>, conf: &'a Config) -> Self {
        Self {
            conf,
            store,
            stack: vec![root.pointers.iter()],
            current: [].iter(),
        }
    }

    pub(crate) fn new_from<Q>(
        store: &'a BS,
        root: &'a Node<K, V, H, Ver>,
        key: &Q,
        conf: &'a Config,
    ) -> Result<Self, Error>
    where
        H: HashAlgorithm,
        K: Borrow<Q> + PartialOrd,
        Q: Hash + Eq + ?Sized,
    {
        let hashed_key = H::hash(key);
        let mut hash = HashBits::new(&hashed_key);
        let mut node = root;
        let mut stack = Vec::new();
        loop {
            let idx = hash.next(conf.bit_width)?;
            stack.push(node.pointers[node.index_for_bit_pos(idx)..].iter());
            node = match stack.last_mut().unwrap().next() {
                Some(p) => match p {
                    Pointer::Link { cid, cache } => cache.get_or_try_init(|| {
                        Node::load(conf, store, cid, stack.len() as u32).map(Box::new)
                    })?,
                    Pointer::Dirty(node) => node,
                    Pointer::Values(values) => {
                        return match values.iter().position(|kv| kv.key().borrow() == key) {
                            Some(offset) => Ok(Self {
                                conf,
                                store,
                                stack,
                                current: values[offset..].iter(),
                            }),
                            None => Err(Error::StartKeyNotFound),
                        }
                    }
                },
                None => continue,
            };
        }
    }
}

impl<'a, K, V, BS, H, Ver> Iterator for IterImpl<'a, BS, V, K, H, Ver>
where
    BS: Blockstore,
    Ver: Version,
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
{
    type Item = Result<(&'a K, &'a V), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(v) = self.current.next() {
            return Some(Ok((v.key(), v.value())));
        }
        loop {
            let Some(next) = self.stack.last_mut()?.next() else {
                self.stack.pop();
                continue;
            };
            match next {
                Pointer::Link { cid, cache } => {
                    let node = match cache.get_or_try_init(|| {
                        Node::load(self.conf, &self.store, cid, self.stack.len() as u32)
                            .map(Box::new)
                    }) {
                        Ok(node) => node,
                        Err(e) => return Some(Err(e)),
                    };
                    self.stack.push(node.pointers.iter())
                }
                Pointer::Dirty(node) => self.stack.push(node.pointers.iter()),
                Pointer::Values(kvs) => {
                    self.current = kvs.iter();
                    if let Some(v) = self.current.next() {
                        return Some(Ok((v.key(), v.value())));
                    }
                }
            }
        }
    }
}

impl<'a, K, V, BS, H, Ver> FusedIterator for IterImpl<'a, BS, V, K, H, Ver>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
    Ver: Version,
    BS: Blockstore,
{
}
