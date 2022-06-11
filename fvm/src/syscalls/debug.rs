use std::fs::DirBuilder;

use crate::kernel::{ClassifyResult, Result};
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

pub fn enabled(context: Context<'_, impl Kernel>) -> Result<i32> {
    Ok(if context.kernel.debug_enabled() {
        0
    } else {
        -1
    })
}

// TODO: make output path more configurable, maybe add extra gaurds/limitations
pub fn capture_artifact(
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
    let src_dir = std::env::current_dir()
        .or_fatal()?
        .canonicalize()
        .or_fatal()?
        .join("artifacts");
    DirBuilder::new().recursive(true).create(src_dir.clone()).or_fatal()?;

    let data = context.memory.try_slice(data_off, data_len)?;
    let name = context.memory.try_slice(name_off, name_len)?;
    let name = String::from_utf8(name.to_owned())
        .or_error(fvm_shared::error::ErrorNumber::IllegalArgument)?;
        
    println!("writing artifact: {} to {:?}", name, src_dir);
    std::fs::write(src_dir.join(name), data).or_fatal()?;

    Ok(())
}
