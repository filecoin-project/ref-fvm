// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use anyhow::{anyhow, Context as _};
use cid::{multihash, Cid};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::CborStore;
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::{Address, Payload};
use fvm_shared::econ::TokenAmount;
use fvm_shared::state::{StateInfo0, StateRoot, StateTreeVersion};
use fvm_shared::{ActorID, HAMT_BIT_WIDTH};
use num_traits::Zero;
#[cfg(feature = "arb")]
use quickcheck::Arbitrary;

use crate::init_actor::State as InitActorState;
use crate::kernel::{ClassifyResult, ExecutionError, Result};
use crate::{syscall_error, EMPTY_ARR_CID};

/// State tree implementation using hamt. This structure is not threadsafe and should only be used
/// in sync contexts.
pub struct StateTree<S> {
    hamt: Hamt<S, ActorState>,

    version: StateTreeVersion,
    info: Option<Cid>,

    /// State snapshots.
    snaps: StateSnapshots,
    /// Read-cache for the backing blockstore. This DOES NOT contain
    /// mutations in progress (contained in the snapshots). Always check
    /// snapshots first, then the read cache, then fall back to the backing
    /// blockstore.
    read_cache: RefCell<HashMap<ActorID, Option<ActorState>>>,
}

/// Collection of state snapshots
struct StateSnapshots {
    /// Current layers. On success, we merge the top layer into the next layer. On abort, we discard
    /// the top layer.
    layers: Vec<StateSnapLayer>,
    /// Number of read-only layers stacked on top of the "layers". When this number is > 0:
    /// 1. Modifications are rejected.
    /// 2. Creating/discarding a layer simply adds/subtracts from this number
    read_only_layers: u32,
}

/// State snap shot layer
#[derive(Debug, Default)]
struct StateSnapLayer {
    actors: RefCell<HashMap<ActorID, Option<ActorState>>>,
    resolve_cache: RefCell<HashMap<Address, ActorID>>,
}

#[allow(clippy::large_enum_variant)]
enum StateCacheResult {
    Uncached,
    Exists(ActorState),
    Deleted,
}

impl StateSnapshots {
    /// State snapshot constructor
    fn new() -> Self {
        Self {
            layers: vec![StateSnapLayer::default()],
            read_only_layers: 0,
        }
    }

    fn add_layer(&mut self, read_only: bool) {
        if read_only || self.is_read_only() {
            self.read_only_layers += 1;
        } else {
            self.layers.push(StateSnapLayer::default())
        }
    }

    fn drop_layer(&mut self) -> Result<()> {
        if self.is_read_only() {
            self.read_only_layers -= 1;
            return Ok(());
        }

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
        if self.is_read_only() {
            self.read_only_layers -= 1;
            return Ok(());
        }

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

    fn top_layer(&self) -> Result<&StateSnapLayer> {
        self.layers
            .last()
            .context("state snapshots empty")
            .or_fatal()
    }

    #[inline(always)]
    fn is_read_only(&self) -> bool {
        self.read_only_layers > 0
    }

    fn assert_writable(&self) -> Result<()> {
        if self.is_read_only() {
            Err(syscall_error!(ReadOnly; "cannot mutate state while in read-only mode").into())
        } else {
            Ok(())
        }
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
        self.top_layer()?
            .resolve_cache
            .borrow_mut()
            .insert(addr, id);
        Ok(())
    }

    /// Returns the actor if present in the snapshots.
    fn get_actor(&self, id: ActorID) -> StateCacheResult {
        for layer in self.layers.iter().rev() {
            if let Some(state) = layer.actors.borrow().get(&id) {
                return state
                    .clone()
                    .map(StateCacheResult::Exists)
                    .unwrap_or(StateCacheResult::Deleted);
            }
        }

        StateCacheResult::Uncached
    }

    fn set_actor(&self, id: ActorID, actor: ActorState) -> Result<()> {
        self.assert_writable()?;

        self.top_layer()?
            .actors
            .borrow_mut()
            .insert(id, Some(actor));
        Ok(())
    }

    fn delete_actor(&self, id: ActorID) -> Result<()> {
        self.assert_writable()?;

        self.top_layer()?.actors.borrow_mut().insert(id, None);

        Ok(())
    }
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
            snaps: StateSnapshots::new(),
            read_cache: Default::default(),
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
                    snaps: StateSnapshots::new(),
                    read_cache: Default::default(),
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
        // Check cache for actor state
        Ok(match self.snaps.get_actor(id) {
            StateCacheResult::Exists(state) => Some(state),
            StateCacheResult::Deleted => None,
            StateCacheResult::Uncached => {
                let mut read_cache = self.read_cache.borrow_mut();
                match read_cache.entry(id) {
                    Entry::Occupied(e) => e.get().clone(),
                    Entry::Vacant(e) => {
                        e.insert({
                            // It's not cached, so we look it up and cache it.
                            let key = Address::new_id(id).to_bytes();
                            self.hamt
                                .get(&key)
                                .with_context(|| format!("failed to lookup actor {}", id))
                                .or_fatal()?
                                .cloned()
                        })
                        .clone()
                    }
                }
            }
        })
    }

