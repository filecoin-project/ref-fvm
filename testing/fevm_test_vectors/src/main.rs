use std::fs::File;
use std::io::BufReader;
use std::iter;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use async_std::task::block_on;
use clap::{Parser, Subcommand};
use colored::Colorize;
use conformance::driver::is_runnable;
use conformance::report;
use conformance::vector::MessageVector;
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use fevm_test_vectors::extractor::transaction::{extract_eth_transaction_test_vector_from_tx, extract_eth_transaction_test_vector_from_tx_hash, get_most_recent_transactions_of_contracts};
use fevm_test_vectors::extractor::types::EthTransactionTestVector;
use fevm_test_vectors::{consume_test_vector, export_test_vector_file, init_log};
use fvm::engine::MultiEngine;
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
    Consume(Consume),
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

    #[clap(long)]
    tag: Option<String>,
}

#[derive(Debug, Parser)]
#[clap(about = "Batch generate from contract address.", long_about = None)]
pub struct Batch {
    #[clap(short, long)]
    geth_rpc_endpoint: String,

    /// multiple contract addresses, such as: 0x1F98431c8aD98523631AE4a59f267346ea31F984,0x5BA1e12693Dc8F9c48aAD8770482f4739bEeD696
    #[clap(short, long)]
    contracts: String,

    #[clap(short, long)]
    tx_num: usize,

    #[clap(short, long)]
    max_block_num: usize,

    /// test vector output dir path
    #[clap(short, long)]
    out_dir: String,

    #[clap(long)]
    tag: Option<String>,
}

#[derive(Debug, Parser)]
#[clap(about = "Rebuild test vector from input.", long_about = None)]
pub struct Rebuild {
    /// test vector input file/dir path
    #[clap(short, long)]
    input: String,
}

#[derive(Debug, Parser)]
#[clap(about = "Comsume test vectors from input", long_about = None)]
struct Consume {
    /// test vector input file/dir path
    #[clap(short, long)]
    input: String,

    #[clap(short, long)]
    out: String,
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
            let evm_input =
                extract_eth_transaction_test_vector_from_tx_hash(&provider, tx_hash, config.tag).await?;
            let path = out_dir.join(format!("{}.json", config.tx_hash));
            block_on(export_test_vector_file(evm_input, path))?;
        }
        SubCommand::Batch(config) => {
            if config.tag.is_some() {
                println!("------ {:?} ------", config.tag.clone().unwrap())
            }
            let out_dir = Path::new(&config.out_dir);
            assert!(out_dir.is_dir(), "out_dir must directory");
            let provider = Provider::<Http>::try_from(config.geth_rpc_endpoint)
                .expect("could not instantiate HTTP Provider");
            let contracts = config.contracts.split(",");
            let contracts = contracts.into_iter()
                .filter(|e| e.trim().len() > 0)
                .map(|e| H160::from_str(&*(e.trim())).expect("contract format error")
                ).collect::<Vec<H160>>();

            let res = block_on(get_most_recent_transactions_of_contracts(&provider, contracts, config.tx_num, config.max_block_num))?;
            for (contract, txs) in res {
                let contract_dir = out_dir.join(contract.encode_hex());
                if !contract_dir.exists() {
                    std::fs::create_dir(contract_dir.clone())?;
                }
                for tx in txs {
                    let path = contract_dir.join(format!("{}.json", tx.hash.encode_hex()));
                    let evm_input = extract_eth_transaction_test_vector_from_tx(&provider, tx, config.tag.clone()).await?;
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
                            "FILE PARSING FAIL".white().on_purple(),
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
        SubCommand::Consume(config) => {
            consume_test_vectors(config.input.as_str(), config.out.as_str())
        }
    }
    Ok(())
}

pub fn consume_test_vectors(input: &str, output: &str) {
    let input_path = Path::new(input);
    let vector_results: Vec<PathBuf> = if input_path.is_file() {
        iter::once(Path::new(input).to_path_buf()).collect()
    } else {
        WalkDir::new(input)
            .into_iter()
            .flat_map(|e| e.ok())
            .filter(is_runnable)
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    let output_csv = Path::new(output);
    let output_csv = File::create(output_csv).unwrap();
    let mut output_csv = csv::Writer::from_writer(output_csv);

    let engines = MultiEngine::default();
    for vector_path in vector_results.into_iter() {
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
                    "FILE PARSING FAIL".white().on_purple(),
                    &vector_path.display().to_string(),
                    "n/a"
                );
                println!("\t|> reason: {:#}", e);
                continue;
            }
        };
        let testresults = consume_test_vector(
            &message_vector,
            &vector_path.display().to_string(),
            &engines,
        )
        .unwrap();

        for testresult in testresults.into_iter() {
            output_csv.serialize(testresult).unwrap();
        }
    }
    output_csv.flush().unwrap();
}
