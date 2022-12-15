use frc46_token::token;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::Cbor;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::ActorID;

use fil_actors_runtime::{ActorError, AsActorError};

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub governor: Address,
    pub token: token::state::TokenState,
}

impl State {
    pub fn new_test<BS: Blockstore>(store: &BS, governor: Address) -> Self {
        let token_state = token::state::TokenState::new(store)
            .unwrap();

        State {
            governor,
            token: token_state
        }
    }
}

impl Cbor for State {}

