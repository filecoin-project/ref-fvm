//! This module contains the types and functions to process the init actor's state.
//! While it may appear leaky to deal with a concrete actor type in FVM-land,
//! truth is that certain syscalls can only be resolved by querying and manipulating
//! the init actor's state.
//!
//! In the future, we will revisit and redesign these components.
//!
//! This module can only deal with the Init Actor as of actors v3 ==
//! network version v10. The reason being that the HAMT layout changed.
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error as StdError;

use lazy_static::lazy_static;

use blockstore::Blockstore;
use cid::multihash;
use cid::Cid;
use fvm_shared::address::{Address, Protocol, FIRST_NON_SINGLETON_ADDR};
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::tuple::*;
use fvm_shared::encoding::Cbor;
use fvm_shared::state::{StateInfo0, StateRoot, StateTreeVersion};
use fvm_shared::{bigint::bigint_ser, ActorID, HAMT_BIT_WIDTH};

use crate::adt::{make_empty_map, make_map_with_root_and_bitwidth, Map};
use crate::state_tree::{ActorState, StateTree};
use crate::store::CborStore;

lazy_static! {
    pub static ref INIT_ACTOR_ADDR: Address = Address::new_id(1);
}

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
    pub fn new<B: Blockstore>(store: &B, network_name: String) -> Result<Self, Box<dyn StdError>> {
        let empty_map = make_empty_map::<_, ()>(store, HAMT_BIT_WIDTH)
            .flush()
            .map_err(|e| format!("failed to create empty map: {}", e))?;
        Ok(Self {
            address_map: empty_map,
            next_id: FIRST_NON_SINGLETON_ADDR,
            network_name,
        })
    }

    /// Loads the init actor state with the supplied CID from the underlying store.
    pub fn load<B: Blockstore>(state_tree: &StateTree<B>) -> anyhow::Result<(Self, ActorState)> {
        let init_act = state_tree
            .get_actor(&INIT_ACTOR_ADDR)?
            .ok_or("Init actor address could not be resolved")?;

        let store = state_tree.store();

        CborStore::from(store)
            .get_cbor(&init_act.state)?
            .ok_or("init actor state not found")
    }

    /// Allocates a new ID address and stores a mapping of the argument address to it.
    /// Returns the newly-allocated address.
    pub fn map_address_to_new_id<B: Blockstore>(
        &mut self,
        store: &B,
        addr: &Address,
    ) -> anyhow::Result<Address> {
        let id = self.next_id;
        self.next_id += 1;

        let mut map = make_map_with_root_and_bitwidth(&self.address_map, store, HAMT_BIT_WIDTH)?;
        map.set(addr.to_bytes().into(), id)?;
        self.address_map = map.flush()?;

        Ok(Address::new_id(id))
    }

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
    pub fn resolve_address<B: Blockstore>(
        &self,
        store: &B,
        addr: &Address,
    ) -> Result<Option<Address>, Box<dyn StdError>> {
        if addr.protocol() == Protocol::ID {
            return Ok(Some(*addr));
        }

        let map = make_map_with_root_and_bitwidth(&self.address_map, store, HAMT_BIT_WIDTH)?;

        Ok(map.get(&addr.to_bytes())?.copied().map(Address::new_id))
    }
}
