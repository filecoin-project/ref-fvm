use std::collections::BTreeMap;

use anyhow::{anyhow, Context as _};
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::blockstore::{Blockstore, Buffered, CborStore};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{actor, ActorID};
use log::debug;
use num_traits::{Signed, Zero};
use wasmtime::Module;

use super::{Engine, Machine, MachineContext};
use crate::blockstore::BufferedBlockstore;
use crate::externs::Externs;
use crate::gas::price_list_by_epoch;
use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::machine::BuiltinActorIndex;
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
    /// Mapping of CIDs to builtin actor types.
    builtin_actors: BTreeMap<Cid, actor::builtin::Type>,
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
        engine: Engine,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        fil_vested: TokenAmount,
        network_version: NetworkVersion,
        state_root: Cid,
        builtin_actors_idx: Cid,
        blockstore: B,
        externs: E,
    ) -> anyhow::Result<Self> {
        debug!(
            "initializing a new machine, epoch={}, base_fee={}, nv={:?}, root={}",
            epoch, &base_fee, network_version, state_root
        );

        if network_version != NetworkVersion::V14 {
            return Err(anyhow!("unsupported network version: {}", network_version));
        }

        let context = MachineContext {
            epoch,
            base_fee,
            fil_vested,
            network_version,
            initial_state_root: state_root,
            price_list: price_list_by_epoch(epoch),
            debug: config.debug,
        };

        if !blockstore
            .has(&context.initial_state_root)
            .context("failed to load initial state-root")?
        {
            return Err(anyhow!(
                "blockstore doesn't have the initial state-root {}",
                &context.initial_state_root
            ));
        }

        // Load the built-in actor index.
        let builtin_actors: BTreeMap<actor::builtin::Type, Cid> = blockstore
            .get_cbor(&builtin_actors_idx)
            .context("failed to load built-in actor index")?
            .ok_or_else(|| {
                anyhow!(
                    "blockstore doesn't contain builtin actors index with CID {}",
                    &builtin_actors_idx
                )
            })?;

        // Invert the map so that it's CID => Type.
        let builtin_actors: BTreeMap<_, _> = builtin_actors
            .into_iter()
            .map(|(typ, cid)| (cid, typ))
            .collect();

        let bstore = BufferedBlockstore::new(blockstore);

        let state_tree = StateTree::new_from_root(bstore, &context.initial_state_root)?;

        Ok(DefaultMachine {
            config,
            context,
            engine,
            externs,
            state_tree,
            builtin_actors,
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

    fn engine(&self) -> &wasmtime::Engine {
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

    fn builtin_actors(&self) -> &BuiltinActorIndex {
        &self.builtin_actors
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

    fn load_module(&self, code: &Cid) -> Result<Module> {
        // If we've already loaded the module, return it.
        if let Some(code) = self.engine.get(code) {
            return Ok(code);
        }

        // Load the Wasm bytecode for the actor from the blockstore.
        // Fail if not found.
        let binary = self
            .state_tree
            .store()
            .get(code)
            .or_fatal()?
            .context("missing wasm binary")
            .or_fatal()?;

        // Then compile & cache it.
        let module = self.engine.load(code, binary.as_slice()).or_fatal()?;
        Ok(module)
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
