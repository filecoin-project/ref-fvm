// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm::externs::{Chain, Consensus, Externs, Rand};
use fvm_ipld_encoding::DAG_CBOR;
use fvm_shared::IDENTITY_HASH;
use multihash::Multihash;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
pub struct DummyExterns;

impl Externs for DummyExterns {}

impl Rand for DummyExterns {
    fn get_chain_randomness(
        &self,
        _pers: i64,
        _round: fvm_shared::clock::ChainEpoch,
        _entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rng: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        Ok(<[u8; 32]>::try_from(rng.into_bytes()).unwrap())
    }

    fn get_beacon_randomness(
        &self,
        _pers: i64,
        _round: fvm_shared::clock::ChainEpoch,
        _entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rng: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        Ok(<[u8; 32]>::try_from(rng.into_bytes()).unwrap())
    }
}

impl Consensus for DummyExterns {
    fn verify_consensus_fault(
        &self,
        _h1: &[u8],
        _h2: &[u8],
        _extra: &[u8],
    ) -> anyhow::Result<(Option<fvm_shared::consensus::ConsensusFault>, i64)> {
        Ok((None, 0))
    }
}

impl Chain for DummyExterns {
    fn get_tipset_cid(&self, epoch: fvm_shared::clock::ChainEpoch) -> anyhow::Result<Cid> {
        Ok(Cid::new_v1(
            DAG_CBOR,
            Multihash::wrap(IDENTITY_HASH, &epoch.to_be_bytes()).unwrap(),
        ))
    }
}
