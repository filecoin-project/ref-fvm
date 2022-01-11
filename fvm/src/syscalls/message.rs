use anyhow::Context as _;
use fvm_shared::sys;

use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};

pub fn caller(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.msg_caller())
}

pub fn receiver(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.msg_receiver())
}

pub fn method_number(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.msg_method_number())
}

pub fn value_received(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    context
        .kernel
        .msg_value_received()
        .try_into()
        .context("invalid token amount")
        .or_fatal()
}
