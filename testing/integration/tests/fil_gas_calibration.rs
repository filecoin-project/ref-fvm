#![feature(slice_group_by)]
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
use lazy_static::lazy_static;
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

lazy_static! {
    /// The maximum parallelism when processing test vectors.
    static ref OUTPUT_DIR: Option<PathBuf> = std::env::var("OUTPUT_DIR")
        .map(|d| Path::new(&d).to_path_buf())
        .ok();
}

/// An observation that we can use to estimate coefficients
/// to model time in terms of some variables.
#[derive(Serialize)]
struct Obs {
    pub label: String,
    pub elapsed_nanos: u128,
    pub variables: Vec<usize>,
}

#[derive(Serialize)]
struct RegressionResult {
    pub label: String,
    pub intercept: f64,
    pub slope: f64,
    pub r_squared: f64,
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
fn hashing() {
    let hashers = vec![
        SupportedHashes::Sha2_256,
        SupportedHashes::Blake2b256,
        SupportedHashes::Blake2b512,
        SupportedHashes::Keccak256,
        SupportedHashes::Ripemd160,
    ];

    let mut sizes: Vec<usize> = vec![0];
    sizes.extend(
        [10, 100, 1_000, 10_000, 100_000]
            .into_iter()
            .flat_map(|i| (1..10).map(move |m| m * i)),
    );
    sizes.push(1_000_000);

    let iterations = 10;

    let mut te = instantiate_tester();
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

    let regs = obs
        .group_by(|a, b| a.label == b.label)
        .map(|g| least_squares(g[0].label.to_owned(), g, 0))
        .collect::<Vec<_>>();

    check_regressions("OnHashing", &regs);

    export("OnHashing", obs, regs).unwrap();
}

fn export(name: &str, obs: Vec<Obs>, regs: Vec<RegressionResult>) -> std::io::Result<()> {
    if let Some(out) = &*OUTPUT_DIR {
        let file_name = format!("{name}.jsonline");
        export_json(&out.join("regressions").join(&file_name), regs)?;
        export_json(&out.join("observations").join(&file_name), obs)?;
    }
    Ok(())
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

fn check_regressions(name: &str, regs: &Vec<RegressionResult>) {
    for reg in regs {
        assert!(
            reg.r_squared >= 0.8,
            "R-squared of {}/{} not good enough: {}",
            name,
            reg.label,
            reg.r_squared
        );

        // NOTE: The intercept is often negative, which suggests we can probably treat it as zero.
    }
}

/// Linear regression between one of the variables and time.
///
/// https://www.mathsisfun.com/data/least-squares-regression.html
fn least_squares(label: String, obs: &[Obs], var_idx: usize) -> RegressionResult {
    let mut sum_x = 0f64;
    let mut sum_y = 0f64;
    let mut sum_x2 = 0f64;
    let mut sum_xy = 0f64;
    let n = obs.len() as f64;

    let xys = obs
        .iter()
        .map(|obs| {
            let x = obs.variables[var_idx] as f64;
            let y = obs.elapsed_nanos as f64;
            (x, y)
        })
        .collect::<Vec<_>>();

    eprintln!("{label}");
    for (x, y) in xys.iter() {
        sum_y += y;
        sum_x += x;
        sum_x2 += x * x;
        sum_xy += x * y;
    }

    let m: f64 = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let b: f64 = (sum_y - m * sum_x) / n;

    eprintln!("({sum_y} - {m} * {sum_x}) / {n} = {b}");

    // R2 = 1 - RSS/TSS
    // RSS = sum of squares of residuals
    // TSS = total sum of squares
    let mean_y = sum_y / n;
    let mut tss = 0f64;
    let mut rss = 0f64;

    for (x, y) in xys.iter() {
        let f = m * x + b;
        let e = y - f;
        rss += e * e;

        let e = y - mean_y;
        tss += e * e;
    }
    let r_squared = 1.0 - rss / tss;

    RegressionResult {
        label,
        intercept: b,
        slope: m,
        r_squared,
    }
}
