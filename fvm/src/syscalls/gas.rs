// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::str;

use super::Context;
use crate::gas::Gas;
use crate::kernel::{ClassifyResult, Result};
use crate::Kernel;

// Injected during build
#[no_mangle]
extern "Rust" {
    fn set_syscall_probe(probe: &'static str) -> ();
}

pub fn charge_gas(
    context: Context<'_, impl Kernel>,
    name_off: u32,
    name_len: u32,
    compute: i64,
) -> Result<()> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.gas.charge_gas") };
    let name =
        str::from_utf8(context.memory.try_slice(name_off, name_len)?).or_illegal_argument()?;
    // Gas charges from actors are always in full gas units. We use milligas internally, so convert here.
    context.kernel.charge_gas(name, Gas::new(compute))
}

pub fn available(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.gas_available().round_down() as u64)
}
