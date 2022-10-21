use anyhow::Context;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::{Deserialize_tuple, Serialize_tuple};
use fvm_ipld_encoding::{Cbor, CborStore};
use fvm_shared::address::Address;

use crate::kernel::{ClassifyResult, Result};
use crate::state_tree::{ActorState, StateTree};

pub const EAM_ACTOR_ADDR: Address = Address::new_id(10);

#[derive(Deserialize_tuple, Serialize_tuple, Debug)]
pub struct State {
    pub empty: Vec<u8>
 }
impl Cbor for State {}

impl State {
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let eam_act = state_tree
            .get_actor(&EAM_ACTOR_ADDR)?
            .context("eam actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&eam_act.state)
            .or_fatal()?
            .context("eam actor state not found")
            .or_fatal()?;

        Ok((state, eam_act))
    }
}