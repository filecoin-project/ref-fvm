// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::Context as _;
use fvm_shared::sys;
use fvm_shared::sys::out::network::NetworkContext;

use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};

/// Returns the network circ supply split as two u64 ordered in little endian.
pub fn total_fil_circ_supply(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    context
        .kernel
        .total_fil_circ_supply()?
        .try_into()
        .context("circulating supply exceeds u128 limit")
        .or_fatal()
}

pub fn context(context: Context<'_, impl Kernel>) -> crate::kernel::Result<NetworkContext> {
    context.kernel.network_context()
}

pub fn tipset_cid(
    context: Context<'_, impl Kernel>,
    epoch: i64,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let cid = context.kernel.tipset_cid(epoch)?;
    context.memory.write_cid(&cid, obuf_off, obuf_len)
}
