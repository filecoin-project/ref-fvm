// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines, Write};
use std::path::{Path, PathBuf};
use std::{env, process};

use anyhow::anyhow;
use fvm_conformance_tests::tracing::TestGasCharge;
use fvm_ipld_encoding::de::DeserializeOwned;
use serde::Serialize;

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = Config::build(args).unwrap_or_else(|err| {
        println!("Invalid args: {err}");
        process::exit(1)
    });

    if let Err(err) = run(&config) {
        println!("Error running with {config:?}: ${err}");
        process::exit(1)
    }
}

#[derive(Debug)]
struct Config {
    /// Path to the JSON file with time and gas aggregated to test vector level.
    file_in: PathBuf,
    /// Path to write the JSON file with regression results.
    file_out: PathBuf,
}

impl Config {
    pub fn build(args: Vec<String>) -> anyhow::Result<Self> {
        if args.len() != 3 {
            return Err(anyhow!("Expected 2 arguments; got {}", args.len()));
        }

        let config = Self {
            file_in: PathBuf::from(&args[1]),
            file_out: PathBuf::from(&args[2]),
        };

        Ok(config)
    }
}

struct Obs {
    elapsed_nanos: f64,
    compute_gas: f64,
}

#[derive(Serialize)]
struct RegressionResult {
    name: String,
    intercept: f64,
    slope: f64,
    r_squared: f64,
}

fn run(config: &Config) -> anyhow::Result<()> {
    let charges = import_json(&config.file_in)?;

    let mut results = Vec::new();
    for (name, charges) in group_charges(charges).into_iter() {
        results.push(least_squares(name, charges));
    }
    results.sort_by(|a, b| a.name.cmp(&b.name));

    export_json(&config.file_out, results)?;

    Ok(())
}

fn group_charges(charges: Vec<TestGasCharge>) -> HashMap<String, Vec<Obs>> {
    let mut groups = HashMap::new();
    for charge in charges {
        if let Some(elapsed_nanos) = charge.elapsed_nanos {
            let group: &mut Vec<Obs> = groups.entry(charge.name).or_default();
            group.push(Obs {
                elapsed_nanos: elapsed_nanos as f64,
                compute_gas: charge.compute_gas as f64,
            });
        }
    }
    groups
}

fn read_lines<P>(filename: P) -> anyhow::Result<Lines<BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}

fn import_json<T: DeserializeOwned>(path: &PathBuf) -> anyhow::Result<Vec<T>> {
    let lines = read_lines(path)?;
    let mut values = Vec::new();
    for line in lines {
        let line = line?;
        let value = serde_json::from_str::<T>(&line)?;
        values.push(value)
    }
    Ok(values)
}

fn export_json<T: Serialize>(path: &PathBuf, values: Vec<T>) -> anyhow::Result<()> {
    let mut output = File::create(path)?;
    for value in values {
        let line = serde_json::to_string(&value).unwrap();
        writeln!(&mut output, "{}", line)?;
    }
    Ok(())
}

// https://www.mathsisfun.com/data/least-squares-regression.html
fn least_squares(name: String, charges: Vec<Obs>) -> RegressionResult {
    let mut sum_x = 0f64;
    let mut sum_y = 0f64;
    let mut sum_x2 = 0f64;
    let mut sum_xy = 0f64;
    let n = charges.len() as f64;

    for charge in charges.iter() {
        sum_y += charge.compute_gas;
        sum_x += charge.elapsed_nanos;
        sum_x2 += charge.elapsed_nanos * charge.elapsed_nanos;
        sum_xy += charge.elapsed_nanos * charge.compute_gas;
    }

    let m: f64 = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let b: f64 = (sum_y - m * sum_x) / n;

    // R2 = 1 - RSS/TSS
    // RSS = sum of squares of residuals
    // TSS = total sum of squares
    let mean_y = sum_y / n;
    let mut tss = 0f64;
    let mut rss = 0f64;

    for charge in charges.iter() {
        let f = m * charge.elapsed_nanos + b;
        let e = charge.compute_gas - f;
        rss += e * e;

        let e = charge.compute_gas - mean_y;
        tss += e * e;
    }
    let r_squared = 1.0 - rss / tss;

    RegressionResult {
        name,
        intercept: b,
        slope: m,
        r_squared,
    }
}
