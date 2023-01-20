// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! (Proper package docs coming shortly; for now this is a holding pen for items
//! we must mention).
//!
//! ## Logging
//!
//! This package emits logs using the log façade. Configure the logging backend
//! of your choice during the initialization of the consuming application.

pub use kernel::default::DefaultKernel;
pub use kernel::Kernel;

pub mod call_manager;
pub mod engine;
pub mod executor;
pub mod externs;
pub mod kernel;
pub mod machine;
pub mod syscalls;

pub mod gas;
pub mod state_tree;

mod blockstore;

#[cfg(not(feature = "testing"))]
mod account_actor;
#[cfg(not(feature = "testing"))]
mod init_actor;
#[cfg(not(feature = "testing"))]
mod system_actor;

#[cfg(feature = "testing")]
pub mod account_actor;
#[cfg(feature = "testing")]
pub mod init_actor;
#[cfg(feature = "testing")]
pub mod system_actor;

mod eam_actor;
mod history_map;
pub mod trace;

use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fvm_ipld_encoding::{to_vec, DAG_CBOR};

lazy_static::lazy_static! {
    /// Cid of the empty array Cbor bytes (`EMPTY_ARR_BYTES`).
    pub static ref EMPTY_ARR_CID: Cid = {
        let empty = to_vec::<[(); 0]>(&[]).unwrap();
        Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty))
    };
}

#[cfg(test)]
mod test {
    use cid::Cid;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::{CborStore, DAG_CBOR};
    use fvm_shared::state::StateTreeVersion;
    use fvm_shared::IDENTITY_HASH;
    use multihash::{Code, Multihash};

    use crate::call_manager::DefaultCallManager;
    use crate::engine::EnginePool;
    use crate::externs::{Chain, Consensus, Externs, Rand};
    use crate::machine::{DefaultMachine, Manifest, NetworkConfig};
    use crate::state_tree::StateTree;
    use crate::{executor, DefaultKernel};

    struct DummyExterns;

    impl Externs for DummyExterns {}

    impl Rand for DummyExterns {
        fn get_chain_randomness(
            &self,
            _pers: i64,
            _round: fvm_shared::clock::ChainEpoch,
            _entropy: &[u8],
        ) -> anyhow::Result<[u8; 32]> {
            let msg = "mel was here".as_bytes();
            let mut out = [0u8; 32];
            out[..msg.len()].copy_from_slice(msg);
            Ok(out)
        }

        fn get_beacon_randomness(
            &self,
            _pers: i64,
            _round: fvm_shared::clock::ChainEpoch,
            _entropy: &[u8],
        ) -> anyhow::Result<[u8; 32]> {
            todo!()
        }
    }

    impl Consensus for DummyExterns {
        fn verify_consensus_fault(
            &self,
            _h1: &[u8],
            _h2: &[u8],
            _extra: &[u8],
        ) -> anyhow::Result<(Option<fvm_shared::consensus::ConsensusFault>, i64)> {
            // consensus is always valid for tests :)
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

    #[test]
    fn test_constructor() {
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V5).unwrap();
        let root = st.flush().unwrap();
        bs = st.into_store();

        // An empty built-in actors manifest.
        let manifest_cid = {
            bs.put_cbor(&Manifest::DUMMY_CODES, Code::Blake2b256)
                .unwrap()
        };

        let actors_cid = bs.put_cbor(&(1, manifest_cid), Code::Blake2b256).unwrap();

        let mc = NetworkConfig::new(fvm_shared::version::NetworkVersion::V18)
            .override_actors(actors_cid)
            .for_epoch(0, 0, root);

        let machine = DefaultMachine::new(&mc, bs, DummyExterns).unwrap();
        let engine = EnginePool::new_default((&mc.network).into()).unwrap();
        let _ = executor::DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(
            engine,
            Box::new(machine),
        );
    }
}
