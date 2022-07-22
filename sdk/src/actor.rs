use core::option::Option; // no_std

use cid::Cid;
use fvm_shared::actor::builtin::Type;
use fvm_shared::address::{Address, Payload};
use fvm_shared::error::ErrorNumber;
use fvm_shared::{actor, ActorID, MAX_CID_LEN};
use num_traits::FromPrimitive;

use crate::{sys, SyscallResult, MAX_ACTOR_ADDR_LEN};

/// Resolves the ID address of an actor. Returns `None` if the address cannot be resolved.
/// Successfully resolving an address doesn't necessarily mean the actor exists (e.g., if the
/// addresss was already an actor ID).
pub fn resolve_address(addr: &Address) -> Option<ActorID> {
    if let &Payload::ID(id) = addr.payload() {
        return Some(id);
    }

    let bytes = addr.to_bytes();
    unsafe {
        match sys::actor::resolve_address(bytes.as_ptr(), bytes.len() as u32) {
            Ok(value) => Some(value),
            Err(ErrorNumber::NotFound) => None,
            Err(other) => panic!("unexpected address resolution failure: {}", other),
        }
    }
}

/// Look up the code ID at an actor address. Returns `None` if the actor cannot be found.
pub fn get_actor_code_cid(addr: &Address) -> Option<Cid> {
    // In most cases, this address will already be resolved (e.g., the caller, receiver, etc.) so
    // this call should be a no-op. But it's more convenient for users to take addresses.
    let id = resolve_address(addr)?;

    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        match sys::actor::get_actor_code_cid(id, buf.as_mut_ptr(), MAX_CID_LEN as u32) {
            Ok(len) => Some(Cid::read_bytes(&buf[..len as usize]).expect("invalid cid returned")),
            Err(ErrorNumber::NotFound) => None,
            Err(other) => panic!("unexpected code cid resolution failure: {}", other),
        }
    }
}

/// Generates a new actor address for an actor deployed
/// by the calling actor.
pub fn new_actor_address() -> Address {
    let mut buf = [0u8; MAX_ACTOR_ADDR_LEN];
    unsafe {
        let len = sys::actor::new_actor_address(buf.as_mut_ptr(), MAX_ACTOR_ADDR_LEN as u32)
            .expect("failed to create a new actor address");
        Address::from_bytes(&buf[..len as usize]).expect("syscall returned invalid address")
    }
}

/// Creates a new actor of the specified type in the state tree, under
/// the provided address.
/// TODO(M2): this syscall will change to calculate the address internally.
pub fn create_actor(actor_id: ActorID, code_cid: &Cid) -> SyscallResult<()> {
    let cid = code_cid.to_bytes();
    unsafe { sys::actor::create_actor(actor_id, cid.as_ptr()) }
}

/// Installs or ensures an actor code CID is valid and loaded.
/// Note: this is a priviledged syscall, restricted to the init actor.
#[cfg(feature = "m2-native")]
pub fn install_actor(code_cid: &Cid) -> SyscallResult<()> {
    let cid = code_cid.to_bytes();
    unsafe { sys::actor::install_actor(cid.as_ptr()) }
}

/// Determines whether the supplied CodeCID belongs to a built-in actor type,
/// and to which.
pub fn get_builtin_actor_type(code_cid: &Cid) -> Option<actor::builtin::Type> {
    let cid = code_cid.to_bytes();
    unsafe {
        let res = sys::actor::get_builtin_actor_type(cid.as_ptr())
            .expect("failed to determine if CID belongs to builtin actor");
        // The zero value represents "unknown" and is not modelled in the enum,
        // so it'll be converted to a None.
        FromPrimitive::from_i32(res)
    }
}

/// Returns the CodeCID for a built-in actor type. Aborts with IllegalArgument
/// if the supplied type is invalid.
pub fn get_code_cid_for_type(typ: Type) -> Cid {
    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        let len =
            sys::actor::get_code_cid_for_type(typ as i32, buf.as_mut_ptr(), MAX_CID_LEN as u32)
                .expect("failed to get CodeCID for type");
        Cid::read_bytes(&buf[..len as usize]).expect("invalid cid returned")
    }
}
