extern crate criterion;

use criterion::*;
use fvm::executor::{ApplyKind, DefaultExecutor, Executor};
use fvm::machine::MultiEngine;
use fvm_conformance_tests::driver::*;
use fvm_conformance_tests::vector::{MessageVector, Variant};
use fvm_conformance_tests::vm::{TestKernel, TestMachine};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::Cbor;
use fvm_shared::address::Protocol;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::message::Message;

/// Applies a list of messages to the VM. Panics if one fails, but this is okay because the caller will test with these messages first.
///
/// # Arguments
///
/// * `messages` - mutable vector of (message, usize) tuples with the message and its raw length. will be removed from vector and applied in order
/// * `exec` - test executor
pub fn apply_messages(messages: Vec<(Message, usize)>, mut exec: DefaultExecutor<TestKernel>) {
    // Apply all messages in the vector.
    for (msg, raw_length) in messages.into_iter() {
        // Execute the message.
        // can assume this works because it passed a test before this ran
        exec.execute_message(msg, ApplyKind::Explicit, raw_length)
            .unwrap();
    }
}

/// Benches one vector variant using criterion. Clones `MessageVector`, clones `Blockstore`, clones a prepared list of message bytes with lengths, creates a new machine, initializes its wasm cache by loading some code, creates an executor, then times applying the messages.
/// Currently needs some serious speedup, probably with respect to WASM caching and also machine setup/teardown.
pub fn bench_vector_variant(
    group: &mut BenchmarkGroup<measurement::WallTime>,
    name: String,
    variant: &Variant,
    vector: &MessageVector,
    messages_with_lengths: Vec<(Message, usize)>,
    bs: &MemoryBlockstore,
    engines: &MultiEngine,
) {
    group.bench_function(name, move |b| {
        b.iter_batched(
            || {
                let vector = &(*vector).clone();
                let bs = bs.clone();
                // NOTE next few lines don't impact the benchmarks.
                let machine = TestMachine::new_for_vector(vector, variant, bs, engines);
                // can assume this works because it passed a test before this ran
                let exec: DefaultExecutor<TestKernel> = DefaultExecutor::new(machine);
                (messages_with_lengths.clone(), exec)
            },
            |(messages, exec)| apply_messages(criterion::black_box(messages), exec),
            BatchSize::LargeInput,
        )
    });
}
/// This tells `bench_vector_file` how hard to do checks on whether things succeed before running benchmark
#[derive(Clone, Copy, Debug)]
pub enum CheckStrength {
    /// making sure everything conforms before benching, for when you're benching the real vector as it came from specs-actors
    #[allow(dead_code)]
    FullTest,
    /// use in cases where we're swapping out the messages to apply and just using the setup (overhead tests, for example)
    #[allow(dead_code)]
    OnlyCheckSuccess,
    /// use if for some reason you want to bench something that errors (or go really fast and dangerous!)
    #[allow(dead_code)]
    NoChecks,
}

/// default is FullTest
impl Default for CheckStrength {
    fn default() -> Self {
        CheckStrength::FullTest
    }
}

/// benches each variant in a vector file. returns an err if a vector fails to parse the messages out in run_variant, or if a test fails before benching if you set FullTest or OnlyCheckSuccess.
pub fn bench_vector_file(
    group: &mut BenchmarkGroup<measurement::WallTime>,
    vector: &MessageVector,
    check_strength: CheckStrength,
    name: &str,
    engines: &MultiEngine,
) -> anyhow::Result<()> {
    let (bs, _) = async_std::task::block_on(vector.seed_blockstore()).unwrap();

    for variant in vector.preconditions.variants.iter() {
        let name = format!("{} | {}", name, variant.id);
        // this tests the variant before we run the benchmark and record the bench results to disk.
        // if we broke the test, it's not a valid optimization :P
        let testresult = match check_strength {
            CheckStrength::FullTest => run_variant(bs.clone(), vector, variant, engines, true)
                .map_err(|e| {
                    anyhow::anyhow!("run_variant failed (probably a test parsing bug): {}", e)
                })?,
            CheckStrength::OnlyCheckSuccess => {
                run_variant(bs.clone(), vector, variant, engines, false).map_err(|e| {
                    anyhow::anyhow!("run_variant failed (probably a test parsing bug): {}", e)
                })?
            }
            CheckStrength::NoChecks => VariantResult::Ok {
                id: variant.id.clone(),
            },
        };

        if let VariantResult::Ok { .. } = testresult {
            let messages_with_lengths: Vec<(Message, usize)> = vector
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
            bench_vector_variant(
                group,
                name,
                variant,
                vector,
                messages_with_lengths,
                &bs,
                engines,
            );
        } else {
            return Err(anyhow::anyhow!("a test failed, get the tests passing/running before running benchmarks in {:?} mode: {}", check_strength, name));
        };
    }
    Ok(())
}
