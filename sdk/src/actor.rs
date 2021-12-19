use crate::{sys, MAX_ACTOR_ADDR_LEN, MAX_CID_LEN};
use cid::Cid;
use core::option::Option;
// no_std
use fvm_shared::address::Address;
use fvm_shared::ActorID;

/// Resolves the ID address of an actor.
pub fn resolve_address(addr: Address) -> Option<ActorID> {
    let bytes = addr.to_bytes();
    unsafe {
        match sys::actor::resolve_address(bytes.as_ptr(), bytes.len() as u32) {
            (0, id) => Some(id),
            _ => None,
        }
    }
}

/// Look up the code ID at an actor address.
pub fn get_actor_code_cid(addr: Address) -> Option<Cid> {
    let bytes = addr.to_bytes();
    let mut buf = [0u8; MAX_CID_LEN];
    let mut ret = 0;
    unsafe {
        ret = sys::actor::get_actor_code_cid(
            bytes.as_ptr(),
            bytes.len() as u32,
            buf.as_mut_ptr(),
            MAX_CID_LEN as u32,
        )
    }
    if ret == 0 {
        // Cid::read_bytes won't read until the end, just the bytes it needs.
        Some(Cid::read_bytes(&buf[..MAX_CID_LEN]).expect("invalid cid returned"))
    } else {
        None
    }
}

/// Generates a new actor address for an actor deployed
/// by the calling actor.
pub fn new_actor_address() -> Address {
    let mut buf = [0u8; MAX_ACTOR_ADDR_LEN];
    let mut len = 0;
    unsafe { len = sys::actor::new_actor_address(buf.as_mut_ptr(), MAX_ACTOR_ADDR_LEN as u32) }
    Address::from_bytes(&buf[..len as usize]).expect("syscall returned invalid address")
}

/// Creates a new actor of the specified type in the state tree, under
/// the provided address.
/// TODO this syscall will change to calculate the address internally.
pub fn create_actor(address: Address, code_cid: Cid) {
    let addr = address.to_bytes();
    let cid = code_cid.to_bytes();
    unsafe {
        sys::actor::create_actor(addr.as_ptr(), addr.len() as u32, cid.as_ptr());
    }
}
