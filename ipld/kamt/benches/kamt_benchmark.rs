// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

extern crate serde;

use std::borrow::Cow;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_kamt::{AsHashedKey, Config, HashedKey, Kamt};

const ITEM_COUNT: u8 = 40;

const TEST_CONFIG: Config = Config {
    bit_width: 5,
    min_data_depth: 0,
    max_array_width: 3,
};

// Struct to simulate a reasonable amount of data per value into the amt
#[derive(Clone, Serialize_tuple, Deserialize_tuple, PartialEq)]
struct BenchData {
    v1: Vec<u8>,
    v2: Vec<u8>,
    v3: Vec<u8>,
    v: u64,
    a: [u8; 32],
    a2: [u8; 32],
}

impl BenchData {
    fn new(val: u8) -> Self {
        Self {
            v1: vec![val; 8],
            v2: vec![val; 20],
            v3: vec![val; 10],
            v: 8,
            a: [val; 32],
            a2: [val; 32],
        }
    }
}

struct VecKey;

impl AsHashedKey<Vec<u8>, 32> for VecKey {
    fn as_hashed_key(key: &Vec<u8>) -> Cow<HashedKey<32>> {
        assert!(key.len() <= 32);
        let mut bytes = [0; 32];
        for (i, b) in key.iter().enumerate() {
            bytes[i] = *b;
        }
        Cow::Owned(bytes)
    }
}

type BKamt<'a> = Kamt<&'a MemoryBlockstore, Vec<u8>, BenchData, VecKey>;

fn insert(c: &mut Criterion) {
    c.bench_function("KAMT bulk insert (no flush)", |b| {
        b.iter(|| {
            let db = fvm_ipld_blockstore::MemoryBlockstore::default();
            let mut a = BKamt::new_with_config(&db, TEST_CONFIG);

            for i in 0..black_box(ITEM_COUNT) {
                a.set(black_box(vec![i; 20]), black_box(BenchData::new(i)))
                    .unwrap();
            }
        })
    });
}

fn insert_load_flush(c: &mut Criterion) {
    c.bench_function("KAMT bulk insert with flushing and loading", |b| {
        b.iter(|| {
            let db = fvm_ipld_blockstore::MemoryBlockstore::default();
            let mut empt = BKamt::new_with_config(&db, TEST_CONFIG);
            let mut cid = empt.flush().unwrap();

            for i in 0..black_box(ITEM_COUNT) {
                let mut a = BKamt::load_with_config(&cid, &db, TEST_CONFIG).unwrap();
                a.set(black_box(vec![i; 20]), black_box(BenchData::new(i)))
                    .unwrap();
                cid = a.flush().unwrap();
            }
        })
    });
}

fn delete(c: &mut Criterion) {
    let db = fvm_ipld_blockstore::MemoryBlockstore::default();
    let mut a = BKamt::new_with_config(&db, TEST_CONFIG);
    for i in 0..black_box(ITEM_COUNT) {
        a.set(vec![i; 20], BenchData::new(i)).unwrap();
    }
    let cid = a.flush().unwrap();

    c.bench_function("KAMT deleting all nodes", |b| {
        b.iter(|| {
            let mut a = BKamt::load_with_config(&cid, &db, TEST_CONFIG).unwrap();
            for i in 0..black_box(ITEM_COUNT) {
                a.delete(black_box(vec![i; 20].as_ref())).unwrap();
            }
        })
    });
}

fn for_each(c: &mut Criterion) {
    let db = fvm_ipld_blockstore::MemoryBlockstore::default();
    let mut a = BKamt::new_with_config(&db, TEST_CONFIG);
    for i in 0..black_box(ITEM_COUNT) {
        a.set(vec![i; 20], BenchData::new(i)).unwrap();
    }
    let cid = a.flush().unwrap();

    c.bench_function("KAMT for_each function", |b| {
        b.iter(|| {
            let a = BKamt::load_with_config(&cid, &db, TEST_CONFIG).unwrap();
            #[allow(deprecated)]
            black_box(a).for_each(|_k, _v: &BenchData| Ok(())).unwrap();
        })
    });
}

criterion_group!(benches, insert, insert_load_flush, delete, for_each);
criterion_main!(benches);
