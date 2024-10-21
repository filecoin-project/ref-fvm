// SPDX-License-Identifier: Apache-2.0, MIT
use regex::Regex;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const HEADER_LINES_TO_CHECK: usize = 3;

lazy_static::lazy_static! {
    static ref PL_LICENSE: Regex = Regex::new(r"// Copyright \\d{4}-\\d{4} Protocol Labs").unwrap();
    static ref CS_LICENSE: Regex = Regex::new(r"// Copyright \\d{4}-\\d{4} ChainSafe Systems").unwrap();
    static ref SPDX_LICENSE: Regex = Regex::new(r"// SPDX-License-Identifier: Apache-2.0, MIT").unwrap();

    /// LICENSE_CHECKS is a static vector containing tuples that define license validation rules.
    /// Each tuple consists of:
    ///  - a regular expression to detect a license pattern,
    ///  - a boolean indicating whether the license is mandatory, and
    ///  - the text to add to file if license is not found.
    static ref LICENSE_CHECKS: Vec<(&'static Regex, bool, &'static str)> = vec![
        (&PL_LICENSE, false, "// Copyright 2021-2024 Protocol Labs"),
        (&CS_LICENSE, false, "// Copyright 2019-2024 ChainSafe Systems"),
        (&SPDX_LICENSE, true, "// SPDX-License-Identifier: Apache-2.0, MIT"),
    ];

    /// IGNORED_DIRECTORIES is a static set containing directories to ignore during the license check.
    static ref IGNORED_DIRECTORIES: HashSet<PathBuf> = {
        let mut set = HashSet::new();
        set.insert(PathBuf::from("./target/"));
        set
    };
}

/// This program expects the target directory as the first command-line argument. It recursively
/// checks all `.rs` files within the specified directory, skipping any directories that are listed
/// in the `IGNORED_DIRECTORIES` set. If a required license is missing, it is automatically added
/// to the beginning of the file.
fn main() {
    let target_directory = std::env::args()
        .nth(1)
        .expect("Please provide the target directory as the first argument.");

    check_and_add_license(&target_directory).expect("Error while checking files.");
}

fn check_and_add_license(directory: &str) -> io::Result<()> {
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();

        if IGNORED_DIRECTORIES
            .iter()
            .any(|ignored_dir| path.starts_with(ignored_dir))
        {
            continue;
        }

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            check_and_add_license_to_file(path)?;
        }
    }
    Ok(())
}

fn check_and_add_license_to_file(path: &Path) -> io::Result<()> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .take(HEADER_LINES_TO_CHECK)
        .filter_map(Result::ok)
        .collect();

    let mut missing_licenses: Vec<&str> = Vec::new();
    for (regex, is_required, text_to_add) in LICENSE_CHECKS.iter() {
        let mut found_match = false;
        for line in &lines {
            if regex.is_match(line) {
                found_match = true;
                break;
            }
        }

        if !found_match && *is_required {
            missing_licenses.push(text_to_add);
        }
    }

    if !missing_licenses.is_empty() {
        println!("Adding license header to: {}", path.display());

        let file_content = fs::read_to_string(path)?;
        let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;

        for license in missing_licenses {
            file.write_all(license.as_bytes())?;
            file.write_all(b"\n")?;
        }

        file.write_all(file_content.as_bytes())?;
    }

    Ok(())
}
