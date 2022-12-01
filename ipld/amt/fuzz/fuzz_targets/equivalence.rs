// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]
use arbitrary::Arbitrary;
use cid::Cid;
use fvm_ipld_amt::Amt;
use itertools::Itertools;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct Operation {
    idx: u16,
    method: Method,
    flush: u8, // flush in 5% of cases on expectation so > (255 - 13)
}

#[derive(Debug, Arbitrary)]
enum Method {
    Insert(u64),
    Remove,
    Get,
}
fn execute(ops: Vec<Operation>) -> (Cid, ahash::AHashMap<u64, u64>) {
    let db = fvm_ipld_blockstore::MemoryBlockstore::default();
    let mut amt = Amt::new(&db);
    let mut elements = ahash::AHashMap::new();

    for (i, Operation { idx, method, flush }) in ops.into_iter().enumerate() {
        let idx = idx as u64;
        if flush > 255 - 13 {
            // Periodic flushing and reloading of Amt to fuzz blockstore usage also
            let cid = amt.flush().unwrap();
            amt = Amt::load(&cid, &db).unwrap();
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

    let simplified_ops = m
        .iter()
        .sorted_by_key(|(_, v)| *v)
        .map(|(k, v)| Operation {
            idx: *k as u16,
            method: Method::Insert(*v),
            flush: 0,
        })
        .collect();

    let (simplified_cid, _) = execute(simplified_ops);
    assert_eq!(res_cid, simplified_cid)
});
