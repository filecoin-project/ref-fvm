// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use async_std::fs::File;
use async_std::io::BufReader;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_car::load_car;

#[async_std::test]
async fn load_into_blockstore() {
    let file = File::open("tests/test.car").await.unwrap();
    let buf_reader = BufReader::new(file);
    let bs = MemoryBlockstore::default();

    let _ = load_car(&bs, buf_reader).await.unwrap();
}
