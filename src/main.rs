mod bundle;
mod fevm;

use clap::Parser;
use fvm_ipld_blockstore::MemoryBlockstore;
use std::fs;
use hex;

/// Run a contract invocation for benchmarking purposes
#[derive(Parser, Debug)]
struct Args {
    /// Execution mode: wasm or fevm
    #[arg(short, long, default_value = "fevm")]
    mode: String,

    /// Builtin actors bundle to use.
    #[arg(short, long)]
    bundle: String,

    /// Contract file.
    contract: String,

    /// Invocation method; solidity entry point for fevm, actor method for wasm.
    method: String,

    /// Invocation parameters, in hex.
    params: String,
}

fn main() {
    let args = Args::parse();
    let mut blockstore = MemoryBlockstore::default();
    let bundle_cid = match bundle::import_bundle(&mut blockstore, args.bundle.as_str()) {
        Ok(cid) => cid,
        Err(what) => {
            exit_with_error(format!("error loading bundle: {}", what));
        }
    };

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
                exit_with_error(format!("error decoding contract entrypoint: {}", what));
            });

            fevm::run(bundle_cid, &contract, &entrypoint, &params).unwrap_or_else(|what| {
                exit_with_error(format!(" contract execution failed: {}", what));
            });
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
