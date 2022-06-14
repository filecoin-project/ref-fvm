use std::path::{self, PathBuf};

use crate::kernel::{ClassifyResult, Result};
use crate::syscalls::context::Context;
use crate::Kernel;

const ENV_ARTIFACT_DIR: &str = "FVM_STORE_ARTIFACT_DIR";

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

// TODO: scope artifacts into subdirectories
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

    // Ensure well formed artifact name
    {
        if name.len() > 256 {
            Err("debug artifact name should not exceed 256 bytes")
        } else if name.chars().any(path::is_separator) {
            Err("debug artifact name should not include any path separators")
        } else if name
            .chars()
            .next()
            .ok_or("debug artifact name should be at least one character")
            .or_error(fvm_shared::error::ErrorNumber::IllegalArgument)?
            == '.'
        {
            Err("debug artifact name should not start with a decimal '.'")
        } else {
            Ok(())
        }
    }
    .or_error(fvm_shared::error::ErrorNumber::IllegalArgument)?;

    if let Ok(dir) = std::env::var(ENV_ARTIFACT_DIR) {
        let dir = PathBuf::from(dir);
        if let Err(e) = std::fs::create_dir_all(dir.clone()) {
            log::error!("failed to make directory to store debug artifacts {}", e);
        } else if let Err(e) = std::fs::write(dir.join(name), data) {
            log::error!("failed to store debug artifact {}", e)
        }
        log::info!("wrote artifact: {} to {:?}", name, dir);
    } else {
        log::error!(
            "store_artifact was ignored, env var {} was not set",
            ENV_ARTIFACT_DIR
        )
    }

    Ok(())
}
