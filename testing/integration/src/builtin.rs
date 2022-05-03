use std::collections::BTreeMap;

use anyhow::{Context, Result};
use cid::Cid;
use futures::executor::block_on;
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, system_actor};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_car::load_car;
use fvm_ipld_encoding::CborStore;
use fvm_shared::actor::builtin::{load_manifest, Type};
use fvm_shared::version::NetworkVersion;
use multihash::Code;

use crate::error::Error::{
    FailedToLoadManifest, FailedToSetActor, FailedToSetState, MultipleRootCid, NoCidInManifest,
};

const BUNDLES: [(NetworkVersion, &[u8]); 2] = [
    (NetworkVersion::V14, actors_v6::BUNDLE_CAR),
    (NetworkVersion::V15, actors_v7::BUNDLE_CAR),
];

// Import built-in actors
pub fn import_builtin_actors(
    blockstore: &MemoryBlockstore,
) -> Result<BTreeMap<NetworkVersion, Cid>> {
    BUNDLES
        .into_iter()
        .map(|(nv, car)| {
            let roots = block_on(async { load_car(blockstore, car).await.unwrap() });
            if roots.len() != 1 {
                return Err(MultipleRootCid(nv).into());
            }
            Ok((nv, roots[0]))
        })
        .collect()
}

// Retrieve system, init and accounts actors code CID
pub fn fetch_builtin_code_cid(
    blockstore: &MemoryBlockstore,
    builtin_actors: &Cid,
    ver: u32,
) -> Result<(Cid, Cid, Cid)> {
    let manifest = load_manifest(blockstore, builtin_actors, ver).context(FailedToLoadManifest)?;
    Ok((
        *manifest
            .get_by_right(&Type::System)
            .ok_or(NoCidInManifest(Type::System))?,
        *manifest
            .get_by_right(&Type::Init)
            .ok_or(NoCidInManifest(Type::Init))?,
        *manifest
            .get_by_right(&Type::Account)
            .ok_or(NoCidInManifest(Type::Init))?,
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
