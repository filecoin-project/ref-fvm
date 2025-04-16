use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sys::BlockId;
use fvm_shared::{ActorID, MethodNum};

use crate::vm::INVOCATION_CONTEXT;
use crate::{NO_DATA_BLOCK_ID, SyscallResult, sys};

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
    INVOCATION_CONTEXT.value_received.into()
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
