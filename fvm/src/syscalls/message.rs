use anyhow::Context as _;
use fvm_shared::sys::out::message::MessageDetails;

use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};

pub fn details(context: Context<'_, impl Kernel>) -> Result<MessageDetails> {
    Ok(MessageDetails {
        caller: context.kernel.msg_caller(),
        receiver: context.kernel.msg_receiver(),
        method_number: context.kernel.msg_method_number(),
        value_received: context
            .kernel
            .msg_value_received()
            .try_into()
            .context("invalid token amount")
            .or_fatal()?,
        curr_epoch: context.kernel.network_epoch(),
        version: context.kernel.network_version() as u32,
        base_fee: context
            .kernel
            .network_base_fee()
            .try_into()
            .context("base-fee exceeds u128 limit")
            .or_fatal()?,
        circulating_supply: context
            .kernel
            .total_fil_circ_supply()?
            .try_into()
            .context("circulating supply exceeds u128 limit")
            .or_fatal()?,
    })
}
