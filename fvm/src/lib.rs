//! (Proper package docs coming shortly; for now this is a holding pen for items
//! we must mention).
//!
//! ## Logging
//!
//! This package emits logs using the log façade. Configure the logging backend
//! of your choice during the initialization of the consuming application.
pub use kernel::default::DefaultKernel;
pub use kernel::{BlockError, Kernel};

pub mod call_manager;
pub mod executor;
pub mod externs;
pub mod kernel;
pub mod machine;
pub mod syscalls;

// TODO Public only for conformance tests.
//  Consider exporting only behind a feature.
pub mod builtin;
pub mod gas;
pub mod state_tree;

mod blockstore;

mod account_actor;
mod init_actor;
mod market_actor;
mod power_actor;
mod reward_actor;

#[derive(Clone)]
pub struct Config {
    /// The maximum call depth.
    pub max_call_depth: u32,
    /// Initial number of memory pages to allocate for the invocation container.
    pub initial_pages: usize,
    /// Maximum number of memory pages an invocation container's memory
    /// can expand to.
    pub max_pages: usize,
    /// Whether debug mode is enabled or not.
    pub debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            initial_pages: 0,
            max_pages: 1024,
            max_call_depth: 4096,
            debug: false,
        }
    }
}

#[cfg(test)]
mod test {
    use fvm_shared::blockstore::MemoryBlockstore;
    use fvm_shared::state::StateTreeVersion;
    use num_traits::Zero;

    use crate::call_manager::DefaultCallManager;
    use crate::externs::{Consensus, Externs, Rand};
    use crate::machine::{DefaultMachine, Engine};
    use crate::state_tree::StateTree;
    use crate::{executor, Config, DefaultKernel};

    struct DummyExterns;

    impl Externs for DummyExterns {}

    impl Rand for DummyExterns {
        fn get_chain_randomness(
            &self,
            _pers: fvm_shared::crypto::randomness::DomainSeparationTag,
            _round: fvm_shared::clock::ChainEpoch,
            _entropy: &[u8],
        ) -> anyhow::Result<[u8; 32]> {
            todo!()
        }

        fn get_beacon_randomness(
            &self,
            _pers: fvm_shared::crypto::randomness::DomainSeparationTag,
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
            todo!()
        }
    }

    #[test]
    fn test_constructor() {
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V4).unwrap();
        let root = st.flush().unwrap();
        bs = st.consume();

        let machine = DefaultMachine::new(
            Config::default(),
            Engine::default(),
            0,
            Zero::zero(),
            Zero::zero(),
            fvm_shared::version::NetworkVersion::V14,
            root,
            bs,
            DummyExterns,
        )
        .unwrap();
        let _ = executor::DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(Box::new(
            machine,
        ));
    }
}
