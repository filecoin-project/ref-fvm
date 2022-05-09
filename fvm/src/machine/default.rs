use std::ops::RangeInclusive;

use anyhow::{anyhow, Context as _};
use cid::Cid;
use fvm_ipld_blockstore::{Blockstore, Buffered};
use fvm_ipld_encoding::CborStore;
use fvm_shared::actor::builtin::{load_manifest, Manifest};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use log::debug;
use num_traits::Signed;

use super::{Engine, Machine, MachineContext};
use crate::blockstore::BufferedBlockstore;
use crate::externs::Externs;
use crate::init_actor::State as InitActorState;
use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::state_tree::{ActorState, StateTree};
use crate::syscall_error;
use crate::system_actor::State as SystemActorState;

pub struct DefaultMachine<B, E> {
    /// The initial execution context for this epoch.
    context: MachineContext,
    /// The WASM engine is created on construction of the DefaultMachine, and
    /// is dropped when the DefaultMachine is dropped.
    engine: Engine,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin client.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    ///
    /// Owned.
    state_tree: StateTree<BufferedBlockstore<B>>,
    /// Mapping of CIDs to builtin actor types.
    builtin_actors: Manifest,
}

impl<B, E> DefaultMachine<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    /// Create a new [`DefaultMachine`].
    ///
    /// # Arguments
    ///
    /// * `engine`: The global wasm [`Engine`] (engine, pooled resources, caches).
    /// * `context`: Machine execution [context][`MachineContext`] (system params, epoch, network
    ///    version, etc.).
    /// * `blockstore`: The underlying [blockstore][`Blockstore`] for reading/writing state.
    /// * `externs`: Client-provided ["external"][`Externs`] methods for accessing chain state.
    pub fn new(
        engine: &Engine,
        context: &MachineContext,
        blockstore: B,
        externs: E,
    ) -> anyhow::Result<Self> {
        const SUPPORTED_VERSIONS: RangeInclusive<NetworkVersion> =
            NetworkVersion::V15..=NetworkVersion::V16;

        debug!(
            "initializing a new machine, epoch={}, base_fee={}, nv={:?}, root={}",
            context.epoch, &context.base_fee, context.network_version, context.initial_state_root
        );

        if !SUPPORTED_VERSIONS.contains(&context.network_version) {
            return Err(anyhow!(
                "unsupported network version: {}",
                context.network_version
            ));
        }

        // Sanity check that the blockstore contains the supplied state root.
        if !blockstore
            .has(&context.initial_state_root)
            .context("failed to load initial state-root")?
        {
            return Err(anyhow!(
                "blockstore doesn't have the initial state-root {}",
                &context.initial_state_root
            ));
        }

        // Create a new state tree from the supplied root.
        let state_tree = {
            let bstore = BufferedBlockstore::new(blockstore);
            StateTree::new_from_root(bstore, &context.initial_state_root)?
        };

        // Load the built-in actors manifest.
        // TODO: Check that the actor bundle is sane for the network version.
        let (builtin_actors_cid, manifest_version) = match context.builtin_actors_override {
            Some(manifest_cid) => {
                let (version, cid): (u32, Cid) = state_tree
                    .store()
                    .get_cbor(&manifest_cid)?
                    .context("failed to load actor manifest")?;
                (cid, version)
            }
            None => {
                let (state, _) = SystemActorState::load(&state_tree)?;
                (state.builtin_actors, 1)
            }
        };
        let builtin_actors =
            load_manifest(state_tree.store(), &builtin_actors_cid, manifest_version)?;

        // Preload any uncached modules.
        // This interface works for now because we know all actor CIDs
        // ahead of time, but with user-supplied code, we won't have that
        // guarantee.
        // Skip preloading all builtin actors when testing. This results in JIT
        // bytecode to machine code compilation, and leads to faster tests.
        #[cfg(not(any(test, feature = "testing")))]
        engine.preload(state_tree.store(), builtin_actors.left_values())?;

        // preload user actors that have been installed
        let (init_state, _) = InitActorState::load(&state_tree)?;
        let installed_actors: Vec<Cid> = state_tree
            .store()
            .get_cbor(&init_state.installed_actors)?
            .context("failed to load installed actor list")?;
        engine.preload(state_tree.store(), &installed_actors)?;

        Ok(DefaultMachine {
            context: context.clone(),
            engine: engine.clone(),
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

    fn engine(&self) -> &Engine {
        &self.engine
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

    fn builtin_actors(&self) -> &Manifest {
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

    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
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

        if &from_actor.balance < value {
            return Err(syscall_error!(InsufficientFunds; "sender does not have funds to transfer (balance {}, transfer {})", &from_actor.balance, value).into());
        }

        if from == to {
            debug!("attempting to self-transfer: noop (from/to: {})", from);
            return Ok(());
        }

        let mut to_actor = self
            .state_tree
            .get_actor_id(to)?
            .context("cannot transfer to non-existent receiver")
            .or_error(ErrorNumber::NotFound)?;

        from_actor.deduct_funds(value)?;
        to_actor.deposit_funds(value);

        self.state_tree.set_actor_id(from, from_actor)?;
        self.state_tree.set_actor_id(to, to_actor)?;

        log::trace!("transferred {} from {} to {}", value, from, to);

        Ok(())
    }

    fn into_store(self) -> Self::Blockstore {
        self.state_tree.into_store()
    }
}
