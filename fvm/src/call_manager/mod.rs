use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
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

use crate::trace::ExecutionTrace;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

/// The `CallManager` manages a single call stack.
///
/// When a top-level message is executed:
///
/// 1. The [`crate::executor::Executor`] creates a [`CallManager`] for that message, giving itself
///    to the [`CallManager`].
/// 2. The [`crate::executor::Executor`] calls the specified actor/method using
///    [`CallManager::send()`].
/// 3. The [`CallManager`] then constructs a [`Kernel`] and executes the actual actor code on that
///    kernel.
/// 4. If an actor calls another actor, the [`Kernel`] will:
///    1. Detach the [`CallManager`] from itself.
///    2. Call [`CallManager::send()`] to execute the new message.
///    3. Re-attach the [`CallManager`].
///    4. Return.
pub trait CallManager: 'static {
    /// The underlying [`Machine`] on top of which this [`CallManager`] executes.
    type Machine: Machine;

    /// Construct a new call manager.
    fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self;

    /// Send a message. The type parameter `K` specifies the the _kernel_ on top of which the target
    /// actor should execute.
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

    /// Finishes execution, returning the gas used, machine, and exec trace if requested.
    fn finish(self) -> (FinishRet, Self::Machine);

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
        self.machine().context().price_list
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
        self.gas_tracker_mut().apply_charge(charge)?;
        Ok(())
    }
}

/// The result of a method invocation.
#[derive(Clone, Debug)]
pub enum InvocationResult {
    /// Indicates that the actor successfully returned. The value may be empty.
    Return(RawBytes),
    /// Indicates that the actor aborted with the given exit code.
    Failure(ExitCode),
}

impl Default for InvocationResult {
    fn default() -> Self {
        Self::Return(Default::default())
    }
}

impl InvocationResult {
    /// Get the exit code for the invocation result. [`ExitCode::Ok`] on success, or the exit code
    /// from the [`Failure`](InvocationResult::Failure) variant otherwise.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Return(_) => ExitCode::OK,
            Self::Failure(e) => *e,
        }
    }
}

/// The returned values upon finishing a call manager.
pub struct FinishRet {
    pub gas_used: i64,
    pub backtrace: Backtrace,
    pub exec_trace: ExecutionTrace,
}
