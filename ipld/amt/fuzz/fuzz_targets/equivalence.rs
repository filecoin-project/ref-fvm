// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]
use arbitrary::Arbitrary;
use fvm_ipld_amt::{Amt, MAX_INDEX};
use libfuzzer_sys::fuzz_target;
use cid::Cid;


#[derive(Debug, Arbitrary)]
struct Operation {
    idx: u64,
    method: Method,
}

#[derive(Debug, Arbitrary)]
enum Method {
    Insert(u64),
    Remove,
    Get,
}

fn execute(ops: Vec<Operation>) -> (Cid, ahash::AHashMap<u64, u64>) {
    let db = fvm_shared::blockstore::MemoryBlockstore::default();
    let mut amt = Amt::new(&db);
    let mut elements = ahash::AHashMap::new();

    for (i, Operation { idx, method }) in ops.into_iter().enumerate() {
        if idx > MAX_INDEX {
            continue;
        }

        match method {
            Method::Insert(v) => {
                elements.insert(idx, v);
                amt.set(idx, v).unwrap();
            }
            Method::Remove => {
                let el = elements.remove(&idx);
                let amt_deleted = amt.delete(idx).unwrap();
                assert_eq!(amt_deleted, el, "step {}", i);
            }
            Method::Get => {
                let ev = elements.get(&idx);
                let av = amt.get(idx).unwrap();
                assert_eq!(av, ev, "step {}", i);
            }
        }
    }
    (amt.flush().unwrap(), elements)
}

// Verifies that AMT created by this order of operations results in the same CID as
// AMT created by minimal number of operations required.
// The aim is to verify lack of past memory in the AMT structures.
// AMT with same elements should have the same CID, regardless of their past.
fuzz_target!(|ops: Vec<Operation>| {
    let (res_cid, m) = execute(ops);

    let simplified_ops = m.iter().map(|(k ,v)| {
        Operation{idx: *k, method: Method::Insert(*v)}
    }).collect();

    let (simplified_cid, _) = execute(simplified_ops);

    assert_eq!(res_cid, simplified_cid)
});
