// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![allow(clippy::slow_vector_initialization)]

/// Placeholder invoke for testing
#[unsafe(no_mangle)]
#[cfg(target_arch = "wasm32")]
pub fn invoke(_blk: u32) -> u32 {
    use fvm_sdk as sdk;
    sdk::initialize(); // gives us debug messages on panic
    let method = sdk::message::method_number();

    match method {
        1 => {
            allocate_max_plus_a_bit();
        }
        2 => {
            allocate_many();
        }
        3 => {
            allocate_some();
            sdk::vm::abort(314, Some(format!("not OOM {}", method).as_str()));
        }
        4 => {
            allocate_max();
            return 0;
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

// Allocate many small chunks until OOm
#[cfg(target_arch = "wasm32")]
fn allocate_many() {
    let mut chunks = Vec::<Vec<u8>>::new();
    for _ in 1.. {
        let mut chunk = Vec::<u8>::new();
        chunk.resize(1024 * 1024, 0);
        chunks.push(chunk);
    }
    std::hint::black_box(chunks);
}

#[cfg(target_arch = "wasm32")]
fn allocate_some() {
    // 64 WASM pages
    std::hint::black_box(Vec::<u8>::with_capacity(64 * 65536));
}

// Allocate 512MiB, minus 1MiB for the rust stack, minus a page and a bit for tables etc.
// This isn't exact, but it should bring us up to our memory limit (approximately).
const TARGET_MEM: usize = 512 * 1024 * 1024;
const BAD_ALLOC: usize = TARGET_MEM - 1024 * 1024 - 64 * 1024;
const GOOD_ALLOC: usize = BAD_ALLOC - 1024;

// Allocate a single big chunk that expands to the last page.
#[allow(unused)]
fn allocate_max() {
    let mem: Vec<u8> = vec![0u8; GOOD_ALLOC];
    let end = mem.as_ptr_range().end as usize;
    assert!(
        end > TARGET_MEM - 1024,
        "we failed to get within 1024 bytes of our memory limit"
    );
    std::hint::black_box(mem);
}

//  Allocate a single big chunk that should be just over the limit.
#[allow(unused)]
fn allocate_max_plus_a_bit() {
    let mem: Vec<u8> = vec![0u8; BAD_ALLOC];
    std::hint::black_box(mem);
}
