// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_shared::MAX_CID_LEN;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;

use crate::error::{ActorDeleteError, StateReadError, StateUpdateError};
use crate::sys;

/// Get the IPLD root CID. Fails if the actor doesn't have state (before the first call to
/// `set_root` and after actor deletion).
pub fn root() -> Result<Cid, StateReadError> {
    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        let len = sys::sself::root(buf.as_mut_ptr(), buf.len() as u32).map_err(|e| match e {
            ErrorNumber::IllegalOperation => StateReadError,
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
pub fn set_root(cid: &Cid) -> Result<(), StateUpdateError> {
    let mut buf = [0u8; MAX_CID_LEN];
    cid.write_bytes(&mut buf[..])
        .expect("CID encoding should not fail");

    unsafe {
        sys::sself::set_root(buf.as_ptr()).map_err(|e| match e {
            ErrorNumber::IllegalOperation => StateUpdateError::ActorDeleted,
            ErrorNumber::ReadOnly => StateUpdateError::ReadOnly,
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

/// Destroys the calling actor, burning any remaining balance.
pub fn self_destruct(burn_funds: bool) -> Result<(), ActorDeleteError> {
    unsafe {
        sys::sself::self_destruct(burn_funds).map_err(|e| match e {
            ErrorNumber::IllegalOperation => ActorDeleteError::UnspentFunds,
            ErrorNumber::ReadOnly => ActorDeleteError::ReadOnly,
            _ => panic!("unexpected error from `self::self_destruct` syscall: {}", e),
        })
    }
}
