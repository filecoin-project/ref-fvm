// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::CBOR;
use fvm_sdk as sdk;

/// Placeholder invoke for testing
#[no_mangle]
#[cfg(target_arch = "wasm32")]
pub fn invoke(blk: u32) -> u32 {
    invoke_method(blk)
}

#[allow(dead_code)]
fn invoke_method(_: u32) -> ! {
    let method = sdk::message::method_number();
    let exit_code = match method {
        0 | 1 | 2 => 0,
        _ => 0x42,
    };

    sdk::vm::exit(
        exit_code,
        Some(IpldBlock {
            codec: CBOR,
            data: vec![1u8, 2u8, 3u8, 3u8, 7u8],
        }),
        None,
    )
}
