// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use cid::Cid;
use fvm_ipld_blockstore::tracking::{BSStats, TrackingBlockstore};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::de::DeserializeOwned;
use fvm_ipld_encoding::strict_bytes::ByteBuf;
use fvm_ipld_encoding::CborStore;
#[cfg(feature = "identity")]
use fvm_ipld_hamt::Identity;
use fvm_ipld_hamt::{BytesKey, Config, Error, Hamt, Hash};
use multihash::Code;
use quickcheck::Arbitrary;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Serialize;

// Redeclaring max array size of Hamt to avoid exposing value
const BUCKET_SIZE: usize = 3;

/// Help reuse tests with different HAMT configurations.
#[derive(Default)]
struct HamtFactory {
    conf: Config,
}

impl HamtFactory {
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    fn new<BS, V, K>(&self, store: BS) -> Hamt<BS, V, K>
    where
        BS: Blockstore,
        K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        Hamt::new_with_config(store, self.conf.clone())
    }

    fn new_with_bit_width<BS, V, K>(&self, store: BS, bit_width: u32) -> Hamt<BS, V, K>
    where
        BS: Blockstore,
        K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let conf = Config {
            bit_width,
            ..self.conf
        };
        Hamt::new_with_config(store, conf)
    }

    fn load<BS, V, K>(&self, cid: &Cid, store: BS) -> Result<Hamt<BS, V, K>, Error>
    where
        BS: Blockstore,
        K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        Hamt::load_with_config(cid, store, self.conf.clone())
    }

    fn load_with_bit_width<BS, V, K>(
        &self,
        cid: &Cid,
        store: BS,
        bit_width: u32,
    ) -> Result<Hamt<BS, V, K>, Error>
    where
        BS: Blockstore,
        K: Hash + Eq + PartialOrd + Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let conf = Config {
            bit_width,
            ..self.conf
        };
        Hamt::load_with_config(cid, store, conf)
    }
}

/// Check hard-coded CIDs during testing.
struct CidChecker {
    checked: usize,
    cids: Option<Vec<&'static str>>,
}

impl CidChecker {
    pub fn new(cids: Vec<&'static str>) -> Self {
        Self {
            cids: Some(cids),
            checked: 0,
        }
    }

    pub fn empty() -> Self {
        Self {
            cids: None,
            checked: 0,
        }
    }

    pub fn check_next(&mut self, cid: Cid) {
        if let Some(cids) = &self.cids {
            assert_ne!(self.checked, cids.len());
            assert_eq!(cid.to_string().as_str(), cids[self.checked]);
            self.checked += 1;
        }
    }
}

impl Drop for CidChecker {
    fn drop(&mut self) {
        if let Some(cids) = &self.cids {
            assert_eq!(self.checked, cids.len())
        }
    }
}

fn test_basics(factory: HamtFactory) {
    let store = MemoryBlockstore::default();
    let mut hamt = factory.new(&store);
    hamt.set(1, "world".to_string()).unwrap();

    assert_eq!(hamt.get(&1).unwrap(), Some(&"world".to_string()));
    hamt.set(1, "world2".to_string()).unwrap();
    assert_eq!(hamt.get(&1).unwrap(), Some(&"world2".to_string()));
}

fn test_load(factory: HamtFactory) {
    let store = MemoryBlockstore::default();

    let mut hamt: Hamt<_, _, usize> = factory.new(&store);
    hamt.set(1, "world".to_string()).unwrap();

    assert_eq!(hamt.get(&1).unwrap(), Some(&"world".to_string()));
    hamt.set(1, "world2".to_string()).unwrap();
    assert_eq!(hamt.get(&1).unwrap(), Some(&"world2".to_string()));
    let c = hamt.flush().unwrap();

    let new_hamt = factory.load(&c, &store).unwrap();
    assert_eq!(hamt, new_hamt);

    // set value in the first one
    hamt.set(2, "stuff".to_string()).unwrap();

    // loading original hash should returnnot be equal now
    let new_hamt = factory.load(&c, &store).unwrap();
    assert_ne!(hamt, new_hamt);

    // loading new hash
    let c2 = hamt.flush().unwrap();
    let new_hamt = factory.load(&c2, &store).unwrap();
    assert_eq!(hamt, new_hamt);

    // loading from an empty store does not work
    let empty_store = MemoryBlockstore::default();
    assert!(factory
        .load::<_, usize, BytesKey>(&c2, &empty_store)
        .is_err());

    // storing the hamt should produce the same cid as storing the root
    let c3 = hamt.flush().unwrap();
    assert_eq!(c3, c2);
}

