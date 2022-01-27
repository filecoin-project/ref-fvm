use anyhow::{anyhow, Context as _};
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::blockstore::{Blockstore, Buffered};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use log::Level::Trace;
use log::{debug, log_enabled, trace};
use num_traits::{Signed, Zero};
use wasmtime::{Engine, Module};

use super::{Machine, MachineContext};
use crate::blockstore::BufferedBlockstore;
use crate::externs::Externs;
use crate::gas::price_list_by_epoch;
use crate::init_actor::{State, INIT_ACTOR_ADDR};
use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::state_tree::{ActorState, StateTree};
use crate::{syscall_error, Config};

/// The core of the FVM.
///
/// ## Generic types
/// * B => Blockstore.
/// * E => Externs.
/// * K => Kernel.
pub struct DefaultMachine<B, E> {
    config: Config,
    /// The context for the execution.
    context: MachineContext,
    /// The wasmtime engine is created on construction of the DefaultMachine, and
    /// is dropped when the DefaultMachine is dropped.
    engine: Engine,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin node.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    ///
    /// Owned.
    state_tree: StateTree<BufferedBlockstore<B>>,
}

impl<B, E> DefaultMachine<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    // ISSUE: #249
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Config,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        base_circ_supply: TokenAmount,
        network_version: NetworkVersion,
        state_root: Cid,
        blockstore: B,
        externs: E,
    ) -> anyhow::Result<Self> {
        debug!(
            "initializing a new machine, epoch={}, base_fee={}, nv={:?}, root={}",
            epoch, &base_fee, network_version, state_root
        );
        let context = MachineContext {
            epoch,
            base_fee,
            base_circ_supply,
            network_version,
            initial_state_root: state_root,
            price_list: price_list_by_epoch(epoch),
            debug: config.debug,
        };

        // Initialize the WASM engine.
        let engine = Engine::new(&config.engine)?;

        if !blockstore
            .has(&context.initial_state_root)
            .context("failed to load initial state-root")?
        {
            return Err(anyhow!(
                "blockstore doesn't have the initial state-root {}",
                &context.initial_state_root
            ));
        }

        let bstore = BufferedBlockstore::new(blockstore);

        let state_tree = StateTree::new_from_root(bstore, &context.initial_state_root)?;

        if log_enabled!(Trace) {
            trace_actors(&state_tree);
        }

        Ok(DefaultMachine {
            config,
            context,
            engine,
            externs,
            state_tree,
        })
    }
}

/// Print a trace of all actors and their state roots.
#[cold]
fn trace_actors<B: Blockstore>(state_tree: &StateTree<B>) {
    trace!("init actor address: {}", INIT_ACTOR_ADDR.to_string());

    state_tree
        .for_each(|addr, actor_state| {
            trace!(
                "state tree: {} ({:?}): {:?}",
                addr.to_string(),
                addr.to_bytes(),
                actor_state
            );
            Ok(())
        })
        .unwrap(); // This will never panic.

    match State::load(state_tree) {
        Ok((state, _)) => trace!("init actor: {:?}", state),
        Err(err) => trace!("init actor: failed to load state; err={:?}", err),
    }
}

