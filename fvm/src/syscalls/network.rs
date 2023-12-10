// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_shared::sys::out::network::NetworkContext;

use super::Context;
use crate::kernel::{NetworkOps, Result};

pub fn context(context: Context<'_, impl NetworkOps>) -> crate::kernel::Result<NetworkContext> {
    context.kernel.network_context()
}

pub fn tipset_cid(
    context: Context<'_, impl NetworkOps>,
    epoch: i64,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let cid = context.kernel.tipset_cid(epoch)?;
    context.memory.write_cid(&cid, obuf_off, obuf_len)
}
