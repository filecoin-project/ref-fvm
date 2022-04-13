use cid::Cid;
use derive_more::{Deref, DerefMut};
use fvm_ipld_blockstore::Blockstore;
use fvm_shared::actor::builtin::Manifest;
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

mod engine;

pub use engine::Engine;

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
    // TODO: Remove
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
}

/// Network-level settings. Except when testing locally, changing any of these likely requires a
/// network upgrade.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// The network version at epoch
    pub network_version: NetworkVersion,

    /// The maximum call depth.
    ///
    /// DEFAULT: 4096
    pub max_call_depth: u32,

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
}

impl NetworkConfig {
    /// Create a new network config for the given network version.
    pub fn new(network_version: NetworkVersion) -> Self {
        NetworkConfig {
            network_version,
            max_call_depth: 4096,
            actor_debugging: false,
            builtin_actors_override: None,
            price_list: price_list_by_network_version(network_version),
        }
    }

    /// Enable actor debugging. This is a consensus-critical option (affects gas usage) so it should
    /// only be enabled for local testing or as a network-wide parameter.
    pub fn enable_actor_debugging(&mut self, enabled: bool) -> &mut Self {
        self.actor_debugging = enabled;
        self
    }

    /// Override actors with the specific manifest. This is primarily useful for testing, or
    /// networks prior to NV16 (where the actor's "manifest" isn't specified on-chain).
    pub fn override_actors(&mut self, manifest: Cid) -> &mut Self {
        self.builtin_actors_override = Some(manifest);
        self
    }

    /// Create a [`MachineContext`] for a given `epoch` with the specified `initial_state`.
    pub fn for_epoch(&self, epoch: ChainEpoch, initial_state: Cid) -> MachineContext {
        MachineContext {
            network: self.clone(),
            epoch,
            initial_state_root: initial_state,
            base_fee: TokenAmount::zero(),
            circ_supply: fvm_shared::TOTAL_FILECOIN.clone(),
        }
    }
}

/// Per-epoch machine context.
#[derive(Clone, Debug, Deref, DerefMut)]
pub struct MachineContext {
    /// Network-level settings.
    #[deref]
    #[deref_mut]
    pub network: NetworkConfig,

    /// The epoch at which the Machine runs.
    pub epoch: ChainEpoch,

    /// The initial state root on which this block is based.
    pub initial_state_root: Cid,

    /// The base fee that's in effect when the Machine runs.
    ///
    /// Default: 0.
    pub base_fee: TokenAmount,

    /// v15 and onwards: The amount of FIL that has vested from genesis actors.
    /// v14 and earlier: The amount of FIL that has vested from genesis msigs
    /// (the remainder of the circ supply must be calculated by the FVM)
    ///
    /// DEFAULT: Total FIL supply (likely not what you want).
    pub circ_supply: TokenAmount,
}

impl MachineContext {
    /// Sets [`EpochContext::base_fee`].
    pub fn set_base_fee(&mut self, amt: TokenAmount) -> &mut Self {
        self.base_fee = amt;
        self
    }

    /// Set [`EpochContext::circ_supply`].
    pub fn set_circulating_supply(&mut self, amt: TokenAmount) -> &mut Self {
        self.circ_supply = amt;
        self
    }
}
