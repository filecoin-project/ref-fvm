use std::io::Write;
use std::path::PathBuf;

use ethers_core::types::Bytes;
use ethers_solc::artifacts::output_selection::OutputSelection;
use ethers_solc::artifacts::Settings;
use ethers_solc::{Project, ProjectPathsConfig, SolcConfig};
use serde::Serialize;

/// Compile all Solidity contracts and put the outputs into the `artifacts` directory.
/// The contracts are used in integration testing.
///
/// It requires `solc` to be installed.
///
/// This would be much easier to achieve with `make` and `solc` directly, but this way
/// we can rely purely on `cargo`.
///
/// See https://docs.rs/ethers/latest/ethers/solc/
fn main() {
    // The following will look for Solidity files in the `contracts` directory.
    let path_config = ProjectPathsConfig::hardhat(env!("CARGO_MANIFEST_DIR")).unwrap();
    let artifacts_dir = path_config.artifacts.clone();

    // Don't think we need the AST, and it's big.
    let mut solc_settings = Settings::default();
    solc_settings.output_selection = OutputSelection::default_output_selection();
    let solc_config = SolcConfig::builder().settings(solc_settings).build();

    let project = Project::builder()
        .paths(path_config)
        .solc_config(solc_config)
        .build()
        .unwrap();

    let output = project.compile().unwrap();

    // I couldn't figure out a way to make `ethers_solc` write out the extra files for us.
    // It looks like it could write the ABI files with [ArtifactOutput::write_contract_extras],
    // but the [ExtraOutputFiles] used by default doesn't write bytecode, so I stopped looking.

    // NOTE: Only running on what changed. If something changes here, either delete the artifacts first to force regeneration,
    // or change the contract source, or change this line to include cached artifacts. The benefit of only working
    // on changed artifacts is that it's faster and also that it won't do anything on CI, so it shouldn't need `solc`.
    for (contract_path, artifacts) in output.compiled_artifacts() {
        assert_eq!(1, artifacts.len());

        let contract_path = PathBuf::from(contract_path);
        let artifacts_dir = artifacts_dir.join(contract_path.file_name().unwrap());

        for (contract_name, artifacts) in artifacts {
            assert_eq!(1, artifacts.len());
            let mk_path = |ext: &str| artifacts_dir.join(format!("{contract_name}.{ext}"));
            let artifact = &artifacts[0].artifact;

            // Export the bytecode as hex so we can load it into FEVM.
            let bytecode = artifact
                .bytecode
                .as_ref()
                .expect("Bytecode is part of the default outputs");

            export_hex(&mk_path("hex"), bytecode.object.as_bytes().unwrap());

            // Export the ABI as JSON so we can use `abigen!` to generate facades.
            let abi = artifact
                .abi
                .as_ref()
                .expect("ABI is part of the default outputs");

            let abi_path = mk_path("abi");
            export_json(&abi_path, abi);
            generate_facade(&abi_path, contract_name);
        }
    }

    // Rerun this script if anything in the `contracts` change.
    project.rerun_if_sources_changed();
}

fn export_json<T: Serialize>(path: &PathBuf, value: &T) {
    let line = serde_json::to_string(&value).unwrap();
    export_str(path, &line);
}

fn export_hex(path: &PathBuf, bytes: &Bytes) {
    let line = format!("{bytes:x}");
    let line = line.trim_start_matches("0x");
    export_str(path, line);
}

fn export_str(path: &PathBuf, line: &str) {
    let mut output = std::fs::File::create(path).unwrap();
    writeln!(&mut output, "{line}").unwrap();
}

/// We can use `abigen!` in the code to create a contract facade on the fly like this:
///
/// ```ignore
/// abigen!(SimpleCoin, "./artifacts/SimpleCoin.sol/SimpleCoin.abi");
/// ```
///
/// However, it doesn't work well with Rust Analyzer (at least for me), often showing `{unknown}`
/// where you'd expect code completion.
///
/// Instead of that, we can actually generate all the Rust code and check it into git,
/// which makes it easier to see what's going on and works better in the editor as well.
fn generate_facade(abi_path: &PathBuf, contract_name: &str) {
    ethers::prelude::Abigen::new(contract_name, abi_path.to_string_lossy())
        .unwrap()
        .generate()
        .unwrap()
        .write_to_file(format!("./src/{}.rs", camel_to_snake(contract_name)))
        .unwrap();
}

/// Convert ContractName to contract_name so we can use it as a Rust module.
///
/// We could just lowercase, but this is what `Abigen` does as well, and it's more readable with complex names.
fn camel_to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        match (i, c) {
            (0, c) if c.is_uppercase() => {
                out.push(c.to_ascii_lowercase());
            }
            (_, c) if c.is_uppercase() => {
                out.push('_');
                out.push(c.to_ascii_lowercase());
            }
            (_, c) => {
                out.push(c);
            }
        }
    }
    out
}
