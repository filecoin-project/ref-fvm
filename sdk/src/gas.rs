// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::sys;

/// Charge gas for the operation identified by name.
pub fn charge(name: &str, compute: u64) {
    unsafe { sys::gas::charge(name.as_ptr(), name.len() as u32, compute) }
        // can only happen if name isn't utf8, memory corruption, etc.
        .expect("failed to charge gas")
}

pub fn available() -> u64 {
    unsafe { sys::gas::available() }.expect("failed to check available gas")
}
