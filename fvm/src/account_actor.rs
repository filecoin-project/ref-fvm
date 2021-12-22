//! This module contains the minimal logic for the FVM to handle account actor
//! auto-creation (on first transfer). This coupling between the FVM and a
//! concrete actor must eventually go. (TODO)

use cid::Cid;
use fvm_shared::{
    address::Address,
    bigint::Zero,
    econ::TokenAmount,
    encoding::{tuple::*, Cbor},
};
use lazy_static::lazy_static;

use crate::{
    builtin::{ACCOUNT_ACTOR_CODE_ID, EMPTY_ARR_CID},
    state_tree::ActorState,
};

pub const SYSTEM_ACTOR_ID: u64 = 0;

lazy_static! {
    pub static ref ZERO_STATE: ActorState = ActorState {
        code: *ACCOUNT_ACTOR_CODE_ID,
        state: *EMPTY_ARR_CID,
        sequence: 0,
        balance: TokenAmount::zero(),
    };
}

/// State specifies the key address for the actor.
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address: Address,
}

impl Cbor for State {}

/// Returns true if the code belongs to an account actor.
pub fn is_account_actor(code: &Cid) -> bool {
    code == &*ACCOUNT_ACTOR_CODE_ID
}
