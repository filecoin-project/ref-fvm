//! This module contains the types and functions to process the init actor's state.
//! While it may appear leaky to deal with a concrete actor type in FVM-land,
//! truth is that certain syscalls can only be resolved by querying and manipulating
//! the init actor's state.
//!
//! In the future, we will revisit and redesign these components.
//!
//! This module can only deal with the Init Actor as of actors v3 ==
//! network version v10. The reason being that the HAMT layout changed.
use anyhow::Context;
use blockstore::Blockstore;
use cid::Cid;

use fvm_shared::address::{Address, Payload};
use fvm_shared::encoding::Cbor;
use fvm_shared::encoding::{tuple::*, CborStore};
use fvm_shared::{ActorID, HAMT_BIT_WIDTH};
use ipld_hamt::Hamt;

use crate::state_tree::{ActorState, StateTree};

pub const INIT_ACTOR_ADDR: Address = Address::new_id(1);

use crate::kernel::{ClassifyResult, Result};

// TODO: should this module really ExecutionError? Or should it just return anyhow errors.
// Everything is "fatal".

// TODO need to untangle all this init actor mess
//  In theory, we should go through the actor version multiplexer to decide which
//  state object to deserialize into. But luckily, the init actor's state hasn't
//  undergone changes over time, so we can use a fixed object.
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address_map: Cid,
    pub next_id: ActorID,
    pub network_name: String,
}

impl Cbor for State {}

impl State {
    /// Loads the init actor state with the supplied CID from the underlying store.
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let init_act = state_tree
            .get_actor(&INIT_ACTOR_ADDR)?
            .context("Init actor address could not be resolved")
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

    // TODO(steb): I've changed this from the actors. It shouldn't make a difference in practice,
    // but trying to distinguish between "resolved" and "unresolved" addresses was getting annoying.
    /// ResolveAddress resolves an address to an ID-address, if possible.
    /// If the provided address is an ID address, it is returned as-is.
    /// This means that mapped ID-addresses (which should only appear as values, not keys) and
    /// singleton actor addresses (which are not in the map) pass through unchanged.
    ///
    /// Returns an ID-address and `true` if the address was already an ID-address or was resolved
    /// in the mapping.
    /// Returns an undefined address and `false` if the address was not an ID-address and not found
    /// in the mapping.
    /// Returns an error only if state was inconsistent.
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
