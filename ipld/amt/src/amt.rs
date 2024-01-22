// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
//! In the context of this code, an Array Mapped Trie (AMT) is a data structure
//! utilizing an IPLD blockstore that solves the problem of referencing shared
//! data without copying an entire array. This implementation is similar to an
//! [IPLD vector](https://github.com/ipld/specs/blob/master/data-structures/vector.md)
//! but supports internal node compression and therefore sparse arrays. This is
//! the Rust implementation of the Go implementation documented [here](https://pkg.go.dev/github.com/filecoin-project/go-amt-ipld/v4#section-readme):
//! "The AMT algorithm produces a tree-like graph, with a single root node addressing
//! a collection of child nodes which connect downward toward leaf nodes which store
//! the actual entries. No terminal entries are stored in intermediate elements
//! of the tree... We can divide up the AMT tree structure into "levels" or "heights",
//! where a height of zero contains the terminal elements, and the maximum height
//! of the tree contains the single root node. Intermediate nodes are used to span
//! across the range of indexes."
//!
//!
//! The maximum width for any node in the AMT structure is determined by
//! `2 ^ bit_width`, meaning a node with the default branching factor of
//! `3` has a maximum index range of `8` and can therefore be indexed from `0`
//! to `(2 ^ 3) - 1 = 7`. The maximum index range for the overall structure is
//! determined by both the branching factor and the height of the structure; the
//! width of this range is `bit_width ^ (height + 1)`. The height is specified
//! using a bottom-up numbering scheme, with the terminal leaves at a height of
//! `0` and the root node at the maximum height. Nodes can be either a `Link` or
//! a `Leaf` variant, which are actually a vector of links or a vector of values,
//! respectively. Each entry in the `Link` variant's vector may contain a CID or
//! a cache which holds a pointer to another `Node`; the pointer's value can only
//! be written once. Clearing the value of the cache and updating the CID requires
//! flushing, which is discussed in more detail below.
//!
//! An example with a single root node that is also a leaf node. This AMT has the
//! default branching factor, a height of `0`, and contains a single element at
//! index `2`.
//! ```text
//!                    ____________
//!                   | root node  |  <--height 0
//!                   |____________|
//!                         |
//!         | 0  | 1  | 2  | 3  | 4  | 5  | 6  | 7  |  <-- index
//!         |None|None|Some|None|None|None|None|None|  <-- value
//! ```
//!
//! A less trivial example is an AMT with a branching factor of `2` and a height
//! of `1`. The children nodes are leaf nodes in this example, and the leaves contain
//! values at indices `2`, `5`, `8`, and `15`.
//! ```text
//!                           ____________
//!                          | root node  |  <-- height 1
//!                          |____________|
//!                                 |
//!           __________________________________________________________________
//!          |                     |                     |                      |
//!    ____________           ____________          ____________           ____________
//!   | child node |         | child node |        | child node |         | child node |  <-- height 0
//!   |____________|         |____________|        |____________|         |____________|
//! | 0  | 1  | 2  | 3  |  | 4  | 5  | 6  | 7  |  | 8  | 9  | 10 | 11 |  | 12 | 13 | 14 | 15 |  <-- index
//! |None|None|Some|None|  |None|Some|None|None|  |Some|None|None|None|  |None|None|None|Some|  <-- value       
//! ```
//!
//! Extending this example a bit further, let's say we create an empty AMT with
//! a branching factor of two using `Amt::new_with_bit_width` and then
//! push a value to index `16` using `.set`. This will cause the AMT to expand
//! to a height of `2` with a structure as follows:
//! ```text
//!                           ____________
//!                          | root node  |  <-- height 2
//!                          |____________|
//!                                 |
//!           ___________________________________________________________________________________
//!          |                     |                                      |                      |
//!    ____________           ____________                          ____________           ____________
//!   | child node |         | child node |                        | child node |         | child node |  <-- height 1
//!   |____________|         |____________|                        |____________|         |____________|
//!          |                      |                                     |                      |
//!     ___________             _______________________              ___________            ___________
//!    |   |   |   |           |               |   |   |            |   |   |   |          |   |   |   |
//!    _   _   _   _      ____________         _   _   _            _   _   _   _          _   _   _   _
//!   |_| |_| |_| |_|    | child node |       |_| |_| |_|          |_| |_| |_| |_|        |_| |_| |_| |_| <-- height 0
//!  (indices 0 to 15)   |____________|       (indices 20 to 31)  (indices 32 to 47)      (indices 48 to 63)
//!                     | 16 | 17 | 18 | 19 |
//!                     |Some|None|None|None|
//! ```
//!
//! In this example, the child node containing indices 16 to 19 is expanded to show
//! detail; other than the value at index 16, all the other child nodes at height
//! `0` are functionally identical.
//!
//! Each parent node contains a CID that represents a hash of the children nodes
//! (CIDs) or leaves (values) under that node. When adding nodes and/or leaves,
//! it would be inefficient to refresh all the parent node CIDs until necessary.
//! As a result, modified nodes are identified using the `Dirty` variant of the
//! `Link` enum; this way the cache can store the updated node information, and
//! the CIDs are only regenerated when the AMT is flushed, which empties the data
//! in the cache.

