// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::fs::File;
use std::io::BufReader;

use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_car::{load_car, CarReader};

#[test]
fn load_into_blockstore() {
    let file = File::open("tests/test.car").unwrap();
    let buf_reader = BufReader::new(file);
    let bs = MemoryBlockstore::default();

    let _ = load_car(&bs, buf_reader).unwrap();
}

#[test]
fn load_car_reader_into_blockstore() {
    let file = File::open("tests/test.car").unwrap();
    let car_reader = CarReader::new(file).unwrap();
    let bs = MemoryBlockstore::default();

    // perform some action with the reader
    let roots = car_reader.header.roots.clone();

    // load it into the blockstore
    let res = car_reader.read_into(&bs).unwrap();

    assert_eq!(res, roots);
}
