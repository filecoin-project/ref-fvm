// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::{ActorID, MethodNum};

use crate::engine::Engine;
use crate::gas::{Gas, GasCharge, GasTimer, GasTracker, PriceList};
use crate::kernel::{self, Result};
use crate::machine::{Machine, MachineContext};
use crate::state_tree::ActorState;
use crate::Kernel;

pub mod backtrace;
mod state_access_tracker;
pub use backtrace::Backtrace;

mod default;

pub use default::DefaultCallManager;
use fvm_shared::event::StampedEvent;

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
    #[allow(clippy::too_many_arguments)]
    fn new(
        machine: Self::Machine,
        engine: Engine,
        gas_limit: u64,
        origin: ActorID,
        origin_address: Address,
        receiver: Option<ActorID>,
        receiver_address: Address,
        nonce: u64,
        gas_premium: TokenAmount,
    ) -> Self;

    /// Send a message. The type parameter `K` specifies the the _kernel_ on top of which the target
    /// actor should execute.
    #[allow(clippy::too_many_arguments)]
    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: Option<kernel::Block>,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        read_only: bool,
    ) -> Result<InvocationResult>;

    /// Execute some operation (usually a send) within a transaction.
    fn with_transaction(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<InvocationResult>,
    ) -> Result<InvocationResult>;

    /// Finishes execution, returning the gas used, machine, and exec trace if requested.
    fn finish(self) -> (Result<FinishRet>, Self::Machine);

    /// Returns a reference to the machine.
    fn machine(&self) -> &Self::Machine;
    /// Returns a mutable reference to the machine.
    fn machine_mut(&mut self) -> &mut Self::Machine;

    /// Returns a reference to the engine
    fn engine(&self) -> &Engine;

    /// Returns a reference to the gas tracker.
    fn gas_tracker(&self) -> &GasTracker;

    /// Returns the gas premium paid by the currently executing message.
    fn gas_premium(&self) -> &TokenAmount;

    /// Getter for origin actor.
    fn origin(&self) -> ActorID;

    /// Get the actor address (f2) that will should be assigned to the next actor created.
    ///
    /// This method doesn't have any side-effects and will continue to return the same address until
    /// `create_actor` is called next.
    fn next_actor_address(&self) -> Address;

    /// Create a new actor with the given code CID, actor ID, and delegated address. This method
    /// does not register the actor with the init actor. It just creates it in the state-tree.
    ///
    /// It handles all appropriate gas charging for creating new actors.
    fn create_actor(
        &mut self,
        code_id: Cid,
        actor_id: ActorID,
        delegated_address: Option<Address>,
    ) -> Result<()>;

    /// Resolve an address into an actor ID, charging gas as appropriate.
    fn resolve_address(&self, address: &Address) -> Result<Option<ActorID>>;

    /// Sets an actor in the state-tree, charging gas as appropriate. Use `create_actor` if you want
    /// to create a new actor.
    fn set_actor(&mut self, id: ActorID, state: ActorState) -> Result<()>;

    /// Looks up an actor in the state-tree, charging gas as appropriate.
    fn get_actor(&self, id: ActorID) -> Result<Option<ActorState>>;

    /// Deletes an actor from the state-tree, charging gas as appropriate.
    fn delete_actor(&mut self, id: ActorID) -> Result<()>;

    /// Transfers tokens from one actor to another, charging gas as appropriate.
    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()>;

    /// Getter for message nonce.
    fn nonce(&self) -> u64;

    /// Gets the total invocations done on this call stack.
    fn invocation_count(&self) -> u64;

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

    /// Charge gas.
    fn charge_gas(&self, charge: GasCharge) -> Result<GasTimer> {
        self.gas_tracker().apply_charge(charge)
    }

    /// Limit memory usage throughout a message execution.
    fn limiter_mut(&mut self) -> &mut <Self::Machine as Machine>::Limiter;

    /// Appends an event to the event accumulator.
    fn append_event(&mut self, evt: StampedEvent);
}

/// The result of a method invocation.
#[derive(Clone, Debug)]
pub struct InvocationResult {
    /// The exit code (0 for success).
    pub exit_code: ExitCode,
    /// The return value, if any.
    pub value: Option<kernel::Block>,
}

impl Default for InvocationResult {
    fn default() -> Self {
        Self {
            value: None,
            exit_code: ExitCode::OK,
        }
    }
}

/// The returned values upon finishing a call manager.
pub struct FinishRet {
    pub gas_used: u64,
    pub backtrace: Backtrace,
    pub exec_trace: ExecutionTrace,
    pub events: Vec<StampedEvent>,
    pub events_root: Option<Cid>,
}
