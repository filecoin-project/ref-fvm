// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
// constants for wasm build artifacts
//
#![allow(dead_code)]

macro_rules! wasm_bin {
    ($x: expr) => {
        concat!(
            env!("OUT_DIR"),
            "/bundle/wasm32-unknown-unknown/wasm/",
            $x,
            ".wasm"
        )
    };
}

// calibration test actors
pub const GAS_CALIBRATION_ACTOR_BIN: &[u8] = include_bytes!(wasm_bin!("fil_gas_calibration_actor"));

// integration test actors
pub const READONLY_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_readonly_actor"));
pub const ADDRESS_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_address_actor"));
pub const HELLO_WORLD_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_hello_world_actor"));
pub const STACK_OVERFLOW_ACTOR_BINARY: &[u8] =
    include_bytes!(wasm_bin!("fil_stack_overflow_actor"));
pub const IPLD_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_ipld_actor"));
pub const MALFORMED_SYSCALL_ACTOR_BINARY: &[u8] =
    include_bytes!(wasm_bin!("fil_malformed_syscall_actor"));
pub const INTEGER_OVERFLOW_ACTOR_BINARY: &[u8] =
    include_bytes!(wasm_bin!("fil_integer_overflow_actor"));
pub const SYSCALL_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_syscall_actor"));
pub const EVENTS_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_events_actor"));
pub const EXIT_DATA_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_exit_data_actor"));
pub const GASLIMIT_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_gaslimit_actor"));
pub const CREATE_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_create_actor"));
pub const OOM_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_oom_actor"));
pub const SSELF_ACTOR_BINARY: &[u8] = include_bytes!(wasm_bin!("fil_sself_actor"));
