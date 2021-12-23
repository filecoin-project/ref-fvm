use fvm_shared::{
    address::Address, econ::TokenAmount, encoding::RawBytes, receipt::Receipt, ActorID, MethodNum,
};

use crate::{
    gas::{GasCharge, GasTracker, PriceList},
    kernel::Result,
    machine::{CallError, Machine, MachineContext},
    state_tree::StateTree,
};

mod default;
pub use default::DefaultCallManager;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

pub trait CallManager: 'static {
    type Machine: Machine;

    /// Construct a new call manager. This should be called by the machine.
    fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self
    where
        Self: Sized;

    /// Send a message.
    fn send(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: &RawBytes,
        value: &TokenAmount,
    ) -> Result<Receipt>;

    /// Execute some operation (usually a send) within a transaction.
    fn with_transaction(&mut self, f: impl FnOnce(&mut Self) -> Result<Receipt>)
        -> Result<Receipt>;

    /// Finishes execution, returning the gas used and the machine.
    fn finish(self) -> (i64, Vec<CallError>, Self::Machine);

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

    /// Record an error in the current backtrace.
    fn push_error(&mut self, e: CallError);

    /// Clear the current backtrace.
    fn clear_error(&mut self);

    /// Returns the current price list.
    fn price_list(&self) -> &PriceList {
        self.machine().context().price_list()
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

    /// Returns the available gas.
    fn gas_available(&self) -> i64 {
        self.gas_tracker().gas_available()
    }

    /// Getter for gas used.
    fn gas_used(&self) -> i64 {
        self.gas_tracker().gas_used()
    }
}
