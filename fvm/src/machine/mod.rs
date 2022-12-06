// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use derive_more::{Deref, DerefMut};
use fvm_ipld_blockstore::Blockstore;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use num_traits::Zero;
use wasmtime::ResourceLimiter;

use crate::externs::Externs;
use crate::gas::{price_list_by_network_version, PriceList};
use crate::kernel::Result;
use crate::state_tree::{ActorState, StateTree};

mod default;

pub use default::DefaultMachine;

pub mod limiter;
mod manifest;

pub use manifest::Manifest;

mod engine;

pub use engine::{Engine, EngineConfig, MultiEngine};
use fvm_shared::event::StampedEvent;

use self::limiter::ExecMemory;

mod boxed;

pub const REWARD_ACTOR_ID: ActorID = 2;

/// Distinguished Account actor that is the destination of all burnt funds.
pub const BURNT_FUNDS_ACTOR_ID: ActorID = 99;

#[derive(Clone, Copy, Debug)]
pub struct ChainID(u64);

impl ChainID {
    pub const ZERO: Self = Self(0);
    pub const WALLABY: Self = Self(31415);
    pub const CALIBRATION: Self = Self(314159);
    pub const CATERPILLER_BUTTERFLY: Self = Self(3141592);
}

impl From<u64> for ChainID {
    fn from(src: u64) -> Self {
        Self(src)
    }
}

impl From<ChainID> for u64 {
    fn from(src: ChainID) -> Self {
        src.0
    }
}

/// The Machine is the top-level object of the FVM.
///
/// The Machine operates at a concrete network version and epoch, over an
/// initial state root, all of which must be specified at instantiation time.
///
/// It is instantiated by the node with concrete Blockstore and Externs
/// implementations.
///
/// The Machine is designed to be used in conjunction with the Executor, which
/// is bound to a concrete Machine and is in charge of facilitating message
/// execution.
pub trait Machine: 'static {
    type Blockstore: Blockstore;
    type Externs: Externs;
    type Limiter: ResourceLimiter + ExecMemory;

    /// Returns the underlying WASM engine. Cloning it will simply create a new handle with a
    /// static lifetime.
    fn engine(&self) -> &Engine;

    /// Returns a reference to the machine's blockstore.
    fn blockstore(&self) -> &Self::Blockstore;

    /// Returns a reference to the machine context: static information about the current execution
    /// context.
    fn context(&self) -> &MachineContext;

    /// Returns a reference to all "node" supplied APIs.
    fn externs(&self) -> &Self::Externs;

    /// Returns the builtin actor index.
    fn builtin_actors(&self) -> &Manifest;

    /// Returns an immutable reference to the state tree.
    fn state_tree(&self) -> &StateTree<Self::Blockstore>;

    /// Returns a mutable reference to the state tree.
    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore>;

    /// Creates an uninitialized actor.
    fn create_actor(&mut self, addr: &Address, act: ActorState) -> Result<ActorID>;

    /// Transfers tokens from one actor to another.
    ///
    /// If either the receiver or the sender do not exist, this method fails with a FATAL error.
    /// Otherwise, if the amounts are invalid, etc., it fails with a syscall error.
    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()>;

    /// Flushes the state-tree and returns the new root CID.
    fn flush(&mut self) -> Result<Cid> {
        self.state_tree_mut().flush()
    }

    /// Consumes the machine and returns the owned blockstore.
    fn into_store(self) -> Self::Blockstore;

    /// Returns a generated ID of a machine
    fn machine_id(&self) -> &str;

    /// Creates a new limiter to track the resources of a message execution.
    fn new_limiter(&self) -> Self::Limiter;

    /// Commits the events to the machine by building the events AMT, and making sure that events
    /// are written to the store.
    fn commit_events(&self, events: &[StampedEvent]) -> Result<Option<Cid>>;
}

