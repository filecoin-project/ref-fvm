// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::node::CollapsedNode;
use crate::node::{Link, Node};
use crate::root::version;
use crate::root::version::Version;
use crate::root::RootImpl;
use crate::Error;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::de::DeserializeOwned;
use fvm_ipld_encoding::CborStore;
use std::borrow::Borrow;
use std::iter::FusedIterator;

// David described a graph traversal algorithm to Josh.
// Here are their notes:
//
// 1) Pop root node to list.
// 2) Pop root and replace with child node/nodes.
// 3) Pop child node/nodes and replace with child node/nodes or leaf/leaves.

// TODO(aatifsyed): is this guaranteed to be acyclic?
pub struct Iter2<'a, V, BS> {
    current_links: Option<std::iter::Flatten<std::slice::Iter<'a, Option<Link<V>>>>>,
    current_nodes: Option<std::iter::Flatten<std::slice::Iter<'a, Option<V>>>>,
    // "abstract data types"
    // stack -> std::vec::Vec
    // queue -> VecDeque
    // list -> Vec // get by index, remove etc
    // Map -> HashMap or a BTreeMap
    stack: Vec<&'a Node<V>>,
    blockstore: BS,
    branching_factor: u32,
}


impl<'a, V, BS> Iterator for Iter2<'a, V, BS>
where
    BS: Blockstore,
    V: DeserializeOwned,
{
    type Item = anyhow::Result<&'a V>;
    fn next(&mut self) -> Option<Self::Item> {
        // do we have any work saved?
        // TODO(jdjaustin): simplify this state machine
        loop {
            if let Some(link) = self.current_links.as_mut().and_then(Iterator::next) {
                match link {
                    Link::Cid { cid, cache } => {
                        match cache.get_or_try_init(|| {
                            self.blockstore
                                .get_cbor::<CollapsedNode<V>>(cid)?
                                .ok_or_else(|| Error::CidNotFound(cid.to_string()))?
                                .expand(self.branching_factor)
                                .map(Box::new) // TODO(jdjaustin): why is this a Box??
                        }) {
                            // failed to load from blockstore
                            Err(e) => return Some(Err(e.into())),
                            Ok(node) => self.stack.push(node),
                        }
                    }
                    Link::Dirty(dirty) => self.stack.push(dirty),
                };
            }

            if let Some(node) = self.current_nodes.as_mut().and_then(Iterator::next) {
                return Some(Ok(node));
            }
            match self.stack.pop() {
                // TODO(jdjaustin): all these need renaming so they're more understandable
                Some(Node::Link { links }) => {
                    // if there are children, expand the stack and continue
                    self.current_links = Some(links.iter().flatten());
                    continue;
                }
                Some(Node::Leaf { vals }) => {
                    self.current_nodes = Some(vals.iter().flatten());
                }
                // all done!
                None => return None,
            }
        }
    }
}

pub struct IterImpl<'a, BS, V> {
    store: &'a BS,
    stack: &'a Node<V>,
    current: std::slice::Iter<'a, V>,
}

/// Iterator over AMT.
// TODO(aatifsyed): this is Go code masquerading as Rust
pub type Iter<'a, BS, V> = IterImpl<'a, BS, V>;

impl<'a, V, BS> IterImpl<'a, BS, V>
where
    V: DeserializeOwned,
    BS: Blockstore,
{
    pub(crate) fn new<Ver>(store: &'a BS, root: &'a RootImpl<V, Ver>) -> Self {
        Self {
            store,
            stack: &root.node,
            current: [].iter(),
        }
    }
}

impl<'a, V, BS> Iterator for IterImpl<'a, BS, V>
where
    BS: Blockstore,
    V: DeserializeOwned + 'a,
{
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let idx = 0;
        loop {
            match &self.stack {
                Node::Leaf { ref vals } => loop {
                    match vals.iter().next() {
                        Some(val) => {
                            if let Some(v) = val {
                                return Some(v);
                            }
                        }
                        None => {
                            return None;
                        }
                    }
                },
                Node::Link { links } => {
                    if let Some(link) = links.get(idx) {
                        match link.as_ref().unwrap() {
                            Link::Cid { cid: _, cache } => {
                                // TODO(aatifsyed): talk about this
                                todo!();
                                match cache.into_inner() {
                                    Some(node) => {}
                                    None => {
                                        return None;
                                    }
                                };
                            }
                            Link::Dirty(_) => {
                                return None;
                            }
                        }
                    }
                    todo!();
                    idx += 1;
                }
            }
        }
    }
}

impl<'a, V, BS> FusedIterator for IterImpl<'a, BS, V>
where
    V: DeserializeOwned,
    BS: Blockstore,
{
}
