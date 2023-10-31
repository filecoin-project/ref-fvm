// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;

use anyhow::{anyhow, Context as _};
use cid::{multihash, Cid};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::{Address, Payload};
use fvm_shared::state::{StateInfo0, StateRoot};
use fvm_shared::{ActorID, HAMT_BIT_WIDTH};

pub use fvm_shared::state::{ActorState, StateTreeVersion};

use crate::history_map::HistoryMap;
use crate::init_actor::State as InitActorState;
use crate::kernel::{ClassifyResult, ExecutionError, Result};

/// State tree implementation using hamt. This structure is not threadsafe and should only be used
/// in sync contexts.
pub struct StateTree<S> {
    hamt: Hamt<S, ActorState>,

    version: StateTreeVersion,
    info: Option<Cid>,

    /// An actor-state cache that internally keeps an undo history.
    actor_cache: RefCell<HistoryMap<ActorID, ActorCacheEntry>>,
    /// An actor-address cache that internally keeps an undo history.
    resolve_cache: RefCell<HistoryMap<Address, ActorID>>,
    /// Snapshot layers. Each layer contains points in the actor/resolve cache histories to which
    /// said caches will be reverted on revert.
    layers: Vec<StateSnapLayer>,
}

/// An entry in the actor cache.
#[derive(Eq, PartialEq)]
struct ActorCacheEntry {
    /// True if this is a change that should be flushed.
    dirty: bool,
    /// The cached actor, or None if the actor doesn't exist and/or has been deleted.
    actor: Option<ActorState>,
}

/// State snap shot layer.
struct StateSnapLayer {
    /// The actor-cache height at which this snapshot was taken.
    actor_cache_height: usize,
    /// The resolve-cache height at which this snapshot was taken.
    resolve_cache_height: usize,
}

