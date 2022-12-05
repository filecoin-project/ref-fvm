use std::io::Write;
use std::path::{Path, PathBuf};

use fvm::gas::Gas;
use fvm_integration_tests::bundle;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use lazy_static::lazy_static;
use num_traits::Zero;
use serde::Serialize;

pub const WASM_COMPILED_PATH: &str =
    "../../target/debug/wbuild/fil_gas_calibration_actor/fil_gas_calibration_actor.compact.wasm";

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: u64,
}

pub struct TestEnv {
    pub tester: Tester<MemoryBlockstore, DummyExterns>,
    pub sender: Account,
    pub actor_address: Address,
}

pub const ENOUGH_GAS: Gas = Gas::new(1_000_000_000);

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
    pub label: String,
    pub elapsed_nanos: u128,
    pub variables: Vec<usize>,
}

#[derive(Serialize)]
pub struct RegressionResult {
    pub label: String,
    pub intercept: f64,
    pub slope: f64,
    pub r_squared: f64,
}

// Utility function to instantiation integration tester
pub fn instantiate_tester() -> TestEnv {
    let blockstore = MemoryBlockstore::default();
    let root = bundle::import_bundle(&blockstore, actors_v9::BUNDLE_CAR).unwrap();
    // Instantiate tester
    let mut tester =
        Tester::new(NetworkVersion::V16, StateTreeVersion::V4, root, blockstore).unwrap();

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