use anyhow::anyhow;
use cid::multihash::Code;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::de::DeserializeOwned;
use fvm_ipld_encoding::ser::Serialize;
use fvm_ipld_encoding::serde::Deserialize;
use fvm_ipld_encoding::CborStore;
use itertools::sorted;

use super::ValueMut;
use crate::node::{CollapsedNode, Link};
use crate::root::version::{Version as AmtVersion, V0, V3};
use crate::root::RootImpl;
use crate::{
    init_sized_vec, nodes_for_height, Error, Node, DEFAULT_BIT_WIDTH, MAX_HEIGHT, MAX_INDEX,
};

#[derive(Debug)]
#[doc(hidden)]
pub struct AmtImpl<V, BS, Ver> {
    pub(crate) root: RootImpl<V, Ver>,
    pub(crate) block_store: BS,
    /// Remember the last flushed CID until it changes.
    flushed_cid: Option<Cid>,
}

/// Array Mapped Trie allows for the insertion and persistence of data, serializable to a CID.
///
/// Amt is not threadsafe and can't be shared between threads.
///
/// Usage:
/// ```
/// use fvm_ipld_amt::Amt;
///
/// let db = fvm_ipld_blockstore::MemoryBlockstore::default();
/// let mut amt = Amt::new(&db);
///
/// // Insert or remove any serializable values
/// amt.set(2, "foo".to_owned()).unwrap();
/// amt.set(1, "bar".to_owned()).unwrap();
/// amt.delete(2).unwrap();
/// assert_eq!(amt.count(), 1);
/// let bar: &String = amt.get(1).unwrap().unwrap();
///
/// // Generate cid by calling flush to remove cache
/// let cid = amt.flush().unwrap();
/// ```
pub type Amt<V, BS> = AmtImpl<V, BS, V3>;
/// Legacy amt V0
pub type Amtv0<V, BS> = AmtImpl<V, BS, V0>;

