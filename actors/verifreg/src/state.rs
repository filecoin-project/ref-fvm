// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use actors_runtime::make_empty_map;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::blockstore::Blockstore;
use fvm_shared::encoding::tuple::*;
use fvm_shared::encoding::Cbor;
use fvm_shared::HAMT_BIT_WIDTH;

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub root_key: Address,
    pub verifiers: Cid,
    pub verified_clients: Cid,
}

impl State {
    pub fn new<BS: Blockstore>(store: &BS, root_key: Address) -> anyhow::Result<State> {
        let empty_map = make_empty_map::<_, ()>(store, HAMT_BIT_WIDTH)
            .flush()
            .map_err(|e| anyhow::anyhow!("Failed to create empty map: {}", e))?;

        Ok(State {
            root_key,
            verifiers: empty_map,
            verified_clients: empty_map,
        })
    }
}

impl Cbor for State {}
