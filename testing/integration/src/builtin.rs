// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{Context, Result};
use cid::Cid;
use fvm::machine::Manifest;
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, system_actor};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use fvm_shared::ActorID;
use multihash::Code;

use crate::error::Error::{FailedToLoadManifest, FailedToSetState};

// Retrieve system, init and accounts actors code CID
pub fn fetch_builtin_code_cid(
    blockstore: &impl Blockstore,
    builtin_actors: &Cid,
    ver: u32,
) -> Result<(Cid, Cid, Cid, Cid, Cid)> {
    let manifest = Manifest::load(blockstore, builtin_actors, ver).context(FailedToLoadManifest)?;
    Ok((
        *manifest.get_system_code(),
        *manifest.get_init_code(),
        *manifest.get_account_code(),
        *manifest.get_placeholder_code(),
        *manifest.get_eam_code(),
    ))
}

pub fn set_sys_actor(
    state_tree: &mut StateTree<impl Blockstore>,
    sys_state: system_actor::State,
    sys_code_cid: Cid,
) -> Result<()> {
    let sys_state_cid = state_tree
        .store()
        .put_cbor(&sys_state, Code::Blake2b256)
        .context(FailedToSetState("system actor".to_owned()))?;

    let sys_actor_state = ActorState {
        code: sys_code_cid,
        state: sys_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };
    state_tree.set_actor(system_actor::SYSTEM_ACTOR_ID, sys_actor_state);
    Ok(())
}

pub fn set_init_actor(
    state_tree: &mut StateTree<impl Blockstore>,
    init_code_cid: Cid,
    init_state: init_actor::State,
) -> Result<()> {
    let init_state_cid = state_tree
        .store()
        .put_cbor(&init_state, Code::Blake2b256)
        .context(FailedToSetState("init actor".to_owned()))?;

    let init_actor_state = ActorState {
        code: init_code_cid,
        state: init_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree.set_actor(init_actor::INIT_ACTOR_ID, init_actor_state);
    Ok(())
}

pub fn set_eam_actor(state_tree: &mut StateTree<impl Blockstore>, eam_code_cid: Cid) -> Result<()> {
    const EAM_ACTOR_ID: ActorID = 10;

    let eam_state_cid = state_tree
        .store()
        .put_cbor(&[(); 0], Code::Blake2b256)
        .context(FailedToSetState("eam actor".to_owned()))?;

    let eam_actor_state = ActorState {
        code: eam_code_cid,
        state: eam_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree.set_actor(EAM_ACTOR_ID, eam_actor_state);
    Ok(())
}
