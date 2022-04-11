use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_shared::actor::builtin::Manifest;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;

use crate::externs::Externs;
use crate::gas::PriceList;
use crate::kernel::Result;
use crate::state_tree::{ActorState, StateTree};
use crate::Config;

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

    /// Returns the FVM's configuration.
    fn config(&self) -> &Config;

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
    fn consume(self) -> Self::Blockstore;
}

/// Execution context supplied to the machine.
#[derive(Clone, Debug)]
pub struct MachineContext {
    /// The epoch at which the Machine runs.
    pub epoch: ChainEpoch,
    /// The base fee that's in effect when the Machine runs.
    pub base_fee: TokenAmount,
    /// v15 and onwards: The amount of FIL that has vested from genesis actors.
    /// v14 and earlier: The amount of FIL that has vested from genesis msigs
    /// (the remainder of the circ supply must be calculated by the FVM)
    pub circ_supply: TokenAmount,
    /// The initial state root on which this block is based.
    pub initial_state_root: Cid,
    /// The price list.
    pub price_list: &'static PriceList,
    /// The network version at epoch
    pub network_version: NetworkVersion,
    /// Whether debug mode is enabled or not.
    pub debug: bool,
}
