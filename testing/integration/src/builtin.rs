// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{Context, Result};
use cid::Cid;
use fvm::machine::Manifest;
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, system_actor, storagemarket_actor, storagepower_actor};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use fvm_shared::ActorID;
use multihash::Code;

use crate::error::Error::{FailedToLoadManifest, FailedToSetActor, FailedToSetState};
use crate::verifiedregistry_actor;
use crate::datacap_actor;
use crate::reward_actor;

// Retrieve system, init and accounts actors code CID
pub fn fetch_builtin_code_cid(
    blockstore: &impl Blockstore,
    builtin_actors: &Cid,
    ver: u32,
) -> Result<(Cid, Cid, Cid, Cid, Cid, Cid, Cid, Cid, Cid, Cid)> {
    let manifest = Manifest::load(blockstore, builtin_actors, ver).context(FailedToLoadManifest)?;
    Ok((
        *manifest.get_system_code(),
        *manifest.get_init_code(),
        *manifest.get_account_code(),
        *manifest.get_placeholder_code(),
        *manifest.get_eam_code(),
        *manifest.get_storagemarket_code(),
        *manifest.get_storagepower_code(),
        *manifest.get_verifiedregistry_code(),
        *manifest.get_datacap_code(),
        *manifest.get_reward_code()
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
    state_tree
        .set_actor(system_actor::SYSTEM_ACTOR_ID, sys_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("system actor".to_owned()))
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

    state_tree
        .set_actor(init_actor::INIT_ACTOR_ID, init_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("init actor".to_owned()))
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

    state_tree
        .set_actor(EAM_ACTOR_ID, eam_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("eam actor".to_owned()))
}

pub fn set_storagemarket_actor(state_tree: &mut StateTree<impl Blockstore>, storagemarket_code_cid: Cid, storagemarket_state: storagemarket_actor::State,) -> Result<()> {
    const STORAGE_MARKET_ACTOR: ActorID = 5;

    let storagemarket_state_cid = state_tree
        .store()
        .put_cbor(&storagemarket_state, Code::Blake2b256)
        .context(FailedToSetState("storagemarket actor".to_owned()))?;

    let storagemarket_actor_state = ActorState {
        code: storagemarket_code_cid,
        state: storagemarket_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree
        .set_actor(STORAGE_MARKET_ACTOR, storagemarket_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("storagemarket actor".to_owned()))
}

pub fn set_storagepower_actor(state_tree: &mut StateTree<impl Blockstore>, storagepower_code_cid: Cid, storagepower_state: storagepower_actor::State,) -> Result<()> {
    const STORAGE_POWER_ACTOR: ActorID = 4;

    let storagepower_state_cid = state_tree
        .store()
        .put_cbor(&storagepower_state, Code::Blake2b256)
        .context(FailedToSetState("storagepower actor".to_owned()))?;

    let storagepower_actor_state = ActorState {
        code: storagepower_code_cid,
        state: storagepower_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree
        .set_actor(STORAGE_POWER_ACTOR, storagepower_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("storagepower actor".to_owned()))
}

pub fn set_verifiedregistry_actor(state_tree: &mut StateTree<impl Blockstore>, verifiedregistry_code_cid: Cid, verifiedregistry_state: verifiedregistry_actor::State,) -> Result<()> {
    const VERIFIED_REGISTRY_ACTOR : ActorID = 6;

    let verifiedregistry_state_cid = state_tree
        .store()
        .put_cbor(&verifiedregistry_state, Code::Blake2b256)
        .context(FailedToSetState("verifiedregistry actor".to_owned()))?;

    let verifiedregistry_actor_state = ActorState {
        code: verifiedregistry_code_cid,
        state: verifiedregistry_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree
        .set_actor(VERIFIED_REGISTRY_ACTOR, verifiedregistry_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("verifiedregistry actor".to_owned()))
}

pub fn set_datacap_actor(state_tree: &mut StateTree<impl Blockstore>, datacap_code_cid: Cid, datacap_state: datacap_actor::State,) -> Result<()> {
    const DATA_CAP_ACTOR : ActorID = 7;

    let datacap_state_cid = state_tree
        .store()
        .put_cbor(&datacap_state, Code::Blake2b256)
        .context(FailedToSetState("datacap actor".to_owned()))?;

    let datacap_actor_state = ActorState {
        code: datacap_code_cid,
        state: datacap_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree
        .set_actor(DATA_CAP_ACTOR, datacap_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("datacap actor".to_owned()))
}

pub fn set_reward_actor(state_tree: &mut StateTree<impl Blockstore>, reward_code_cid: Cid, reward_state: reward_actor::State,) -> Result<()> {
    const REWARD_ACTOR_ID: ActorID = 2;

    let reward_state_cid = state_tree
        .store()
        .put_cbor(&reward_state, Code::Blake2b256)
        .context(FailedToSetState("reward actor".to_owned()))?;

    let reward_actor_state = ActorState {
        code: reward_code_cid,
        state: reward_state_cid,
        sequence: 0,
        balance: Default::default(),
        delegated_address: None,
    };

    state_tree
        .set_actor(REWARD_ACTOR_ID, reward_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("reward actor".to_owned()))
}
