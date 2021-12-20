use crate::sys;
// no_std
use fvm_shared::encoding::{from_slice, to_vec};
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;

/// Resolves the ID address of an actor.
pub fn send(msg: Message) -> Receipt {
    let bytes = to_vec(&msg).expect("failed to serialize message");
    unsafe {
        let id = sys::send::send(bytes.as_ptr(), bytes.len() as u32);
        let (_, length) = sys::ipld::stat(id);
        let mut bytes = Vec::with_capacity(length as usize);
        let read = sys::ipld::read(id, 0, bytes.as_mut_ptr(), length);
        assert_eq!(read, length);
        from_slice::<Receipt>(bytes.as_slice()).expect("invalid receipt returned")
    }
}
