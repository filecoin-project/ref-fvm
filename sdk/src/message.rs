use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::DAG_CBOR;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::sys::{BlockId, Codec};
use fvm_shared::{ActorID, MethodNum};

use crate::{sys, SyscallResult};

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

/// Returns the ID address of the caller.
#[inline(always)]
pub fn caller() -> ActorID {
    unsafe { sys::message::caller().expect("failed to lookup caller ID") }
}

/// Returns the ID address of the actor.
#[inline(always)]
pub fn receiver() -> ActorID {
    unsafe { sys::message::receiver().expect("failed to lookup actor ID") }
}

/// Returns the message's method number.
#[inline(always)]
pub fn method_number() -> MethodNum {
    unsafe { sys::message::method_number().expect("failed to lookup method number") }
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
    unsafe {
        sys::message::value_received()
            .expect("failed to lookup received value")
            .into()
    }
}
