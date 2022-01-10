use crate::kernel::{ClassifyResult, Kernel, Result};
use anyhow::Context as _;
use fvm_shared::sys;

use super::Context;

pub fn curr_epoch(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.network_epoch() as u64)
}

pub fn version(context: Context<'_, impl Kernel>) -> Result<u32> {
    Ok(context.kernel.network_version() as u32)
}

/// Returns the base fee split as two u64 ordered in little endian.
pub fn base_fee(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    context
        .kernel
        .network_base_fee()
        .try_into()
        .context("base-fee exceeds u128 limit")
        .or_fatal()
}

/// Returns the network circ supply split as two u64 ordered in little endian.
pub fn total_fil_circ_supply(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    context
        .kernel
        .total_fil_circ_supply()?
        .try_into()
        .context("circulating supply exceeds u128 limit")
        .or_fatal()
}