impl<B, E> Machine for DefaultMachine<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    type Blockstore = BufferedBlockstore<B>;
    type Externs = E;

    fn engine(&self) -> &Engine {
        &self.engine
    }

    fn config(&self) -> &Config {
        &self.config
    }

    fn blockstore(&self) -> &Self::Blockstore {
        self.state_tree.store()
    }

    fn context(&self) -> &MachineContext {
        &self.context
    }

    fn externs(&self) -> &Self::Externs {
        &self.externs
    }

    fn state_tree(&self) -> &StateTree<Self::Blockstore> {
        &self.state_tree
    }

    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
        &mut self.state_tree
    }

    /// Flushes the state-tree and returns the new root CID.
    ///
    /// This method also flushes all new blocks (reachable from this new root CID) from the write
    /// buffer into the underlying blockstore (the blockstore with which the machine was
    /// constructed).
    fn flush(&mut self) -> Result<Cid> {
        let root = self.state_tree_mut().flush()?;
        self.blockstore().flush(&root).or_fatal()?;
        Ok(root)
    }

    /// Creates an uninitialized actor.
    // TODO: Remove
    fn create_actor(&mut self, addr: &Address, act: ActorState) -> Result<ActorID> {
        let state_tree = self.state_tree_mut();

        let addr_id = state_tree
            .register_new_address(addr)
            .context("failed to register new address")
            .or_fatal()?;

        state_tree
            .set_actor(&Address::new_id(addr_id), act)
            .context("failed to set actor")
            .or_fatal()?;
        Ok(addr_id)
    }

    #[cfg(feature = "builtin_actors")]
    fn load_module(&self, code: &Cid) -> Result<Module> {
        use anyhow::Context;
        // TODO: cache compiled code, and modules?
        let binary = if code == &*crate::builtin::SYSTEM_ACTOR_CODE_ID {
            fvm_actor_system::wasm::WASM_BINARY
        } else if code == &*crate::builtin::INIT_ACTOR_CODE_ID {
            fvm_actor_init::wasm::WASM_BINARY
        } else if code == &*crate::builtin::CRON_ACTOR_CODE_ID {
            fvm_actor_cron::wasm::WASM_BINARY
        } else if code == &*crate::builtin::ACCOUNT_ACTOR_CODE_ID {
            fvm_actor_account::wasm::WASM_BINARY
        } else if code == &*crate::builtin::POWER_ACTOR_CODE_ID {
            fvm_actor_power::wasm::WASM_BINARY
        } else if code == &*crate::builtin::MINER_ACTOR_CODE_ID {
            fvm_actor_miner::wasm::WASM_BINARY
        } else if code == &*crate::builtin::MARKET_ACTOR_CODE_ID {
            fvm_actor_market::wasm::WASM_BINARY
        } else if code == &*crate::builtin::PAYCH_ACTOR_CODE_ID {
            fvm_actor_paych::wasm::WASM_BINARY
        } else if code == &*crate::builtin::MULTISIG_ACTOR_CODE_ID {
            fvm_actor_multisig::wasm::WASM_BINARY
        } else if code == &*crate::builtin::REWARD_ACTOR_CODE_ID {
            fvm_actor_reward::wasm::WASM_BINARY
        } else if code == &*crate::builtin::VERIFREG_ACTOR_CODE_ID {
            fvm_actor_verifreg::wasm::WASM_BINARY
        } else {
            None
        };

        let binary = binary.context("missing wasm binary").or_fatal()?;
        let module = Module::new(&self.engine, binary).or_fatal()?;
        Ok(module)
    }

    #[cfg(not(feature = "builtin_actors"))]
    fn load_module(&self, _code: &Cid) -> Result<Module> {
        Err(crate::kernel::ExecutionError::Fatal(anyhow!(
            "built-in actors not embedded; please run build enabling the builtin_actors feature"
        )))
    }

    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
        if from == to || value.is_zero() {
            return Ok(());
        }

        if value.is_negative() {
            return Err(syscall_error!(IllegalArgument;
                "attempted to transfer negative transfer value {}", value)
            .into());
        }

        // If the from actor doesn't exist, we return "insufficient funds" to distinguish between
        // that and the case where the _receiving_ actor doesn't exist.
        let mut from_actor = self
            .state_tree
            .get_actor_id(from)?
            .context("cannot transfer from non-existent sender")
            .or_error(ErrorNumber::InsufficientFunds)?;

        let mut to_actor = self
            .state_tree
            .get_actor_id(to)?
            .context("cannot transfer to non-existent receiver")
            .or_error(ErrorNumber::NotFound)?;

        from_actor.deduct_funds(value).map_err(|e| {
            syscall_error!(InsufficientFunds;
                           "transfer failed when deducting funds ({}) from balance ({}): {}",
                           value, &from_actor.balance, e)
        })?;
        to_actor.deposit_funds(value);

        self.state_tree.set_actor_id(from, from_actor)?;
        self.state_tree.set_actor_id(to, to_actor)?;

        log::trace!("transfered {} from {} to {}", value, from, to);

        Ok(())
    }

    fn consume(self) -> Self::Blockstore {
        self.state_tree.consume()
    }
}
