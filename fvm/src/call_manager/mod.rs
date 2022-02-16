use std::time::Duration;

use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ExitCode;
use fvm_shared::{ActorID, MethodNum};

use crate::gas::{GasCharge, GasTracker, PriceList};
use crate::kernel::Result;
use crate::machine::{Machine, MachineContext};
use crate::state_tree::StateTree;
use crate::Kernel;

pub mod backtrace;
pub use backtrace::Backtrace;
mod default;
pub use default::DefaultCallManager;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

pub trait CallManager: 'static {
    type Machine: Machine;

    fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self;

    /// Send a message.
    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: &RawBytes,
        value: &TokenAmount,
    ) -> Result<InvocationResult>;

    /// Execute some operation (usually a send) within a transaction.
    fn with_transaction(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<InvocationResult>,
    ) -> Result<InvocationResult>;

    /// Finishes execution, returning the gas used and the machine.
    fn finish(self) -> (i64, backtrace::Backtrace, WasmStats, Self::Machine);

    /// Returns a reference to the machine.
    fn machine(&self) -> &Self::Machine;
    /// Returns a mutable reference to the machine.
    fn machine_mut(&mut self) -> &mut Self::Machine;

    /// Returns reference to the gas tracker.
    fn gas_tracker(&self) -> &GasTracker;
    /// Returns a mutable reference to the gas tracker.
    fn gas_tracker_mut(&mut self) -> &mut GasTracker;

    /// Getter for origin actor.
    fn origin(&self) -> Address;

    /// Getter for message nonce.
    fn nonce(&self) -> u64;

    /// Gets and increment the call-stack actor creation index.
    fn next_actor_idx(&mut self) -> u64;

    /// Returns the current price list.
    fn price_list(&self) -> &PriceList {
        &self.machine().context().price_list
    }

    /// Returns the machine context.
    fn context(&self) -> &MachineContext {
        self.machine().context()
    }

    /// Returns the blockstore.
    fn blockstore(&self) -> &<Self::Machine as Machine>::Blockstore {
        self.machine().blockstore()
    }

    /// Returns the externs.
    fn externs(&self) -> &<Self::Machine as Machine>::Externs {
        self.machine().externs()
    }

    /// Returns the state tree.
    fn state_tree(&self) -> &StateTree<<Self::Machine as Machine>::Blockstore> {
        self.machine().state_tree()
    }

    /// Returns a mutable state-tree.
    fn state_tree_mut(&mut self) -> &mut StateTree<<Self::Machine as Machine>::Blockstore> {
        self.machine_mut().state_tree_mut()
    }

    /// Charge gas.
    fn charge_gas(&mut self, charge: GasCharge) -> Result<()> {
        self.gas_tracker_mut().charge_gas(charge)?;
        Ok(())
    }
}

/// The result of a method invocation.
pub enum InvocationResult {
    Return(RawBytes),
    Failure(ExitCode),
}

impl Default for InvocationResult {
    fn default() -> Self {
        Self::Return(Default::default())
    }
}

impl InvocationResult {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Return(_) => ExitCode::Ok,
            Self::Failure(e) => *e,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct WasmStats {
    /// Wasm fuel used over the course of the message execution.
    pub fuel_used: u64,
    /// Time spent inside wasm code.
    pub wasm_duration: Duration,
}
