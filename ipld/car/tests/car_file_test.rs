// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use async_std::fs::File;
use async_std::io::BufReader;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_car::{load_car, CarReader};

#[async_std::test]
async fn load_into_blockstore() {
    let file = File::open("tests/test.car").await.unwrap();
    let buf_reader = BufReader::new(file);
    let bs = MemoryBlockstore::default();

    let _ = load_car(&bs, buf_reader).await.unwrap();
}

#[async_std::test]
async fn load_car_reader_into_blockstore() {
    let file = File::open("tests/test.car").await.unwrap();
    let car_reader = CarReader::new(file).await.unwrap();
    let bs = MemoryBlockstore::default();

    // perform some action with the reader
    let roots = car_reader.header.roots.clone();

    // load it into the blockstore
    let res = car_reader.read_into(&bs).await.unwrap();

    assert_eq!(res, roots);
}
