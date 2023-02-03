// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::Context as _;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sys::{self, SendFlags};

use super::Context;
use crate::gas::Gas;
use crate::kernel::{ClassifyResult, Result, SendResult};
use crate::Kernel;

/// Send a message to another actor. The result is placed as a CBOR-encoded
/// receipt in the block registry, and can be retrieved by the returned BlockId.
#[allow(clippy::too_many_arguments)]
pub fn send(
    context: Context<'_, impl Kernel>,
    recipient_off: u32,
    recipient_len: u32,
    method: u64,
    params_id: u32,
    value_hi: u64,
    value_lo: u64,
    gas_limit: u64,
    flags: u64,
) -> Result<sys::out::send::Send> {
    let recipient: Address = context.memory.read_address(recipient_off, recipient_len)?;
    let value = TokenAmount::from_atto((value_hi as u128) << 64 | value_lo as u128);

    // If that gas is u64::MAX, treat it as "all gas". Although really, this doesn't matter. Any gas
    // exceeding the current gas available is treated as "all remaining gas".
    let gas_limit = (gas_limit < u64::MAX).then(|| Gas::new(gas_limit));

    let flags = SendFlags::from_bits(flags)
        .with_context(|| format!("invalid send flags: {flags}"))
        .or_illegal_argument()?;

    // An execution error here means that something went wrong in the FVM.
    // Actor errors are communicated in the receipt.
    let SendResult {
        block_id,
        block_stat,
        exit_code,
    } = context
        .kernel
        .send(&recipient, method, params_id, &value, gas_limit, flags)?;

    Ok(sys::out::send::Send {
        exit_code: exit_code.value(),
        return_id: block_id,
        return_codec: block_stat.codec,
        return_size: block_stat.size,
    })
}