    /// Set actor state with an actor ID.
    pub fn set_actor(&mut self, id: ActorID, actor: ActorState) -> Result<()> {
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

        let a = match state.resolve_address(self.store(), addr)? {
            Some(a) => a,
            None => return Ok(None),
        };

        self.snaps.cache_resolve_address(*addr, a)?;

        Ok(Some(a))
    }

    /// Delete actor identified by the supplied ID. Returns no error if the actor doesn't exist.
    pub fn delete_actor(&mut self, id: ActorID) -> Result<()> {
        // Record that we've deleted the actor.
        self.snaps.delete_actor(id)?;

        Ok(())
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
        self.set_actor(id, act)?;
        Ok(true)
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

        self.set_actor(crate::init_actor::INIT_ACTOR_ID, actor)?;

        Ok(new_addr)
    }

    /// Begin a new state transaction. Transactions stack.
    pub fn begin_transaction(&mut self, read_only: bool) {
        self.snaps.add_layer(read_only);
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

        // Clear the read-cache as it only reflects the state of the current state-tree, not
        // modifications contained in the snaps.
        self.read_cache.borrow_mut().clear();

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

    pub fn is_read_only(&self) -> bool {
        self.snaps.is_read_only()
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
    pub balance: TokenAmount,
    /// The actor's "delegated" address, if assigned.
    ///
    /// This field is set on actor creation and never modified.
    pub delegated_address: Option<Address>,
}

impl ActorState {
    /// Constructor for actor state
    pub fn new(
        code: Cid,
        state: Cid,
        balance: TokenAmount,
        sequence: u64,
        address: Option<Address>,
    ) -> Self {
        Self {
            code,
            state,
            sequence,
            balance,
            delegated_address: address,
        }
    }

    /// Construct a new empty actor with the specified code.
    pub fn new_empty(code: Cid, delegated_address: Option<Address>) -> Self {
        ActorState {
            code,
            state: *EMPTY_ARR_CID,
            sequence: 0,
            balance: TokenAmount::zero(),
            delegated_address,
        }
    }

    /// Safely deducts funds from an Actor
    pub fn deduct_funds(&mut self, amt: &TokenAmount) -> Result<()> {
        if &self.balance < amt {
            return Err(
                syscall_error!(InsufficientFunds; "when deducting funds ({}) from balance ({})", amt, self.balance).into(),
            );
        }
        self.balance -= amt;

        Ok(())
    }
    /// Deposits funds to an Actor
    pub fn deposit_funds(&mut self, amt: &TokenAmount) {
        self.balance += amt;
    }
}

#[cfg(feature = "arb")]
impl Arbitrary for ActorState {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        let cid = Cid::new_v1(
            u64::arbitrary(g),
            cid::multihash::Multihash::wrap(u64::arbitrary(g), &[u8::arbitrary(g)]).unwrap(),
        );
        Self {
            code: cid,
            state: cid,
            sequence: u64::arbitrary(g),
            balance: TokenAmount::from_atto(u64::arbitrary(g)),
            address: None,
        }
    }
}

#[cfg(feature = "json")]
pub mod json {
    use std::str::FromStr;

    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

    use super::*;
    use crate::TokenAmount;

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

#[cfg(test)]
mod tests {
    use cid::multihash::Code::Blake2b256;
    use cid::multihash::Multihash;
    use cid::Cid;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::{CborStore, DAG_CBOR};
    use fvm_shared::address::{Address, SECP_PUB_LEN};
    use fvm_shared::econ::TokenAmount;
    use fvm_shared::state::StateTreeVersion;
    use fvm_shared::{ActorID, IDENTITY_HASH, IPLD_RAW};
    use lazy_static::lazy_static;

    use crate::init_actor;
    use crate::init_actor::INIT_ACTOR_ID;
    use crate::state_tree::{ActorState, StateTree};

    lazy_static! {
        pub static ref DUMMY_ACCOUNT_ACTOR_CODE_ID: Cid = Cid::new_v1(
            IPLD_RAW,
            Multihash::wrap(IDENTITY_HASH, b"fil/test/dummyaccount").unwrap()
        );
        pub static ref DUMMY_INIT_ACTOR_CODE_ID: Cid = Cid::new_v1(
            IPLD_RAW,
            Multihash::wrap(IDENTITY_HASH, b"fil/test/dummyinit").unwrap()
        );
    }

    fn empty_cid() -> Cid {
        Cid::new_v1(DAG_CBOR, Multihash::wrap(IDENTITY_HASH, &[]).unwrap())
    }

