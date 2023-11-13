// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::node::CollapsedNode;
use crate::node::{Link, Node};
use crate::AmtImpl;
use crate::Error;
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
    pub fn iter(&self) -> Iter<'_, V, &BS, Ver> {
        Iter {
            stack: vec![IterStack {
                node: Some(&self.root.node),
                idx: 0,
            }],
            blockstore: &self.block_store,
            bit_width: self.bit_width(),
            ver: PhantomData,
            key: 0,
        }
    }

    pub fn values(&self) -> impl Iterator<Item = Result<&V, Error>> {
        self.iter().map(|res| res.map(|(_, v)| v))
    }

    pub fn keys(&self) -> impl Iterator<Item = Result<usize, Error>> + '_ {
        self.iter().map(|res| res.map(|(k, _)| k))
    }

    pub fn iter_from(&self, key: usize) -> Vec<Result<(usize, &V), crate::Error>> {
        let mut iter = self.iter();
        let mut results = Vec::new();
        while let Some(res) = iter.next() {
            let (k, v) = res.expect("Failed to generate iterator from AMT");
            if k >= key {
                results.push(Ok((k, v)));
            }
        }
        results
    }
}

impl<'a, V, BS, Ver> IntoIterator for &'a crate::AmtImpl<V, BS, Ver>
where
    V: Serialize + DeserializeOwned,
    Ver: crate::root::version::Version,
    BS: Blockstore,
{
    type IntoIter = Iter<'a, V, &'a BS, Ver>;
    type Item = Result<(usize, &'a V), crate::Error>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<V, BS, Ver> AmtImpl<V, BS, Ver>
where
    V: DeserializeOwned + Serialize,
    BS: Blockstore,
    Ver: crate::root::version::Version,
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
    /// let mut values: Vec<(usize, String)> = Vec::new();
    /// map.for_each(|i, v| {
    ///    values.push((*i, v.clone()));
    ///    Ok(())
    /// }).unwrap();
    /// assert_eq!(&values, &[(1, "One".to_owned()), (4, "Four".to_owned())]);
    /// ```
    #[inline]
    #[deprecated = "use `.iter()` instead"]
    pub fn for_each<F>(&self, mut f: F) -> Result<(), Error>
    where
        F: FnMut(&usize, &V) -> anyhow::Result<()>,
    {
        for res in self {
            let (k, v) = res?;
            (f)(&k, v)?;
        }
        Ok(())
    }
}

pub struct Iter<'a, V, BS, Ver> {
    stack: Vec<IterStack<'a, V>>,
    blockstore: BS,
    bit_width: u32,
    ver: PhantomData<Ver>,
    key: usize,
}

pub struct IterStack<'a, V> {
    pub(crate) node: Option<&'a Node<V>>,
    pub(crate) idx: usize,
}

impl<'a, V, BS, Ver> Iterator for Iter<'a, V, BS, Ver>
where
    BS: Blockstore,
    V: Serialize + DeserializeOwned,
{
    type Item = Result<(usize, &'a V), crate::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let stack = self.stack.last_mut()?;
            match stack.node {
                Some(Node::Leaf { vals }) => {
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
                Some(Node::Link { links }) => {
                    if stack.idx < links.len() {
                        let link = &links[stack.idx];
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
                                        stack.idx += 1;
                                        self.stack.push(IterStack {
                                            node: Some(node.as_ref()),
                                            idx: 0,
                                        });
                                    }
                                    Err(e) => return Some(Err(e)),
                                }
                            }
                            Some(Link::Dirty(node)) => {
                                stack.idx += 1;
                                self.stack.push(IterStack {
                                    node: Some(node.as_ref()),
                                    idx: 0,
                                });
                            }
                            None => {
                                stack.idx += 1;
                                self.key += 2_usize.pow(self.bit_width);
                            }
                        };
                    } else {
                        self.stack.pop();
                    }
                }
                None => return None,
            }
        }
    }
}

// TODO(aatifsyed): is this guaranteed to be acyclic?
#[cfg(test)]
mod tests {
    use crate::Amt;
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
        assert!(amt.values().next().is_none());
        assert!(amt.keys().next().is_none());
    }

    #[test]
    fn check_iter_next_single_element() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(0, "foo".to_owned()).unwrap();
        assert_eq!(amt.values().next().unwrap().unwrap(), "foo");
        assert_eq!(amt.keys().next().unwrap().unwrap(), 0);
    }

    #[test]
    fn check_iter_next_with_none() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 1);
        amt.set(1, "foo".to_owned()).unwrap();
        assert_eq!(amt.values().next().unwrap().unwrap(), "foo");
        assert_eq!(amt.keys().next().unwrap().unwrap(), 1);
    }

    #[test]
    fn check_iter_next_with_two_sets() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new_with_bit_width(&db, 2);
        amt.set(1, "foo".to_owned()).unwrap();
        amt.set(2, "bar".to_owned()).unwrap();
        let mut amt_values = amt.values();
        let mut amt_keys = amt.keys();
        assert_eq!(amt_values.next().unwrap().unwrap(), "foo");
        assert_eq!(amt_values.next().unwrap().unwrap(), "bar");
        assert_eq!(amt_keys.next().unwrap().unwrap(), 1);
        assert_eq!(amt_keys.next().unwrap().unwrap(), 2);
    }

    #[test]
    fn check_iter_next_with_link() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new(&db);
        amt.set(8, "foo".to_owned()).unwrap();
        dbg!(&amt);
        assert_eq!(amt.values().next().unwrap().unwrap(), "foo");
        assert_eq!(amt.keys().next().unwrap().unwrap(), 8);
    }

    #[test]
    fn check_iter_next_with_links() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::default();
        let mut amt = Amt::new(&db);
        amt.set(8, "foo".to_owned()).unwrap();
        amt.set(64, "bar".to_owned()).unwrap();
        let mut amt_iter = amt.iter();
        assert_eq!(
            amt_iter.next().unwrap().unwrap(),
            (8_usize, &"foo".to_owned())
        );
        assert_eq!(
            amt_iter.next().unwrap().unwrap(),
            (64_usize, &"bar".to_owned())
        );
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
            restored.push((*k, v.clone()));
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
                if *k as u64 != indexes[x] {
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
            (idx as usize, &"foo".to_owned())
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
        for item in amt.values().enumerate() {
            assert_eq!(item.1.unwrap(), &("foo".to_owned() + &item.0.to_string()));
        }
    }

    #[test]
    fn from_iter() {
        use crate::Amt;
        use fvm_ipld_blockstore::MemoryBlockstore;
    
        let store = MemoryBlockstore::default();
        
        // Create an AMT with 5 keys.
        let mut amt = Amt::new(store);
        let kvs: Vec<usize> = (0..=5).collect();
        let _ = kvs.iter().map(|k| amt.set(u64::try_from(*k).unwrap(), k.to_string())).collect::<Vec<_>>();
        let kvs = kvs.iter().map(|k| (k.clone(), k.to_string())).collect::<Vec<_>>();
        
    
        // Read 2 elements.
        let mut results = amt.iter().take(2).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(results.len(), 2);
    
        dbg!(results.last().unwrap().0);
        // Read the rest.
        for res in amt.iter_from(results.last().unwrap().0 + 1) {
            results.push(res.unwrap());
        }
        
        // Assert that we got out what we put in.
        let results: Vec<_> = results.into_iter().map(|(k, v)|(k.clone(), v.clone())).collect();
        assert_eq!(kvs, results);
    }
}
