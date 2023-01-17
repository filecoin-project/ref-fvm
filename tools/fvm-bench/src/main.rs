mod bundle;
mod fevm;

use clap::Parser;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::Tester;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use std::fs;

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

pub struct Options {
    pub debug: bool,
    pub trace: bool,
    pub events: bool,
    pub gas_limit: i64,
}

fn main() {
    let args = Args::parse();
    let options = Options {
        debug: args.debug,
        trace: args.trace,
        events: args.events,
        gas_limit: args.gas_limit,
    };
    let blockstore = MemoryBlockstore::default();
    let bundle_cid = match bundle::import_bundle(&blockstore, args.bundle.as_str()) {
        Ok(cid) => cid,
        Err(what) => {
            exit_with_error(format!("error loading bundle: {}", what));
        }
    };
    let mut tester: Tester<MemoryBlockstore, DummyExterns> = Tester::new(
        NetworkVersion::V18,  // TODO make this a program argument
        StateTreeVersion::V5, // TODO infer this from network version
        bundle_cid,
        blockstore,
    )
    .unwrap_or_else(|what| {
        exit_with_error(format!("error creating execution framework: {}", what));
    });

    match args.mode.as_str() {
        "fevm" => {
            let contract_hex = fs::read_to_string(args.contract).unwrap_or_else(|what| {
                exit_with_error(format!("error reading contract: {}", what));
            });
            let contract = hex::decode(contract_hex).unwrap_or_else(|what| {
                exit_with_error(format!("error decoding contract: {}", what));
            });

            let entrypoint = hex::decode(args.method).unwrap_or_else(|what| {
                exit_with_error(format!("error decoding contract entrypoint: {}", what));
            });
            let params = hex::decode(args.params).unwrap_or_else(|what| {
                exit_with_error(format!("error decoding contract params: {}", what));
            });

            fevm::run(&mut tester, &options, &contract, &entrypoint, &params, options.gas_limit).unwrap_or_else(
                |what| {
                    exit_with_error(format!(" contract execution failed: {}", what));
                },
            );
        }

        "wasm" => {
            exit_with_error("wasm actors not supported yet".to_owned());
        }

        _ => {
            exit_with_error(format!("unknown mode {}", args.mode));
        }
    }
}

fn exit_with_error(msg: String) -> ! {
    println!("{}", msg);
    std::process::exit(1);
}
