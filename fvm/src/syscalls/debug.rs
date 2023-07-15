// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::kernel::{ClassifyResult, Result, SpanId};
use crate::syscalls::context::Context;
use crate::Kernel;

pub fn log(context: Context<'_, impl Kernel>, msg_off: u32, msg_len: u32) -> Result<()> {
    // No-op if disabled.
    if !context.kernel.debug_enabled() {
        return Ok(());
    }

    let msg = context.memory.try_slice(msg_off, msg_len)?;
    let msg = String::from_utf8(msg.to_owned()).or_illegal_argument()?;
    context.kernel.log(msg);
    Ok(())
}

pub fn span_begin(
    context: Context<'_, impl Kernel>,
    label_off: u32,
    label_len: u32,
    tag_off: u32,
    tag_len: u32,
    parent: SpanId,
) -> Result<SpanId> {
    // No-op if disabled.
    if !context.kernel.debug_enabled() {
        return Ok(0);
    }

    let label = context.memory.try_slice(label_off, label_len)?;
    let label = String::from_utf8(label.to_owned()).or_illegal_argument()?;
    let tag = context.memory.try_slice(tag_off, tag_len)?;
    let tag = String::from_utf8(tag.to_owned()).or_illegal_argument()?;
    context.kernel.span_begin(label, tag, parent)
}

pub fn span_end(context: Context<'_, impl Kernel>, span: SpanId) -> Result<()> {
    // No-op if disabled.
    if context.kernel.debug_enabled() {
        context.kernel.span_end(span);
    }

    Ok(())
}

pub fn enabled(context: Context<'_, impl Kernel>) -> Result<i32> {
    Ok(if context.kernel.debug_enabled() {
        0
    } else {
        -1
    })
}

pub fn store_artifact(
    context: Context<'_, impl Kernel>,
    name_off: u32,
    name_len: u32,
    data_off: u32,
    data_len: u32,
) -> Result<()> {
    // No-op if disabled.
    if !context.kernel.debug_enabled() {
        return Ok(());
    }

    let data = context.memory.try_slice(data_off, data_len)?;
    let name = context.memory.try_slice(name_off, name_len)?;
    let name =
        std::str::from_utf8(name).or_error(fvm_shared::error::ErrorNumber::IllegalArgument)?;

    context.kernel.store_artifact(name, data)?;

    Ok(())
}
