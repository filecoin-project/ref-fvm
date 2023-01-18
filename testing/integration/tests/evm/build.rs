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

    // NOTE: Only running on what changed.
    for (contract_path, artifacts) in output.compiled_artifacts() {
        assert_eq!(1, artifacts.len());

        let contract_path = PathBuf::from(contract_path);
        let artifacts_dir = artifacts_dir.join(contract_path.file_name().unwrap());

        for (contract_name, artifacts) in artifacts {
            assert_eq!(1, artifacts.len());
            let mk_path = |ext: &str| artifacts_dir.join(format!("{contract_name}.{ext}"));
            let artifact = &artifacts[0].artifact;

            let abi = artifact
                .abi
                .as_ref()
                .expect("ABI is part of the default outputs");

            let bytecode = artifact
                .bytecode
                .as_ref()
                .expect("Bytecode is part of the default outputs");

            export_json(&mk_path("abi"), abi);
            export_hex(&mk_path("hex"), &bytecode.object.as_bytes().unwrap());
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
