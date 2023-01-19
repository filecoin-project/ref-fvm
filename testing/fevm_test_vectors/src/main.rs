use std::fs::File;
use std::io::BufReader;
use std::iter;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use async_std::task::block_on;
use clap::{Parser, Subcommand};
use colored::Colorize;
use conformance::report;
use conformance::vector::MessageVector;
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use fevm_test_vectors::extractor::transaction::{extract_eth_transaction_test_vector_from_tx, extract_eth_transaction_test_vector_from_tx_hash, get_most_recent_transactions_of_contracts};
use fevm_test_vectors::extractor::types::EthTransactionTestVector;
use fevm_test_vectors::{export_test_vector_file, init_log};
use walkdir::{DirEntry, WalkDir};
use crate::abi::AbiEncode;

#[derive(Parser, Debug)]
pub struct Cli {
    #[clap(subcommand)]
    cmd: SubCommand,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    Generate(Generate),
    Batch(Batch),
    Rebuild(Rebuild),
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

#[derive(Debug, Parser)]
#[clap(about = "Batch generate from contract address.", long_about = None)]
pub struct Batch {
    #[clap(short, long)]
    geth_rpc_endpoint: String,

    #[clap(long, multiple_values=true)]
    contracts: Vec<String>,

    #[clap(short, long)]
    tx_num: usize,

    #[clap(short, long)]
    furthest_block_num: Option<u64>,

    /// test vector output dir path
    #[clap(short, long)]
    out_dir: String,
}

#[derive(Debug, Parser)]
#[clap(about = "Rebuild test vector from input.", long_about = None)]
pub struct Rebuild {
    /// test vector input file/dir path
    #[clap(short, long)]
    input: String,
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
            let evm_input = extract_eth_transaction_test_vector_from_tx_hash(&provider, tx_hash).await?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            block_on(export_test_vector_file(evm_input, path))?;
        }
        SubCommand::Batch(config) => {
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let provider = Provider::<Http>::try_from(config.geth_rpc_endpoint)
                .expect("could not instantiate HTTP Provider");
            let contracts = config.contracts.into_iter().map(|e| H160::from_str(&*e).expect("contract format error")).collect::<Vec<H160>>();
            let furthest_block_num = match config.furthest_block_num {
                Some(e) => Some(provider.get_block_number().await? - U64::from(e)),
                None => None
            };
            let res = block_on(get_most_recent_transactions_of_contracts(&provider, contracts, config.tx_num, furthest_block_num))?;
            for (contract, txs) in res {
                let contract_dir = out_dir.join(contract.encode_hex());
                if !contract_dir.exists() {
                    std::fs::create_dir(contract_dir.clone())?;
                }
                for tx in txs {
                    let path = contract_dir.join(format!("{}.json", tx.hash.encode_hex()));
                    let evm_input = extract_eth_transaction_test_vector_from_tx(&provider, tx).await?;
                    block_on(export_test_vector_file(evm_input, path))?;
                }
            }
        }
        SubCommand::Rebuild(config) => {
            let input = Path::new(&config.input);
            let vector_results: Vec<PathBuf> = if input.is_dir() {
                WalkDir::new(input)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(is_runnable)
                    .map(|e| e.path().to_path_buf())
                    .collect()
            } else {
                iter::once(Path::new(input).to_path_buf()).collect()
            };
            for vector_path in vector_results {
                let message_vector = match MessageVector::from_file(&vector_path) {
                    Ok(mv) => {
                        if !mv.is_supported() {
                            report!(
                                "SKIPPING FILE DUE TO SELECTOR".on_yellow(),
                                &vector_path.display().to_string(),
                                "n/a"
                            );
                            continue;
                        }
                        mv
                    }
                    Err(e) => {
                        report!(
                            "FILE PARSING FAIL/NOT BENCHED".white().on_purple(),
                            &vector_path.display().to_string(),
                            "n/a"
                        );
                        println!("\t|> reason: {:#}", e);
                        continue;
                    }
                };
                if message_vector.meta.is_none() {
                    report!(
                        "META IS NONE".white().on_purple(),
                        &vector_path.display().to_string(),
                        "n/a"
                    );
                    continue;
                }
                let evm_input = match serde_json::from_str::<EthTransactionTestVector>(
                    &message_vector.meta.unwrap()._debug,
                ) {
                    Ok(e) => e,
                    Err(e) => {
                        report!(
                            e.to_string().red().on_purple(),
                            &vector_path.display().to_string(),
                            "n/a"
                        );
                        continue;
                    }
                };
                block_on(export_test_vector_file(evm_input, vector_path))?;
            }
        }
    }
    Ok(())
}

pub fn is_runnable(entry: &DirEntry) -> bool {
    let file_name = match entry.path().to_str() {
        Some(file) => file,
        None => return false,
    };

    file_name.ends_with(".json")
}
