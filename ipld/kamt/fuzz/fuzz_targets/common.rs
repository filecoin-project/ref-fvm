// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use arbitrary::Arbitrary;
use fvm_ipld_kamt::id::Identity;
use fvm_ipld_kamt::{Config, Kamt};

#[derive(Debug, Arbitrary)]
pub struct Operation {
    key: u64,
    method: Method,
}

#[derive(Debug, Arbitrary)]
pub enum Method {
    Insert(u64),
    Remove,
    Get,
}

pub fn run(flush_rate: u8, operations: Vec<Operation>, conf: Config) {
    let db = fvm_ipld_blockstore::MemoryBlockstore::default();
    let mut kamt = Kamt::<_, u64, u64, Identity>::new_with_config(&db, conf);
    let mut elements = ahash::AHashMap::new();

    let flush_rate = (flush_rate as usize).saturating_add(5);
    for (i, Operation { key, method }) in operations.into_iter().enumerate() {
        if i % flush_rate == 0 {
            // Periodic flushing of Kamt to fuzz blockstore usage also
            kamt.flush().unwrap();
        }

        match method {
            Method::Insert(v) => {
                elements.insert(key, v);
                kamt.set(key, v).unwrap();
            }
            Method::Remove => {
                let el = elements.remove(&key);
                let kamt_deleted = kamt.delete(&key).unwrap();
                assert_eq!(kamt_deleted, el);
            }
            Method::Get => {
                let ev = elements.get(&key);
                let av = kamt.get(&key).unwrap();
                assert_eq!(av, ev);
            }
        }
    }
}
