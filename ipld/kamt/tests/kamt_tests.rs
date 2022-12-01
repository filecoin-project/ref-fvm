// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use cid::Cid;
use forest_hash_utils::BytesKey;
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::de::DeserializeOwned;
use fvm_ipld_encoding::CborStore;
use fvm_ipld_kamt::id::Identity;
use fvm_ipld_kamt::{Config, Error, HashedKey, Kamt};
use multihash::Code;
use quickcheck::Arbitrary;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Serialize;

type HKamt<BS, V, K = u32> = Kamt<BS, K, V, Identity, 32>;

/// Help reuse tests with different KAMT configurations.
#[derive(Default)]
struct KamtFactory {
    conf: Config,
}

impl KamtFactory {
    #[allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
    fn new<BS, K, V>(&self, store: BS) -> HKamt<BS, V, K>
    where
        BS: Blockstore,
        V: Serialize + DeserializeOwned,
        K: Serialize + DeserializeOwned,
    {
        Kamt::new_with_config(store, self.conf.clone())
    }

    fn load<BS, K, V>(&self, cid: &Cid, store: BS) -> Result<HKamt<BS, V, K>, Error>
    where
        BS: Blockstore,
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        Kamt::load_with_config(cid, store, self.conf.clone())
    }
}

fn test_basics(factory: KamtFactory) {
    let store = MemoryBlockstore::default();
    let mut kamt: HKamt<_, _> = factory.new(&store);
    kamt.set(1, "world".to_string()).unwrap();

    assert_eq!(kamt.get(&1).unwrap(), Some(&"world".to_string()));
    kamt.set(1, "world2".to_string()).unwrap();
    assert_eq!(kamt.get(&1).unwrap(), Some(&"world2".to_string()));
}

fn test_load(factory: KamtFactory) {
    let store = MemoryBlockstore::default();

    let mut kamt: HKamt<_, _> = factory.new(&store);
    kamt.set(1, "world".to_string()).unwrap();

    assert_eq!(kamt.get(&1).unwrap(), Some(&"world".to_string()));
    kamt.set(1, "world2".to_string()).unwrap();
    assert_eq!(kamt.get(&1).unwrap(), Some(&"world2".to_string()));
    let c = kamt.flush().unwrap();

    let new_kamt = factory.load(&c, &store).unwrap();
    assert_eq!(kamt, new_kamt);

    // set value in the first one
    kamt.set(2, "stuff".to_string()).unwrap();

    // loading original hash should returnnot be equal now
    let new_kamt = factory.load(&c, &store).unwrap();
    assert_ne!(kamt, new_kamt);

    // loading new hash
    let c2 = kamt.flush().unwrap();
    let new_kamt = factory.load(&c2, &store).unwrap();
    assert_eq!(kamt, new_kamt);

    // loading from an empty store does not work
    let empty_store = MemoryBlockstore::default();
    assert!(factory.load::<_, u32, BytesKey>(&c2, &empty_store).is_err());

    // storing the kamt should produce the same cid as storing the root
    let c3 = kamt.flush().unwrap();
    assert_eq!(c3, c2);
}

fn test_set_if_absent(factory: KamtFactory) {
    let store = MemoryBlockstore::default();

    let mut kamt: HKamt<_, _, HashedKey<32>> = factory.new(&store);
    assert!(kamt
        .set_if_absent(kstring("favorite-animal"), tstring("owl bear"))
        .unwrap());

    // Next two are negatively asserted, shouldn't change
    assert!(!kamt
        .set_if_absent(kstring("favorite-animal"), tstring("bright green bear"))
        .unwrap());
    assert!(!kamt
        .set_if_absent(kstring("favorite-animal"), tstring("owl bear"))
        .unwrap());

    let c = kamt.flush().unwrap();

    let mut h2: HKamt<_, _, HashedKey<32>> = factory.load(&c, &store).unwrap();
    // Reloading should still have same effect
    assert!(!h2
        .set_if_absent(kstring("favorite-animal"), tstring("bright green bear"))
        .unwrap());
}

fn reload_empty(factory: KamtFactory) {
    let store = MemoryBlockstore::default();

    let kamt: HKamt<_, ()> = factory.new(&store);
    let c1 = store.put_cbor(&kamt, Code::Blake2b256).unwrap();

    let h2: HKamt<_, ()> = factory.load(&c1, &store).unwrap();
    let c2 = store.put_cbor(&h2, Code::Blake2b256).unwrap();
    assert_eq!(c1, c2);
}

