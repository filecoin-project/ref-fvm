// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_shared::sector::WindowPoStVerifyInfo;

use crate::kernel::FilecoinKernel;

use super::Context;
use crate::kernel::Result;

/// Verifies a window proof of spacetime.
///
/// The return i32 indicates the status code of the verification:
///  - 0: verification ok.
///  - -1: verification failed.
pub fn verify_post(
    context: Context<'_, impl FilecoinKernel>,
    info_off: u32, // WindowPoStVerifyInfo,
    info_len: u32,
) -> Result<i32> {
    let info = context
        .memory
        .read_cbor::<WindowPoStVerifyInfo>(info_off, info_len)?;
    context
        .kernel
        .verify_post(&info)
        .map(|v| if v { 0 } else { -1 })
}
