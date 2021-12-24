use blockstore::Blockstore;
use cid::Cid;
use wasmtime::{Engine, Module};

use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;

use crate::externs::Externs;
use crate::gas::PriceList;
use crate::kernel::Result;
use crate::state_tree::{ActorState, StateTree};
use crate::Config;

mod default;
pub use default::DefaultMachine;

mod boxed;

pub const REWARD_ACTOR_ADDR: Address = Address::new_id(2);
/// Distinguished AccountActor that is the destination of all burnt funds.
pub const BURNT_FUNDS_ACTOR_ADDR: Address = Address::new_id(99);

pub trait Machine: 'static {
    type Blockstore: Blockstore;
    type Externs: Externs;

    fn engine(&self) -> &Engine;

    fn config(&self) -> Config;

    fn blockstore(&self) -> &Self::Blockstore;

    fn context(&self) -> &MachineContext;

    fn externs(&self) -> &Self::Externs;

    fn state_tree(&self) -> &StateTree<Self::Blockstore>;

    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore>;

    /// Creates an uninitialized actor.
    // TODO: Remove
    fn create_actor(&mut self, addr: &Address, act: ActorState) -> Result<ActorID>;

    fn load_module(&self, code: &Cid) -> Result<Module>;

    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()>;
}

/// An error included in a message's backtrace on failure.
#[derive(Clone, Debug)]
pub struct CallError {
    /// The source of the error or 0 for a syscall error.
    pub source: ActorID,
    /// The error code.
    pub code: ExitCode,
    /// The error message.
    pub message: String,
}

/// Execution context supplied to the machine. All fields are private.
/// Epoch and base fee cannot be mutated. The state_root corresponds to the
/// initial state root, and gets updated internally with every message execution.
pub struct MachineContext {
    /// The epoch at which the Machine runs.
    epoch: ChainEpoch,
    /// The base fee that's in effect when the Machine runs.
    base_fee: TokenAmount,
    /// The initial state root.
    initial_state_root: Cid,
    /// The price list.
    price_list: PriceList,
    /// The network version at epoch
    network_version: NetworkVersion,
}

impl MachineContext {
    fn new(
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        state_root: Cid,
        price_list: PriceList,
        network_version: NetworkVersion,
    ) -> MachineContext {
        MachineContext {
            epoch,
            base_fee,
            price_list,
            network_version,
            initial_state_root: state_root,
        }
    }

    pub fn epoch(&self) -> ChainEpoch {
        self.epoch
    }

    pub fn base_fee(&self) -> &TokenAmount {
        &self.base_fee
    }

    pub fn state_root(&self) -> Cid {
        self.initial_state_root
    }

    pub fn network_version(&self) -> NetworkVersion {
        self.network_version
    }

    pub fn price_list(&self) -> &PriceList {
        &self.price_list
    }
}
