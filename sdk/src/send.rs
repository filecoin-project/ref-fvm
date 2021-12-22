use fvm_shared::{
    encoding::{from_slice, to_vec},
    message::Message,
    receipt::Receipt,
};

// no_std
use crate::{
    error::{IntoSyscallResult, SyscallResult},
    sys,
};

/// Sends a message to another actor.
/// TODO https://github.com/filecoin-project/fvm/issues/178
pub fn send(msg: Message) -> SyscallResult<Receipt> {
    let bytes = to_vec(&msg).expect("failed to serialize message");
    unsafe {
        // Send the message.
        let id = sys::send::send(bytes.as_ptr(), bytes.len() as u32).into_syscall_result()?;
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
