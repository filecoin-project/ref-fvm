// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
//#[macro_use]
extern crate criterion;

use std::env::var;
use std::fs::File;
use std::io::BufReader;
use std::iter;
use std::path::{Path, PathBuf};
use std::time::Duration;

use conformance_tests::test_utils::*;
use conformance_tests::vector::{MessageVector, Selector, TestVector, Variant};
use conformance_tests::vm::{TestKernel, TestMachine};
use criterion::*;
use fvm::executor::{ApplyKind, DefaultExecutor, Executor};
use fvm_shared::address::Protocol;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;
use walkdir::WalkDir;
use fvm_shared::blockstore::MemoryBlockstore;

fn apply_messages(messages: &mut Vec<(Message, usize)>, exec: &mut DefaultExecutor<TestKernel>) {
    // Apply all messages in the vector.
    for (msg, raw_length) in messages.drain(..) {
        // Execute the message.
        exec.execute_message(msg, ApplyKind::Explicit, raw_length)
            .unwrap();
    }
}

fn bench_vector_variant(group: &mut BenchmarkGroup<measurement::WallTime>,
                        name: String,
                        variant: &Variant,
                        v: &MessageVector,
                        bs: &MemoryBlockstore,
) {
    group.bench_function(name, move |b| {
        b.iter_batched_ref(
            || {
                let v = v.clone();
                let bs = bs.clone();
                // TODO next few lines don't impact the benchmarks, but it makes them run more slowly... ought to make a base copy of the machine and exec and deepcopy them each time.
                let machine = TestMachine::new_for_vector(v, variant, bs);
                machine.load_builtin_actors_modules().unwrap();
                let exec: DefaultExecutor<TestKernel> = DefaultExecutor::new(machine);
                let messages = v
                    .apply_messages
                    .iter()
                    .map(|m| {
                        let unmarshalled = Message::unmarshal_cbor(&m.bytes).unwrap();
                        let mut raw_length = m.bytes.len();
                        if unmarshalled.from.protocol() == Protocol::Secp256k1 {
                            // 65 bytes signature + 1 byte type + 3 bytes for field info.
                            raw_length += SECP_SIG_LEN + 4;
                        }
                        (unmarshalled, raw_length)
                    })
                    .collect();
                (messages, exec)
            },
            |(messages, exec)| apply_messages(messages, exec),
            BatchSize::LargeInput,
        )
    });
}

fn bench_vector_file(group: &mut BenchmarkGroup<measurement::WallTime>, path: PathBuf) {
    let file = File::open(&path).unwrap();
    let reader = BufReader::new(file);
    let vector: TestVector = serde_json::from_reader(reader).unwrap();

    let TestVector::Message(vector) = vector;
    let skip = !vector.selector.as_ref().map_or(true, Selector::supported);
    if skip {
        // TODO support skipping- selector not supported idk what this means? something with chaos actor. support this.
        return;
    }

    // TODO ought to run the relevant conformance test and ensure it passes before we run the benchmark.
    let (bs, _) = async_std::task::block_on(vector.seed_blockstore()).unwrap();

    for variant in vector.preconditions.variants.iter() {
        let name = format!("{} | {}", path.display(), variant.id);
        bench_vector_variant(group, name, variant, &vector, &bs);
    }
}

fn bench_noops() {
    ()
}

fn bench(c: &mut Criterion) {
    // TODO: this is 30 seconds per benchmark... once we get the setup running faster (by cloning VMs more efficiently), we can bring this down.
    let mut group = c.benchmark_group("conformance-tests");
    group.measurement_time(Duration::new(30, 0));

    //let vector_name = "test-vectors/corpus/specs_actors_v6/TestCronCatchedCCExpirationsAtDeadlineBoundary/cabb8135a017bfee049180ec827d4dffdd94cd2c7253180252ed6bcb9361ddd2-t0100-t0102-storageminer-5.json";

    // TODO add pretty logging when you iterate over everything
    // pretty_env_logger::init();

    // TODO match globs?
    let (mut vector_results, _is_many) : (Vec<PathBuf>, bool) = match var("VECTOR") {
        Ok(v) => (iter::once(Path::new(v.as_str()).to_path_buf()).collect(), false),
        Err(_) => (
            WalkDir::new("test-vectors/corpus")
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_runnable)
                .map(|e| e.path().to_path_buf()).collect(),
            true,
        ),
    };

    for vector in vector_results.drain(..) {
        bench_vector_file(&mut group, vector);
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
