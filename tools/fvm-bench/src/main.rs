// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

mod fevm;

use std::fs;

use anyhow::{anyhow, Context};
use clap::Parser;
use fvm_integration_tests::tester;

/// Run a contract invocation for benchmarking purposes
#[derive(Parser, Debug)]
struct Args {
    /// Execution mode: wasm or fevm
    #[arg(short, long, default_value = "fevm")]
    mode: String,

    /// Emit debug logs
    #[arg(short, long, default_value = "false")]
    debug: bool,

    /// Emit detailed gas tracing information
    #[arg(short, long, default_value = "false")]
    trace: bool,

    /// Emit user generated logs
    #[arg(short, long, default_value = "false")]
    events: bool,

    /// Builtin actors bundle to use.
    #[arg(short, long)]
    bundle: String,

    /// Contract file.
    contract: String,

    /// Invocation method; solidity entry point for fevm, actor method for wasm.
    method: String,

    /// Invocation parameters, in hex.
    params: String,

    #[arg(short, long, default_value = "10000000000")]
    /// Gas limit in atto precision to use during invocation.
    /// Default: 10 billion gas
    gas_limit: u64,
}

fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let options = tester::ExecutionOptions {
        debug: args.debug,
        trace: args.trace,
        events: args.events,
    };
    let mut tester = tester::BasicTester::new_basic_tester(args.bundle, options)?;

    match args.mode.as_str() {
        "fevm" => {
            let contract_hex =
                fs::read_to_string(args.contract).context("error reading contract")?;
            let contract = hex::decode(contract_hex).context("error decoding contract")?;
            let entrypoint =
                hex::decode(args.method).context("error decoding contract entrypoint")?;
            let params = hex::decode(args.params).context("error decoding contract params")?;

            fevm::run(&mut tester, &contract, &entrypoint, &params, args.gas_limit)
                .context("contract execution failed")
        }

        "wasm" => Err(anyhow!("wasm actors not supported yet")),
        _ => Err(anyhow!("unknown mode {}", args.mode)),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("ERROR: {:?}", e);
        std::process::exit(1);
    }
}
