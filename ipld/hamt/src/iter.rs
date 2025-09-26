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
use crate::pointer::{Pointer, version};
use crate::{Config, Error, Hash, HashAlgorithm, KeyValuePair, Sha256};

#[doc(hidden)]
pub struct IterImpl<'a, BS, V, K = BytesKey, H = Sha256, Ver = version::V3> {
    store: &'a BS,
    conf: &'a Config,
    stack: Vec<StackItem<'a, Pointer<K, V, H, Ver>>>,
    current: StackItem<'a, KeyValuePair<K, V>>,
}

/// Iterator over HAMT Key/Value tuples (hamt v0).
pub type Iterv0<'a, BS, V, K = BytesKey, H = Sha256> = IterImpl<'a, BS, V, K, H, version::V0>;

/// Iterator over HAMT Key/Value tuples.
pub type Iter<'a, BS, V, K = BytesKey, H = Sha256> = IterImpl<'a, BS, V, K, H, version::V3>;

enum StackItem<'a, V> {
    Iter(std::slice::Iter<'a, V>),
    IntoIter(std::vec::IntoIter<V>),
}

impl<'a, V> From<std::slice::Iter<'a, V>> for StackItem<'a, V> {
    fn from(value: std::slice::Iter<'a, V>) -> Self {
        Self::Iter(value)
    }
}

impl<V> From<std::vec::IntoIter<V>> for StackItem<'_, V> {
    fn from(value: std::vec::IntoIter<V>) -> Self {
        Self::IntoIter(value)
    }
}

impl<'a, V> Iterator for StackItem<'a, V> {
    type Item = IterItem<'a, V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Iter(it) => it.next().map(|i| i.into()),
            Self::IntoIter(it) => it.next().map(|i| i.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IterItem<'a, V> {
    Borrowed(&'a V),
    Owned(V),
}

impl<V> AsRef<V> for IterItem<'_, V> {
    fn as_ref(&self) -> &V {
        match self {
            Self::Borrowed(v) => v,
            Self::Owned(v) => v,
        }
    }
}

impl<V> From<V> for IterItem<'_, V> {
    fn from(value: V) -> Self {
        Self::Owned(value)
    }
}

impl<'a, V> From<&'a V> for IterItem<'a, V> {
    fn from(value: &'a V) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a, K, V, BS, H, Ver> IterImpl<'a, BS, V, K, H, Ver>
where
    K: DeserializeOwned + Clone,
    V: DeserializeOwned + Clone,
    Ver: Version,
    BS: Blockstore,
{
    pub(crate) fn new(store: &'a BS, root: &'a Node<K, V, H, Ver>, conf: &'a Config) -> Self {
        Self {
            conf,
            store,
            stack: vec![root.pointers.iter().into()],
            current: [].iter().into(),
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
        let mut node = IterItem::Borrowed(root);
        let mut stack = Vec::new();
        loop {
            let idx = hash.next(conf.bit_width)?;
            match node.clone() {
                IterItem::Borrowed(node) => {
                    stack.push(StackItem::from(
                        node.pointers[node.index_for_bit_pos(idx)..].iter(),
                    ));
                }
                IterItem::Owned(node) => {
                    stack.push(StackItem::from(
                        #[allow(clippy::unnecessary_to_owned)]
                        node.pointers[node.index_for_bit_pos(idx)..]
                            .to_vec()
                            .into_iter(),
                    ));
                }
            }
            node = match stack.last_mut().unwrap().next() {
                Some(p) => match p {
                    IterItem::Borrowed(Pointer::Link { cid, cache: _ }) => {
                        Node::load(conf, store, cid, stack.len() as u32)?.into()
                    }
                    IterItem::Owned(Pointer::Link { cid, cache: _ }) => {
                        Node::load(conf, store, &cid, stack.len() as u32)?.into()
                    }
                    IterItem::Borrowed(Pointer::Dirty(node)) => node.as_ref().into(),
                    IterItem::Owned(Pointer::Dirty(node)) => (*node).into(),
                    IterItem::Borrowed(Pointer::Values(values)) => {
                        return match values.iter().position(|kv| kv.key().borrow() == key) {
                            Some(offset) => Ok(Self {
                                conf,
                                store,
                                stack,
                                current: values[offset..].iter().into(),
                            }),
                            None => Err(Error::StartKeyNotFound),
                        };
                    }
                    IterItem::Owned(Pointer::Values(values)) => {
                        return match values.iter().position(|kv| kv.key().borrow() == key) {
                            Some(offset) => Ok(Self {
                                conf,
                                store,
                                stack,
                                #[allow(clippy::unnecessary_to_owned)]
                                current: values[offset..].to_vec().into_iter().into(),
                            }),
                            None => Err(Error::StartKeyNotFound),
                        };
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
    type Item = Result<(IterItem<'a, K>, IterItem<'a, V>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current.next() {
            Some(IterItem::Borrowed(v)) => return Some(Ok((v.key().into(), v.value().into()))),
            Some(IterItem::Owned(KeyValuePair(k, v))) => return Some(Ok((k.into(), v.into()))),
            _ => {}
        }
        loop {
            let Some(next) = self.stack.last_mut()?.next() else {
                self.stack.pop();
                continue;
            };
            match next {
                IterItem::Borrowed(Pointer::Link { cid, cache: _ }) => {
                    let node =
                        match Node::load(self.conf, &self.store, cid, self.stack.len() as u32) {
                            Ok(node) => node,
                            Err(e) => return Some(Err(e)),
                        };
                    self.stack.push(node.pointers.into_iter().into())
                }
                IterItem::Owned(Pointer::Link { cid, cache: _ }) => {
                    let node =
                        match Node::load(self.conf, &self.store, &cid, self.stack.len() as u32) {
                            Ok(node) => node,
                            Err(e) => return Some(Err(e)),
                        };
                    self.stack.push(node.pointers.into_iter().into())
                }
                IterItem::Borrowed(Pointer::Dirty(node)) => {
                    self.stack.push(node.pointers.iter().into())
                }
                IterItem::Owned(Pointer::Dirty(node)) => {
                    self.stack.push(node.pointers.into_iter().into())
                }
                IterItem::Borrowed(Pointer::Values(kvs)) => {
                    self.current = kvs.iter().into();
                    match self.current.next() {
                        Some(IterItem::Borrowed(v)) => {
                            return Some(Ok((v.key().into(), v.value().into())));
                        }
                        Some(IterItem::Owned(KeyValuePair(k, v))) => {
                            return Some(Ok((k.into(), v.into())));
                        }
                        _ => {}
                    }
                }
                IterItem::Owned(Pointer::Values(kvs)) => {
                    self.current = kvs.into_iter().into();
                    match self.current.next() {
                        Some(IterItem::Borrowed(v)) => {
                            return Some(Ok((v.key().into(), v.value().into())));
                        }
                        Some(IterItem::Owned(KeyValuePair(k, v))) => {
                            return Some(Ok((k.into(), v.into())));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl<K, V, BS, H, Ver> FusedIterator for IterImpl<'_, BS, V, K, H, Ver>
where
    K: DeserializeOwned + PartialOrd,
    V: DeserializeOwned,
    Ver: Version,
    BS: Blockstore,
{
}
