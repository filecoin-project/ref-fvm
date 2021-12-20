use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, DAG_CBOR};
use fvm_shared::error::ExitCode;
use fvm_shared::{ActorID, MethodNum};

use crate::ipld::{BlockId, Codec};
use crate::{abort, sys};

/// Returns the ID address of the caller.
#[inline(always)]
pub fn caller() -> ActorID {
    unsafe { sys::message::caller() }
}

/// Returns the ID address of the actor.
#[inline(always)]
pub fn receiver() -> ActorID {
    unsafe { sys::message::receiver() }
}

/// Returns the message's method number.
#[inline(always)]
pub fn method_number() -> MethodNum {
    unsafe { sys::message::method_number() }
}

/// Returns the message codec and parameters.
pub fn params_raw(id: BlockId) -> (Codec, Vec<u8>) {
    unsafe {
        let (codec, size) = sys::ipld::stat(id);
        let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
        let ptr = buf.as_mut_ptr();
        let bytes_read = sys::ipld::read(id, 0, ptr, size);
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        (codec, buf)
    }
}

/// Fetches the input parameters as raw bytes, and decodes them locally
/// into type T using cbor serde. Failing to decode will abort execution.
pub fn params_cbor<T: Cbor>(id: BlockId) -> T {
    let (codec, raw) = params_raw(id);
    debug_assert!(codec == DAG_CBOR, "parameters codec was not cbor");
    match fvm_shared::encoding::from_slice(raw.as_slice()) {
        Ok(v) => v,
        Err(e) => abort(
            ExitCode::ErrSerialization as u32,
            Some(format!("could not deserialize parameters as cbor: {:?}", e).as_str()),
        ),
    }
}

/// Returns the value received from the caller in AttoFIL.
#[inline(always)]
pub fn value_received() -> TokenAmount {
    unsafe {
        let (lo, hi) = sys::message::value_received();
        // TODO not sure if this is the correct endianness.
        TokenAmount::from(hi) << 64 | TokenAmount::from(lo)
    }
}
