// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub mod cidjson;
pub mod driver;
pub mod externs;
pub mod rand;
pub mod vector;
pub mod vm;

// Output the result to stdout.
// Doing this here instead of in an inspect so that we get streaming output.
#[macro_export]
macro_rules! report {
    ($status:expr, $path:expr, $id:expr) => {
        println!("[{}] vector: {} | variant: {}", $status, $path, $id);
    };
}
