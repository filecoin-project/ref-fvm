// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::node::CollapsedNode;
use crate::node::{Link, Node};
use crate::MAX_INDEX;
use crate::{nodes_for_height, Error};
use anyhow::anyhow;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::ser::Serialize;
use fvm_ipld_encoding::CborStore;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

impl<V, BS, Ver> crate::AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned + Serialize,
    BS: Blockstore,
    Ver: crate::root::version::Version,
{
    /// Iterate over the AMT. Alternatively, you can directly iterate over the AMT without calling
    /// this method:
    ///
    /// ```rust
    /// use fvm_ipld_amt::Amt;
    /// use fvm_ipld_blockstore::MemoryBlockstore;
    ///
    /// let store = MemoryBlockstore::default();
    ///
    /// let mut amt = Amt::new(store);
    /// let kvs: Vec<u64> = (0..=5).collect();
    /// kvs
    ///     .iter()
    ///     .map(|k| amt.set(*k, k.to_string()))
    ///     .collect::<Vec<_>>();
    ///
    /// for kv in &amt {
    ///     let (k, v) = kv?;
    ///     println!("{k:?}: {v:?}");
    /// }
    ///
    /// # anyhow::Ok(())
    /// ```
    pub fn iter(&self) -> Iter<'_, V, &BS, Ver> {
        Iter::new(
            &self.root.node,
            &self.block_store,
            self.height(),
            self.bit_width(),
            0,
        )
    }

    /// Iterate over the AMT from the given starting point.
    ///
    /// ```rust
    /// use fvm_ipld_amt::Amt;
    /// use fvm_ipld_blockstore::MemoryBlockstore;
    ///
    /// let store = MemoryBlockstore::default();
    ///
    /// let mut amt = Amt::new(store);
    /// let kvs: Vec<u64> = (0..=5).collect();
    /// kvs
    ///     .iter()
    ///     .map(|k| amt.set(*k, k.to_string()))
    ///     .collect::<Vec<_>>();
    ///
    /// for kv in amt.iter_from(3)? {
    ///     let (k, v) = kv?;
    ///     println!("{k:?}: {v:?}");
    /// }
    ///
    /// # anyhow::Ok(())
    /// ```
    pub fn iter_from(&self, start: u64) -> Result<Iter<'_, V, &BS, Ver>, Error> {
        // Short-circuit when we're starting at 0.
        if start == 0 {
            return Ok(self.iter());
        }

        let height = self.height();
        let bit_width = self.bit_width();

        // Fast-path for case where start is beyond what we know this amt could currently contain.
        if start >= nodes_for_height(bit_width, height + 1) {
            return Ok(Iter {
                height,
                bit_width,
                stack: Vec::new(),
                blockstore: &self.block_store,
                ver: PhantomData,
                key: start,
            });
        }

        let mut stack = Vec::with_capacity(height as usize);
        let mut node = &self.root.node;
        let mut offset = 0;
        loop {
            let start_idx = start.saturating_sub(offset);
            match node {
                Node::Leaf { vals } => {
                    if start_idx >= vals.len() as u64 {
                        // Not deep enough.
                        return Err(anyhow!("incorrect height for tree depth: expected values at depth {}, found them at {}", height, stack.len()).into());
                    }
                    stack.push(IterStack {
                        node,
                        idx: start_idx as usize,
                    });
                    break;
                }
                Node::Link { links } => {
                    let nfh =
                        nodes_for_height(self.bit_width(), self.height() - stack.len() as u32);
                    let idx: usize = (start_idx / nfh).try_into().expect("index overflow");
                    assert!(idx < links.len(), "miscalculated nodes for height");
                    let Some(l) = &links[idx] else {
                        // If there's nothing here, mark this as the starting point. We'll start
                        // scanning here when we iterate.
                        stack.push(IterStack { node, idx });
                        break;
                    };
                    let sub = match l {
                        Link::Dirty(sub) => sub,
                        Link::Cid { cid, cache } => cache.get_or_try_init(|| {
                            self.block_store
                                .get_cbor::<CollapsedNode<V>>(cid)?
                                .ok_or_else(|| Error::CidNotFound(cid.to_string()))?
                                .expand(self.bit_width())
                                .map(Box::new)
                        })?,
                    };
                    // Push idx+1 because we've already processed this node.
                    stack.push(IterStack { node, idx: idx + 1 });
                    node = sub;
                    offset += idx as u64 * nfh;
                }
            }
        }
        Ok(Iter {
            stack,
            height,
            bit_width,
            blockstore: &self.block_store,
            ver: PhantomData,
            key: start,
        })
    }
}

