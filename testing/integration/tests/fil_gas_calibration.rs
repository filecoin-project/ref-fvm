use std::io::Write;
use std::path::{Path, PathBuf};

use fil_gas_calibration_actor::{HashingParams, Method};
use fvm::executor::{ApplyKind, Executor};
use fvm::gas::Gas;
use fvm::trace::ExecutionEvent;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

const WASM_COMPILED_PATH: &str =
    "../../target/debug/wbuild/fil_gas_calibration_actor/fil_gas_calibration_actor.compact.wasm";

mod bundles;
use bundles::*;
use serde::Serialize;

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
struct State {
    pub count: u64,
}

struct TestEnv {
    tester: Tester<MemoryBlockstore, DummyExterns>,
    sender: Account,
    actor_address: Address,
}

const ENOUGH_GAS: Gas = Gas::new(1_000_000_000);

/// An observation that we can use to estimate coefficients
/// to model time in terms of some variables.
#[derive(Serialize)]
struct Obs {
    pub label: String,
    pub elapsed_nanos: u128,
    pub variables: Vec<usize>,
}

// Utility function to instantiation integration tester
fn instantiate_tester() -> TestEnv {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V16,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    // Get wasm bin
    let wasm_path = std::env::current_dir()
        .unwrap()
        .join(WASM_COMPILED_PATH)
        .canonicalize()
        .unwrap();

    let wasm_bin = std::fs::read(wasm_path).expect("Unable to read file");

    tester
        .set_actor_from_bin(&wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    tester
        .instantiate_machine_with_config(
            DummyExterns,
            |_| (),
            |mc| {
                mc.enable_tracing();
            },
        )
        .unwrap();

    TestEnv {
        tester,
        sender: sender[0],
        actor_address,
    }
}

#[test]
fn collect_gas_metrics() {
    if let Ok(traces_dir) = std::env::var("TRACE_DIR").map(|d| Path::new(&d).to_path_buf()) {
        let mut te = instantiate_tester();
        hashing(&mut te, &traces_dir);
    }
}

fn hashing(te: &mut TestEnv, out: &PathBuf) {
    let hashers = vec![
        SupportedHashes::Sha2_256,
        SupportedHashes::Blake2b256,
        SupportedHashes::Keccak256,
    ];
    let mut sizes: Vec<usize> = vec![0];
    sizes.extend([10, 100, 1_000, 10_000, 100_000].into_iter().flat_map(|i| {
        let fi: f64 = i.into();
        [1.0, 2.5, 5.0, 7.5].map(move |m| (m * fi).ceil() as usize)
    }));
    sizes.push(1_000_000);

    let iterations = 10;

    let mut obs = Vec::new();
    let mut sequence = 0;

    for hasher in hashers.iter() {
        let label = format!("{hasher:?}");
        for size in sizes.iter() {
            let params = HashingParams {
                hasher: *hasher as u64,
                size: *size,
                iterations,
            };

            let raw_params = RawBytes::serialize(&params).unwrap();

            let message = Message {
                from: te.sender.1,
                to: te.actor_address,
                sequence,
                gas_limit: ENOUGH_GAS.as_milligas(),
                method_num: Method::Hashing as u64,
                params: raw_params,
                ..Message::default()
            };
            sequence += 1;

            let ret = te
                .tester
                .executor
                .as_mut()
                .unwrap()
                .execute_message(message, ApplyKind::Explicit, 100)
                .unwrap();

            if let Some(failure) = ret.failure_info {
                panic!("message execution failed: {failure}");
            }
            assert_eq!(ret.msg_receipt.exit_code, ExitCode::OK);

            obs.extend(ret.exec_trace.iter().filter_map(|t| match t {
                ExecutionEvent::GasCharge(charge) if charge.name == "OnHashing" => Some(Obs {
                    label: label.clone(),
                    elapsed_nanos: charge.elapsed.get().unwrap().as_nanos(),
                    variables: vec![*size],
                }),
                _ => None,
            }));
        }
    }

    export_json(&out.join("OnHashing.jsonline"), obs).unwrap();
}

fn export_json<T: Serialize>(path: &PathBuf, values: Vec<T>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut output = std::fs::File::create(path)?;

    for value in values {
        let line = serde_json::to_string(&value).unwrap();
        writeln!(&mut output, "{}", line)?;
    }

    Ok(())
}
