// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::io::Write;
use std::path::{Path, PathBuf};

use fvm::executor::{ApplyKind, ApplyRet, Executor};
use fvm::gas::Gas;
use fvm::trace::ExecutionEvent;
use fvm_integration_tests::bundle;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use lazy_static::lazy_static;
use num_traits::Zero;
use serde::Serialize;

pub const WASM_COMPILED_PATH: &str =
    "../../target/release/wbuild/fil_gas_calibration_actor/fil_gas_calibration_actor.compact.wasm";

pub const ENOUGH_GAS: Gas = Gas::new(1_000_000_000);

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: u64,
}

pub struct TestEnv {
    pub tester: Tester<MemoryBlockstore, DummyExterns>,
    pub sender: Account,
    pub actor_address: Address,
    pub actor_sequence: u64,
}

impl TestEnv {
    /// Call a method with some parameters and return the results.
    ///
    /// Panics if the message hasn't executed successfully.
    pub fn execute_or_die<P: Serialize>(&mut self, method_num: u64, params: &P) -> ApplyRet {
        let raw_params = RawBytes::serialize(params).unwrap();
        let message = Message {
            from: self.sender.1,
            to: self.actor_address,
            sequence: self.actor_sequence,
            gas_limit: ENOUGH_GAS.as_milligas(),
            method_num,
            params: raw_params,
            ..Message::default()
        };

        self.actor_sequence += 1;

        let ret = self
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

        ret
    }
}

lazy_static! {
    /// The maximum parallelism when processing test vectors.
    pub static ref OUTPUT_DIR: PathBuf = std::env::var("OUTPUT_DIR")
        .map(|d| Path::new(&d).to_path_buf())
        .ok().unwrap_or_else(|| {
          Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf().join("measurements").join("out")
        });
}

/// An observation that we can use to estimate coefficients
/// to model time in terms of some variables.
#[derive(Serialize)]
pub struct Obs {
    pub charge: String,
    pub label: String,
    pub elapsed_nanos: u128,
    pub variables: Vec<usize>,
    pub compute_gas: u64,
}

#[derive(Serialize)]
pub struct RegressionResult {
    pub label: String,
    pub intercept: f64,
    pub slope: f64,
    pub r_squared: f64,
}

const NOP_ACTOR: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "invoke") (param $x i32) (result i32)
    (i32.const 0)
  )
)
"#;

// Utility function to instantiation integration tester
pub fn instantiate_tester() -> TestEnv {
    let blockstore = MemoryBlockstore::default();
    let root = bundle::import_bundle(&blockstore, actors_v10::BUNDLE_CAR).unwrap();
    // Instantiate tester
    let mut tester =
        Tester::new(NetworkVersion::V18, StateTreeVersion::V5, root, blockstore).unwrap();

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
        .set_actor_from_bin(
            &wasm_bin,
            state_cid,
            actor_address,
            TokenAmount::from_whole(100),
        )
        .unwrap();

    // Setup a basic no-op actor.
    let nop_actor_bin = wat::parse_str(NOP_ACTOR).unwrap();
    let nop_actor_address = Address::new_id(10001);
    tester
        .set_actor_from_bin(
            &nop_actor_bin,
            state_cid,
            nop_actor_address,
            TokenAmount::zero(),
        )
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
        actor_sequence: 0,
    }
}

pub fn export(name: &str, obs: &Vec<Obs>, regs: &Vec<RegressionResult>) -> std::io::Result<()> {
    let out = &*OUTPUT_DIR;
    let file_name = format!("{name}.jsonline");
    export_json(&out.join("regressions").join(&file_name), regs)?;
    export_json(&out.join("observations").join(&file_name), obs)?;
    Ok(())
}

pub fn export_json<T: Serialize>(path: &PathBuf, values: &Vec<T>) -> std::io::Result<()> {
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

/// Linear regression between one of the variables and time.
///
/// https://www.mathsisfun.com/data/least-squares-regression.html
pub fn least_squares(label: String, obs: &[Obs], var_idx: usize) -> RegressionResult {
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

    for (x, y) in xys.iter() {
        sum_y += y;
        sum_x += x;
        sum_x2 += x * x;
        sum_xy += x * y;
    }

    let m: f64 = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let b: f64 = (sum_y - m * sum_x) / n;

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

pub fn collect_obs(ret: &ApplyRet, name: &str, label: &str, size: usize) -> Vec<Obs> {
    ret.exec_trace
        .iter()
        .filter_map(|t| match t {
            ExecutionEvent::GasCharge(charge) if charge.name == name => Some(Obs {
                charge: charge.name.to_string(),
                label: label.to_owned(),
                elapsed_nanos: charge.elapsed.get().unwrap().as_nanos(),
                variables: vec![size],
                compute_gas: charge.compute_gas.as_milligas(),
            }),
            _ => None,
        })
        .collect()
}

/// Drop a certain fraction of the observations with the highest time as outliers.
pub fn eliminate_outliers(mut obs: Vec<Obs>, drop: f32, eliminate: Eliminate) -> Vec<Obs> {
    obs.sort_by_key(|obs| obs.elapsed_nanos);
    let size = obs.len();
    let drop = (size as f32 * drop) as usize;
    match eliminate {
        Eliminate::Top => obs.into_iter().take(size - drop).collect(),
        Eliminate::Bottom => obs.into_iter().skip(drop).collect(),
        Eliminate::Both => obs.into_iter().skip(drop).take(size - 2 * drop).collect(),
    }
}

pub enum Eliminate {
    Top,
    Bottom,
    Both,
}

pub fn common_sizes() -> Vec<usize> {
    let mut sizes: Vec<usize> = vec![0];
    sizes.extend(
        [10, 100, 1_000, 10_000, 100_000]
            .into_iter()
            .flat_map(|i| (1..10).map(move |m| m * i)),
    );
    sizes.push(1_000_000);
    sizes
}

pub fn secp_sign(sk: &libsecp256k1::SecretKey, data: &[u8]) -> [u8; SECP_SIG_LEN] {
    let hash: [u8; 32] = blake2b_simd::Params::new()
        .hash_length(32)
        .to_state()
        .update(data)
        .finalize()
        .as_bytes()
        .try_into()
        .unwrap();

    let (sig, recovery_id) = libsecp256k1::sign(&libsecp256k1::Message::parse(&hash), sk);

    let mut signature = [0u8; SECP_SIG_LEN];
    signature[..64].copy_from_slice(&sig.serialize());
    signature[64] = recovery_id.serialize();
    signature
}
