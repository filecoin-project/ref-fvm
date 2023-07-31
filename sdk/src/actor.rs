// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use core::option::Option;
use std::ptr; // no_std

use cid::Cid;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::{Address, Payload, MAX_ADDRESS_LEN};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::{ActorID, Response, MAX_CID_LEN};
use log::error;

use crate::{sys, SyscallResult, NO_DATA_BLOCK_ID};

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

/// Looks up the delegated (f4) address of the specified actor. Returns `None` if the actor doesn't
/// exist or it doesn't have f4 address.
pub fn lookup_delegated_address(addr: ActorID) -> Option<Address> {
    let mut out_buffer = [0u8; MAX_ADDRESS_LEN];
    unsafe {
        match sys::actor::lookup_delegated_address(
            addr,
            out_buffer.as_mut_ptr(),
            out_buffer.len() as u32,
        ) {
            Ok(0) => None,
            Ok(length) => match Address::from_bytes(&out_buffer[..length as usize]) {
                Ok(addr) => Some(addr),
                // Ok, so, we _log_ this error (if debugging is enabled) but otherwise move on.
                // Why? Because the system may add _new_ address classes. In that case, the "least
                // bad" thing to do here is to claim that the target actor doesn't have an f1/f3/f4
                // address, which is likely correct.
                //
                // https://github.com/filecoin-project/builtin-actors/issues/738
                Err(e) => {
                    error!(
                        "unexpected address from 'lookup_delegated_address' with protocol {}: {}",
                        out_buffer[0], e
                    );
                    None
                }
            },
            // We're flattening the "not found" error here, but that's probably reasonable for most users.
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

/// Generates a new actor address for an actor deployed by the calling actor.
pub fn next_actor_address() -> Address {
    let mut buf = [0u8; MAX_ADDRESS_LEN];
    unsafe {
        let len = sys::actor::next_actor_address(buf.as_mut_ptr(), MAX_ADDRESS_LEN as u32)
            .expect("failed to create a new actor address");
        Address::from_bytes(&buf[..len as usize]).expect("syscall returned invalid address")
    }
}

/// Creates a new actor of the specified type in the state tree, under the provided address.
pub fn create_actor(
    actor_id: ActorID,
    code_cid: &Cid,
    delegated_address: Option<Address>,
    params: Option<IpldBlock>,
    value: TokenAmount,
    gas_limit: Option<u64>,
) -> SyscallResult<Response> {
    let value: sys::TokenAmount = value
        .try_into()
        .map_err(|_| ErrorNumber::InsufficientFunds)?;
    unsafe {
        let cid = code_cid.to_bytes();
        let addr_bytes = delegated_address.map(|addr| addr.to_bytes());
        let (addr_off, addr_len) = addr_bytes
            .as_deref()
            .map(|v| (v.as_ptr(), v.len()))
            .unwrap_or((ptr::null(), 0));

        // Insert parameters as a block. Missing parameters are represented as the
        // NO_DATA_BLOCK_ID block ID in the FFI interface.
        let params_id = match params {
            Some(p) => sys::ipld::block_create(p.codec, p.data.as_ptr(), p.data.len() as u32)?,
            None => NO_DATA_BLOCK_ID,
        };

        let fvm_shared::sys::out::send::Send {
            exit_code,
            return_id,
            return_codec,
            return_size,
        } = sys::actor::create_actor(
            actor_id,
            cid.as_ptr(),
            addr_off,
            addr_len as u32,
            params_id,
            value.hi,
            value.lo,
            gas_limit.unwrap_or(u64::MAX),
        )?;

        // Process the result.
        let exit_code = ExitCode::new(exit_code);
        let return_data = if return_id == NO_DATA_BLOCK_ID {
            None
        } else {
            // Allocate a buffer to read the return data.
            let mut bytes = vec![0; return_size as usize];

            // Now read the return data.
            let unread = sys::ipld::block_read(return_id, 0, bytes.as_mut_ptr(), return_size)?;
            assert_eq!(0, unread);
            Some(IpldBlock {
                codec: return_codec,
                data: bytes.to_vec(),
            })
        };

        Ok(Response {
            exit_code,
            return_data,
        })
    }
}

/// Installs or ensures an actor code CID is valid and loaded.
/// Note: this is a privileged syscall, restricted to the init actor.
#[cfg(feature = "m2-native")]
pub fn install_actor(code_cid: &Cid) -> SyscallResult<()> {
    let cid = code_cid.to_bytes();
    unsafe { sys::actor::install_actor(cid.as_ptr()) }
}

/// Determines whether the supplied CodeCID belongs to a built-in actor type,
/// and to which.
pub fn get_builtin_actor_type(code_cid: &Cid) -> Option<i32> {
    let cid = code_cid.to_bytes();
    unsafe {
        let res = sys::actor::get_builtin_actor_type(cid.as_ptr())
            .expect("failed to determine if CID belongs to builtin actor");
        // The zero value represents "unknown" and is not modelled in the enum,
        // so it'll be converted to a None.
        if res == 0 {
            None
        } else {
            Some(res)
        }
    }
}

/// Returns the CodeCID for a built-in actor type. Aborts with IllegalArgument
/// if the supplied type is invalid.
pub fn get_code_cid_for_type(typ: i32) -> Cid {
    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        let len = sys::actor::get_code_cid_for_type(typ, buf.as_mut_ptr(), MAX_CID_LEN as u32)
            .expect("failed to get CodeCID for type");
        Cid::read_bytes(&buf[..len as usize]).expect("invalid cid returned")
    }
}

/// Retrieves the balance of the specified actor, or None if the actor doesn't exist.
pub fn balance_of(actor_id: ActorID) -> Option<TokenAmount> {
    unsafe {
        match sys::actor::balance_of(actor_id) {
            Ok(balance) => Some(balance.into()),
            Err(ErrorNumber::NotFound) => None,
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
