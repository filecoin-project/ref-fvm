use anyhow::{anyhow, Context};
use blockstore::buffered::BufferedBlockstore;
use blockstore::{Blockstore, Buffered};
use cid::Cid;
use num_traits::Signed;
use wasmtime::{Engine, Module};

use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;

use crate::externs::Externs;
use crate::gas::price_list_by_epoch;
use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::state_tree::{ActorState, StateTree};
use crate::syscall_error;
use crate::Config;

use super::{Machine, MachineContext};

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
    pub fn new(
        config: Config,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        network_version: NetworkVersion,
        state_root: Cid,
        blockstore: B,
        externs: E,
    ) -> anyhow::Result<Self> {
        let context = MachineContext {
            epoch,
            base_fee,
            network_version,
            initial_state_root: state_root,
            price_list: price_list_by_epoch(epoch),
        };

        // Initialize the WASM engine.
        let engine = Engine::new(&config.engine)?;

        let bstore = BufferedBlockstore::new(blockstore);

        let state_tree = StateTree::new_from_root(bstore, &context.initial_state_root)?;

        Ok(DefaultMachine {
            config,
            context,
            engine,
            externs,
            state_tree,
        })
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

    fn load_module(&self, code: &Cid) -> Result<Module> {
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

    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
        if from == to {
            return Ok(());
        }
        if value.is_negative() {
            return Err(syscall_error!(SysErrForbidden;
                "attempted to transfer negative transfer value {}", value)
            .into());
        }

        // TODO: make sure these are actually fatal.
        let mut from_actor = self
            .state_tree
            .get_actor_id(from)?
            .ok_or_else(|| anyhow!("sender actor does not exist in state during transfer"))
            .or_fatal()?;

        let mut to_actor = self
            .state_tree
            .get_actor_id(to)?
            .ok_or_else(|| anyhow!("receiver actor does not exist in state during transfer"))
            .or_fatal()?;

        from_actor.deduct_funds(value).map_err(|e| {
            syscall_error!(SysErrInsufficientFunds;
                "transfer failed when deducting funds ({}): {}", value, e)
        })?;
        to_actor.deposit_funds(value);

        // TODO turn failures into fatal errors
        self.state_tree.set_actor_id(from, from_actor)?;
        // .map_err(|e| e.downcast_fatal("failed to set from actor"))?;
        // TODO turn failures into fatal errors
        self.state_tree.set_actor_id(to, to_actor)?;
        //.map_err(|e| e.downcast_fatal("failed to set to actor"))?;

        Ok(())
    }
}
