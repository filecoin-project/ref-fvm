// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::hint::black_box;
use std::time::Duration;

use cid::Cid;
use criterion::{criterion_group, criterion_main, Criterion};
use fvm::engine::EnginePool;
use fvm::machine::{Manifest, NetworkConfig};

use fvm_integration_tests::bundle;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::CborStore;
use fvm_shared::version::NetworkVersion;

fn bench_compile(c: &mut Criterion) {
    c.bench_function("bench actor compile", |b| {
        let blockstore = MemoryBlockstore::default();
        let bundle_cid = bundle::import_bundle(&blockstore, actors_v12::BUNDLE_CAR).unwrap();

        let (manifest_version, manifest_cid): (u32, Cid) =
            blockstore.get_cbor(&bundle_cid).unwrap().unwrap();
        let manifest = Manifest::load(&blockstore, &manifest_cid, manifest_version).unwrap();
        let nc = NetworkConfig::new(NetworkVersion::V21);
        b.iter_batched(
            || EnginePool::new((&nc).into()).unwrap(),
            |engine| {
                black_box(
                    engine
                        .acquire()
                        .preload_all(&blockstore, manifest.builtin_actor_codes())
                        .unwrap(),
                );
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(30));
    targets = bench_compile
}

criterion_main!(benches);
