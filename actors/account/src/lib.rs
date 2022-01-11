// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use actors_runtime::builtin::singletons::SYSTEM_ACTOR_ADDR;
use actors_runtime::runtime::{ActorCode, Runtime};
use actors_runtime::{actor_error, wasm_trampoline, ActorError};
use fvm_shared::address::{Address, Protocol};
use fvm_shared::blockstore::Blockstore;
use fvm_shared::encoding::RawBytes;
use fvm_shared::{MethodNum, METHOD_CONSTRUCTOR};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

pub use self::state::State;

mod state;

/// Export the wasm binary
#[cfg(not(feature = "runtime-wasm"))]
pub mod wasm {
    include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_wasm_binaries() {
            assert!(!WASM_BINARY.unwrap().is_empty());
            assert!(!WASM_BINARY_BLOATY.unwrap().is_empty());
        }
    }
}

wasm_trampoline!(Actor);

// * Updated to specs-actors commit: 845089a6d2580e46055c24415a6c32ee688e5186 (v3.0.0)

/// Account actor methods available
#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    Constructor = METHOD_CONSTRUCTOR,
    PubkeyAddress = 2,
}

/// Account Actor
pub struct Actor;
impl Actor {
    /// Constructor for Account actor
    pub fn constructor<BS, RT>(rt: &mut RT, address: Address) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        rt.validate_immediate_caller_is(std::iter::once(&*SYSTEM_ACTOR_ADDR))?;
        match address.protocol() {
            Protocol::Secp256k1 | Protocol::BLS => {}
            protocol => {
                return Err(actor_error!(ErrIllegalArgument;
                    "address must use BLS or SECP protocol, got {}", protocol));
            }
        }
        rt.create(&State { address })?;
        Ok(())
    }

    // Fetches the pubkey-type address from this actor.
    pub fn pubkey_address<BS, RT>(rt: &mut RT) -> Result<Address, ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        rt.validate_immediate_caller_accept_any()?;
        let st: State = rt.state()?;
        Ok(st.address)
    }
}

impl ActorCode for Actor {
    fn invoke_method<BS, RT>(
        rt: &mut RT,
        method: MethodNum,
        params: &RawBytes,
    ) -> Result<RawBytes, ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        match FromPrimitive::from_u64(method) {
            Some(Method::Constructor) => {
                Self::constructor(rt, rt.deserialize_params(params)?)?;
                Ok(RawBytes::default())
            }
            Some(Method::PubkeyAddress) => {
                let addr = Self::pubkey_address(rt)?;
                Ok(RawBytes::serialize(addr)?)
            }
            None => Err(actor_error!(SysErrInvalidMethod; "Invalid method")),
        }
    }
}
