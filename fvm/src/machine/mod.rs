use cid::Cid;
use derive_more::{Deref, DerefMut};
use fvm_ipld_blockstore::Blockstore;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use num_traits::Zero;

use crate::externs::Externs;
use crate::gas::{price_list_by_network_version, PriceList};
use crate::kernel::Result;
use crate::state_tree::{ActorState, StateTree};

mod default;

pub use default::DefaultMachine;

mod manifest;

pub use manifest::Manifest;

mod engine;

pub use engine::{Engine, EngineConfig, MultiEngine};

mod boxed;

pub const REWARD_ACTOR_ADDR: Address = Address::new_id(2);

/// Distinguished Account actor that is the destination of all burnt funds.
pub const BURNT_FUNDS_ACTOR_ADDR: Address = Address::new_id(99);

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
}

/// Network-level settings. Except when testing locally, changing any of these likely requires a
/// network upgrade.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// The network version at epoch
    pub network_version: NetworkVersion,

    /// The maximum call depth.
    ///
    /// DEFAULT: 1024
    pub max_call_depth: u32,

    /// The maximum number of elements on wasm stack
    /// DEFAULT: 64Ki (512KiB of u64 elements)
    pub max_wasm_stack: u32,

    /// Maximum size of memory of any Wasm instance, ie. each level of the recursion, in bytes.
    ///
    /// DEFAULT: 4GB
    pub max_inst_memory_bytes: u64,

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
            network_version,
            max_call_depth: 1024,
            max_wasm_stack: 2048,
            max_inst_memory_bytes: 4 * (1 << 30),
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

    /// Create a [`MachineContext`] for a given `epoch` with the specified `initial_state`.
    pub fn for_epoch(&self, epoch: ChainEpoch, initial_state: Cid) -> MachineContext {
        MachineContext {
            network: self.clone(),
            network_context: NetworkContext {
                epoch,
                // TODO #933
                timestamp: 0,
                tipsets: vec![],
                base_fee: TokenAmount::zero(),
            },
            initial_state_root: initial_state,
            circ_supply: fvm_shared::TOTAL_FILECOIN.clone(),
            tracing: false,
        }
    }

    /// Create a ['MachineContext'] for a given network context with the specified `initial_state`
    pub fn for_network_context(
        &self,
        net_ctx: NetworkContext,
        initial_state: Cid,
    ) -> MachineContext {
        MachineContext {
            network: self.clone(),
            network_context: net_ctx,
            initial_state_root: initial_state,
            circ_supply: fvm_shared::TOTAL_FILECOIN.clone(),
            tracing: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NetworkContext {
    /// The network epoch at which the Machine runs.
    pub epoch: ChainEpoch,

    /// The UNIX timestamp (in seconds) of the current tipset
    pub timestamp: u64,

    /// The tipset CIDs for the last finality
    pub tipsets: Vec<Cid>,

    /// The base fee that's in effect when the Machine runs.
    ///
    /// Default: 0.
    pub base_fee: TokenAmount,
}

/// Per-epoch machine context.
#[derive(Clone, Debug, Deref, DerefMut)]
pub struct MachineContext {
    /// Network-level settings.
    #[deref]
    #[deref_mut]
    pub network: NetworkConfig,

    /// The network context with which the Machine runs.
    pub network_context: NetworkContext,

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
        self.network_context.base_fee = amt;
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
