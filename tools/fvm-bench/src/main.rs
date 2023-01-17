// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

mod fevm;

use std::fs;

use clap::Parser;

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
    gas_limit: i64,
}

fn main() {
    let args = Args::parse();
    let options = testkit::ExecutionOptions {
        debug: args.debug,
        trace: args.trace,
        events: args.events,
    };
    let mut tester = testkit::new_tester(args.bundle);

    match args.mode.as_str() {
        "fevm" => {
            let contract_hex = fs::read_to_string(args.contract).unwrap_or_else(|what| {
                testkit::exit_with_error(format!("error reading contract: {}", what));
            });
            let contract = hex::decode(contract_hex).unwrap_or_else(|what| {
                testkit::exit_with_error(format!("error decoding contract: {}", what));
            });

            let entrypoint = hex::decode(args.method).unwrap_or_else(|what| {
                testkit::exit_with_error(format!("error decoding contract entrypoint: {}", what));
            });
            let params = hex::decode(args.params).unwrap_or_else(|what| {
                testkit::exit_with_error(format!("error decoding contract params: {}", what));
            });

            fevm::run(
                &mut tester,
                &options,
                &contract,
                &entrypoint,
                &params,
                args.gas_limit,
            )
            .unwrap_or_else(|what| {
                testkit::exit_with_error(format!(" contract execution failed: {}", what));
            });
        }

        "wasm" => {
            testkit::exit_with_error("wasm actors not supported yet".to_owned());
        }

        _ => {
            testkit::exit_with_error(format!("unknown mode {}", args.mode));
        }
    }
}
