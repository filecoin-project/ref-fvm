// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::borrow::Borrow;
use std::iter::FusedIterator;

use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::de::DeserializeOwned;

use crate::hash_bits::HashBits;
use crate::node::{match_extension, ExtensionMatch, Node};
use crate::pointer::Pointer;
use crate::{AsHashedKey, Config, Error, KeyValuePair};

/// Iterator over a KAMT. Items are ordered by-key, ascending.
pub struct Iter<'a, BS, V, K, H, const N: usize = 32> {
    store: &'a BS,
    conf: &'a Config,
    stack: Vec<std::slice::Iter<'a, Pointer<K, V, H, N>>>,
    current: std::slice::Iter<'a, KeyValuePair<K, V>>,
}

impl<'a, K, V, BS, H, const N: usize> Iter<'a, BS, V, K, H, N>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
    BS: Blockstore,
{
    pub(crate) fn new(store: &'a BS, root: &'a Node<K, V, H, N>, conf: &'a Config) -> Self {
        Self {
            conf,
            store,
            stack: vec![root.pointers.iter()],
            current: [].iter(),
        }
    }

    pub(crate) fn new_from<Q>(
        store: &'a BS,
        root: &'a Node<K, V, H, N>,
        key: &Q,
        conf: &'a Config,
    ) -> Result<Self, Error>
    where
        K: Borrow<Q> + PartialOrd,
        Q: PartialEq + Sized,
        H: AsHashedKey<Q, N>,
    {
        let hashed_key = H::as_hashed_key(key);
        let mut hash = HashBits::new(&hashed_key);
        let mut node = root;
        let mut stack = Vec::new();

        loop {
            let idx = hash.next(conf.bit_width)?;
            let ext;
            stack.push(node.pointers[node.index_for_bit_pos(idx)..].iter());
            (node, ext) = match stack.last_mut().unwrap().next() {
                Some(p) => match p {
                    Pointer::Link {
                        cid, cache, ext, ..
                    } => (
                        cache.get_or_try_init(|| {
                            Node::load(conf, store, cid, stack.len() as u32).map(Box::new)
                        })?,
                        ext,
                    ),
                    Pointer::Dirty { node, ext, .. } => (node, ext),
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

            match match_extension(conf, &mut hash, ext)? {
                ExtensionMatch::Full { .. } => {}
                ExtensionMatch::Partial { .. } => return Err(Error::StartKeyNotFound),
            }
        }
    }
}
impl<'a, K, V, BS, H, const N: usize> Iterator for Iter<'a, BS, V, K, H, N>
where
    BS: Blockstore,
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
                Pointer::Link { cid, cache, .. } => {
                    let node = match cache.get_or_try_init(|| {
                        Node::load(self.conf, self.store, cid, self.stack.len() as u32)
                            .map(Box::new)
                    }) {
                        Ok(node) => node,
                        Err(e) => return Some(Err(e)),
                    };
                    self.stack.push(node.pointers.iter())
                }
                Pointer::Dirty { node, .. } => self.stack.push(node.pointers.iter()),
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

impl<'a, K, V, BS, H, const N: usize> FusedIterator for Iter<'a, BS, V, K, H, N>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
    BS: Blockstore,
{
}
