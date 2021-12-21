use crate::sys;
use fvm_shared::address::Address;
// no_std
use crate::error::{IntoSyscallResult, SyscallResult};
use fvm_shared::encoding::{from_slice, RawBytes, DAG_CBOR};
use fvm_shared::receipt::Receipt;

/// Sends a message to another actor.
pub fn send(
    to: Address,
    method: u64,
    params: RawBytes,
    value_hi: u64,
    value_lo: u64,
) -> SyscallResult<Receipt> {
    let recipient = to.to_bytes();
    unsafe {
        // Send the message.
        let params_id = sys::ipld::create(DAG_CBOR, params.as_ptr(), params.len() as u32)
            .into_syscall_result()?;
        let id = sys::send::send(
            recipient.as_ptr(),
            recipient.len() as u32,
            method,
            params_id,
            value_hi,
            value_lo,
        )
        .into_syscall_result()?;
        // Allocate a buffer to read the result.
        let (_, length) = sys::ipld::stat(id).into_syscall_result()?;
        let mut bytes = Vec::with_capacity(length as usize);
        // Now read the result.
        let read = sys::ipld::read(id, 0, bytes.as_mut_ptr(), length).into_syscall_result()?;
        assert_eq!(read, length);
        // Deserialize the receipt.
        let ret = from_slice::<Receipt>(bytes.as_slice()).expect("invalid receipt returned");
        Ok(ret)
    }
}
