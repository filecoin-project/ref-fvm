// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;

const ACTORS: &[(&str, &str)] = &[
    // calibration test actors
    ("GAS_CALIBRATION_ACTOR_BINARY", "fil_gas_calibration_actor"),
    // integration test
    ("HELLO_WORLD_ACTOR_BINARY", "fil_hello_world_actor"),
    ("STACK_OVERFLOW_ACTOR_BINARY", "fil_stack_overflow_actor"),
    ("IPLD_ACTOR_BINARY", "fil_ipld_actor"),
    (
        "MALFORMED_SYSCALL_ACTOR_BINARY",
        "fil_malformed_syscall_actor",
    ),
    (
        "INTEGER_OVERFLOW_ACTOR_BINARY",
        "fil_integer_overflow_actor",
    ),
    ("SYSCALL_ACTOR_BINARY", "fil_syscall_actor"),
    ("ADDRESS_ACTOR_BINARY", "fil_address_actor"),
    ("EVENTS_ACTOR_BINARY", "fil_events_actor"),
    ("EXIT_DATA_ACTOR_BINARY", "fil_exit_data_actor"),
    ("GASLIMIT_ACTOR_BINARY", "fil_gaslimit_actor"),
    ("READONLY_ACTOR_BINARY", "fil_readonly_actor"),
    ("CREATE_ACTOR_BINARY", "fil_create_actor"),
    ("OOM_ACTOR_BINARY", "fil_oom_actor"),
    ("SSELF_ACTOR_BINARY", "fil_sself_actor"),
    ("UPGRADE_ACTOR_BINARY", "fil_upgrade_actor"),
    ("UPGRADE_RECEIVE_ACTOR_BINARY", "fil_upgrade_receive_actor"),
    ("CUSTOM_SYSCALL_ACTOR_BINARY", "fil_custom_syscall_actor"),
];

const WASM_TARGET: &str = "wasm32-unknown-unknown";

fn main() -> Result<(), Box<dyn Error>> {
    // Cargo executable location.
    let cargo = std::env::var_os("CARGO").expect("no CARGO env var");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("no OUT_DIR env var"));
    let bundle_dir = out_dir.join("bundle");
    println!("cargo:warning=bundle_dir: {:?}", &bundle_dir);

    let manifest_path =
        Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR unset"))
            .join("Cargo.toml");

    for file in ["Cargo.toml", "src", "actors"] {
        println!("cargo:rerun-if-changed={}", file);
    }

    // Cargo build command for all actors at once.
    let mut cmd = Command::new(cargo.clone());
    cmd.arg("build")
        .args(ACTORS.iter().map(|(_, pkg)| "-p=".to_owned() + pkg))
        .arg(format!("--target={WASM_TARGET}"))
        .arg("--profile=wasm")
        .arg("--locked")
        .arg("--manifest-path=".to_owned() + manifest_path.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // We are supposed to only generate artifacts under OUT_DIR,
        // so set OUT_DIR as the target directory for this build.
        .env("CARGO_TARGET_DIR", &bundle_dir)
        // As we are being called inside a build-script, this env variable is set. However, we set
        // our own `RUSTFLAGS` and thus, we need to remove this. Otherwise cargo favors this
        // env variable.
        .env_remove("CARGO_ENCODED_RUSTFLAGS");

    // Print out the command line we're about to run.
    println!("cargo:warning=cmd={:?}", &cmd);

    // Launch the command.
    let child = cmd.spawn().expect("failed to launch cargo build");
    let result = wait_cmd_and_print_output(child)?;
    if !result.success() {
        return Err("actor build failed".into());
    }

    let wasm_bin_file =
        File::create(out_dir.join("wasm_bin.rs")).expect("failed to create manifest");
    let mut wasm_bin_file = BufWriter::new(wasm_bin_file);
    for (var, pkg) in ACTORS {
        let bin = bundle_dir
            .join(WASM_TARGET)
            .join("wasm")
            .join(format!("{pkg}.wasm"));
        let moved_bin = bundle_dir.join(format!("{var}.wasm"));
        std::fs::rename(bin, &moved_bin).unwrap();
        writeln!(
            &mut wasm_bin_file,
            "pub const {var}: &[u8] = include_bytes!({moved_bin:?});"
        )
        .expect("failed to write to manifest");
    }

    // Generate syscall actor with verify-signature feature.
    {
        let (var, pkg) = ("SYSCALL_ACTOR_BINARY_FIP0079", "fil_syscall_actor");
        let mut cmd = Command::new(cargo);
        cmd.arg("build")
            .arg(format!("-p={pkg}"))
            .arg("--features=verify-signature")
            .arg(format!("--target={WASM_TARGET}"))
            .arg("--profile=wasm")
            .arg("--locked")
            .arg("--manifest-path=".to_owned() + manifest_path.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // We are supposed to only generate artifacts under OUT_DIR,
            // so set OUT_DIR as the target directory for this build.
            .env("CARGO_TARGET_DIR", &bundle_dir)
            // As we are being called inside a build-script, this env variable is set. However, we set
            // our own `RUSTFLAGS` and thus, we need to remove this. Otherwise cargo favors this
            // env variable.
            .env_remove("CARGO_ENCODED_RUSTFLAGS");

        // Print out the command line we're about to run.
        println!("cargo:warning=cmd={:?}", &cmd);

        // Launch the command.
        let child = cmd.spawn().expect("failed to launch cargo build");
        let result = wait_cmd_and_print_output(child)?;
        if !result.success() {
            return Err("actor build failed".into());
        }
        let bin = bundle_dir
            .join(WASM_TARGET)
            .join("wasm")
            .join(format!("{pkg}.wasm"));
        let moved_bin = bundle_dir.join(format!("{var}.wasm"));
        std::fs::rename(bin, &moved_bin).unwrap();
        writeln!(
            &mut wasm_bin_file,
            "pub const {var}: &[u8] = include_bytes!({moved_bin:?});"
        )
        .expect("failed to write to manifest");
    }

    wasm_bin_file.flush().expect("failed to flush manifest");
    Ok(())
}

fn wait_cmd_and_print_output(mut child: Child) -> Result<ExitStatus, Box<dyn Error>> {
    // Pipe the output as cargo warnings. Unfortunately this is the only way to
    // get cargo build to print the output.
    let stdout = child.stdout.take().expect("no stdout");
    let stderr = child.stderr.take().expect("no stderr");
    let j1 = thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            println!("cargo:warning={:?}", line.unwrap());
        }
    });
    let j2 = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            println!("cargo:warning={:?}", line.unwrap());
        }
    });

    j1.join().unwrap();
    j2.join().unwrap();

    let result = child.wait().expect("failed to wait for build to finish");
    Ok(result)
}
