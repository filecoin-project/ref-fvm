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
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::CborStore;
    use fvm_shared::actor::builtin::Manifest;
    use fvm_shared::state::StateTreeVersion;
    use multihash::Code;

    use crate::call_manager::DefaultCallManager;
    use crate::externs::{Consensus, Externs, Rand};
    use crate::machine::{DefaultMachine, Engine, NetworkConfig};
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

    #[test]
    fn test_constructor() {
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V4).unwrap();
        let root = st.flush().unwrap();
        bs = st.into_store();

        // An empty built-in actors manifest.
        let manifest_cid = {
            let manifest = Manifest::new();
            bs.put_cbor(&manifest, Code::Blake2b256).unwrap()
        };

        let actors_cid = bs.put_cbor(&(0, manifest_cid), Code::Blake2b256).unwrap();

        let mc = NetworkConfig::new(fvm_shared::version::NetworkVersion::V15)
            .override_actors(actors_cid)
            .for_epoch(0, root);

        let machine = DefaultMachine::new(
            &Engine::new_default((&mc.network).into()).unwrap(),
            &mc,
            bs,
            DummyExterns,
        )
        .unwrap();
        let _ = executor::DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(Box::new(
            machine,
        ));
    }
}