/// Network-level settings. Except when testing locally, changing any of these likely requires a
/// network upgrade.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// The network version at epoch
    pub network_version: NetworkVersion,

    /// The Chain ID of the network.
    ///
    /// DEFAULT: 0 (Invalid)
    pub chain_id: ChainID,

    /// The maximum call depth.
    ///
    /// DEFAULT: 1024
    pub max_call_depth: u32,

    /// The maximum number of elements on wasm stack
    /// DEFAULT: 64Ki (512KiB of u64 elements)
    pub max_wasm_stack: u32,

    /// Maximum size of memory of any Wasm instance, ie. each level of the recursion, in bytes.
    ///
    /// DEFAULT: 512MiB
    pub max_inst_memory_bytes: u64,

    /// Maximum size of memory used during the entire (recursive) message execution.
    ///
    /// DEFAULT: 2GiB
    pub max_exec_memory_bytes: u64,

    /// An override for builtin-actors. If specified, this should be the CID of a builtin-actors
    /// "manifest".
    ///
    /// DEFAULT: `None`
    pub builtin_actors_override: Option<Cid>,

    /// Enable actor debugging.
    ///
    /// DEFAULT: `false`
    pub actor_debugging: bool,

    /// The price list.
    ///
    /// DEFAULT: The price-list for the current network version.
    pub price_list: &'static PriceList,

    /// Actor redirects for debug execution
    pub actor_redirect: Vec<(Cid, Cid)>,
}

impl NetworkConfig {
    /// Create a new network config for the given network version.
    pub fn new(network_version: NetworkVersion) -> Self {
        NetworkConfig {
            chain_id: ChainID::ZERO,
            network_version,
            max_call_depth: 1024,
            max_wasm_stack: 2048,
            max_inst_memory_bytes: 512 * (1 << 20),
            max_exec_memory_bytes: 2 * (1 << 30),
            actor_debugging: false,
            builtin_actors_override: None,
            price_list: price_list_by_network_version(network_version),
            actor_redirect: vec![],
        }
    }

    /// Enable actor debugging. This is a consensus-critical option (affects gas usage) so it should
    /// only be enabled for local testing or as a network-wide parameter.
    pub fn enable_actor_debugging(&mut self) -> &mut Self {
        self.actor_debugging = true;
        self
    }

    /// Override actors with the specific manifest. This is primarily useful for testing, or
    /// networks prior to NV16 (where the actor's "manifest" isn't specified on-chain).
    pub fn override_actors(&mut self, manifest: Cid) -> &mut Self {
        self.builtin_actors_override = Some(manifest);
        self
    }

    /// Set actor redirects for debug execution
    pub fn redirect_actors(&mut self, actor_redirect: Vec<(Cid, Cid)>) -> &mut Self {
        self.actor_redirect = actor_redirect;
        self
    }

    /// Create a ['MachineContext'] for a given epoch, timestamp, and initial state.
    pub fn for_epoch(
        &self,
        epoch: ChainEpoch,
        timestamp: u64,
        initial_state: Cid,
    ) -> MachineContext {
        MachineContext {
            network: self.clone(),
            base_fee: TokenAmount::zero(),
            epoch,
            timestamp,
            initial_state_root: initial_state,
            circ_supply: fvm_shared::TOTAL_FILECOIN.clone(),
            tracing: false,
        }
    }

    /// Set Chain ID of the network.
    pub fn chain_id(&mut self, id: ChainID) -> &mut Self {
        self.chain_id = id;
        self
    }
}

/// Per-epoch machine context.
#[derive(Clone, Debug, Deref, DerefMut)]
pub struct MachineContext {
    /// Network-level settings.
    #[deref]
    #[deref_mut]
    pub network: NetworkConfig,

    /// The current epoch
    ///
    /// Default: 0
    pub epoch: ChainEpoch,

    /// The UNIX timestamp (in seconds) of the current tipset
    ///
    /// Default: 0
    pub timestamp: u64,

    /// The base fee that's in effect when the Machine runs.
    ///
    /// Default: 0.
    pub base_fee: TokenAmount,

    /// The initial state root on which this block is based.
    pub initial_state_root: Cid,

    /// v15 and onwards: The amount of FIL that has vested from genesis actors.
    /// v14 and earlier: The amount of FIL that has vested from genesis msigs
    /// (the remainder of the circ supply must be calculated by the FVM)
    ///
    /// DEFAULT: Total FIL supply (likely not what you want).
    pub circ_supply: TokenAmount,

    /// Whether or not to produce execution traces in the returned result.
    /// Not consensus-critical, but has a performance impact.
    pub tracing: bool,
}

impl MachineContext {
    /// Sets [`MachineContext::base_fee`].
    pub fn set_base_fee(&mut self, amt: TokenAmount) -> &mut Self {
        self.base_fee = amt;
        self
    }

    /// Set [`MachineContext::circ_supply`].
    pub fn set_circulating_supply(&mut self, amt: TokenAmount) -> &mut Self {
        self.circ_supply = amt;
        self
    }

    /// Enable execution traces. [`MachineContext::tracing`].
    pub fn enable_tracing(&mut self) -> &mut Self {
        self.tracing = true;
        self
    }
}
