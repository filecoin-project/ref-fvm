// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fvm_ipld_amt::{Amt, AmtImpl};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::strict_bytes::ByteBuf;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const ITEM_COUNT: usize = 60;

// Struct to simulate a reasonable amount of data per value into the amt
#[derive(Clone)]
struct BenchData {
    #[allow(dead_code)]
    s: String,
    #[allow(dead_code)]
    s2: String,
    #[allow(dead_code)]
    bz: Vec<u8>,
    #[allow(dead_code)]
    v: u64,
    #[allow(dead_code)]
    a: [u8; 64],
    #[allow(dead_code)]
    a2: [u8; 64],
}

// Serializations ignored for benchmarking
impl Serialize for BenchData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BenchData {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer)?;
        Ok(Self::default())
    }
}

impl Default for BenchData {
    fn default() -> Self {
        Self {
            s: "testing string".to_owned(),
            s2: "other".to_owned(),
            bz: vec![5, 3, 1, 5, 7, 8],
            v: 0,
            a: [8; 64],
            a2: [5; 64],
        }
    }
}

const VALUES: &[u64] = &[
    0x20, 0xfc, 0x40, 0xc2, 0xcc, 0xe5, 0xd8, 0xc1, 0xe1, 0x1e, 0x23, 0xd3, 0x02, 0x2e, 0xcd, 0x03,
    0x3e, 0x83, 0x16, 0x26, 0x3a, 0x5c, 0x30, 0x8e, 0x00, 0x05, 0xcc, 0x24, 0x0e, 0x96, 0x15, 0x48,
    0xa0, 0x2a, 0x40, 0x04, 0x92, 0x0e, 0x94, 0xc0, 0x48, 0x91, 0xee, 0x05, 0x28, 0x98, 0x55, 0x90,
    0xa0, 0xa4, 0x50, 0x98, 0x14, 0x40, 0x04, 0x59, 0x0a, 0x22, 0x00, 0x74, 0xb0, 0x40, 0x66, 0x30,
    0xf9, 0x66, 0x90, 0x51, 0xc7, 0x70, 0x74, 0x40, 0x48, 0x7f, 0xf0, 0x80, 0x24, 0x20, 0x85, 0x58,
    0x06, 0x07, 0x66, 0x04, 0x87, 0xe5, 0x05, 0x28, 0x00, 0xa4, 0xf0, 0x61, 0x2e, 0x90, 0x08, 0x44,
    0x70, 0x38, 0x34, 0xf0, 0x08, 0x4f, 0x70, 0x68, 0xad, 0xd0, 0x90, 0x1e, 0x90, 0x38, 0xc1, 0x85,
    0x76, 0x04, 0x15, 0x7c, 0x04, 0x28, 0x28, 0x17, 0x70, 0xe0, 0x15, 0x00, 0x82, 0xfb, 0x11, 0x0b,
    0x76, 0x09, 0x22, 0xb8, 0x2f, 0x90, 0x20, 0x5f, 0x80, 0x84, 0xc9, 0x10, 0x85, 0x66, 0x09, 0x05,
    0xc9, 0x03, 0x2d, 0x19, 0xa4, 0x5a, 0x70, 0x45, 0xa5, 0x40, 0xc2, 0x05, 0x6c, 0x4a, 0x0f, 0x64,
    0xf4, 0x19, 0xb0, 0x40, 0x28, 0x8b, 0x02, 0x6c, 0x20, 0x3e, 0x90, 0x40, 0x01, 0x19, 0xea, 0x09,
    0x20, 0x60, 0x2f, 0x50, 0x60, 0x15, 0x00, 0x04, 0x69, 0x06, 0xb1, 0x8c, 0xa9, 0x85, 0xc6, 0x1f,
    0x13, 0x54, 0x3e, 0x58, 0x40, 0x17, 0x60, 0x41, 0x2e, 0x8a, 0x42, 0x3c, 0x8b, 0x0b, 0x3f, 0x08,
    0x10, 0x5c, 0x32, 0x38, 0xd4, 0x1e, 0x68, 0x18, 0x5b, 0x70, 0xc1, 0x2c, 0x06, 0x17, 0x6c, 0x17,
    0xf8, 0x74, 0x36, 0x28, 0x9c, 0x0e, 0x20, 0x02, 0xa1, 0x84, 0x0f, 0xa2, 0x82, 0x0e, 0xbf, 0x82,
];

fn insert(c: &mut Criterion) {
    c.bench_function("AMT bulk insert (no flush)", |b| {
        b.iter(|| {
            let db = fvm_ipld_blockstore::MemoryBlockstore::default();
            let mut a = Amt::new(&db);

            for i in 0..black_box(ITEM_COUNT) {
                a.set(black_box(i as u64), black_box(BenchData::default()))
                    .unwrap();
            }
        })
    });
}

fn insert_load_flush(c: &mut Criterion) {
    c.bench_function("AMT bulk insert with flushing and loading", |b| {
        b.iter(|| {
            let db = fvm_ipld_blockstore::MemoryBlockstore::default();
            let mut empt = Amt::<(), _>::new(&db);
            let mut cid = empt.flush().unwrap();

            for i in 0..black_box(ITEM_COUNT) {
                let mut a = Amt::load(&cid, &db).unwrap();
                a.set(black_box(i as u64), black_box(BenchData::default()))
                    .unwrap();
                cid = a.flush().unwrap();
            }
        })
    });
}

fn from_slice(c: &mut Criterion) {
    c.bench_function("AMT initialization from slice", |b| {
        b.iter(|| {
            let db = fvm_ipld_blockstore::MemoryBlockstore::default();
            Amt::new_from_iter(&db, black_box(VALUES.iter().copied())).unwrap();
        })
    });
}

fn for_each(c: &mut Criterion) {
    let db = fvm_ipld_blockstore::MemoryBlockstore::default();
    let cid = Amt::new_from_iter(&db, black_box(VALUES.iter().copied())).unwrap();

    c.bench_function("AMT for_each function", |b| {
        b.iter(|| {
            let a: AmtImpl<ByteBuf, &MemoryBlockstore, fvm_ipld_amt::V3> = Amt::load(&cid, &db).unwrap();
            black_box(a).iter().for_each(|_| ());
        })
    });
}

criterion_group!(benches, insert, insert_load_flush, from_slice, for_each);
criterion_main!(benches);
