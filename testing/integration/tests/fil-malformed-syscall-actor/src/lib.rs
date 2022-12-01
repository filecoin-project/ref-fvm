// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[no_mangle]
#[cfg(target_arch = "wasm32")]
pub fn invoke(_: u32) -> u32 {
    use fvm_sdk::sys::crypto::compute_unsealed_sector_cid;

    let piece: Vec<u8> = vec![];
    let mut cid: Vec<u8> = vec![];

    // Should fail for unknown proof type
    unsafe {
        compute_unsealed_sector_cid(100000, piece.as_ptr(), 100000, cid.as_mut_ptr(), 100000)
            .expect("");
    }

    0
}
