use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;

use crate::error::{ActorDeleteError, NoStateError};
use crate::{sys, MAX_CID_LEN};

/// Get the IPLD root CID. Fails if the actor doesn't have state (before the first call to
/// `set_root` and after actor deletion).
pub fn root() -> Result<Cid, NoStateError> {
    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        let len = sys::sself::root(buf.as_mut_ptr(), buf.len() as u32).map_err(|e| match e {
            ErrorNumber::IllegalOperation => NoStateError,
            e => panic!("unexpected error from `self::root` syscall: {}", e),
        })? as usize;

        Ok(Cid::read_bytes(&buf[..len]).expect("runtime returned an invalid CID"))
    }
}

/// Set the actor's state-tree root.
///
/// Fails if:
///
/// - The new root is not in the actor's "reachable" set.
/// - Fails if the actor has been deleted.
pub fn set_root(cid: &Cid) -> Result<(), NoStateError> {
    let mut buf = [0u8; MAX_CID_LEN];
    cid.write_bytes(&mut buf[..])
        .expect("CID encoding should not fail");

    unsafe {
        sys::sself::set_root(buf.as_ptr()).map_err(|e| match e {
            ErrorNumber::IllegalOperation => NoStateError,
            e => panic!("unexpected error from `self::set_root` syscall: {}", e),
        })
    }
}

/// Gets the current balance for the calling actor.
#[inline(always)]
pub fn current_balance() -> TokenAmount {
    unsafe {
        sys::sself::current_balance()
            .expect("failed to get current balance")
            .into()
    }
}

/// Destroys the calling actor, sending its current balance
/// to the supplied address, which cannot be itself.
///
/// Fails if the beneficiary doesn't exist or is the actor being deleted.
pub fn self_destruct(beneficiary: &Address) -> Result<(), ActorDeleteError> {
    let bytes = beneficiary.to_bytes();
    unsafe {
        sys::sself::self_destruct(bytes.as_ptr(), bytes.len() as u32).map_err(|e| match e {
            ErrorNumber::Forbidden => ActorDeleteError::BeneficiaryIsSelf,
            ErrorNumber::NotFound => ActorDeleteError::BeneficiaryDoesNotExist,
            _ => panic!("unexpected error from `self::self_destruct` syscall: {}", e),
        })
    }
}
