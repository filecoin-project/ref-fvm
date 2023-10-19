// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::node::CollapsedNode;
use crate::node::{Link, Node};
use crate::Error;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::ser::Serialize;
use fvm_ipld_encoding::CborStore;
use serde::de::DeserializeOwned;

impl<V, BS, Ver> crate::AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned,
    Ver: crate::root::version::Version,
{
    pub fn iter(&self) -> Iter<'_, V, &BS> {
        Iter {
            current_links: None,
            current_nodes: None,
            stack: vec![&self.root.node],
            blockstore: &self.block_store,
            bit_width: self.bit_width(),
        }
    }
}

impl<'a, V, BS, Ver> IntoIterator for &'a crate::AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned,
    Ver: crate::root::version::Version,
    BS: Blockstore,
{
    type IntoIter = Iter<'a, V, &'a BS>;
    type Item = anyhow::Result<&'a V>;
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

// TODO(aatifsyed): is this guaranteed to be acyclic?
#[cfg(test)]
mod tests {
    use crate::Amt;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn vary_bit_width(bit_width: u32) {
        let bit_width = bit_width % 20;
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt: crate::amt::AmtImpl<
            String,
            &fvm_ipld_blockstore::MemoryBlockstore,
            crate::root::version::V3,
        > = Amt::new_with_bit_width(&db, bit_width);
        amt.set(0, "foo".to_owned()).unwrap();
        // dbg!(amt);
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

    #[quickcheck]
    fn multiple_random_set_and_iterate(idx: u64, bit_width: u32) {
        // `bit_width` is only limited due to the test taking too long to run at higher values.
        let bit_width = bit_width % 20;
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt: crate::amt::AmtImpl<
            String,
            &fvm_ipld_blockstore::MemoryBlockstore,
            crate::root::version::V3,
        > = Amt::new_with_bit_width(&db, bit_width);
        let mut idx = match bit_width {
            0 => 0,
            _ => idx % u64::pow(bit_width as u64, (amt.height() + 1) - 1),
        };
        while idx > 0 {
            idx -= 1;
            amt.set(idx, "foo".to_owned() + &idx.to_string()).unwrap();
        }
        for item in amt.iter().enumerate() {
            assert_eq!(
                item.1.unwrap().to_owned(),
                "foo".to_owned() + &item.0.to_string()
            );
        }
    }
}

pub struct Iter<'a, V, BS> {
    current_links: Option<std::iter::Flatten<std::slice::Iter<'a, Option<Link<V>>>>>,
    current_nodes: Option<std::iter::Flatten<std::slice::Iter<'a, Option<V>>>>,
    stack: Vec<&'a Node<V>>,
    blockstore: BS,
    bit_width: u32,
}

impl<'a, V, BS> Iterator for Iter<'a, V, BS>
where
    BS: Blockstore,
    V: DeserializeOwned,
{
    type Item = anyhow::Result<&'a V>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(link) = self.current_links.as_mut().and_then(Iterator::next) {
                match link {
                    Link::Cid { cid, cache } => {
                        match cache.get_or_try_init(|| {
                            self.blockstore
                                .get_cbor::<CollapsedNode<V>>(cid)?
                                .ok_or_else(|| Error::CidNotFound(cid.to_string()))?
                                .expand(self.bit_width)
                                .map(Box::new)
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
                Some(Node::Link { links }) => {
                    // if there are children, expand the stack and continue
                    self.current_links = Some(links.iter().flatten());
                    continue;
                }
                Some(Node::Leaf { vals }) => {
                    self.current_nodes = Some(vals.iter().flatten());
                    continue;
                }
                // all done!
                None => return None,
            }
        }
    }
}
