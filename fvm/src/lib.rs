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
    use cid::Cid;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::CborStore;
    use fvm_shared::actor::builtin::Manifest;
    use fvm_shared::address::Address;
    use fvm_shared::state::StateTreeVersion;
    use multihash::Code;

    use crate::blockstore::BufferedBlockstore;
    use crate::call_manager::{CallManager, DefaultCallManager};
    use crate::executor::DefaultExecutor;
    use crate::externs::{Consensus, Externs, Rand};
    use crate::gas::{GasTracker, Gas};
    use crate::kernel::{BlockRegistry, SelfOps, MessageOps, IpldBlockOps, GasOps, SendOps, ActorOps};
    use crate::machine::{DefaultMachine, Engine, Machine, NetworkConfig, MachineContext};
    use crate::state_tree::StateTree;
    use crate::{executor, DefaultKernel, Kernel};

    struct DummyExterns;

    impl Externs for DummyExterns {}

    impl Rand for DummyExterns {
        fn get_chain_randomness(
            &self,
            _pers: i64,
            _round: fvm_shared::clock::ChainEpoch,
            _entropy: &[u8],
        ) -> anyhow::Result<[u8; 32]> {
            todo!()
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
            todo!()
        }
    }

    type DummyExecutor = DefaultExecutor<
        DefaultKernel<DefaultCallManager<Box<DefaultMachine<MemoryBlockstore, DummyExterns>>>>,
    >;
    type DummyKernel =
        DefaultKernel<DefaultCallManager<Box<DefaultMachine<MemoryBlockstore, DummyExterns>>>>;

    fn dummy_constructor() -> DummyExecutor {
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
        DummyExecutor::new(Box::new(machine))
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

    #[test]
    fn test_resolve_key() -> anyhow::Result<()> {
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

        let m_ctx = NetworkConfig::new(fvm_shared::version::NetworkVersion::V15)
            .override_actors(actors_cid)
            .for_epoch(0, root);

        let engine = Engine::new_default((&m_ctx.network).into()).unwrap();
        let machine = DefaultMachine::new(&engine, &m_ctx, bs, DummyExterns)?;
        let mgr = DefaultCallManager::new(machine, 64, Address::new_actor(&[]), 0);
        DefaultKernel::new(mgr, BlockRegistry::default(), 0, 0, 0, 32.into()); //todo check caller id & method
        Ok(())
    }

    
    struct DummyMachine {
        state_tree: StateTree<MemoryBlockstore>,
        ctx: MachineContext,
    }

    impl Default for DummyMachine {
        fn default() -> Self {
            let cid = Cid::default(); // this probably shouldnt work
            Self {
                state_tree: StateTree::new(MemoryBlockstore::new(), StateTreeVersion::V4).unwrap(), // is v4 right?
                ctx: NetworkConfig::new(fvm_shared::version::NetworkVersion::V16).for_epoch(0, cid),
            }
        }
    }

    impl Machine for DummyMachine {
        type Blockstore = MemoryBlockstore;

        type Externs = DummyExterns;

        fn engine(&self) -> &Engine {
            todo!()
        }

        fn blockstore(&self) -> &Self::Blockstore {
            self.state_tree.store()
        }

        fn context(&self) -> &crate::machine::MachineContext {
            &self.ctx
        }

        fn externs(&self) -> &Self::Externs {
            todo!()
        }

        fn builtin_actors(&self) -> &Manifest {
            todo!()
        }

        fn state_tree(&self) -> &StateTree<Self::Blockstore> {
            todo!()
        }

        fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
            todo!()
        }

        fn create_actor(
            &mut self,
            addr: &Address,
            act: crate::state_tree::ActorState,
        ) -> crate::kernel::Result<fvm_shared::ActorID> {
            todo!()
        }

        fn transfer(
            &mut self,
            from: fvm_shared::ActorID,
            to: fvm_shared::ActorID,
            value: &fvm_shared::econ::TokenAmount,
        ) -> crate::kernel::Result<()> {
            todo!()
        }

        fn into_store(self) -> Self::Blockstore {
            todo!()
        }
    }

    struct DummyCallManager {
        machine: DummyMachine,
        gas_tracker: GasTracker,
    }

    impl DummyCallManager {
        fn ffs() -> Self {
            Self { 
                machine: DummyMachine::default(), 
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // this will need to be modified for gas limit testing
            }
        }
    }

    impl CallManager for DummyCallManager {
        type Machine = DummyMachine;

        fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self {
            todo!()
        }

        fn send<K: Kernel<CallManager = Self>>(
            &mut self,
            from: fvm_shared::ActorID,
            to: Address,
            method: fvm_shared::MethodNum,
            params: Option<crate::kernel::Block>,
            value: &fvm_shared::econ::TokenAmount,
        ) -> crate::kernel::Result<crate::call_manager::InvocationResult> {
            todo!()
        }

        fn with_transaction(
            &mut self,
            f: impl FnOnce(&mut Self) -> crate::kernel::Result<crate::call_manager::InvocationResult>,
        ) -> crate::kernel::Result<crate::call_manager::InvocationResult> {
            todo!()
        }

        fn finish(self) -> (crate::call_manager::FinishRet, Self::Machine) {
            todo!()
        }

        fn machine(&self) -> &Self::Machine {
            &self.machine
        }

        fn machine_mut(&mut self) -> &mut Self::Machine {
            todo!()
        }

        fn gas_tracker(&self) -> &crate::gas::GasTracker {
            &self.gas_tracker
        }

        fn gas_tracker_mut(&mut self) -> &mut crate::gas::GasTracker {
            &mut self.gas_tracker
        }

        fn origin(&self) -> Address {
            todo!()
        }

        fn nonce(&self) -> u64 {
            todo!()
        }

        fn next_actor_idx(&mut self) -> u64 {
            todo!()
        }
    }
    
    mod default_kernel {
        use crate::kernel::IpldBlockOps;

        use super::*;

        #[test]
        fn test_msg() -> anyhow::Result<()> {
            type TestingKernel = DefaultKernel<DummyCallManager>;

            let mut bs = MemoryBlockstore::default();

            let kern = TestingKernel::new(DummyCallManager::ffs(), BlockRegistry::default(), 0, 0, 0, 0.into());

            kern.msg_receiver();
            assert_eq!(0, kern.msg_receiver());

            Ok(())
        }

        #[test]
        fn test_ipld_block_open() -> anyhow::Result<()> {
            type TestingKernel = DefaultKernel<DummyCallManager>;

            let mut bs = MemoryBlockstore::default();

            let cid = bs.put_cbor(&"foo", multihash::Code::Blake2b256)?; // should this be actually hashed beforehand or does this hash automatically?

            let mut kern = TestingKernel::new(DummyCallManager::ffs(), BlockRegistry::default(), 0, 0, 0, 0.into());

            kern.charge_gas("foo", Gas::new(1))?;

            kern.gas_available();

            // assert gas charge

            // assert block ID

            // assert block stat

            Ok(())
        }
    }
    
}
