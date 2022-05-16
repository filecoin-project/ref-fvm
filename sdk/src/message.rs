use std::convert::TryInto;

use fvm_ipld_encoding::DAG_CBOR;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sys::{BlockId, Codec};
use fvm_shared::{ActorID, MethodNum};

use crate::vm::INVOCATION_CONTEXT;
use crate::{sys, SyscallResult, NO_DATA_BLOCK_ID};

/// Returns the ID address of the caller.
#[inline(always)]
pub fn caller() -> ActorID {
    INVOCATION_CONTEXT.caller
}

/// Returns the ID address of the actor.
#[inline(always)]
pub fn receiver() -> ActorID {
    INVOCATION_CONTEXT.receiver
}

/// Returns the message's method number.
#[inline(always)]
pub fn method_number() -> MethodNum {
    INVOCATION_CONTEXT.method_number
}

/// Returns the value received from the caller in AttoFIL.
#[inline(always)]
pub fn value_received() -> TokenAmount {
    INVOCATION_CONTEXT
        .value_received
        .try_into()
        .expect("invalid bigint")
}

/// Returns the message codec and parameters.
pub fn params_raw(id: BlockId) -> SyscallResult<(Codec, Vec<u8>)> {
    if id == NO_DATA_BLOCK_ID {
        return Ok((DAG_CBOR, Vec::default())); // DAG_CBOR is a lie, but we have no nil codec.
    }
    unsafe {
        let fvm_shared::sys::out::ipld::IpldStat { codec, size } = sys::ipld::block_stat(id)?;
        Ok((codec, crate::ipld::get_block(id, Some(size))?))
    }
}
