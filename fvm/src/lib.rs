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
    use std::sync::{Arc, Weak};

    use anyhow::{anyhow, Context};
    use cid::Cid;
    use derive_more::{Deref, DerefMut};
    use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
    use fvm_ipld_encoding::CborStore;
    use fvm_shared::actor::builtin::Manifest;
    use fvm_shared::address::Address;
    use fvm_shared::state::StateTreeVersion;
    use fvm_shared::version::NetworkVersion;
    use multihash::Code;

    use crate::call_manager::{
        Backtrace, CallManager, DefaultCallManager, FinishRet, InvocationResult,
    };
    use crate::externs::{Consensus, Externs, Rand};
    use crate::gas::{Gas, GasTracker, WasmGasPrices};
    use crate::kernel::{
        ActorOps, BlockRegistry, GasOps, IpldBlockOps, MessageOps, SelfOps, SendOps,
    };
    use crate::machine::{
        DefaultMachine, Engine, EngineConfig, Machine, MachineContext, NetworkConfig,
    };
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
            // consensus is always valid for tests :)
            anyhow::Result::Ok((None, 0))
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

    // TODO remove or refactor, this was my first stab at a test
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

    /// this is essentially identical to DefaultMachine, but has pub fields for reaching inside
    struct DummyMachine {
        pub engine: Engine,
        pub state_tree: StateTree<MemoryBlockstore>,
        pub ctx: MachineContext,
        pub builtin_actors: Manifest,
    }

    const DUMMY_PRICES: &'static WasmGasPrices = &WasmGasPrices {
        exec_instruction_cost: Gas::new(0),
    };
    // hardcoded elsewhere till relavant TODOs are solved
    const STUB_NETWORK_VER: NetworkVersion = NetworkVersion::V16;

    impl DummyMachine {
        // pub fn new_cfg(
        //     bs: MemoryBlockstore,
        //     engine: &Engine,
        //     manifest_ver: u32,
        // ) -> anyhow::Result<Self> {
        //     let mut state_tree = StateTree::new(bs, StateTreeVersion::V4)?;
        //     let root = state_tree.flush()?;
        //     let bs = state_tree.into_store();

        //     // An empty built-in actors manifest.
        //     let manifest = Manifest::new();
        //     let manifest_cid = bs.put_cbor(&manifest, Code::Blake2b256)?;

        //     let actors_cid = bs.put_cbor(&(0, manifest_cid), Code::Blake2b256).unwrap();

        //     let mut state_tree = StateTree::new_from_root(bs, &root)?;

        //     let ctx = NetworkConfig::new(fvm_shared::version::NetworkVersion::V16)
        //         .override_actors(actors_cid)
        //         .for_epoch(0, root);

        //     Ok(Self {
        //         ctx,
        //         engine: engine.clone(),
        //         state_tree,
        //         builtin_actors: manifest,
        //     })
        // }

        /// build a dummy machine with no builtin actors, and from empty & new state tree for unit tests
        pub fn new_stub() -> anyhow::Result<Self> {
            let mut bs = MemoryBlockstore::new();

            // generate new state root
            let mut state_tree = StateTree::new(bs, StateTreeVersion::V4)?;
            let root = state_tree.flush()?;
            let bs = state_tree.into_store();

            // Add empty built-in actors manifest to blockstore.
            let manifest = Manifest::new();
            let manifest_cid = bs.put_cbor(&manifest, Code::Blake2b256)?;

            // sanity checks
            bs.has(&root).context("failed to load initial state-root")?;
            bs.has(&manifest_cid)
                .context("failed to load builtin actor manifest")?;

            // TODO find and document why this needs to be this
            // TODO V15 requires this and IDK what V16 expects
            // TODO why is a tuple of (num_actors, manifest) the expected manifest CID?
            let actors_cid = bs.put_cbor(&(0, manifest_cid), Code::Blake2b256).unwrap();

            // construct state tree from empty root state
            let state_tree = StateTree::new_from_root(bs, &root)?;

            // generate context from the new generated root and override actors with empty list
            // TODO should this stay as V15?
            let ctx = NetworkConfig::new(fvm_shared::version::NetworkVersion::V15)
                .override_actors(actors_cid)
                .for_epoch(0, root);

            Ok(Self {
                ctx,
                engine: Engine::new_default(EngineConfig {
                    max_wasm_stack: 1024,
                    wasm_prices: DUMMY_PRICES,
                })?,
                state_tree,
                builtin_actors: manifest,
            })
        }
    }

    impl Machine for DummyMachine {
        type Blockstore = MemoryBlockstore;

        type Externs = DummyExterns;

        fn engine(&self) -> &Engine {
            &self.engine
        }

        fn blockstore(&self) -> &Self::Blockstore {
            self.state_tree.store()
        }

        fn context(&self) -> &crate::machine::MachineContext {
            &self.ctx
        }

        fn externs(&self) -> &Self::Externs {
            &DummyExterns
        }

        fn builtin_actors(&self) -> &Manifest {
            &self.builtin_actors
        }

        fn state_tree(&self) -> &StateTree<Self::Blockstore> {
            &self.state_tree
        }

        fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
            &mut self.state_tree
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

    #[derive(Deref, DerefMut)]
    // similar to `DefaultCallManager` but with public variables for modification by unit tests
    struct InnerDummyCallManager {
        #[deref]
        #[deref_mut]
        pub machine: DummyMachine,
        pub gas_tracker: GasTracker,
        pub origin: Address,
        pub nonce: u64,
    }

    impl InnerDummyCallManager {
        fn machine(&self) -> &DummyMachine {
            &self.machine
        }
    }

    // a wrapper to let us inspect values during testing, all borrows are done with .borrow() or .borrow_mut() and will panic, so this should be used in single thereaded tests only
    struct DummyCallManager(Arc<InnerDummyCallManager>);

    impl DummyCallManager {
        fn new_stub() -> Self {
            Self(Arc::new(InnerDummyCallManager {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
                origin: Address::new_actor(&[]),
                nonce: 0,
            }))
        }

        fn borrow(&self) -> &InnerDummyCallManager {
            self.0.as_ref()
        }

        // TODO this needs proper safety review
        /// similar to https://doc.rust-lang.org/src/alloc/sync.rs.html#1587
        /// SAFETY: Any other Arc or Weak pointers to the same allocation must not be dereferenced for the duration of the returned borrow
        unsafe fn borrow_mut(&mut self) -> &mut InnerDummyCallManager {
            &mut *(Arc::as_ptr(&self.0) as *mut InnerDummyCallManager)
        }

        /// while not unsafe on its own, there are rules for the safe usage of the returned value
        /// the only truly safe way to use this is to deref the returned struct **after or at the same time of DummyCallManager**
        /// see `borrow_mut()`
        unsafe fn weak(&self) -> Weak<InnerDummyCallManager> {
            Arc::downgrade(&self.0)
        }
    }

    impl CallManager for DummyCallManager {
        type Machine = DummyMachine;

        fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self {
            Self(Arc::new(InnerDummyCallManager {
                machine,
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
                origin,
                nonce,
            }))
        }

        fn send<K: Kernel<CallManager = Self>>(
            &mut self,
            from: fvm_shared::ActorID,
            to: Address,
            method: fvm_shared::MethodNum,
            params: Option<crate::kernel::Block>,
            value: &fvm_shared::econ::TokenAmount,
        ) -> crate::kernel::Result<crate::call_manager::InvocationResult> {
            Ok(InvocationResult::Return(None))
        }

        fn with_transaction(
            &mut self,
            f: impl FnOnce(&mut Self) -> crate::kernel::Result<crate::call_manager::InvocationResult>,
        ) -> crate::kernel::Result<crate::call_manager::InvocationResult> {
            Ok(InvocationResult::Return(None))
        }

        fn finish(self) -> (crate::call_manager::FinishRet, Self::Machine) {
            (
                FinishRet {
                    gas_used: 0,
                    backtrace: Backtrace {
                        frames: Vec::new(),
                        cause: None,
                    },
                    exec_trace: Vec::new(),
                },
                match Arc::try_unwrap(self.0) {
                    Ok(x) => x.machine,
                    _ => panic!(
                        "all refrences to DummyCallManager must be dropped before calling finish()"
                    ),
                },
            )
        }

        fn machine(&self) -> &Self::Machine {
            &self.0.as_ref().machine
        }

        fn machine_mut(&mut self) -> &mut Self::Machine {
            unsafe { &mut self.borrow_mut().machine }
        }

        fn gas_tracker(&self) -> &crate::gas::GasTracker {
            &self.borrow().gas_tracker
        }

        fn gas_tracker_mut(&mut self) -> &mut crate::gas::GasTracker {
            unsafe { &mut self.borrow_mut().gas_tracker }
        }

        fn origin(&self) -> Address {
            self.0.as_ref().origin
        }

        fn nonce(&self) -> u64 {
            self.0.as_ref().nonce
        }

        fn next_actor_idx(&mut self) -> u64 {
            0
        }
    }

    mod default_kernel {

        use fvm_ipld_encoding::DAG_CBOR;

        use super::*;
        use crate::kernel::IpldBlockOps;

        type TestingKernel = DefaultKernel<DummyCallManager>;

        #[test]
        fn test_msg() -> anyhow::Result<()> {
            let kern = TestingKernel::new(
                DummyCallManager::new_stub(),
                BlockRegistry::default(),
                0,
                0,
                0,
                0.into(),
            );

            kern.msg_receiver();
            assert_eq!(0, kern.msg_receiver());

            Ok(())
        }

        #[test]
        fn ipld_ops() -> anyhow::Result<()> {
            let call_manager = DummyCallManager::new_stub();
            // variable for value inspection
            let refcell = unsafe { call_manager.weak() };

            let mut kern =
                TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());

            let id = kern.block_create(DAG_CBOR, "foo".as_bytes())?;

            let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;

            kern.block_open(&cid)?;

            // assert gas charge

            // assert block ID

            // assert block stat

            Ok(())
        }

        // actor ops broken because origin addr is empty for now
    }
}
