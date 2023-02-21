// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Placeholder invoke for testing
#[no_mangle]
#[cfg(target_arch = "wasm32")]
pub fn invoke(blk: u32) -> u32 {
    invoke_method(blk)
}

#[cfg(target_arch = "wasm32")]
fn invoke_method(_: u32) -> ! {
    use fvm_sdk as sdk;
    let method = sdk::message::method_number();

    match method {
        1 => {
            allocate_one();
        }
        2 => {
            allocate_many();
        }
        3 => {
            allocate_some();
            sdk::vm::abort(314, Some(format!("not OOM {}", method).as_str()));
        }
        _ => {
            sdk::vm::abort(
                fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
                Some(format!("bad method {}", method).as_str()),
            );
        }
    }

    sdk::vm::abort(
        fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
        Some("should have run out of memory..."),
    )
}

//  Allocate a single big chunk and keep resizing until OOm
#[cfg(target_arch = "wasm32")]
fn allocate_one() {
    let mut mem = Vec::<u8>::new();
    mem.resize(1024 * 1024, 0);
    for _ in 1.. {
        let cap = mem.len();
        mem.resize(2 * cap, 0);
    }
}

// Allocate many small chunks until OOm
#[cfg(target_arch = "wasm32")]
fn allocate_many() {
    let mut chunks = Vec::<Vec<u8>>::new();
    for _ in 1.. {
        let mut chunk = Vec::<u8>::new();
        chunk.resize(1024 * 1024, 0);
        chunks.push(chunk);
    }
}

#[cfg(target_arch = "wasm32")]
fn allocate_some() {
    // 64 WASM pages
    let _ = Vec::<u8>::with_capacity(64 * 65536);
}
