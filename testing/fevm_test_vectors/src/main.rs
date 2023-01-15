use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use async_std::task::block_on;
use clap::{Parser, Subcommand};
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use fevm_test_vectors::extractor::transaction::extract_eth_transaction_test_vector;
use fevm_test_vectors::extractor::types::EthTransactionTestVector;
use fevm_test_vectors::{export_test_vector_file, init_log};
use walkdir::{DirEntry, WalkDir};

#[derive(Parser, Debug)]
pub struct Cli {
    #[clap(subcommand)]
    cmd: SubCommand,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    Generate(Generate),
}

#[derive(Debug, Parser)]
#[clap(about = "Generate test vector from geth rpc directly.", long_about = None)]
pub struct Generate {
    #[clap(short, long)]
    geth_rpc_endpoint: String,

    /// eth transaction hash
    #[clap(short, long)]
    tx_hash: String,

    /// test vector output dir path
    #[clap(short, long)]
    out_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_log();
    let cli = Cli::parse();
    match cli.cmd {
        SubCommand::Generate(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let tx_hash = H256::from_str(&*config.tx_hash)?;
            let provider = Provider::<Http>::try_from(config.geth_rpc_endpoint)
                .expect("could not instantiate HTTP Provider");
            let evm_input = extract_eth_transaction_test_vector(&provider, tx_hash).await?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            block_on(export_test_vector_file(evm_input, path))?;
        }
    }
    Ok(())
}
