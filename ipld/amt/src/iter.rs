// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::node::CollapsedNode;
use crate::node::{Link, Node};
use crate::Error;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use serde::de::DeserializeOwned;

impl<V, BS> crate::Amt<V, BS>
where
    V: DeserializeOwned,
{
    pub fn iter(&self) -> Iter<'_, V, &BS> {
        Iter {
            current_links: None,
            current_nodes: None,
            stack: vec![&self.root.node],
            blockstore: &self.block_store,
            branching_factor: self.branching_factor(),
        }
    }
}

// TODO(aatifsyed): is this guaranteed to be acyclic?
pub struct Iter<'a, V, BS> {
    current_links: Option<std::iter::Flatten<std::slice::Iter<'a, Option<Link<V>>>>>,
    current_nodes: Option<std::iter::Flatten<std::slice::Iter<'a, Option<V>>>>,
    stack: Vec<&'a Node<V>>,
    blockstore: BS,
    branching_factor: u32,
}

impl<'a, V, BS> Iterator for Iter<'a, V, BS>
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