impl<V: PartialEq, BS: Blockstore, Ver: PartialEq> PartialEq for AmtImpl<V, BS, Ver> {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl<V, BS, Ver> AmtImpl<V, BS, Ver>
where
    Ver: AmtVersion,
{
    /// Constructor for Root AMT node
    pub fn new(block_store: BS) -> Self {
        Self::new_with_bit_width(block_store, DEFAULT_BIT_WIDTH)
    }

    /// Construct new Amt with given bit width
    pub fn new_with_bit_width(block_store: BS, bit_width: u32) -> Self {
        Self {
            root: RootImpl::new_with_bit_width(bit_width),
            block_store,
            flushed_cid: None,
        }
    }

    pub(super) fn bit_width(&self) -> u32 {
        self.root.bit_width
    }

    /// Gets the height of the `Amt`.
    pub fn height(&self) -> u32 {
        self.root.height
    }

    /// Gets count of elements added in the `Amt`.
    pub fn count(&self) -> u64 {
        self.root.count
    }
}

impl<V, BS, Ver> AmtImpl<V, BS, Ver>
where
    Ver: AmtVersion,
    BS: Blockstore,
    V: Serialize,
{
    /// Generates an AMT from an array of serializable objects.
    ///
    /// This can be called with an iterator of _references_ to values to avoid copying.
    pub fn new_from_iter(block_store: BS, vals: impl IntoIterator<Item = V>) -> Result<Cid, Error> {
        Self::new_from_iter_with_bit_width(block_store, DEFAULT_BIT_WIDTH, vals)
    }

    /// Generates an AMT with the requested bitwidth from an array of serializable objects.
    ///
    /// This can be called with an iterator of _references_ to values to avoid copying.
    pub fn new_from_iter_with_bit_width(
        block_store: BS,
        bit_width: u32,
        vals: impl IntoIterator<Item = V>,
    ) -> Result<Cid, Error> {
        #[derive(serde::Serialize)]
        #[serde(transparent)]
        struct FakeDeserialize<V>(V);

        impl<'de, V> Deserialize<'de> for FakeDeserialize<V> {
            fn deserialize<D>(_: D) -> Result<Self, D::Error>
            where
                D: fvm_ipld_encoding::serde_bytes::Deserializer<'de>,
            {
                use serde::de::Error;
                Err(D::Error::custom(
                    "can't deserialize when constructing an AMT from an iterator",
                ))
            }
        }

        let mut t = AmtImpl::<_, BS, Ver>::new_with_bit_width(block_store, bit_width);

        t.batch_set(vals.into_iter().map(FakeDeserialize))?;

        t.flush()
    }
}

impl<V, BS, Ver> AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned + Serialize,
    BS: Blockstore,
    Ver: AmtVersion,
{
    /// Constructs an AMT with a blockstore and a Cid of the root of the AMT
    pub fn load(cid: &Cid, block_store: BS) -> Result<Self, Error> {
        // Load root bytes from database
        let root: RootImpl<V, Ver> = block_store
            .get_cbor(cid)?
            .ok_or_else(|| Error::CidNotFound(cid.to_string()))?;

        // Sanity check, this should never be possible.
        if root.height > MAX_HEIGHT {
            return Err(Error::MaxHeight(root.height, MAX_HEIGHT));
        }

        Ok(Self {
            root,
            block_store,
            flushed_cid: Some(*cid),
        })
    }

    /// Get value at index of AMT
    pub fn get(&self, i: u64) -> Result<Option<&V>, Error> {
        if i > MAX_INDEX {
            return Err(Error::OutOfRange(i));
        }

        if i >= nodes_for_height(self.bit_width(), self.height() + 1) {
            return Ok(None);
        }

        self.root
            .node
            .get(&self.block_store, self.height(), self.bit_width(), i)
    }

    /// Set value at index
    pub fn set(&mut self, i: u64, val: V) -> Result<(), Error> {
        if i > MAX_INDEX {
            return Err(Error::OutOfRange(i));
        }

        while i >= nodes_for_height(self.bit_width(), self.height() + 1) {
            // node at index exists
            if !self.root.node.is_empty() {
                // Parent node for expansion
                let mut new_links: Vec<Option<Link<V>>> = init_sized_vec(self.root.bit_width);

                // Take root node to be moved down
                let node = std::mem::replace(&mut self.root.node, Node::empty());

                // Set link to child node being expanded
                new_links[0] = Some(Link::Dirty(Box::new(node)));

                self.root.node = Node::Link { links: new_links };
            } else {
                // If first expansion is before a value inserted, convert base node to Link
                self.root.node = Node::Link {
                    links: init_sized_vec(self.bit_width()),
                };
            }
            // Incrememnt height after each iteration
            self.root.height += 1;
        }

        if self
            .root
            .node
            .set(&self.block_store, self.height(), self.bit_width(), i, val)?
            .is_none()
        {
            self.root.count += 1;
        }

        // There's no equality constraint on `V` so we could check if the content changed.
        self.flushed_cid = None;

        Ok(())
    }

    /// Batch set (naive for now)
    // TODO Implement more efficient batch set to not have to traverse tree and keep cache for each
    pub fn batch_set(&mut self, vals: impl IntoIterator<Item = V>) -> Result<(), Error> {
        for (i, val) in (0u64..).zip(vals) {
            self.set(i, val)?;
        }

        Ok(())
    }

    /// Delete item from AMT at index
    pub fn delete(&mut self, i: u64) -> Result<Option<V>, Error> {
        if i > MAX_INDEX {
            return Err(Error::OutOfRange(i));
        }

        if i >= nodes_for_height(self.bit_width(), self.height() + 1) {
            // Index was out of range of current AMT
            return Ok(None);
        }

        // Delete node from AMT
        let deleted =
            self.root
                .node
                .delete(&self.block_store, self.height(), self.bit_width(), i)?;

        if deleted.is_none() {
            return Ok(None);
        }

        self.flushed_cid = None;
        self.root.count -= 1;

        if self.root.node.is_empty() {
            // Last link was removed, replace root with a leaf node and reset height.
            self.root.node = Node::Leaf {
                vals: init_sized_vec(self.root.bit_width),
            };
            self.root.height = 0;
        } else {
            // Handle collapsing node when the root is a link node with only one link,
            // sub node can be moved up into the root.
            while self.root.node.can_collapse() && self.height() > 0 {
                let sub_node: Node<V> = match &mut self.root.node {
                    Node::Link { links, .. } => match &mut links[0] {
                        Some(Link::Dirty(node)) => {
                            *std::mem::replace(node, Box::new(Node::empty()))
                        }
                        Some(Link::Cid { cid, cache }) => {
                            let cache_node = std::mem::take(cache);
                            if let Some(sn) = cache_node.into_inner() {
                                *sn
                            } else {
                                // Only retrieve sub node if not found in cache
                                self.block_store
                                    .get_cbor::<CollapsedNode<V>>(cid)?
                                    .ok_or_else(|| Error::CidNotFound(cid.to_string()))?
                                    .expand(self.root.bit_width)?
                            }
                        }
                        _ => unreachable!("First index checked to be Some in `can_collapse`"),
                    },
                    Node::Leaf { .. } => unreachable!("Non zero height cannot be a leaf node"),
                };

                self.root.node = sub_node;
                self.root.height -= 1;
            }
        }

        Ok(deleted)
    }

    /// Deletes multiple items from AMT
    /// If `strict` is true, all indices are expected to be present, and this will
    /// return an error if one is not found.
    ///
    /// Returns true if items were deleted.
    pub fn batch_delete(
        &mut self,
        iter: impl IntoIterator<Item = u64>,
        strict: bool,
    ) -> Result<bool, Error> {
        // TODO: optimize this
        let mut modified = false;

        // Iterate sorted indices. Sorted to safely optimize later.
        for i in sorted(iter) {
            let found = self.delete(i)?.is_some();
            if strict && !found {
                return Err(anyhow!("no such index {} in Amt for batch delete", i).into());
            }
            modified |= found;
        }
        Ok(modified)
    }

    /// flush root and return Cid used as key in block store
    pub fn flush(&mut self) -> Result<Cid, Error> {
        if let Some(cid) = self.flushed_cid {
            return Ok(cid);
        }
        self.root.node.flush(&self.block_store)?;
        let cid = self.block_store.put_cbor(&self.root, Code::Blake2b256)?;
        self.flushed_cid = Some(cid);
        Ok(cid)
    }

    /// Iterates over each value in the Amt and runs a function on the values.
    ///
    /// The index in the amt is a `u64` and the value is the generic parameter `V` as defined
    /// in the Amt.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_amt::Amt;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Amt<String, _> = Amt::new(&store);
    /// map.set(1, "One".to_owned()).unwrap();
    /// map.set(4, "Four".to_owned()).unwrap();
    ///
    /// let mut values: Vec<(u64, String)> = Vec::new();
    /// map.for_each(|i, v| {
    ///    values.push((i, v.clone()));
    ///    Ok(())
    /// }).unwrap();
    /// assert_eq!(&values, &[(1, "One".to_owned()), (4, "Four".to_owned())]);
    /// ```
    #[inline]
    #[deprecated = "use `.iter()` instead"]
    pub fn for_each<F>(&self, mut f: F) -> Result<(), Error>
    where
        F: FnMut(u64, &V) -> anyhow::Result<()>,
    {
        for res in self {
            let (k, v) = res?;
            (f)(k, v)?;
        }
        Ok(())
    }

    /// Iterates over each value in the Amt and runs a function on the values, for as long as that
    /// function keeps returning `true`.
    #[deprecated = "use `.iter()` instead"]
    pub fn for_each_while<F>(&self, mut f: F) -> Result<(), Error>
    where
        F: FnMut(u64, &V) -> anyhow::Result<bool>,
    {
        for res in self.iter() {
            let (i, v) = res?;
            if !f(i, v)? {
                break;
            }
        }
        Ok(())
    }

    /// Iterates over values in the Amt and runs a function on the values.
    ///
    /// The index in the amt is a `u64` and the value is the generic parameter `V` as defined
    /// in the Amt. If `start_at` is provided traversal begins at the first index >= `start_at`,
    /// otherwise it begins from the first element. If `limit` is provided, traversal will stop after
    /// `limit` elements have been traversed. Returns a tuple describing the number of elements
    /// iterated over and optionally the index of the next element in the AMT if more elements
    /// remain.
    ///
    /// # Examples
    ///
    /// ```
    /// use fvm_ipld_amt::Amt;
    ///
    /// let store = fvm_ipld_blockstore::MemoryBlockstore::default();
    ///
    /// let mut map: Amt<String, _> = Amt::new(&store);
    /// map.set(1, "One".to_owned()).unwrap();
    /// map.set(4, "Four".to_owned()).unwrap();
    /// map.set(5, "Five".to_owned()).unwrap();
    /// map.set(6, "Six".to_owned()).unwrap();
    /// map.set(10, "Ten".to_owned()).unwrap();
    ///
    /// let mut values: Vec<(u64, String)> = Vec::new();
    /// let (num_traversed, next_idx) = map.for_each_ranged(Some(4), Some(3), |i, v| {
    ///    values.push((i, v.clone()));
    ///    Ok(())
    /// }).unwrap();
    /// assert_eq!(&values, &[(4, "Four".to_owned()), (5, "Five".to_owned()), (6, "Six".to_owned())]);
    /// assert_eq!(num_traversed, 3);
    /// assert_eq!(next_idx, Some(10));
    /// ```
    #[deprecated = "use `.iter_from()` and `.take(limit)` instead"]
    pub fn for_each_ranged<F>(
        &self,
        start_at: Option<u64>,
        limit: Option<u64>,
        mut f: F,
    ) -> Result<(u64, Option<u64>), Error>
    where
        F: FnMut(u64, &V) -> anyhow::Result<()>,
    {
        let mut num_traversed = 0;
        for kv in self.iter_from(start_at.unwrap_or(0))? {
            let (k, v) = kv?;
            if limit.map(|l| num_traversed >= l).unwrap_or(false) {
                return Ok((num_traversed, Some(k)));
            }
            num_traversed += 1;
            f(k, v)?;
        }
        Ok((num_traversed, None))
    }

    /// Iterates over values in the Amt and runs a function on the values, for as long as that
    /// function keeps returning true.
    ///
    /// The index in the amt is a `u64` and the value is the generic parameter `V` as defined
    /// in the Amt. If `start_at` is provided traversal begins at the first index >= `start_at`,
    /// otherwise it begins from the first element. If `limit` is provided, traversal will stop after
    /// `limit` elements have been traversed. Returns a tuple describing the number of elements
    /// iterated over and optionally the index of the next element in the AMT if more elements
    /// remain.
    #[deprecated = "use `.iter_from()` and `.take(limit)` instead"]
    pub fn for_each_while_ranged<F>(
        &self,
        start_at: Option<u64>,
        limit: Option<u64>,
        mut f: F,
    ) -> Result<(u64, Option<u64>), Error>
    where
        F: FnMut(u64, &V) -> anyhow::Result<bool>,
    {
        let mut num_traversed = 0;
        let mut keep_going = true;
        for kv in self.iter_from(start_at.unwrap_or(0))? {
            let (k, v) = kv?;
            if !keep_going || limit.map(|l| num_traversed >= l).unwrap_or(false) {
                return Ok((num_traversed, Some(k)));
            }
            num_traversed += 1;
            keep_going = f(k, v)?;
        }
        Ok((num_traversed, None))
    }

    /// Iterates over each value in the Amt and runs a function on the values that allows modifying
    /// each value.
    pub fn for_each_mut<F>(&mut self, mut f: F) -> Result<(), Error>
    where
        F: FnMut(u64, &mut ValueMut<'_, V>) -> anyhow::Result<()>,
    {
        self.for_each_while_mut(|i, x| {
            f(i, x)?;
            Ok(true)
        })
    }

    /// Iterates over each value in the Amt and runs a function on the values that allows modifying
    /// each value, for as long as that function keeps returning `true`.
    pub fn for_each_while_mut<F>(&mut self, mut f: F) -> Result<(), Error>
    where
        F: FnMut(u64, &mut ValueMut<'_, V>) -> anyhow::Result<bool>,
    {
        let (_, did_mutate) = self.root.node.for_each_while_mut(
            &self.block_store,
            self.height(),
            self.bit_width(),
            0,
            &mut f,
        )?;

        if did_mutate {
            self.flushed_cid = None;
        }

        Ok(())
    }
}
