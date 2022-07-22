//! This module contains the types and functions to process the init actor's state.
//! It does not contain the logic of the init actor: that lives on-chain as a WASM actor.
//!
//! The init actor is the special system actor that:
//! 1. Acts as a factory for other non-singleton actors.
//! 2. Stores the global mapping between the ID address and the robust address
//!    of all actors.
//!
//! ## Version compatibility
//!
//! This module does not handle historical actors and network versions, nor
//! does it support versioning. It handles actors v3, and network version v10,
//! onwards. The HAMT layout changes introduced in that upgrade (codename: Trust)
//! remain active today.

use anyhow::Context;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{Cbor, CborStore};
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::{Address, Payload};
use fvm_shared::{ActorID, HAMT_BIT_WIDTH};

use crate::state_tree::{ActorState, StateTree};

pub const INIT_ACTOR_ADDR: Address = Address::new_id(1);

use crate::kernel::{ClassifyResult, Result};

#[derive(Serialize_tuple, Deserialize_tuple, Debug)]
pub struct State {
    pub address_map: Cid,
    pub next_id: ActorID,
    pub network_name: String,
    #[cfg(feature = "m2-native")]
    pub installed_actors: Cid,
}

impl Cbor for State {}

impl State {
    // ideally we would just #[cfg(test)] this, but it is used by non test-gated code in
    // integration/tester.
    #[allow(unused)]
    pub fn new_test<B: Blockstore>(store: &B) -> Self {
        #[cfg(feature = "m2-native")]
        use cid::multihash::Code::Blake2b256;

        // Empty hamt Cid used for testing
        let e_cid = Hamt::<_, String>::new_with_bit_width(&store, 5)
            .flush()
            .unwrap();

        // Empty list Cid used for testing
        #[cfg(feature = "m2-native")]
        let el_cid = store.put_cbor(&Vec::<Cid>::new(), Blake2b256).unwrap();

        State {
            address_map: e_cid,
            next_id: 100,
            network_name: "test".to_owned(),
            #[cfg(feature = "m2-native")]
            installed_actors: el_cid,
        }
    }

    /// Loads the init actor state from the supplied state tree.
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let init_act = state_tree
            .get_actor(&INIT_ACTOR_ADDR)?
            .context("init actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&init_act.state)
            .or_fatal()?
            .context("init actor state not found")
            .or_fatal()?;

        Ok((state, init_act))
    }

    /// Allocates a new ID address and stores a mapping of the argument address to it.
    /// Returns the newly-allocated address.
    pub fn map_address_to_new_id<B>(&mut self, store: B, addr: &Address) -> Result<ActorID>
    where
        B: Blockstore,
    {
        let id = self.next_id;
        self.next_id += 1;

        let mut map = Hamt::<B, _>::load_with_bit_width(&self.address_map, store, HAMT_BIT_WIDTH)
            .or_fatal()?;
        map.set(addr.to_bytes().into(), id).or_fatal()?;
        self.address_map = map.flush().or_fatal()?;

        Ok(id)
    }

    /// ResolveAddress resolves an address to an ID-address, if possible.
    /// If the provided address is an ID address, it is returned as-is.
    /// This means that mapped ID-addresses (which should only appear as values,
    /// not keys) and singleton actor addresses (which are not in the map) pass
    /// through unchanged.
    ///
    /// This method returns:
    /// * Ok and Some ID address and if the input was already an ID address, or
    ///   if it was successfully resolved.
    /// * Ok and None if the address was not an ID address, and no mapping was
    ///   found during resolution.
    /// * Err, if state was inconsistent.
    pub fn resolve_address<B>(&self, store: B, addr: &Address) -> Result<Option<u64>>
    where
        B: Blockstore,
    {
        if let &Payload::ID(id) = addr.payload() {
            return Ok(Some(id));
        }

        let map = Hamt::<B, _>::load_with_bit_width(&self.address_map, store, HAMT_BIT_WIDTH)
            .or_fatal()?;

        Ok(map.get(&addr.to_bytes()).or_fatal()?.copied())
    }
}
