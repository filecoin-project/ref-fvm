// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::convert::TryInto;

use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sys::out::vm::MessageContext;
use fvm_shared::sys::BlockId;
use fvm_shared::{ActorID, MethodNum};

use crate::{sys, SyscallResult, NO_DATA_BLOCK_ID};

lazy_static::lazy_static! {
    pub(crate) static ref MESSAGE_CONTEXT: MessageContext = {
        unsafe {
            sys::vm::message_context().expect("failed to lookup message context")
        }
    };
}

/// Returns the nonce from the (explicit) message.
#[inline(always)]
pub fn nonce() -> u64 {
    MESSAGE_CONTEXT.nonce
}

/// Returns the ID address of the caller.
#[inline(always)]
pub fn caller() -> ActorID {
    MESSAGE_CONTEXT.caller
}

/// Returns the ID address of the origin
#[inline(always)]
pub fn origin() -> ActorID {
    MESSAGE_CONTEXT.origin
}

/// Returns the ID address of the actor.
#[inline(always)]
pub fn receiver() -> ActorID {
    MESSAGE_CONTEXT.receiver
}

/// Returns the message's method number.
#[inline(always)]
pub fn method_number() -> MethodNum {
    MESSAGE_CONTEXT.method_number
}

/// Returns the value received from the caller in AttoFIL.
#[inline(always)]
pub fn value_received() -> TokenAmount {
    MESSAGE_CONTEXT
        .value_received
        .try_into()
        .expect("invalid bigint")
}

/// Returns the execution gas premium
pub fn gas_premium() -> TokenAmount {
    MESSAGE_CONTEXT
        .gas_premium
        .try_into()
        .expect("invalid bigint")
}

/// Returns the message parameters as an Option<IpldBlock>.
pub fn params_raw(id: BlockId) -> SyscallResult<Option<IpldBlock>> {
    if id == NO_DATA_BLOCK_ID {
        return Ok(None);
    }
    unsafe {
        let fvm_shared::sys::out::ipld::IpldStat { codec, size } = sys::ipld::block_stat(id)?;
        Ok(Some(IpldBlock {
            codec,
            data: crate::ipld::get_block(id, Some(size))?,
        }))
    }
}
