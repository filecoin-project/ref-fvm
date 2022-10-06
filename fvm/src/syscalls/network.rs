use anyhow::Context as _;
use fvm_shared::sys;

use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};

/// Returns the base fee split as two u64 ordered in little endian.
pub fn base_fee(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    crate::assert_validator!(context.kernel, "Validator can't fetch the base fee.");

    context
        .kernel
        .network_base_fee()
        .try_into()
        .context("base-fee exceeds u128 limit")
        .or_fatal()
}

/// Returns the network circ supply split as two u64 ordered in little endian.
pub fn total_fil_circ_supply(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    crate::assert_validator!(context.kernel, "Validator can't get total circulating FIL supply.");

    context
        .kernel
        .total_fil_circ_supply()?
        .try_into()
        .context("circulating supply exceeds u128 limit")
        .or_fatal()
}

pub fn tipset_timestamp(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.tipset_timestamp())
}

pub fn tipset_cid(
    context: Context<'_, impl Kernel>,
    epoch: i64,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    if let Some(cid) = context.kernel.tipset_cid(epoch)? {
        context.memory.write_cid(&cid, obuf_off, obuf_len)
    } else {
        Ok(0)
    }
}
