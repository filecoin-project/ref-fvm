// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error as StdError;

use cid::{multihash, Cid};

use fvm_shared::address::{Address, Payload};
use fvm_shared::bigint::bigint_ser;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::tuple::*;
use fvm_shared::state::{StateInfo0, StateRoot, StateTreeVersion};
use fvm_shared::ActorID;
use ipld_blockstore::BlockStore;

use crate::adt::Map;
use crate::init_actor::State as InitActorState;

/// State tree implementation using hamt. This structure is not threadsafe and should only be used
/// in sync contexts.
pub struct StateTree<'db, S> {
    hamt: Map<'db, S, ActorState>,

    version: StateTreeVersion,
    info: Option<Cid>,

    /// State cache
    snaps: StateSnapshots,
}

/// Collection of state snapshots
struct StateSnapshots {
    layers: Vec<StateSnapLayer>,
}

/// State snap shot layer
#[derive(Debug, Default)]
struct StateSnapLayer {
    actors: RefCell<HashMap<ActorID, Option<ActorState>>>,
    resolve_cache: RefCell<HashMap<Address, ActorID>>,
}

impl StateSnapshots {
    /// State snapshot constructor
    fn new() -> Self {
        Self {
            layers: vec![StateSnapLayer::default()],
        }
    }

    fn add_layer(&mut self) {
        self.layers.push(StateSnapLayer::default())
    }

    fn drop_layer(&mut self) -> Result<(), String> {
        self.layers.pop().ok_or_else(|| {
            format!(
                "drop layer failed to index snapshot layer at index {}",
                &self.layers.len() - 1
            )
        })?;

        Ok(())
    }

    fn merge_last_layer(&mut self) -> Result<(), String> {
        self.layers
            .get(&self.layers.len() - 2)
            .ok_or_else(|| {
                format!(
                    "merging layers failed to index snapshot layer at index: {}",
                    &self.layers.len() - 2
                )
            })?
            .actors
            .borrow_mut()
            .extend(
                self.layers[&self.layers.len() - 1]
                    .actors
                    .borrow_mut()
                    .drain(),
            );

        self.layers
            .get(&self.layers.len() - 2)
            .ok_or_else(|| {
                format!(
                    "merging layers failed to index snapshot layer at index: {}",
                    &self.layers.len() - 2
                )
            })?
            .resolve_cache
            .borrow_mut()
            .extend(
                self.layers[&self.layers.len() - 1]
                    .resolve_cache
                    .borrow_mut()
                    .drain(),
            );

        self.drop_layer()
    }

    fn resolve_address(&self, addr: &Address) -> Option<ActorID> {
        if let &Payload::ID(id) = addr.payload() {
            return Some(id);
        }
        for layer in self.layers.iter().rev() {
            if let Some(res_addr) = layer.resolve_cache.borrow().get(addr).cloned() {
                return Some(res_addr);
            }
        }

        None
    }

    fn cache_resolve_address(&self, addr: Address, id: ActorID) -> Result<(), Box<dyn StdError>> {
        self.layers
            .last()
            .ok_or_else(|| {
                format!(
                    "caching address failed to index snapshot layer at index: {}",
                    &self.layers.len() - 1
                )
            })?
            .resolve_cache
            .borrow_mut()
            .insert(addr, id);

        Ok(())
    }

    fn get_actor(&self, id: ActorID) -> Option<ActorState> {
        for layer in self.layers.iter().rev() {
            if let Some(state) = layer.actors.borrow().get(&id) {
                return state.clone();
            }
        }

        None
    }

    fn set_actor(&self, id: ActorID, actor: ActorState) -> Result<(), Box<dyn StdError>> {
        self.layers
            .last()
            .ok_or_else(|| {
                format!(
                    "set actor failed to index snapshot layer at index: {}",
                    &self.layers.len() - 1
                )
            })?
            .actors
            .borrow_mut()
            .insert(id, Some(actor));
        Ok(())
    }

    fn delete_actor(&self, id: ActorID) -> Result<(), Box<dyn StdError>> {
        self.layers
            .last()
            .ok_or_else(|| {
                format!(
                    "delete actor failed to index snapshot layer at index: {}",
                    &self.layers.len() - 1
                )
            })?
            .actors
            .borrow_mut()
            .insert(id, None);

        Ok(())
    }
}