impl<S> StateTree<S>
where
    S: Blockstore,
{
    pub fn new(store: S, version: StateTreeVersion) -> Result<Self> {
        let info = match version {
            StateTreeVersion::V0
            | StateTreeVersion::V1
            | StateTreeVersion::V2
            | StateTreeVersion::V3
            | StateTreeVersion::V4 => {
                return Err(ExecutionError::Fatal(anyhow!(
                    "unsupported state tree version: {:?}",
                    version
                )))
            }
            StateTreeVersion::V5 => {
                let cid = store
                    .put_cbor(&StateInfo0::default(), multihash::Code::Blake2b256)
                    .context("failed to put state info")
                    .or_fatal()?;
                Some(cid)
            }
        };

        // Both V3 and V4 use bitwidt=5.
        let hamt = Hamt::new_with_bit_width(store, HAMT_BIT_WIDTH);
        Ok(Self {
            hamt,
            version,
            info,
            actor_cache: Default::default(),
            resolve_cache: Default::default(),
            layers: Vec::new(),
        })
    }

    /// Constructor for a hamt state tree given an IPLD store
    pub fn new_from_root(store: S, c: &Cid) -> Result<Self> {
        // Try to load state root, if versioned
        let (version, info, actors) = match store.get_cbor(c) {
            Ok(Some(StateRoot {
                version,
                info,
                actors,
            })) => (version, Some(info), actors),
            Ok(None) => {
                return Err(ExecutionError::Fatal(anyhow!(
                    "failed to find state tree {}",
                    c
                )))
            }
            Err(e) => {
                return Err(ExecutionError::Fatal(anyhow!(
                    "failed to load state tree {}: {}",
                    c,
                    e
                )))
            }
        };

        match version {
            StateTreeVersion::V0
            | StateTreeVersion::V1
            | StateTreeVersion::V2
            | StateTreeVersion::V3
            | StateTreeVersion::V4 => Err(ExecutionError::Fatal(anyhow!(
                "unsupported state tree version: {:?}",
                version
            ))),

            StateTreeVersion::V5 => {
                let hamt = Hamt::load_with_bit_width(&actors, store, HAMT_BIT_WIDTH)
                    .context("failed to load state tree")
                    .or_fatal()?;

                Ok(Self {
                    hamt,
                    version,
                    info,
                    actor_cache: Default::default(),
                    resolve_cache: Default::default(),
                    layers: Vec::new(),
                })
            }
        }
    }

    /// Retrieve store reference to modify db.
    pub fn store(&self) -> &S {
        self.hamt.store()
    }

    /// Get actor state from an address. Will be resolved to ID address.
    #[cfg(feature = "testing")]
    pub fn get_actor_by_address(&self, addr: &Address) -> Result<Option<ActorState>> {
        let id = match self.lookup_id(addr)? {
            Some(id) => id,
            None => return Ok(None),
        };
        self.get_actor(id)
    }

    /// Get actor state from an actor ID.
    pub fn get_actor(&self, id: ActorID) -> Result<Option<ActorState>> {
        self.actor_cache
            .borrow_mut()
            .get_or_try_insert_with(id, || {
                // It's not cached/dirty, so we look it up and cache it.
                let key = Address::new_id(id).to_bytes();
                Ok(ActorCacheEntry {
                    dirty: false,
                    actor: self
                        .hamt
                        .get(&key)
                        .with_context(|| format!("failed to lookup actor {}", id))
                        .or_fatal()?
                        .cloned(),
                })
            })
            .map(|ActorCacheEntry { actor, .. }| actor.clone())
    }

    /// Set actor state with an actor ID.
    pub fn set_actor(&mut self, id: ActorID, actor: ActorState) {
        self.actor_cache.borrow_mut().insert(
            id,
            ActorCacheEntry {
                actor: Some(actor),
                dirty: true,
            },
        )
    }

    /// Get an ID address from any Address
    pub fn lookup_id(&self, addr: &Address) -> Result<Option<ActorID>> {
        if let &Payload::ID(id) = addr.payload() {
            return Ok(Some(id));
        }

        if let Some(&res_address) = self.resolve_cache.borrow().get(addr) {
            return Ok(Some(res_address));
        }

        let (state, _) = InitActorState::load(self)?;

        let a = match state.resolve_address(self.store(), addr)? {
            Some(a) => a,
            None => return Ok(None),
        };

        self.resolve_cache.borrow_mut().insert(*addr, a);

        Ok(Some(a))
    }

    /// Delete actor identified by the supplied ID.
    pub fn delete_actor(&mut self, id: ActorID) {
        // Record that we've deleted the actor.
        self.actor_cache.borrow_mut().insert(
            id,
            ActorCacheEntry {
                dirty: true,
                actor: None,
            },
        );
    }

    /// Mutate and set actor state identified by the supplied ID. Returns a fatal error if the actor
    /// doesn't exist.
    pub fn mutate_actor<F>(&mut self, id: ActorID, mutate: F) -> Result<()>
    where
        F: FnOnce(&mut ActorState) -> Result<()>,
    {
        self.maybe_mutate_actor_id(id, mutate).and_then(|found| {
            if found {
                Ok(())
            } else {
                Err(anyhow!("failed to lookup actor {}", id)).or_fatal()
            }
        })
    }

    /// Try to mutate the actor state identified by the supplied ID, returning false if the actor
    /// doesn't exist.
    pub fn maybe_mutate_actor_id<F>(&mut self, id: ActorID, mutate: F) -> Result<bool>
    where
        F: FnOnce(&mut ActorState) -> Result<()>,
    {
        // Retrieve actor state from address
        let mut act = match self.get_actor(id)? {
            Some(act) => act,
            None => return Ok(false),
        };

        // Apply function of actor state
        mutate(&mut act)?;
        // Set the actor
        self.set_actor(id, act);
        Ok(true)
    }

    /// Register a new address through the init actor.
    pub fn register_new_address(&mut self, addr: &Address) -> Result<ActorID> {
        let (mut state, mut actor) = InitActorState::load(self)?;

        let new_id = state.map_address_to_new_id(self.store(), addr)?;

        // Set state for init actor in store and update root Cid
        actor.state = self
            .store()
            .put_cbor(&state, multihash::Code::Blake2b256)
            .or_fatal()?;

        self.set_actor(crate::init_actor::INIT_ACTOR_ID, actor);
        self.resolve_cache.borrow_mut().insert(*addr, new_id);

        Ok(new_id)
    }

    /// Begin a new state transaction. Transactions stack.
    pub fn begin_transaction(&mut self) {
        self.layers.push(StateSnapLayer {
            actor_cache_height: self.actor_cache.get_mut().history_len(),
            resolve_cache_height: self.resolve_cache.get_mut().history_len(),
        })
    }

    /// End a transaction, reverting if requested.
    pub fn end_transaction(&mut self, revert: bool) -> Result<()> {
        let layer = self
            .layers
            .pop()
            .context("state snapshots empty")
            .or_fatal()?;
        if revert {
            self.actor_cache
                .get_mut()
                .rollback(layer.actor_cache_height);
            self.resolve_cache
                .get_mut()
                .rollback(layer.resolve_cache_height);
        }
        // When we end the last transaction, discard the undo history.
        if !self.in_transaction() {
            self.actor_cache.get_mut().discard_history();
            self.resolve_cache.get_mut().discard_history();
        }
        Ok(())
    }

    /// Returns true if we're inside of a transaction.
    pub fn in_transaction(&self) -> bool {
        !self.layers.is_empty()
    }

    /// Flush state tree and return Cid root.
    pub fn flush(&mut self) -> Result<Cid> {
        if self.in_transaction() {
            return Err(ExecutionError::Fatal(anyhow!(
                "cannot flush while inside of a transaction",
            )));
        }
        for (&id, entry) in self.actor_cache.get_mut().iter_mut() {
            if !entry.dirty {
                continue;
            }
            entry.dirty = false;
            let addr = Address::new_id(id);
            match entry.actor {
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
                    .expect("malformed state tree, version 1+ require info");
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

    /// Consumes this StateTree and returns the Blockstore it owns via the HAMT.
    pub fn into_store(self) -> S {
        self.hamt.into_store()
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
