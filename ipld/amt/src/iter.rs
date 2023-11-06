// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::node::CollapsedNode;
use crate::node::{Link, Node};
use crate::Error;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::ser::Serialize;
use fvm_ipld_encoding::CborStore;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

impl<V, BS, Ver> crate::AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned,
    Ver: crate::root::version::Version,
{
    pub fn iter(&self) -> Iter<'_, V, &BS, Ver> {
        Iter {
            stack: vec![IterStack {
                node: &self.root.node,
                idx: 0,
            }],
            blockstore: &self.block_store,
            bit_width: self.bit_width(),
            ver: PhantomData,
        }
    }
}

impl<'a, V, BS, Ver> IntoIterator for &'a crate::AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned,
    Ver: crate::root::version::Version,
    BS: Blockstore,
{
    type IntoIter = Iter<'a, V, &'a BS, Ver>;
    type Item = Result<&'a V, crate::Error>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<V, BS> crate::Amt<V, BS>
where
    V: DeserializeOwned + Serialize,
    BS: Blockstore,
{
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
        for (ix, res) in self.iter().enumerate() {
            let v = res?;
            f(ix as u64, v)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Iter<'a, V, BS, Ver> {
    stack: Vec<IterStack<'a, V>>,
    blockstore: BS,
    bit_width: u32,
    ver: PhantomData<Ver>,
}

#[derive(Debug)]
pub struct IterStack<'a, V> {
    pub(crate) node: &'a Node<V>,
    pub(crate) idx: usize,
}

impl<'a, V, BS, Ver> Iterator for Iter<'a, V, BS, Ver>
where
    BS: Blockstore,
    V: DeserializeOwned,
{
    type Item = Result<&'a V, crate::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let stack = self.stack.last_mut()?;
            match stack.node {
                Node::Leaf { vals } => {
                    let mut idx = 0;
                    stack.idx += 1;
                    while idx < vals.len() {
                        match vals[idx] {
                            Some(ref v) => return Some(Ok(v)),
                            None => {
                                idx += 1;
                            }
                        }
                    }
                }
                Node::Link { links } => {
                    let mut idx = 0;
                    stack.idx += 1;
                    while idx < links.len() {
                        dbg!(idx);
                        dbg!("matching link");
                        let link = &links[idx];
                        match link {
                            Some(Link::Cid { cid, cache }) => {
                                match cache.get_or_try_init(|| {
                                    self.blockstore
                                        .get_cbor::<CollapsedNode<V>>(cid)?
                                        .ok_or_else(|| Error::CidNotFound(cid.to_string()))?
                                        .expand(self.bit_width)
                                        .map(Box::new)
                                }) {
                                    Ok(node) => {
                                        self.stack.push(IterStack {
                                            node: node.as_ref(),
                                            idx: idx,
                                        });
                                    }
                                    Err(e) => return Some(Err(e)),
                                }
                                break;
                            }
                            Some(Link::Dirty(node)) => {
                                self.stack.push(IterStack {
                                    node: node.as_ref(),
                                    idx: idx,
                                });
                                break;
                            }
                            None => {
                                idx += 1;
                            }
                        };
                    }
                }
            }
        }
    }
}

// TODO(aatifsyed): is this guaranteed to be acyclic?
#[cfg(test)]
mod tests {
    use crate::Amt;
    use quickcheck_macros::quickcheck;

    #[test]
    fn check_iter() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(0, "foo".to_owned()).unwrap();
        dbg!(amt.iter());
    }

    #[test]
    fn check_iter_next_single_element() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(0, "foo".to_owned()).unwrap();
        dbg!(amt.iter().next().unwrap().unwrap());
        assert_eq!(amt.iter().next().unwrap().unwrap(), "foo");
    }

    #[test]
    fn check_iter_next_with_none() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(1, "foo".to_owned()).unwrap();
        dbg!(&amt);
        dbg!(amt.iter());
        dbg!(amt.iter().next());
        assert_eq!(amt.iter().next().unwrap().unwrap(), "foo");
    }

    #[test]
    fn check_iter_next_with_links() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new(&db);
        amt.set(8, "foo".to_owned()).unwrap();
        dbg!(&amt);
        dbg!(amt.iter());
        dbg!(amt.iter().next());
        assert_eq!(amt.iter().next().unwrap().unwrap(), "foo");
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
    fn set_and_iterate() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new(&db);
        amt.set(8, "foo".to_owned()).unwrap();
        assert_eq!(amt.iter().next().unwrap().unwrap(), "foo");
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
        assert_eq!(amt.iter().next().unwrap().unwrap(), "foo");
    }

    // TODO: fix this test
    // #[quickcheck]
    // fn multiple_random_set_and_iterate(idx: u64, bit_width: u32) {
    //     // `bit_width` is only limited due to the test taking too long to run at higher values.
    //     let bit_width = bit_width % 3;
    //     // Starting at a bit_width of 0 causes the test to take too long to run.
    //     let bit_width = match bit_width {
    //         0 => 1,
    //         _ => bit_width,
    //     };
    //     dbg!(bit_width);
    //     dbg!(idx);
    //     let db = fvm_ipld_blockstore::MemoryBlockstore::default();
    //     let mut amt: crate::amt::AmtImpl<
    //         String,
    //         &fvm_ipld_blockstore::MemoryBlockstore,
    //         crate::root::version::V3,
    //     > = Amt::new_with_bit_width(&db, bit_width);
    //     // We don't want the test to take too long at higher indexes.
    //     let mut idx = idx % 42;
    //     while idx > 0 {
    //         idx -= 1;
    //         amt.set(idx, "foo".to_owned() + &idx.to_string()).unwrap();
    //     }
    //     dbg!(&amt);
    //     for item in amt.iter().enumerate() {
    //         assert_eq!(
    //             item.1.unwrap().to_owned(),
    //             "foo".to_owned() + &item.0.to_string()
    //         );
    //     }
    // }
}