    #[test]
    fn get_set_cache() {
        let act_s = ActorState::new(empty_cid(), empty_cid(), Default::default(), 1, None);
        let act_a = ActorState::new(empty_cid(), empty_cid(), Default::default(), 2, None);
        let actor_id = 1;
        let store = MemoryBlockstore::default();
        let mut tree = StateTree::new(&store, StateTreeVersion::V5).unwrap();

        // test address not in cache
        assert_eq!(tree.get_actor(actor_id).unwrap(), None);
        // test successful insert
        assert!(tree.set_actor(actor_id, act_s).is_ok());
        // test inserting with different data
        assert!(tree.set_actor(actor_id, act_a.clone()).is_ok());
        // Assert insert with same data returns ok
        assert!(tree.set_actor(actor_id, act_a.clone()).is_ok());
        // test getting set item
        assert_eq!(tree.get_actor(actor_id).unwrap().unwrap(), act_a);
    }

    #[test]
    fn delete_actor() {
        let store = MemoryBlockstore::default();
        let mut tree = StateTree::new(&store, StateTreeVersion::V5).unwrap();

        let actor_id = 3;
        let act_s = ActorState::new(empty_cid(), empty_cid(), Default::default(), 1, None);
        tree.set_actor(actor_id, act_s.clone()).unwrap();
        assert_eq!(tree.get_actor(actor_id).unwrap(), Some(act_s));
        tree.delete_actor(actor_id).unwrap();
        assert_eq!(tree.get_actor(actor_id).unwrap(), None);
    }

    #[test]
    fn get_set_non_id() {
        let store = MemoryBlockstore::default();
        let mut tree = StateTree::new(&store, StateTreeVersion::V5).unwrap();
        let init_state = init_actor::State::new_test(&store);

        let state_cid = tree
            .store()
            .put_cbor(&init_state, Blake2b256)
            .map_err(|e| e.to_string())
            .unwrap();

        let act_s = ActorState::new(
            *DUMMY_INIT_ACTOR_CODE_ID,
            state_cid,
            Default::default(),
            1,
            None,
        );

        tree.begin_transaction(false);
        tree.set_actor(INIT_ACTOR_ID, act_s).unwrap();

        // Test mutate function
        tree.mutate_actor(INIT_ACTOR_ID, |mut actor| {
            actor.sequence = 2;
            Ok(())
        })
        .unwrap();
        let new_init_s = tree.get_actor(INIT_ACTOR_ID).unwrap();
        assert_eq!(
            new_init_s,
            Some(ActorState {
                code: *DUMMY_INIT_ACTOR_CODE_ID,
                state: state_cid,
                balance: Default::default(),
                sequence: 2,
                delegated_address: None
            })
        );

        // Register new address
        let addr = Address::new_secp256k1(&[2; SECP_PUB_LEN]).unwrap();
        let assigned_addr = tree.register_new_address(&addr).unwrap();

        assert_eq!(assigned_addr, 100);
    }

    #[test]
    fn test_transactions() {
        let store = MemoryBlockstore::default();
        let mut tree = StateTree::new(&store, StateTreeVersion::V5).unwrap();

        let addresses: &[ActorID] = &[101, 102, 103];

        tree.begin_transaction(false);
        tree.set_actor(
            addresses[0],
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None,
            ),
        )
        .unwrap();

        tree.set_actor(
            addresses[1],
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None,
            ),
        )
        .unwrap();
        tree.set_actor(
            addresses[2],
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None,
            ),
        )
        .unwrap();
        tree.end_transaction(false).unwrap();
        tree.flush().unwrap();

        assert_eq!(
            tree.get_actor(addresses[0]).unwrap().unwrap(),
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None,
            )
        );
        assert_eq!(
            tree.get_actor(addresses[1]).unwrap().unwrap(),
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None,
            )
        );

        assert_eq!(
            tree.get_actor(addresses[2]).unwrap().unwrap(),
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None
            )
        );
    }

    #[test]
    fn revert_transaction() {
        let store = MemoryBlockstore::default();
        let mut tree = StateTree::new(&store, StateTreeVersion::V5).unwrap();

        let actor_id: ActorID = 1;

        tree.begin_transaction(false);
        tree.set_actor(
            actor_id,
            ActorState::new(
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                *DUMMY_ACCOUNT_ACTOR_CODE_ID,
                TokenAmount::from_atto(55),
                1,
                None,
            ),
        )
        .unwrap();
        tree.end_transaction(true).unwrap();

        tree.flush().unwrap();

        assert_eq!(tree.get_actor(actor_id).unwrap(), None);
    }

    #[test]
    fn unsupported_versions() {
        let unsupported = vec![
            StateTreeVersion::V0,
            StateTreeVersion::V1,
            StateTreeVersion::V2,
            StateTreeVersion::V3,
            StateTreeVersion::V4,
        ];
        let store = MemoryBlockstore::default();
        for v in unsupported {
            // expect a fatal error.
            let err = StateTree::new(&store, v).err().unwrap();
            assert!(err.is_fatal());
        }
    }
}