fn for_each(factory: KamtFactory) {
    let store = MemoryBlockstore::default();

    let mut kamt: HKamt<_, i32, u16> = factory.new(&store);

    for i in 0..200 {
        kamt.set(i, i as i32).unwrap();
    }

    // Iterating through kamt with dirty caches.
    let mut sum = 0;
    let expected_sum = 199 * 200 / 2;
    kamt.for_each(|k, v| {
        assert_eq!(*k as i32, *v);
        sum += v;
        Ok(())
    })
    .unwrap();
    assert_eq!(sum, expected_sum);

    let c = kamt.flush().unwrap();

    let kamt: HKamt<_, i32, u16> = factory.load(&c, &store).unwrap();

    // Iterating through kamt with no cache.
    let mut sum = 0;
    kamt.for_each(|_, v| {
        sum += v;
        Ok(())
    })
    .unwrap();
    assert_eq!(sum, expected_sum);

    // Iterating through kamt with cached nodes.
    let mut sum = 0;
    kamt.for_each(|_, v| {
        sum += v;
        Ok(())
    })
    .unwrap();
    assert_eq!(sum, expected_sum);
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

/// Test that insertion order doesn't matter, the resulting KAMT has the same CID.
fn prop_cid_indep_of_insert_order(
    factory: KamtFactory,
    kvs: UniqueKeyValuePairs<u8, i64>,
    seed: u64,
) -> bool {
    let store = MemoryBlockstore::default();
    let kvs1 = kvs.0;

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut kvs2 = kvs1.clone();
    kvs2.shuffle(&mut rng);

    let mut kamt1: HKamt<_, _, u8> = factory.new(&store);
    let mut kamt2: HKamt<_, _, u8> = factory.new(&store);

    for (k, v) in kvs1 {
        kamt1.set(k, v).unwrap();
    }
    for (k, v) in kvs2 {
        kamt2.set(k, v).unwrap();
    }

    let cid1 = kamt1.flush().unwrap();
    let cid2 = kamt2.flush().unwrap();

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
fn prop_cid_ops_reduced<const N: u32>(factory: KamtFactory, ops: LimitedKeyOps<N>) -> bool {
    let store = MemoryBlockstore::default();

    let reduced = ops.iter().fold(HashMap::new(), |mut m, op| {
        match op {
            Operation::Set((k, v)) => m.insert(k.0, *v),
            Operation::Delete(k) => m.remove(&k.0),
        };
        m
    });

    let mut kamt1: HKamt<_, _, u32> = ops.into_iter().fold(factory.new(&store), |mut kamt, op| {
        match op {
            Operation::Set((k, v)) => {
                kamt.set(k.0, v).unwrap();
            }
            Operation::Delete(k) => {
                kamt.delete(&k.0).unwrap();
            }
        };
        kamt
    });

    let mut kamt2: HKamt<_, _, u32> =
        reduced
            .into_iter()
            .fold(factory.new(&store), |mut kamt, (k, v)| {
                kamt.set(k, v).unwrap();
                kamt
            });

    let cid1 = kamt1.flush().unwrap();
    let cid2 = kamt2.flush().unwrap();

    cid1 == cid2
}

fn tstring(v: impl Display) -> BytesKey {
    BytesKey(v.to_string().into_bytes())
}

fn kstring(v: impl Display) -> HashedKey<32> {
    let mut k = [0; 32];
    let bs = v.to_string().into_bytes();
    assert!(bs.len() <= 32);
    for (i, b) in bs.into_iter().rev().enumerate() {
        k[31 - i] = b;
    }
    k
}

/// Run all the tests with a different configuration.
///
/// For example:
/// ```text
/// test_kamt_mod!(test_extension, || {
///   KamtFactory {
///       conf: Config {
///           use_extensions: true,
///           bit_width: 2,
///           min_data_depth: 1,
///       },
///   }
/// });
/// ```
#[macro_export]
macro_rules! test_kamt_mod {
    ($name:ident, $factory:expr) => {
        mod $name {
            use fvm_ipld_kamt::Config;
            use quickcheck_macros::quickcheck;
            use $crate::{KamtFactory, LimitedKeyOps, UniqueKeyValuePairs};

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
                super::test_set_if_absent($factory)
            }

            #[test]
            fn reload_empty() {
                super::reload_empty($factory)
            }

            #[test]
            fn for_each() {
                super::for_each($factory)
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

test_kamt_mod!(
    test_extension,
    KamtFactory {
        conf: Config {
            bit_width: 1, // Use smaller bit width to induce more overlap in key prefixes
            min_data_depth: 0,
            ..Default::default()
        },
    }
);

test_kamt_mod!(
    test_min_data_depth,
    KamtFactory {
        conf: Config {
            bit_width: 4,
            min_data_depth: 1,
            ..Default::default()
        },
    }
);

test_kamt_mod!(
    test_max_array_width,
    KamtFactory {
        conf: Config {
            max_array_width: 0, // Just to make sure a seemingly silly config like this doesn't cause a problem.
            bit_width: 2,
            ..Default::default()
        },
    }
);
