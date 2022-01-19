// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
//#[macro_use]
extern crate criterion;

// TODO support skipping
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use async_std::sync;
use conformance_tests::vector::{Selector, TestVector};
use conformance_tests::vm::{TestKernel, TestMachine};
use criterion::*;
use fvm::executor::{ApplyKind, DefaultExecutor, Executor};
use fvm_shared::address::Protocol;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;

fn apply_messages(messages: &mut Vec<(Message, usize)>, exec: &mut DefaultExecutor<TestKernel>) {
    // Apply all messages in the vector.
    for (msg, raw_length) in messages.drain(..) {
        // Execute the message.
        // TODO real error handling
        match exec.execute_message(msg, ApplyKind::Explicit, raw_length) {
            Ok(_ret) => (),
            Err(_e) => break,
        }
    }
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("conformance-tests");

    // TODO: this goes in a loop of benchmarks to run in the group!
    let vector_name = "test-vectors/corpus/specs_actors_v6/TestCronCatchedCCExpirationsAtDeadlineBoundary/c70afe9fa5e05990cac8ab8d4e49522919ad29e5be3f81ee4b59752c36c4a701-t0100-t0102-storageminer-6.json";
    let path = Path::new(vector_name).to_path_buf();
    let file = File::open(&path).unwrap();
    let reader = BufReader::new(file);
    let vector: TestVector = serde_json::from_reader(reader).unwrap();

    let TestVector::Message(vector) = vector ;
        let skip = !vector.selector.as_ref().map_or(true, Selector::supported);
        if skip {
            // selector not supported idk what this means
            return;
        }

        // TODO should i check the roots?
        let (bs, _imported_root) = async_std::task::block_on(vector.seed_blockstore()).unwrap();

        let v = sync::Arc::new(vector);

        // TODO: become another iterator over variants woo woo
        let variant_num = 0;
        let variant = v.preconditions.variants[variant_num].clone();
        let name = format!("{} | {}", path.display(), variant.id);

        group.bench_function(name,
                             move |b| {
                                 b.iter_batched_ref(
                                         || {
                                             let v = v.clone();
                                             let bs = bs.clone();
                                             let machine = TestMachine::new_for_vector(v.as_ref(), &variant, bs);
                                             let exec: DefaultExecutor<TestKernel> = DefaultExecutor::new(machine);
                                             let messages = v.apply_messages.iter().map(|m| {
                                                 let unmarshalled = Message::unmarshal_cbor(&m.bytes).unwrap();
                                                 let mut raw_length = m.bytes.len();
                                                 if unmarshalled.from.protocol() == Protocol::Secp256k1 {
                                                     // 65 bytes signature + 1 byte type + 3 bytes for field info.
                                                     raw_length += SECP_SIG_LEN + 4;
                                                 }
                                                 (unmarshalled, raw_length)
                                             }).collect();
                                             (messages, exec)
                                         },
                                         |(messages, exec)| apply_messages(messages, exec),
                                         BatchSize::LargeInput,
                                     )
                             });


    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
