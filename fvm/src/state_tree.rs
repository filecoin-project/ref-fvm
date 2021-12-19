// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;
use std::collections::HashMap;

use anyhow::{anyhow, Context as _};
use blockstore::Blockstore;
use cid::{multihash, Cid};

use fvm_shared::address::{Address, Payload};
use fvm_shared::bigint::bigint_ser;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{tuple::*, CborStore};
use fvm_shared::state::{StateInfo0, StateRoot, StateTreeVersion};
use fvm_shared::ActorID;

use ipld_hamt::Hamt;

use crate::init_actor::State as InitActorState;
use crate::kernel::{ClassifyResult, Context as _, ExecutionError, Result};
use crate::syscall_error;

/// State tree implementation using hamt. This structure is not threadsafe and should only be used
/// in sync contexts.
pub struct StateTree<S> {
    hamt: Hamt<S, ActorState>,

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

    fn drop_layer(&mut self) -> Result<()> {
        self.layers
            .pop()
            .with_context(|| {
                format!(
                    "drop layer failed to index snapshot layer at index {}",
                    &self.layers.len() - 1
                )
            })
            .or_fatal()?;

        Ok(())
    }

    fn merge_last_layer(&mut self) -> Result<()> {
        self.layers
            .get(&self.layers.len() - 2)
            .with_context(|| {
                format!(
                    "merging layers failed to index snapshot layer at index: {}",
                    &self.layers.len() - 2
                )
            })
            .or_fatal()?
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
            .with_context(|| {
                format!(
                    "merging layers failed to index snapshot layer at index: {}",
                    &self.layers.len() - 2
                )
            })
            .or_fatal()?
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

    fn cache_resolve_address(&self, addr: Address, id: ActorID) -> Result<()> {
        self.layers
            .last()
            .with_context(|| {
                format!(
                    "caching address failed to index snapshot layer at index: {}",
                    &self.layers.len() - 1
                )
            })
            .or_fatal()?
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

    fn set_actor(&self, id: ActorID, actor: ActorState) -> Result<()> {
        self.layers
            .last()
            .with_context(|| {
                format!(
                    "set actor failed to index snapshot layer at index: {}",
                    &self.layers.len() - 1
                )
            })
            .or_fatal()?
            .actors
            .borrow_mut()
            .insert(id, Some(actor));
        Ok(())
    }

    fn delete_actor(&self, id: ActorID) -> Result<()> {
        self.layers
            .last()
            .with_context(|| {
                format!(
                    "delete actor failed to index snapshot layer at index: {}",
                    &self.layers.len() - 1
                )
            })
            .or_fatal()?
            .actors
            .borrow_mut()
            .insert(id, None);

        Ok(())
    }
}

impl<S> StateTree<S>
where
    S: Blockstore,
{
    pub fn new(store: S, version: StateTreeVersion) -> Result<Self> {
        let info = match version {
            StateTreeVersion::V0 => None,
            StateTreeVersion::V1
            | StateTreeVersion::V2
            | StateTreeVersion::V3
            | StateTreeVersion::V4 => {
                let cid = store
                    .put_cbor(&StateInfo0::default(), multihash::Code::Blake2b256)
                    .context("failed to put state info")
                    .or_fatal()?;
                Some(cid)
            }
        };

        // TODO: restore multiple version support? Or drop it entirely?
        //let hamt = Map::new(store, ActorVersion::from(version));
        let hamt = Hamt::new(store);
        Ok(Self {
            hamt,
            version,
            info,
            snaps: StateSnapshots::new(),
        })
    }

    /// Constructor for a hamt state tree given an IPLD store
    pub fn new_from_root(store: S, c: &Cid) -> Result<Self> {
        // Try to load state root, if versioned
        let (version, info, actors) = if let Ok(Some(StateRoot {
            version,
            info,
            actors,
        })) = store.get_cbor(c)
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
                let hamt = Hamt::load(&actors, store)
                    .context("failed to load state tree")
                    .or_fatal()?;

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
    pub fn get_actor(&self, addr: &Address) -> Result<Option<ActorState>> {
        let id = match self.lookup_id(addr)? {
            Some(id) => id,
            None => return Ok(None),
        };
        self.get_actor_id(id)
    }

    /// Get actor state from an actor ID.
    pub fn get_actor_id(&self, id: ActorID) -> Result<Option<ActorState>> {
        // Check cache for actor state
        if let Some(actor_state) = self.snaps.get_actor(id) {
            return Ok(Some(actor_state));
        }

        // if state doesn't exist, find using hamt
        let act = self
            .hamt
            .get(&Address::new_id(id).to_bytes())
            .with_context(|| format!("failed to lookup actor {}", id))
            .or_fatal()?
            .cloned();

        // Update cache if state was found
        if let Some(act_s) = &act {
            self.snaps.set_actor(id, act_s.clone())?;
        }

        Ok(act)
    }

    /// Set actor state for an address. Will set state at ID address.
    pub fn set_actor(&mut self, addr: &Address, actor: ActorState) -> Result<()> {
        let id = self
            .lookup_id(addr)?
            .with_context(|| format!("Resolution lookup failed for {}", addr))
            .or_fatal()?;

        self.set_actor_id(id, actor)
    }

    /// Set actor state with an actor ID.
    pub fn set_actor_id(&mut self, id: ActorID, actor: ActorState) -> Result<()> {
        self.snaps.set_actor(id, actor)
    }

    /// Get an ID address from any Address
    pub fn lookup_id(&self, addr: &Address) -> Result<Option<ActorID>> {
        if let &Payload::ID(id) = addr.payload() {
            return Ok(Some(id));
        }

        if let Some(res_address) = self.snaps.resolve_address(addr) {
            return Ok(Some(res_address));
        }

        let (state, _) = InitActorState::load(self)?;

        let a = match state
            .resolve_address(self.store(), addr)
            .context("Could not resolve address")
            .or_fatal()?
        {
            Some(a) => a,
            None => return Ok(None),
        };

        self.snaps.cache_resolve_address(*addr, a)?;

        Ok(Some(a))
    }

    /// Delete actor for an address. Will resolve to ID address to delete.
    pub fn delete_actor(&mut self, addr: &Address) -> Result<()> {
        let id = self
            .lookup_id(addr)?
            .with_context(|| format!("Resolution lookup failed for {}", addr))
            .or_fatal()?;

        self.delete_actor_id(id)
    }

    /// Delete actor identified by the supplied ID.
    pub fn delete_actor_id(&mut self, id: ActorID) -> Result<()> {
        // Remove value from cache
        self.snaps.delete_actor(id)?;

        Ok(())
    }

    /// Mutate and set actor state for an Address.
    pub fn mutate_actor<F>(&mut self, addr: &Address, mutate: F) -> Result<()>
    where
        F: FnOnce(&mut ActorState) -> Result<()>,
    {
        // Retrieve actor state from address
        let mut act: ActorState = self
            .get_actor(addr)?
            .with_context(|| format!("Actor for address: {} does not exist", addr))
            .or_fatal()?;

        // Apply function of actor state
        mutate(&mut act)?;
        // Set the actor
        self.set_actor(addr, act)
    }

    /// Register a new address through the init actor.
    pub fn register_new_address(&mut self, addr: &Address) -> Result<ActorID> {
        let (mut state, mut actor) = InitActorState::load(self)?;

        let new_addr = state.map_address_to_new_id(self.store(), addr)?;

        // Set state for init actor in store and update root Cid
        actor.state = self
            .store()
            .put_cbor(&state, multihash::Code::Blake2b256)
            .or_fatal()?;

        self.set_actor(&crate::init_actor::INIT_ACTOR_ADDR, actor)?;

        Ok(new_addr)
    }

    /// Begin a new state transaction. Transactions stack.
    pub fn begin_transaction(&mut self) {
        self.snaps.add_layer();
    }

    /// End a transaction, reverting if requested.
    pub fn end_transaction(&mut self, revert: bool) -> Result<()> {
        if revert {
            self.snaps.drop_layer()
        } else {
            self.snaps.merge_last_layer()
        }
    }

    /// Flush state tree and return Cid root.
    pub fn flush(&mut self) -> Result<Cid> {
        if self.snaps.layers.len() != 1 {
            return Err(ExecutionError::Fatal(anyhow!(
                "tried to flush state tree with snapshots on the stack: {:?}",
                self.snaps.layers.len()
            )));
        }

        for (&id, sto) in self.snaps.layers[0].actors.borrow().iter() {
            let addr = Address::new_id(id);
            match sto {
                None => {
                    self.hamt.delete(&addr.to_bytes()).or_fatal()?;
                }
                Some(ref state) => {
                    self.hamt
                        .set(addr.to_bytes().into(), state.clone())
                        .or_fatal()?;
                }
            }
        }

        let root = self.hamt.flush().or_fatal()?;

        match self.version {
            StateTreeVersion::V0 => Ok(root),
            _ => {
                let cid = self
                    .info
                    .expect("malformed state tree, version 1 and version 2 require info");
                let obj = &StateRoot {
                    version: self.version,
                    actors: root,
                    info: cid,
                };
                let root = self
                    .store()
                    .put_cbor(obj, multihash::Code::Blake2b256)
                    .or_fatal()?;
                Ok(root)
            }
        }
    }

    pub fn for_each<F>(&self, mut f: F) -> anyhow::Result<()>
    where
        F: FnMut(Address, &ActorState) -> anyhow::Result<()>,
    {
        self.hamt.for_each(|k, v| {
            let addr = Address::from_bytes(&k.0)?;
            f(addr, v)
        })?;
        Ok(())
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
    /// TODO return a system error with exit code "insufficient funds"
    pub fn deduct_funds(&mut self, amt: &TokenAmount) -> Result<()> {
        if &self.balance < amt {
            return Err(syscall_error!(SysErrInsufficientFunds; "not enough funds").into());
        }
        self.balance -= amt;

        Ok(())
    }
    /// Deposits funds to an Actor
    pub fn deposit_funds(&mut self, amt: &TokenAmount) {
        self.balance += amt;
    }
}

#[cfg(feature = "json")]
pub mod json {
    use std::str::FromStr;

    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

    use crate::TokenAmount;

    use super::*;

    /// Wrapper for serializing and deserializing a SignedMessage from JSON.
    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct ActorStateJson(#[serde(with = "self")] pub ActorState);

    /// Wrapper for serializing a SignedMessage reference to JSON.
    #[derive(Serialize)]
    #[serde(transparent)]
    pub struct ActorStateJsonRef<'a>(#[serde(with = "self")] pub &'a ActorState);

    impl From<ActorStateJson> for ActorState {
        fn from(wrapper: ActorStateJson) -> Self {
            wrapper.0
        }
    }

    pub fn serialize<S>(m: &ActorState, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct ActorStateSer<'a> {
            #[serde(with = "cid::json")]
            pub code: &'a Cid,
            #[serde(rename = "Head", with = "cid::json")]
            pub state: &'a Cid,
            #[serde(rename = "Nonce")]
            pub sequence: u64,
            pub balance: String,
        }
        ActorStateSer {
            code: &m.code,
            state: &m.state,
            sequence: m.sequence,
            balance: m.balance.to_str_radix(10),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ActorState, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct ActorStateDe {
            #[serde(with = "cid::json")]
            pub code: Cid,
            #[serde(rename = "Head", with = "cid::json")]
            pub state: Cid,
            #[serde(rename = "Nonce")]
            pub sequence: u64,
            pub balance: String,
        }
        let ActorStateDe {
            code,
            state,
            sequence,
            balance,
        } = Deserialize::deserialize(deserializer)?;
        Ok(ActorState {
            code,
            state,
            sequence,
            balance: TokenAmount::from_str(&balance).map_err(de::Error::custom)?,
        })
    }
}
