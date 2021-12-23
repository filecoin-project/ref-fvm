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

/// A `StaticCallManager` is a `CallManager` that can be constructed without a factory. To use this
/// kind of call manager, either don't specify the factory in the machine, or specify `()`.
pub trait StaticCallManager: CallManager {
    /// Construct a new call manager without a factory.
    fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self
    where
        Self: Sized;
}

/// A `CallManagerFactory` is a factory for creating call managers.
///
/// The `Machine` will _usually_ give the `CallManager` everything it needs, but sometimes (e.g.,
/// for testing), you need something else.
pub trait CallManagerFactory<C>: Clone
where
    C: CallManager,
{
    /// Construct a new call manager with the current factory.
    fn make(self, machine: C::Machine, gas_limit: i64, origin: Address, nonce: u64) -> C
    where
        Self: Sized;
}

/// `CallManagerFactory` is implemented for `()` for all `StaticCallManager`s.
impl<C> CallManagerFactory<C> for ()
where
    C: StaticCallManager,
{
    fn make(self, machine: C::Machine, gas_limit: i64, origin: Address, nonce: u64) -> C
    where
        Self: Sized,
    {
        C::new(machine, gas_limit, origin, nonce)
    }
}
