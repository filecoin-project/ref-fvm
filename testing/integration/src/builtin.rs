use anyhow::{Context, Result};
use cid::Cid;
use fvm::machine::Manifest;
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, system_actor};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use multihash::Code;

use crate::error::Error::{FailedToLoadManifest, FailedToSetActor, FailedToSetState};

// Retrieve system, init and accounts actors code CID
pub fn fetch_builtin_code_cid(
    blockstore: &impl Blockstore,
    builtin_actors: &Cid,
    ver: u32,
) -> Result<(Cid, Cid, Cid)> {
    let manifest = Manifest::load(blockstore, builtin_actors, ver).context(FailedToLoadManifest)?;
    Ok((
        *manifest.get_system_code(),
        *manifest.get_init_code(),
        *manifest.get_account_code(),
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
    };
    state_tree
        .set_actor(&system_actor::SYSTEM_ACTOR_ADDR, sys_actor_state)
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
    };

    state_tree
        .set_actor(&init_actor::INIT_ACTOR_ADDR, init_actor_state)
        .map_err(anyhow::Error::from)
        .context(FailedToSetActor("init actor".to_owned()))
}
