// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::convert::TryInto;

use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::sys::SendFlags;
use fvm_shared::{MethodNum, Response};

use crate::{build_response, sys, SyscallResult, NO_DATA_BLOCK_ID};

/// Sends a message to another actor.
pub fn send(
    to: &Address,
    method: MethodNum,
    params: Option<IpldBlock>,
    value: TokenAmount,
    gas_limit: Option<u64>,
    flags: SendFlags,
) -> SyscallResult<Response> {
    let recipient = to.to_bytes();
    let value: sys::TokenAmount = value
        .try_into()
        .map_err(|_| ErrorNumber::InsufficientFunds)?;
    unsafe {
        // Insert parameters as a block. Nil parameters is represented as the
        // NO_DATA_BLOCK_ID block ID in the FFI interface.
        let params_id = match params {
            Some(p) => sys::ipld::block_create(p.codec, p.data.as_ptr(), p.data.len() as u32)?,
            None => NO_DATA_BLOCK_ID,
        };

        // Perform the syscall to send the message.
        let send = sys::send::send(
            recipient.as_ptr(),
            recipient.len() as u32,
            method,
            params_id,
            value.hi,
            value.lo,
            gas_limit.unwrap_or(u64::MAX),
            flags,
        )?;

        build_response(send)
    }
}
