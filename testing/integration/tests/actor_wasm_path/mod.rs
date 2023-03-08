// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
// constants for wasm build artifacts
//
#[allow(dead_code)]
pub const READONLY_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_readonly_actor.wasm"
));
#[allow(dead_code)]
pub const ADDRESS_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_address_actor.wasm"
));
#[allow(dead_code)]
pub const HELLO_WORLD_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_hello_world_actor.wasm"
));
#[allow(dead_code)]
pub const STACK_OVERFLOW_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_stack_overflow_actor.wasm"
));
#[allow(dead_code)]
pub const IPLD_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_ipld_actor.wasm"
));
#[allow(dead_code)]
pub const MALFORMED_SYSCALL_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_malformed_syscall_actor.wasm"
));
#[allow(dead_code)]
pub const INTEGER_OVERFLOW_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_integer_overflow_actor.wasm"
));
#[allow(dead_code)]
pub const SYSCALL_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_syscall_actor.wasm"
));
#[allow(dead_code)]
pub const EVENTS_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_events_actor.wasm"
));
#[allow(dead_code)]
pub const EXIT_DATA_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_exit_data_actor.wasm"
));
#[allow(dead_code)]
pub const GASLIMIT_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_gaslimit_actor.wasm"
));
#[allow(dead_code)]
pub const CREATE_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_create_actor.wasm"
));
#[allow(dead_code)]
pub const OOM_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_oom_actor.wasm"
));
#[allow(dead_code)]
pub const SSELF_ACTOR_BINARY: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/bundle/wasm32-unknown-unknown/wasm/fil_sself_actor.wasm"
));
