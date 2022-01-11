// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use actors_runtime::runtime::{ActorCode, Runtime};
use actors_runtime::{actor_error, wasm_trampoline, ActorError, SYSTEM_ACTOR_ADDR};
use fvm_shared::blockstore::Blockstore;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::{MethodNum, METHOD_CONSTRUCTOR};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};

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

/// System actor methods.
#[derive(FromPrimitive)]
#[repr(u64)]
pub enum Method {
    Constructor = METHOD_CONSTRUCTOR,
}

/// System actor state.
#[derive(Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct State([(); 0]);
impl Cbor for State {}

/// System actor.
pub struct Actor;
impl Actor {
    /// System actor constructor.
    pub fn constructor<BS, RT>(rt: &mut RT) -> Result<(), ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        rt.validate_immediate_caller_is(std::iter::once(&*SYSTEM_ACTOR_ADDR))?;

        rt.create(&State::default())?;
        Ok(())
    }
}

impl ActorCode for Actor {
    fn invoke_method<BS, RT>(
        rt: &mut RT,
        method: MethodNum,
        _params: &RawBytes,
    ) -> Result<RawBytes, ActorError>
    where
        BS: Blockstore,
        RT: Runtime<BS>,
    {
        match FromPrimitive::from_u64(method) {
            Some(Method::Constructor) => {
                Self::constructor(rt)?;
                Ok(RawBytes::default())
            }
            None => Err(actor_error!(SysErrInvalidMethod; "Invalid method")),
        }
    }
}
