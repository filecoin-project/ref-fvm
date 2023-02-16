// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::ops::RangeInclusive;

use anyhow::{anyhow, Context as _};
use cid::Cid;
use fvm_ipld_blockstore::{Block, Blockstore, Buffered};
use fvm_ipld_encoding::{to_vec, CborStore, DAG_CBOR};
use fvm_shared::version::NetworkVersion;
use log::debug;
use multihash::Code::Blake2b256;

use super::{Machine, MachineContext};
use crate::blockstore::BufferedBlockstore;
use crate::externs::Externs;
#[cfg(feature = "m2-native")]
use crate::init_actor::State as InitActorState;
use crate::kernel::{ClassifyResult, Result};
use crate::machine::limiter::DefaultMemoryLimiter;
use crate::machine::Manifest;
use crate::state_tree::StateTree;
use crate::system_actor::State as SystemActorState;
use crate::EMPTY_ARR_CID;

lazy_static::lazy_static! {
    /// Pre-serialized block containing the empty array
    pub static ref EMPTY_ARRAY_BLOCK: Block<Vec<u8>> = {
        Block::new(DAG_CBOR, to_vec::<[(); 0]>(&[]).unwrap())
    };
}

pub struct DefaultMachine<B, E> {
    /// The initial execution context for this epoch.
    context: MachineContext,
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
    /// Somewhat unique ID of the machine consisting of (epoch, randomness)
    /// randomness is generated with `initial_state_root`
    id: String,
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
    /// * `context`: Machine execution [context][`MachineContext`] (system params, epoch, network
    ///    version, etc.).
    /// * `blockstore`: The underlying [blockstore][`Blockstore`] for reading/writing state.
    /// * `externs`: Client-provided ["external"][`Externs`] methods for accessing chain state.
    pub fn new(context: &MachineContext, blockstore: B, externs: E) -> anyhow::Result<Self> {
        #[cfg(not(feature = "hyperspace"))]
        const SUPPORTED_VERSIONS: RangeInclusive<NetworkVersion> =
            NetworkVersion::V18..=NetworkVersion::V18;

        #[cfg(feature = "hyperspace")]
        const SUPPORTED_VERSIONS: RangeInclusive<NetworkVersion> =
            NetworkVersion::V18..=NetworkVersion::MAX;

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

        put_empty_blocks(&blockstore)?;

        // Create a new state tree from the supplied root.
        let state_tree = {
            let bstore = BufferedBlockstore::new(blockstore);
            StateTree::new_from_root(bstore, &context.initial_state_root)?
        };

        // Load the built-in actors manifest.
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
            Manifest::load(state_tree.store(), &builtin_actors_cid, manifest_version)?;

        // 16 bytes is random _enough_
        let randomness: [u8; 16] = rand::random();

        Ok(DefaultMachine {
            context: context.clone(),
            externs,
            state_tree,
            builtin_actors,
            id: format!(
                "{}-{}",
                context.epoch,
                cid::multibase::encode(cid::multibase::Base::Base32Lower, randomness)
            ),
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
    type Limiter = DefaultMemoryLimiter;

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

    fn into_store(self) -> Self::Blockstore {
        self.state_tree.into_store()
    }

    fn machine_id(&self) -> &str {
        &self.id
    }

    fn new_limiter(&self) -> Self::Limiter {
        DefaultMemoryLimiter::for_network(&self.context().network)
    }
}

// Helper method that puts certain "empty" types in the blockstore.
// These types are privileged by some parts of the system (eg. as the default actor state).
fn put_empty_blocks<B: Blockstore>(blockstore: B) -> anyhow::Result<()> {
    let empty_arr_cid = blockstore.put(Blake2b256, &EMPTY_ARRAY_BLOCK)?;

    debug_assert!(
        empty_arr_cid == *EMPTY_ARR_CID,
        "empty CID sanity check failed",
    );

    Ok(())
}