impl<'db, S> StateTree<'db, S>
where
    S: BlockStore,
{
    pub fn new(store: &'db S, version: StateTreeVersion) -> Result<Self, Box<dyn StdError>> {
        let info = match version {
            StateTreeVersion::V0 => None,
            StateTreeVersion::V1
            | StateTreeVersion::V2
            | StateTreeVersion::V3
            | StateTreeVersion::V4 => {
                let cid = store.put(&StateInfo0::default(), multihash::Code::Blake2b256)?;
                Some(cid)
            }
        };

        // TODO: restore multiple version support? Or drop it entirely?
        //let hamt = Map::new(store, ActorVersion::from(version));
        let hamt = Map::new(store);
        Ok(Self {
            hamt,
            version,
            info,
            snaps: StateSnapshots::new(),
        })
    }

    /// Constructor for a hamt state tree given an IPLD store
    pub fn new_from_root(store: &'db S, c: &Cid) -> Result<Self, Box<dyn StdError>> {
        // Try to load state root, if versioned
        let (version, info, actors) = if let Ok(Some(StateRoot {
            version,
            info,
            actors,
        })) = store.get(c)
        {
            (version, Some(info), actors)
        } else {
            // Fallback to v0 state tree if retrieval fails
            (StateTreeVersion::V0, None, *c)
        };

        match version {
            StateTreeVersion::V0
            | StateTreeVersion::V1
            | StateTreeVersion::V2
            | StateTreeVersion::V3
            | StateTreeVersion::V4 => {
                // TODO: use the version.
                let hamt = Map::load(&actors, store)?;

                Ok(Self {
                    hamt,
                    version,
                    info,
                    snaps: StateSnapshots::new(),
                })
            }
        }
    }

    /// Retrieve store reference to modify db.
    pub fn store(&self) -> &S {
        self.hamt.store()
    }

    /// Get actor state from an address. Will be resolved to ID address.
    pub fn get_actor(&self, addr: &Address) -> Result<Option<ActorState>, Box<dyn StdError>> {
        let id = match self.lookup_id(addr)? {
            Some(id) => id,
            None => return Ok(None),
        };

        // Check cache for actor state
        if let Some(actor_state) = self.snaps.get_actor(id) {
            return Ok(Some(actor_state));
        }

        // if state doesn't exist, find using hamt
        let act = self.hamt.get(&Address::new_id(id).to_bytes())?.cloned();

        // Update cache if state was found
        if let Some(act_s) = &act {
            self.snaps.set_actor(id, act_s.clone())?;
        }

        Ok(act)
    }

    /// Set actor state for an address. Will set state at ID address.
    pub fn set_actor(
        &mut self,
        addr: &Address,
        actor: ActorState,
    ) -> Result<(), Box<dyn StdError>> {
        let id = self
            .lookup_id(addr)?
            .ok_or_else(|| format!("Resolution lookup failed for {}", addr))?;

        self.snaps.set_actor(id, actor)
    }

    /// Get an ID address from any Address
    pub fn lookup_id(&self, addr: &Address) -> Result<Option<ActorID>, Box<dyn StdError>> {
        if let &Payload::ID(id) = addr.payload() {
            return Ok(Some(id));
        }

        if let Some(res_address) = self.snaps.resolve_address(addr) {
            return Ok(Some(res_address));
        }

        let (state, _) = InitActorState::load(&self)?;

        let a = match state
            .resolve_address(self.store(), addr)
            .map_err(|e| format!("Could not resolve address: {:?}", e))?
        {
            Some(a) => a,
            None => return Ok(None),
        };

        self.snaps.cache_resolve_address(*addr, a)?;

        Ok(Some(a))
    }

    /// Delete actor for an address. Will resolve to ID address to delete.
    pub fn delete_actor(&mut self, addr: &Address) -> Result<(), Box<dyn StdError>> {
        let addr = self
            .lookup_id(addr)?
            .ok_or_else(|| format!("Resolution lookup failed for {}", addr))?;

        // Remove value from cache
        self.snaps.delete_actor(addr)?;

        Ok(())
    }

    /// Mutate and set actor state for an Address.
    pub fn mutate_actor<F>(&mut self, addr: &Address, mutate: F) -> Result<(), Box<dyn StdError>>
    where
        F: FnOnce(&mut ActorState) -> Result<(), String>,
    {
        // Retrieve actor state from address
        let mut act: ActorState = self
            .get_actor(addr)?
            .ok_or(format!("Actor for address: {} does not exist", addr))?;

        // Apply function of actor state
        mutate(&mut act)?;
        // Set the actor
        self.set_actor(addr, act)
    }

    /// Register a new address through the init actor.
    pub fn register_new_address(&mut self, addr: &Address) -> Result<ActorID, Box<dyn StdError>> {
        let (mut state, mut actor) = InitActorState::load(&self)?;

        let new_addr = state.map_address_to_new_id(self.store(), addr)?;

        // Set state for init actor in store and update root Cid
        actor.state = self.store().put(&state, multihash::Code::Blake2b256)?;

        self.set_actor(&crate::init_actor::INIT_ACTOR_ADDR, actor)?;

        Ok(new_addr)
    }

    /// Add snapshot layer to stack.
    pub fn snapshot(&mut self) -> Result<(), String> {
        self.snaps.add_layer();
        Ok(())
    }

    /// Merges last two snap shot layers.
    pub fn clear_snapshot(&mut self) -> Result<(), String> {
        self.snaps.merge_last_layer()
    }

    /// Revert state cache by removing last snapshot
    pub fn revert_to_snapshot(&mut self) -> Result<(), String> {
        self.snaps.drop_layer()?;
        self.snaps.add_layer();
        Ok(())
    }

    /// Flush state tree and return Cid root.
    pub fn flush(&mut self) -> Result<Cid, Box<dyn StdError>> {
        if self.snaps.layers.len() != 1 {
            return Err(format!(
                "tried to flush state tree with snapshots on the stack: {:?}",
                self.snaps.layers.len()
            )
            .into());
        }

        for (&id, sto) in self.snaps.layers[0].actors.borrow().iter() {
            let addr = Address::new_id(id);
            match sto {
                None => {
                    self.hamt.delete(&addr.to_bytes())?;
                }
                Some(ref state) => {
                    self.hamt.set(addr.to_bytes().into(), state.clone())?;
                }
            }
        }

        let root = self.hamt.flush()?;

        if matches!(self.version, StateTreeVersion::V0) {
            Ok(root)
        } else {
            let cid = self
                .info
                .expect("malformed state tree, version 1 and version 2 require info");
            let obj = &StateRoot {
                version: self.version,
                actors: root,
                info: cid,
            };
            self.store()
                .put(obj, multihash::Code::Blake2b256)
                .map_err(|e| Box::from(e))
        }
    }

    pub fn for_each<F>(&self, mut f: F) -> Result<(), Box<dyn StdError>>
    where
        F: FnMut(Address, &ActorState) -> Result<(), Box<dyn StdError>>,
        S: BlockStore,
    {
        self.hamt.for_each(|k, v| f(Address::from_bytes(&k.0)?, v))
    }
}

/// State of all actor implementations.
#[derive(PartialEq, Eq, Clone, Debug, Serialize_tuple, Deserialize_tuple)]
pub struct ActorState {
    /// Link to code for the actor.
    pub code: Cid,
    /// Link to the state of the actor.
    pub state: Cid,
    /// Sequence of the actor.
    pub sequence: u64,
    /// Tokens available to the actor.
    #[serde(with = "bigint_ser")]
    pub balance: TokenAmount,
}

impl ActorState {
    /// Constructor for actor state
    pub fn new(code: Cid, state: Cid, balance: TokenAmount, sequence: u64) -> Self {
        Self {
            code,
            state,
            sequence,
            balance,
        }
    }
    /// Safely deducts funds from an Actor
    pub fn deduct_funds(&mut self, amt: &TokenAmount) -> Result<(), String> {
        if &self.balance < amt {
            return Err("Not enough funds".to_owned());
        }
        self.balance -= amt;

        Ok(())
    }
    /// Deposits funds to an Actor
    pub fn deposit_funds(&mut self, amt: &TokenAmount) {
        self.balance += amt;
    }
}
