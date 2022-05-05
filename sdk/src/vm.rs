use std::convert::TryInto;
use std::ptr;

use fvm_ipld_encoding::DAG_CBOR;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sys::out::vm::InvocationContext;
use fvm_shared::sys::{BlockId, Codec};
use fvm_shared::{ActorID, MethodNum};

use crate::{sys, SyscallResult};

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

lazy_static::lazy_static! {
    pub(crate) static ref INVOCATION_CONTEXT: InvocationContext = {
        unsafe {
            sys::vm::context().expect("failed to lookup invocation context")
        }
    };
}

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

/// Returns the message codec and parameters.
pub fn params_raw(id: BlockId) -> SyscallResult<(Codec, Vec<u8>)> {
    if id == NO_DATA_BLOCK_ID {
        return Ok((DAG_CBOR, Vec::default())); // DAG_CBOR is a lie, but we have no nil codec.
    }
    unsafe {
        let fvm_shared::sys::out::ipld::IpldStat { codec, size } = sys::ipld::stat(id)?;
        log::trace!(
            "params_raw -> ipld stat: size={:?}; codec={:?}",
            size,
            codec
        );

        let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
        let ptr = buf.as_mut_ptr();
        let bytes_read = sys::ipld::read(id, 0, ptr, size)?;
        buf.set_len(bytes_read as usize);
        log::trace!(
            "params_raw -> ipld read: bytes_read={:?}, data: {:x?}",
            bytes_read,
            &buf
        );
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        Ok((codec, buf))
    }
}

/// Returns the value received from the caller in AttoFIL.
#[inline(always)]
pub fn value_received() -> TokenAmount {
    INVOCATION_CONTEXT
        .value_received
        .try_into()
        .expect("invalid bigint")
}

/// Abort execution.
pub fn abort(code: u32, message: Option<&str>) -> ! {
    unsafe {
        let (message, message_len) = if let Some(m) = message {
            (m.as_ptr(), m.len())
        } else {
            (ptr::null(), 0)
        };

        sys::vm::abort(code, message, message_len as u32);
    }
}
