use anyhow::Context;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::{Deserialize_tuple, Serialize_tuple};
use fvm_ipld_encoding::{Cbor, CborStore};
use fvm_shared::address::Address;

use crate::kernel::{ClassifyResult, Result};
use crate::state_tree::{ActorState, StateTree};

pub const SYSTEM_ACTOR_ADDR: Address = Address::new_id(0);

#[derive(Default, Deserialize_tuple, Serialize_tuple)]
pub struct State {
    // builtin actor registry: Vec<(String, Cid)>
    pub builtin_actors: Cid,
}
impl Cbor for State {}

impl State {
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let system_act = state_tree
            .get_actor(&SYSTEM_ACTOR_ADDR)?
            .context("system actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&system_act.state)
            .or_fatal()?
            .context("system actor state not found")
            .or_fatal()?;

        Ok((state, system_act))
    }
}
