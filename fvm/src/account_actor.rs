//! This module contains the minimal logic for the FVM to handle account actor
//! auto-creation (on first transfer).
//!
//! ## Future direction
//!
//! This coupling between the FVM and a concrete actor must eventually be
//! eliminated. Refer to https://github.com/filecoin-project/fvm/issues/229 for
//! details.

use cid::Cid;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::Cbor;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use num_traits::Zero;

use crate::state_tree::ActorState;
use crate::EMPTY_ARR_CID;

pub const SYSTEM_ACTOR_ID: u64 = 0;

/// State specifies the key address for the actor.
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address: Address,
}

/// Returns an ActorState representing a brand new account with no balance.
pub fn zero_state(code_cid: Cid) -> ActorState {
    ActorState {
        code: code_cid,
        state: *EMPTY_ARR_CID,
        sequence: 0,
        balance: TokenAmount::zero(),
    }
}

impl Cbor for State {}