fn test_set_if_absent(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut hamt: Hamt<_, _> = factory.new(&store);
    assert!(hamt
        .set_if_absent(tstring("favorite-animal"), tstring("owl bear"))
        .unwrap());

    // Next two are negatively asserted, shouldn't change
    assert!(!hamt
        .set_if_absent(tstring("favorite-animal"), tstring("bright green bear"))
        .unwrap());
    assert!(!hamt
        .set_if_absent(tstring("favorite-animal"), tstring("owl bear"))
        .unwrap());

    let c = hamt.flush().unwrap();

    let mut h2 = factory.load(&c, &store).unwrap();
    // Reloading should still have same effect
    assert!(!h2
        .set_if_absent(tstring("favorite-animal"), tstring("bright green bear"))
        .unwrap());

    cids.check_next(c);

    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn set_with_no_effect_does_not_put(
    factory: HamtFactory,
    stats: Option<BSStats>,
    mut cids: CidChecker,
) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut begn: Hamt<_, _> = factory.new_with_bit_width(&store, 1);
    let entries = 2 * BUCKET_SIZE * 5;
    for i in 0..entries {
        begn.set(tstring(i), tstring("filler")).unwrap();
    }

    let c = begn.flush().unwrap();
    cids.check_next(c);

    begn.set(tstring("favorite-animal"), tstring("bright green bear"))
        .unwrap();
    let c2 = begn.flush().unwrap();
    cids.check_next(c2);
    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
    // This insert should not change value or affect reads or writes
    begn.set(tstring("favorite-animal"), tstring("bright green bear"))
        .unwrap();
    let c3 = begn.flush().unwrap();
    cids.check_next(c3);

    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn delete(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut hamt: Hamt<_, _> = factory.new(&store);
    hamt.set(tstring("foo"), tstring("cat dog bear")).unwrap();
    hamt.set(tstring("bar"), tstring("cat dog")).unwrap();
    hamt.set(tstring("baz"), tstring("cat")).unwrap();
    assert!(hamt.get(&tstring("foo")).unwrap().is_some());

    let c = hamt.flush().unwrap();
    cids.check_next(c);

    let mut h2: Hamt<_, BytesKey> = factory.load(&c, &store).unwrap();
    assert!(h2.get(&b"foo".to_vec()).unwrap().is_some());
    assert!(h2.delete(&b"foo".to_vec()).unwrap().is_some());
    assert_eq!(h2.get(&b"foo".to_vec()).unwrap(), None);

    let c2 = h2.flush().unwrap();
    cids.check_next(c2);
    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn delete_case(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut hamt: Hamt<_, _> = factory.new(&store);

    hamt.set([0].to_vec().into(), ByteBuf(b"Test data".as_ref().into()))
        .unwrap();

    let c = hamt.flush().unwrap();
    cids.check_next(c);

    let mut h2: Hamt<_, ByteBuf> = factory.load(&c, &store).unwrap();
    assert!(h2.delete(&[0].to_vec()).unwrap().is_some());
    assert_eq!(h2.get(&[0].to_vec()).unwrap(), None);

    let c2 = h2.flush().unwrap();
    cids.check_next(c2);
    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn reload_empty(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let hamt: Hamt<_, ()> = factory.new(&store);
    let c = store.put_cbor(&hamt, Code::Blake2b256).unwrap();

    let h2: Hamt<_, ()> = factory.load(&c, &store).unwrap();
    let c2 = store.put_cbor(&h2, Code::Blake2b256).unwrap();
    assert_eq!(c, c2);
    cids.check_next(c);
    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn set_delete_many(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    // Test vectors setup specifically for bit width of 5
    let mut hamt: Hamt<_, BytesKey> = factory.new_with_bit_width(&store, 5);

    for i in 0..200 {
        hamt.set(tstring(i), tstring(i)).unwrap();
    }

    let c1 = hamt.flush().unwrap();
    cids.check_next(c1);

    for i in 200..400 {
        hamt.set(tstring(i), tstring(i)).unwrap();
    }

    let cid_all = hamt.flush().unwrap();
    cids.check_next(cid_all);

    for i in 200..400 {
        assert!(hamt.delete(&tstring(i)).unwrap().is_some());
    }
    // Ensure first 200 keys still exist
    for i in 0..200 {
        assert_eq!(hamt.get(&tstring(i)).unwrap(), Some(&tstring(i)));
    }

    let cid_d = hamt.flush().unwrap();
    cids.check_next(cid_d);
    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn for_each(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut hamt: Hamt<_, BytesKey> = factory.new_with_bit_width(&store, 5);

    for i in 0..200 {
        hamt.set(tstring(i), tstring(i)).unwrap();
    }

    // Iterating through hamt with dirty caches.
    let mut count = 0;
    hamt.for_each(|k, v| {
        assert_eq!(k, v);
        count += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(count, 200);

    let c = hamt.flush().unwrap();
    cids.check_next(c);

    let mut hamt: Hamt<_, BytesKey> = factory.load_with_bit_width(&c, &store, 5).unwrap();

    // Iterating through hamt with no cache.
    let mut count = 0;
    hamt.for_each(|k, v| {
        assert_eq!(k, v);
        count += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(count, 200);

    // Iterating through hamt with cached nodes.
    let mut count = 0;
    hamt.for_each(|k, v| {
        assert_eq!(k, v);
        count += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(count, 200);

    let c = hamt.flush().unwrap();
    cids.check_next(c);

    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

fn for_each_ranged(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut hamt: Hamt<_, usize> = factory.new_with_bit_width(&store, 5);

    const RANGE: usize = 200;
    for i in 0..RANGE {
        hamt.set(tstring(i), i).unwrap();
    }

    // collect all KV paris by iterating through the entire hamt
    let mut kvs = Vec::new();
    hamt.for_each(|k, v| {
        assert_eq!(k, &tstring(v));
        kvs.push((k.clone(), *v));
        Ok(())
    })
    .unwrap();

    // Iterate through the array, requesting pages of different sizes
    for page_size in 0..RANGE {
        let mut kvs_variable_page = Vec::new();
        let (num_traversed, next_key) = hamt
            .for_each_ranged::<BytesKey, _>(None, Some(page_size), |k, v| {
                kvs_variable_page.push((k.clone(), *v));
                Ok(())
            })
            .unwrap();

        assert_eq!(num_traversed, page_size);
        assert_eq!(kvs_variable_page.len(), num_traversed);
        assert_eq!(next_key.unwrap(), kvs[page_size].0);

        // Items iterated over should match the ordering of for_each
        assert_eq!(kvs_variable_page, kvs[..page_size]);
    }

    // Iterate through the array, requesting more items than are remaining
    let (num_traversed, next_key) = hamt
        .for_each_ranged::<BytesKey, _>(None, Some(RANGE + 10), |_k, _v| Ok(()))
        .unwrap();
    assert_eq!(num_traversed, RANGE);
    assert_eq!(next_key, None);

    // Iterate through it again starting at a certain key
    for start_at in 0..RANGE as usize {
        let mut kvs_variable_start = Vec::new();
        let (num_traversed, next_key) = hamt
            .for_each_ranged(Some(&kvs[start_at].0), None, |k, v| {
                assert_eq!(k, &tstring(v));
                kvs_variable_start.push((k.clone(), *v));

                Ok(())
            })
            .unwrap();

        // No limit specified, iteration should be exhaustive
        assert_eq!(next_key, None);
        assert_eq!(num_traversed, kvs_variable_start.len());
        assert_eq!(kvs_variable_start.len(), kvs.len() - start_at,);

        // Items iterated over should match the ordering of for_each
        assert_eq!(kvs_variable_start, kvs[start_at..]);
    }

    // Chain paginated requests to iterate over entire HAMT
    {
        let mut kvs_paginated_requests = Vec::new();
        let mut iterations = 0;
        let mut cursor: Option<BytesKey> = None;

        // Request all items in pages of 20 items each
        const PAGE_SIZE: usize = 20;
        loop {
            let (page_size, next) = match cursor {
                Some(ref start) => hamt
                    .for_each_ranged::<BytesKey, _>(Some(start), Some(PAGE_SIZE), |k, v| {
                        kvs_paginated_requests.push((k.clone(), *v));
                        Ok(())
                    })
                    .unwrap(),
                None => hamt
                    .for_each_ranged::<BytesKey, _>(None, Some(PAGE_SIZE), |k, v| {
                        kvs_paginated_requests.push((k.clone(), *v));
                        Ok(())
                    })
                    .unwrap(),
            };
            iterations += 1;
            assert_eq!(page_size, PAGE_SIZE);
            assert_eq!(kvs_paginated_requests.len(), iterations * PAGE_SIZE);

            if next.is_none() {
                break;
            } else {
                assert_eq!(next.clone().unwrap(), kvs[(iterations * PAGE_SIZE)].0);
                cursor = next;
            }
        }

        // should have retrieved all key value pairs in the same order
        assert_eq!(kvs_paginated_requests.len(), kvs.len(), "{}", iterations);
        assert_eq!(kvs_paginated_requests, kvs);
        // should have used the expected number of iterations
        assert_eq!(iterations, RANGE / PAGE_SIZE);
    }

    let c = hamt.flush().unwrap();
    cids.check_next(c);

    // Chain paginated requests over a HAMT with committed nodes
    let mut hamt: Hamt<_, usize> = factory.load_with_bit_width(&c, &store, 5).unwrap();
    {
        let mut kvs_paginated_requests = Vec::new();
        let mut iterations = 0;
        let mut cursor: Option<BytesKey> = None;

        // Request all items in pages of 20 items each
        const PAGE_SIZE: usize = 20;
        loop {
            let (page_size, next) = match cursor {
                Some(ref start) => hamt
                    .for_each_ranged::<BytesKey, _>(Some(start), Some(PAGE_SIZE), |k, v| {
                        kvs_paginated_requests.push((k.clone(), *v));
                        Ok(())
                    })
                    .unwrap(),
                None => hamt
                    .for_each_ranged::<BytesKey, _>(None, Some(PAGE_SIZE), |k, v| {
                        kvs_paginated_requests.push((k.clone(), *v));
                        Ok(())
                    })
                    .unwrap(),
            };
            iterations += 1;
            assert_eq!(page_size, PAGE_SIZE);
            assert_eq!(kvs_paginated_requests.len(), iterations * PAGE_SIZE);

            if next.is_none() {
                break;
            } else {
                assert_eq!(next.clone().unwrap(), kvs[(iterations * PAGE_SIZE)].0);
                cursor = next;
            }
        }

        // should have retrieved all key value pairs in the same order
        assert_eq!(kvs_paginated_requests.len(), kvs.len(), "{}", iterations);
        assert_eq!(kvs_paginated_requests, kvs);
        // should have used the expected number of iterations
        assert_eq!(iterations, RANGE / PAGE_SIZE);
    }

    let c = hamt.flush().unwrap();
    cids.check_next(c);

    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

#[cfg(feature = "identity")]
fn add_and_remove_keys(
    bit_width: u32,
    keys: &[&[u8]],
    extra_keys: &[&[u8]],
    expected: &'static str,
    stats: BSStats,
) {
    let all: Vec<(BytesKey, BytesKey)> = keys
        .iter()
        .enumerate()
        // Value doesn't matter for this test, only checking cids against previous
        .map(|(i, k)| (k.to_vec().into(), tstring(i)))
        .collect();

    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut hamt: Hamt<_, _, _, Identity> = Hamt::new_with_bit_width(&store, bit_width);

    for (k, v) in all.iter() {
        hamt.set(k.clone(), v.clone()).unwrap();
    }
    let cid = hamt.flush().unwrap();

    let mut h1: Hamt<_, _, BytesKey, Identity> =
        Hamt::load_with_bit_width(&cid, &store, bit_width).unwrap();

    for (k, v) in all {
        assert_eq!(Some(&v), h1.get(&k).unwrap());
    }

    // Set and delete extra keys
    for k in extra_keys.iter() {
        hamt.set(k.to_vec().into(), tstring(0)).unwrap();
    }
    for k in extra_keys.iter() {
        hamt.delete(*k).unwrap();
    }
    let cid2 = hamt.flush().unwrap();
    let mut h2: Hamt<_, BytesKey, BytesKey, Identity> =
        Hamt::load_with_bit_width(&cid2, &store, bit_width).unwrap();

    let cid1 = h1.flush().unwrap();
    let cid2 = h2.flush().unwrap();
    assert_eq!(cid1, cid2);
    assert_eq!(cid1.to_string().as_str(), expected);
    assert_eq!(*store.stats.borrow(), stats);
}

#[test]
#[cfg(feature = "identity")]
fn canonical_structure() {
    // Champ mutation semantics test
    #[rustfmt::skip]
    add_and_remove_keys(
        8,
        &[b"K"],
        &[b"B"],
        "bafy2bzacecosy45hp4sz2t4o4flxvntnwjy7yaq43bykci22xycpeuj542lse",
        BSStats {r: 2, w: 2, br: 38, bw: 38},
    );

    #[rustfmt::skip]
    add_and_remove_keys(
        8,
        &[b"K0", b"K1", b"KAA1", b"KAA2", b"KAA3"],
        &[b"KAA4"],
        "bafy2bzaceaqdaj5aqkwugr7wx4to3fahynoqlxuo5j6xznly3khazgyxihkbo",
        BSStats {r:3, w:4, br:163, bw:214},
    );
}

#[test]
#[cfg(feature = "identity")]
fn canonical_structure_alt_bit_width() {
    let kb_cases = [
        "bafy2bzacec3cquclaqkb32cntwtizgij55b7isb4s5hv5hv5ujbbeu6clxkug",
        "bafy2bzacebj7b2jahw7nxmu6mlhkwzucjmfq7aqlj52jusqtufqtaxcma4pdm",
        "bafy2bzacedrwwndijql6lmmtyicjwyehxtgey5fhzocc43hrzhetrz25v2k2y",
    ];

    let other_cases = [
        "bafy2bzacedbiipe7l7gbtjandyyl6rqlkuqr2im2nl7d4bljidv5mta22rjqk",
        "bafy2bzaceb3c76qlbsiv3baogpao3zah56eqonsowpkof33o5hmncfow4seso",
        "bafy2bzacebhkyrwfexokaoygsx2crydq3fosiyfoa5bthphntmicsco2xf442",
    ];

    #[rustfmt::skip]
    let kb_stats = [
        BSStats {r: 2, w: 2, br: 22, bw: 22},
        BSStats {r: 2, w: 2, br: 24, bw: 24},
        BSStats {r: 2, w: 2, br: 28, bw: 28},
    ];

    #[rustfmt::skip]
    let other_stats = [
        BSStats {r: 3, w: 4, br: 139, bw: 182},
        BSStats {r: 3, w: 4, br: 146, bw: 194},
        BSStats {r: 3, w: 4, br: 154, bw: 206},
    ];

    for i in 5..8 {
        #[rustfmt::skip]
        add_and_remove_keys(
            i,
            &[b"K"],
            &[b"B"],
            kb_cases[(i - 5) as usize],
            kb_stats[(i - 5) as usize],
        );
        #[rustfmt::skip]
        add_and_remove_keys(
            i,
            &[b"K0", b"K1", b"KAA1", b"KAA2", b"KAA3"],
            &[b"KAA4"],
            other_cases[(i - 5) as usize],
            other_stats[(i - 5) as usize],
        );
    }
}

fn clean_child_ordering(factory: HamtFactory, stats: Option<BSStats>, mut cids: CidChecker) {
    let make_key = |i: u64| -> BytesKey {
        let mut key = unsigned_varint::encode::u64_buffer();
        let n = unsigned_varint::encode::u64(i, &mut key);
        n.to_vec().into()
    };

    let dummy_value: u8 = 42;

    let mem = MemoryBlockstore::default();
    let store = TrackingBlockstore::new(&mem);

    let mut h: Hamt<_, _> = factory.new_with_bit_width(&store, 5);

    for i in 100..195 {
        h.set(make_key(i), dummy_value).unwrap();
    }

    let root = h.flush().unwrap();
    cids.check_next(root);
    let mut h: Hamt<_, u8> = factory.load_with_bit_width(&root, &store, 5).unwrap();

    h.delete(&make_key(104)).unwrap();
    h.delete(&make_key(108)).unwrap();
    let root = h.flush().unwrap();
    let _: Hamt<_, u8> = factory.load_with_bit_width(&root, &store, 5).unwrap();

    cids.check_next(root);

    if let Some(stats) = stats {
        assert_eq!(*store.stats.borrow(), stats);
    }
}

/// Test that a HAMT produced by `factory1` has a larger root size than one produced by `factory2`
/// after inserting the same data into both versions.
fn test_reduced_root_size(factory1: HamtFactory, factory2: HamtFactory) {
    let mem = MemoryBlockstore::default();
    let mut hamt1 = factory1.new(&mem);
    let mut hamt2 = factory2.new(&mem);

    for i in 0..100 {
        hamt1.set(i, vec![1u8; 1000]).unwrap();
        hamt2.set(i, vec![1u8; 1000]).unwrap();
    }
    let c1 = hamt1.flush().unwrap();
    let c2 = hamt2.flush().unwrap();

    assert!(c1 != c2);

    let bytes_read_during_load = |c, f: &HamtFactory| {
        let store = TrackingBlockstore::new(&mem);
        let _: Hamt<_, Vec<u8>, i32> = f.load(c, &store).unwrap();
        let stats = store.stats.borrow();
        stats.br
    };

    let br1 = bytes_read_during_load(&c1, &factory1);
    let br2 = bytes_read_during_load(&c2, &factory2);

    assert!(br2 < br1);
}

#[test]
fn min_data_depth_reduces_root_size() {
    let mk_factory = |min_data_depth| HamtFactory {
        conf: Config {
            min_data_depth,
            ..Default::default()
        },
    };

    let factory1 = mk_factory(0);
    let factory2 = mk_factory(1);

    test_reduced_root_size(factory1, factory2);
}

#[test]
fn max_array_width_reduces_root_size() {
    let mk_factory = |max_array_width| HamtFactory {
        conf: Config {
            max_array_width,
            ..Default::default()
        },
    };

    let factory1 = mk_factory(3);
    let factory2 = mk_factory(1);

    test_reduced_root_size(factory1, factory2);
}

/// List of key value pairs with unique keys.
///
/// Uniqueness is used so insert order doesn't cause overwrites.
/// Not using a `HashMap` so the iteration order is deterministic.
#[derive(Clone, Debug)]
struct UniqueKeyValuePairs<K, V>(Vec<(K, V)>);

impl<K, V> Arbitrary for UniqueKeyValuePairs<K, V>
where
    K: Arbitrary + Eq + std::hash::Hash,
    V: Arbitrary,
{
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        let kvs: Vec<(K, V)> = Arbitrary::arbitrary(g);
        let (kvs, _) =
            kvs.into_iter()
                .fold((Vec::new(), HashSet::new()), |(mut kvs, mut ks), (k, v)| {
                    if !ks.contains(&k) {
                        ks.insert(k.clone());
                        kvs.push((k, v));
                    }
                    (kvs, ks)
                });
        Self(kvs)
    }
}

/// Test that insertion order doesn't matter, the resulting HAMT has the same CID.
fn prop_cid_indep_of_insert_order(
    factory: HamtFactory,
    kvs: UniqueKeyValuePairs<u8, i64>,
    seed: u64,
) -> bool {
    let store = MemoryBlockstore::default();
    let kvs1 = kvs.0;

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut kvs2 = kvs1.clone();
    kvs2.shuffle(&mut rng);

    let mut hamt1 = factory.new(&store);
    let mut hamt2 = factory.new(&store);

    for (k, v) in kvs1 {
        hamt1.set(k, v).unwrap();
    }
    for (k, v) in kvs2 {
        hamt2.set(k, v).unwrap();
    }

    let cid1 = hamt1.flush().unwrap();
    let cid2 = hamt2.flush().unwrap();

    cid1 == cid2
}

#[derive(Clone, Debug)]
enum Operation<K, V> {
    Set((K, V)),
    Delete(K),
}

impl<K, V> Arbitrary for Operation<K, V>
where
    K: Arbitrary + Eq + std::hash::Hash,
    V: Arbitrary,
{
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        match bool::arbitrary(g) {
            false => Operation::Delete(K::arbitrary(g)),
            true => Operation::Set((K::arbitrary(g), V::arbitrary(g))),
        }
    }
}

/// A numeric key with a maximum value.
#[derive(Clone, Debug, PartialEq, Eq, std::hash::Hash)]
struct LimitedU32<const L: u32>(u32);

impl<const L: u32> Arbitrary for LimitedU32<L> {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        Self(u32::arbitrary(g) % L)
    }
}

/// Operations with a limited key range, to induce lots of overlaps in sets and deletes.
type LimitedKeyOps<const N: u32> = Vec<Operation<LimitedU32<N>, i32>>;

/// Test that randomly inserting, updating and deleting random elements is equivalent to just doing the reduced insertions.
fn prop_cid_ops_reduced<const N: u32>(factory: HamtFactory, ops: LimitedKeyOps<N>) -> bool {
    let store = MemoryBlockstore::default();

    let reduced = ops.iter().fold(HashMap::new(), |mut m, op| {
        match op {
            Operation::Set((k, v)) => m.insert(k.0, *v),
            Operation::Delete(k) => m.remove(&k.0),
        };
        m
    });

    let mut hamt1 = ops.into_iter().fold(factory.new(&store), |mut hamt, op| {
        match op {
            Operation::Set((k, v)) => {
                hamt.set(k.0, v).unwrap();
            }
            Operation::Delete(k) => {
                hamt.delete(&k.0).unwrap();
            }
        };
        hamt
    });

    let mut hamt2 = reduced
        .into_iter()
        .fold(factory.new(&store), |mut hamt, (k, v)| {
            hamt.set(k, v).unwrap();
            hamt
        });

    let cid1 = hamt1.flush().unwrap();
    let cid2 = hamt2.flush().unwrap();

    cid1 == cid2
}

fn tstring(v: impl Display) -> BytesKey {
    BytesKey(v.to_string().into_bytes())
}

mod test_default {
    use fvm_ipld_blockstore::tracking::BSStats;
    use quickcheck_macros::quickcheck;

    use crate::{CidChecker, HamtFactory, LimitedKeyOps, UniqueKeyValuePairs};

    #[test]
    fn test_basics() {
        super::test_basics(HamtFactory::default())
    }

    #[test]
    fn test_load() {
        super::test_load(HamtFactory::default())
    }

    #[test]
    fn test_set_if_absent() {
        #[rustfmt::skip]
        let stats = BSStats {r: 1, w: 1, br: 63, bw: 63};
        let cids = CidChecker::new(vec![
            "bafy2bzaced2tgnlsq4n2ioe6ldy75fw3vlrrkyfv4bq6didbwoob2552zvpuk",
        ]);
        super::test_set_if_absent(HamtFactory::default(), Some(stats), cids)
    }

    #[test]
    fn set_with_no_effect_does_not_put() {
        #[rustfmt::skip]
        let stats = BSStats {r:0, w:18, br:0, bw:1282};
        let cids = CidChecker::new(vec![
            "bafy2bzacebjilcrsqa4uyxuh36gllup4rlgnvwgeywdm5yqq2ks4jrsj756qq",
            "bafy2bzacea7biyabzk7v7le2rrlec5tesjbdnymh5sk4lfprxibg4rtudwtku",
            "bafy2bzacea7biyabzk7v7le2rrlec5tesjbdnymh5sk4lfprxibg4rtudwtku",
        ]);
        super::set_with_no_effect_does_not_put(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn delete() {
        #[rustfmt::skip]
        let stats = BSStats {r:1, w:2, br:79, bw:139};
        let cids = CidChecker::new(vec![
            "bafy2bzacebql36crv4odvxzstx2ubaczmawy2tlljxezvorcsoqeyyojxkrom",
            "bafy2bzaced7up7wkm7cirieh5bs4iyula5inrprihmjzozmku3ywvekzzmlyi",
        ]);
        super::delete(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn delete_case() {
        #[rustfmt::skip]
        let stats = BSStats {r: 1, w: 2, br: 31, bw: 34};
        let cids = CidChecker::new(vec![
            "bafy2bzaceb2hikcc6tfuuuuehjstbiq356oruwx6ejyse77zupq445unranv6",
            "bafy2bzaceamp42wmmgr2g2ymg46euououzfyck7szknvfacqscohrvaikwfay",
        ]);
        super::delete_case(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn reload_empty() {
        #[rustfmt::skip]
        let stats = BSStats {r: 1, w: 2, br: 3, bw: 6};
        let cids = CidChecker::new(vec![
            "bafy2bzaceamp42wmmgr2g2ymg46euououzfyck7szknvfacqscohrvaikwfay",
        ]);
        super::reload_empty(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn set_delete_many() {
        #[rustfmt::skip]
        let stats = BSStats {r: 0, w: 93, br: 0, bw: 11734};
        let cids = CidChecker::new(vec![
            "bafy2bzaceczhz54xmmz3xqnbmvxfbaty3qprr6dq7xh5vzwqbirlsnbd36z7a",
            "bafy2bzacecxcp736xkl2mcyjlors3tug6vdlbispbzxvb75xlrhthiw2xwxvw",
            "bafy2bzaceczhz54xmmz3xqnbmvxfbaty3qprr6dq7xh5vzwqbirlsnbd36z7a",
        ]);
        super::set_delete_many(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn for_each() {
        #[rustfmt::skip]
        let stats = BSStats {r: 30, w: 30, br: 3209, bw: 3209};
        let cids = CidChecker::new(vec![
            "bafy2bzaceczhz54xmmz3xqnbmvxfbaty3qprr6dq7xh5vzwqbirlsnbd36z7a",
            "bafy2bzaceczhz54xmmz3xqnbmvxfbaty3qprr6dq7xh5vzwqbirlsnbd36z7a",
        ]);
        super::for_each(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn for_each_ranged() {
        #[rustfmt::skip]
        let stats = BSStats {r: 30, w: 30, br: 2895, bw: 2895};
        let cids = CidChecker::new(vec![
            "bafy2bzacedy4ypl2vedhdqep3llnwko6vrtfiys5flciz2f3c55pl4whlhlqm",
            "bafy2bzacedy4ypl2vedhdqep3llnwko6vrtfiys5flciz2f3c55pl4whlhlqm",
        ]);
        super::for_each_ranged(HamtFactory::default(), Some(stats), cids);
    }

    #[test]
    fn clean_child_ordering() {
        #[rustfmt::skip]
        let stats = BSStats {r: 3, w: 11, br: 1449, bw: 1751};
        let cids = CidChecker::new(vec![
            "bafy2bzacebqox3gtng4ytexyacr6zmaliyins3llnhbnfbcrqmhzuhmuuawqk",
            "bafy2bzacedlyeuub3mo4aweqs7zyxrbldsq2u4a2taswubudgupglu2j4eru6",
        ]);
        super::clean_child_ordering(HamtFactory::default(), Some(stats), cids);
    }

    #[quickcheck]
    fn prop_cid_indep_of_insert_order(kvs: UniqueKeyValuePairs<u8, i64>, seed: u64) -> bool {
        super::prop_cid_indep_of_insert_order(HamtFactory::default(), kvs, seed)
    }

    #[quickcheck]
    fn prop_cid_ops_reduced(ops: LimitedKeyOps<10>) -> bool {
        super::prop_cid_ops_reduced(HamtFactory::default(), ops)
    }
}

/// Run all the tests with a different configuration.
///
/// For example:
/// ```text
/// test_hamt_mod!(test_extension, || {
///   HamtFactory {
///       conf: Config {
///           use_extensions: true,
///           bit_width: 2,
///           min_data_depth: 1,
///           data_in_leaves_only: false,
///       },
///   }
/// });
/// ```
#[macro_export]
macro_rules! test_hamt_mod {
    ($name:ident, $factory:expr) => {
        mod $name {
            use fvm_ipld_hamt::Config;
            use quickcheck_macros::quickcheck;
            use $crate::{CidChecker, HamtFactory, LimitedKeyOps, UniqueKeyValuePairs};

            #[test]
            fn test_basics() {
                super::test_basics($factory)
            }

            #[test]
            fn test_load() {
                super::test_load($factory)
            }

            #[test]
            fn test_set_if_absent() {
                super::test_set_if_absent($factory, None, CidChecker::empty())
            }

            #[test]
            fn set_with_no_effect_does_not_put() {
                super::set_with_no_effect_does_not_put($factory, None, CidChecker::empty())
            }

            #[test]
            fn delete() {
                super::delete($factory, None, CidChecker::empty())
            }

            #[test]
            fn delete_case() {
                super::delete_case($factory, None, CidChecker::empty())
            }

            #[test]
            fn reload_empty() {
                super::reload_empty($factory, None, CidChecker::empty())
            }

            #[test]
            fn set_delete_many() {
                super::set_delete_many($factory, None, CidChecker::empty())
            }

            #[test]
            fn for_each() {
                super::for_each($factory, None, CidChecker::empty())
            }

            #[test]
            fn clean_child_ordering() {
                super::clean_child_ordering($factory, None, CidChecker::empty())
            }

            #[quickcheck]
            fn prop_cid_indep_of_insert_order(
                kvs: UniqueKeyValuePairs<u8, i64>,
                seed: u64,
            ) -> bool {
                super::prop_cid_indep_of_insert_order($factory, kvs, seed)
            }

            #[quickcheck]
            fn prop_cid_ops_reduced(ops: LimitedKeyOps<10>) -> bool {
                super::prop_cid_ops_reduced($factory, ops)
            }
        }
    };
}

test_hamt_mod!(
    test_binary_tree,
    HamtFactory {
        conf: Config {
            bit_width: 1,
            min_data_depth: 0,
            max_array_width: 3
        },
    }
);

test_hamt_mod!(
    test_min_data_depth,
    HamtFactory {
        conf: Config {
            bit_width: 4,
            min_data_depth: 2,
            max_array_width: 1
        },
    }
);
