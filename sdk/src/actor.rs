use core::option::Option; // no_std

use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::ActorID;

use crate::{sys, SyscallResult, MAX_ACTOR_ADDR_LEN, MAX_CID_LEN};

/// Resolves the ID address of an actor.
pub fn resolve_address(addr: &Address) -> Option<ActorID> {
    let bytes = addr.to_bytes();
    unsafe {
        match sys::actor::resolve_address(bytes.as_ptr(), bytes.len() as u32)
            // Can only happen due to memory corruption.
            .expect("error when resolving address")
        {
            fvm_shared::sys::out::actor::ResolveAddress { resolved: 0, value } => Some(value),
            _ => None,
        }
    }
}

/// Look up the code ID at an actor address.
pub fn get_actor_code_cid(addr: &Address) -> Option<Cid> {
    let bytes = addr.to_bytes();
    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        let ok = sys::actor::get_actor_code_cid(
            bytes.as_ptr(),
            bytes.len() as u32,
            buf.as_mut_ptr(),
            MAX_CID_LEN as u32,
        )
        // Can only fail due to memory corruption
        .expect("failed to lookup actor code cid");
        if ok == 0 {
            // Cid::read_bytes won't read until the end, just the bytes it needs.
            Some(Cid::read_bytes(&buf[..]).expect("invalid cid returned"))
        } else {
            None
        }
    }
}

/// Generates a new actor address for an actor deployed
/// by the calling actor.
pub fn new_actor_address() -> SyscallResult<Address> {
    let mut buf = [0u8; MAX_ACTOR_ADDR_LEN];
    unsafe {
        let len = sys::actor::new_actor_address(buf.as_mut_ptr(), MAX_ACTOR_ADDR_LEN as u32)?;
        Ok(Address::from_bytes(&buf[..len as usize]).expect("syscall returned invalid address"))
    }
}

/// Creates a new actor of the specified type in the state tree, under
/// the provided address.
/// TODO this syscall will change to calculate the address internally.
pub fn create_actor(actor_id: ActorID, code_cid: &Cid) -> SyscallResult<()> {
    let cid = code_cid.to_bytes();
    unsafe { sys::actor::create_actor(actor_id, cid.as_ptr()) }
}