impl<'a, V, BS, Ver> IntoIterator for &'a crate::AmtImpl<V, BS, Ver>
where
    V: Serialize + DeserializeOwned,
    Ver: crate::root::version::Version,
    BS: Blockstore,
{
    type IntoIter = Iter<'a, V, &'a BS, Ver>;
    type Item = Result<(u64, &'a V), crate::Error>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct Iter<'a, V, BS, Ver> {
    stack: Vec<IterStack<'a, V>>,
    height: u32,
    blockstore: BS,
    bit_width: u32,
    ver: PhantomData<Ver>,
    key: u64,
}

impl<'a, V, BS, Ver> Iter<'a, V, &'a BS, Ver> {
    pub(crate) fn new(
        node: &'a Node<V>,
        blockstore: &'a BS,
        height: u32,
        bit_width: u32,
        offset: u64,
    ) -> Self {
        let mut stack = Vec::with_capacity(height as usize);
        stack.push(IterStack { node, idx: 0 });
        Iter {
            stack,
            height,
            blockstore,
            bit_width,
            ver: PhantomData,
            key: offset,
        }
    }
}

pub struct IterStack<'a, V> {
    pub(crate) node: &'a Node<V>,
    pub(crate) idx: usize,
}

impl<'a, V, BS, Ver> Iterator for Iter<'a, V, BS, Ver>
where
    BS: Blockstore,
    V: Serialize + DeserializeOwned,
{
    type Item = Result<(u64, &'a V), crate::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        while self.key <= MAX_INDEX {
            let stack = self.stack.last_mut()?;
            match stack.node {
                Node::Leaf { vals } => {
                    while stack.idx < vals.len() {
                        match vals[stack.idx] {
                            Some(ref v) => {
                                stack.idx += 1;
                                self.key += 1;
                                return Some(Ok((self.key - 1, v)));
                            }
                            None => {
                                stack.idx += 1;
                                self.key += 1;
                            }
                        }
                    }
                    self.stack.pop();
                }
                Node::Link { links } => {
                    match links.get(stack.idx) {
                        Some(Some(Link::Cid { cid, cache })) => {
                            match cache.get_or_try_init(|| {
                                self.blockstore
                                    .get_cbor::<CollapsedNode<V>>(cid)?
                                    .ok_or_else(|| Error::CidNotFound(cid.to_string()))?
                                    .expand(self.bit_width)
                                    .map(Box::new)
                            }) {
                                Ok(node) => {
                                    stack.idx += 1;
                                    self.stack.push(IterStack {
                                        node: node.as_ref(),
                                        idx: 0,
                                    });
                                }
                                Err(e) => {
                                    stack.idx += 1;
                                    return Some(Err(e));
                                }
                            }
                        }
                        Some(Some(Link::Dirty(node))) => {
                            stack.idx += 1;
                            self.stack.push(IterStack {
                                node: node.as_ref(),
                                idx: 0,
                            });
                        }
                        Some(&None) => {
                            stack.idx += 1;
                            self.key += nodes_for_height(
                                self.bit_width,
                                self.height - self.stack.len() as u32 + 1,
                            );
                        }
                        None => {
                            self.stack.pop();
                        }
                    };
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::Amt;
    use crate::MAX_INDEX;
    use fvm_ipld_blockstore::tracking::TrackingBlockstore;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::BytesDe;
    use quickcheck_macros::quickcheck;

    #[test]
    fn check_iter_empty() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(0, "foo".to_owned()).unwrap();
        amt.delete(0).unwrap();
        assert!(amt.iter().next().is_none());
    }

    #[test]
    fn check_iter_next_single_element() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(0, "foo".to_owned()).unwrap();
        assert_eq!(amt.iter().next().unwrap().unwrap(), (0, &"foo".to_owned()));
    }

    #[test]
    fn check_iter_next_with_none() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(1, "foo".to_owned()).unwrap();
        assert_eq!(amt.iter().next().unwrap().unwrap(), (1, &"foo".to_owned()));
    }

    #[test]
    fn check_iter_next_with_two_sets() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 2);
        amt.set(1, "foo".to_owned()).unwrap();
        amt.set(2, "bar".to_owned()).unwrap();
        let mut amt_iter = amt.iter();
        assert_eq!(amt_iter.next().unwrap().unwrap(), (1, &"foo".to_owned()));
        assert_eq!(amt_iter.next().unwrap().unwrap(), (2, &"bar".to_owned()));
    }

    #[test]
    fn check_iter_next_with_link() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new(&db);
        amt.set(8, "foo".to_owned()).unwrap();
        assert_eq!(amt.iter().next().unwrap().unwrap(), (8, &"foo".to_owned()));
    }

    #[test]
    fn check_iter_next_with_links() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new(&db);
        amt.set(8, "foo".to_owned()).unwrap();
        amt.set(500, "bar".to_owned()).unwrap();
        let mut amt_iter = amt.iter();
        assert_eq!(amt_iter.next().unwrap().unwrap(), (8, &"foo".to_owned()));
        assert_eq!(amt_iter.next().unwrap().unwrap(), (500, &"bar".to_owned()));
    }

    #[test]
    fn minimal_new_from_iter() {
        let mem = MemoryBlockstore::default();
        let data: Vec<String> = (0..1).map(|i| format!("thing{i}")).collect();
        let k = Amt::<&str, _>::new_from_iter(&mem, data.iter().map(|s| &**s)).unwrap();
        let a: Amt<String, _> = Amt::load(&k, &mem).unwrap();
        let mut restored = Vec::new();
        #[allow(deprecated)]
        a.for_each(|k, v| {
            restored.push((k as usize, v.clone()));
            Ok(())
        })
        .unwrap();
        let expected: Vec<_> = data.into_iter().enumerate().collect();
        assert_eq!(expected, restored);
    }

    // Helper function for `for_each` test
    fn tbytes(bz: &[u8]) -> BytesDe {
        BytesDe(bz.to_vec())
    }

    #[test]
    fn minimal_for_each() {
        let mem = MemoryBlockstore::default();
        let db = TrackingBlockstore::new(&mem);
        let mut a = Amt::new(&db);

        let mut indexes = Vec::new();
        for i in 0..10000 {
            if (i + 1) % 3 == 0 {
                indexes.push(i);
            }
        }

        // Set all indices in the Amt
        for i in indexes.iter() {
            a.set(*i, tbytes(b"value")).unwrap();
        }

        // Flush and regenerate amt
        let c = a.flush().unwrap();
        let new_amt = Amt::load(&c, &db).unwrap();

        let mut x = 0;
        #[allow(deprecated)]
        new_amt
            .for_each(|k, _: &BytesDe| {
                if k != indexes[x] {
                    panic!(
                        "for each found wrong index: expected {} got {}",
                        indexes[x], k
                    );
                }
                x += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(x, indexes.len());
    }

    #[quickcheck]
    fn vary_bit_width(bit_width: u32) {
        // `bit_width` is only limited due to the test taking too long to run at higher values.
        let bit_width = bit_width % 20;
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt: crate::amt::AmtImpl<
            String,
            &fvm_ipld_blockstore::MemoryBlockstore,
            crate::root::version::V3,
        > = Amt::new_with_bit_width(&db, bit_width);
        amt.set(0, "foo".to_owned()).unwrap();
    }

    #[quickcheck]
    fn random_set_and_iterate(idx: u64, bit_width: u32) {
        // `bit_width` is only limited due to the test taking too long to run at higher values.
        let bit_width = bit_width % 20;
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt: crate::amt::AmtImpl<
            String,
            &fvm_ipld_blockstore::MemoryBlockstore,
            crate::root::version::V3,
        > = Amt::new_with_bit_width(&db, bit_width);
        let idx = match bit_width {
            0 => 0,
            _ => idx % u64::pow(bit_width as u64, (amt.height() + 1) - 1),
        };
        amt.set(idx, "foo".to_owned()).unwrap();
        assert_eq!(
            amt.iter().next().unwrap().unwrap(),
            (idx, &"foo".to_owned())
        );
    }

    #[quickcheck]
    fn multiple_random_set_and_iterate(idx: u64, bit_width: u32) {
        // `bit_width` is only limited due to the test taking too long to run at higher values.
        let bit_width = bit_width % 3;
        // Starting at a bit_width of 0 causes the test to take too long to run.
        let bit_width = match bit_width {
            0 => 1,
            _ => bit_width,
        };
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt: crate::amt::AmtImpl<
            String,
            &fvm_ipld_blockstore::MemoryBlockstore,
            crate::root::version::V3,
        > = Amt::new_with_bit_width(&db, bit_width);
        // We don't want the test to take too long at higher indexes.
        let mut idx = idx % 42;
        while idx > 0 {
            idx -= 1;
            amt.set(idx, "foo".to_owned() + &idx.to_string()).unwrap();
        }
        for item in &amt {
            let (idx, val) = item.unwrap();
            assert_eq!(val, &("foo".to_owned() + &idx.to_string()));
        }
    }

    #[test]
    fn max_index() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt: crate::amt::AmtImpl<
            String,
            &fvm_ipld_blockstore::MemoryBlockstore,
            crate::root::version::V3,
        > = Amt::new(&db);
        amt.set(MAX_INDEX, "foo".to_owned()).unwrap();
        let mut amt_iter = amt.iter();
        assert_eq!(
            amt_iter.next().unwrap().unwrap(),
            (MAX_INDEX, &"foo".to_owned())
        );
        // This should not panic at `attempt to add with overflow`.
        assert!(amt_iter.next().is_none());
    }
}
