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
    use std::mem::ManuallyDrop;
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

    /// WASM execution prices is 1 for counting
    const DUMMY_WASM_PRICES: WasmGasPrices = WasmGasPrices {
        exec_instruction_cost: Gas::from_milligas(1),
    };

    // hardcoded elsewhere till relavant TODOs are solved
    const STUB_NETWORK_VER: NetworkVersion = NetworkVersion::V16;

    impl DummyMachine {
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
                    wasm_prices: &DUMMY_WASM_PRICES,
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
        pub charge_gas_calls: usize,
        pub origin: Address,
        pub nonce: u64,
    }
    /// a wrapper to let us inspect values during testing, all borrows are done with .borrow() or .borrow_mut(), so this should be used in single thereaded tests only
    /// TODO this introduces some rough edges that might need to be cleaned up 
    struct DummyCallManager(Arc<InnerDummyCallManager>);

    impl DummyCallManager {
        fn new_stub() -> Self {
            Self(Arc::new(InnerDummyCallManager {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
                origin: Address::new_actor(&[]),
                charge_gas_calls: 0,
                nonce: 0,
            }))
        }

        fn borrow(&self) -> &InnerDummyCallManager {
            self.0.as_ref()
        }

        // TODO this needs proper safety review
        /// similar to https://doc.rust-lang.org/src/alloc/sync.rs.html#1545
        /// panics if the Arc isn't the only strong pointer that exists.
        /// SAFETY: Any other Arc or Weak pointers to the same allocation must not be dereferenced for the duration of the returned borrow
        unsafe fn borrow_mut(&mut self) -> &mut InnerDummyCallManager {
            if Arc::strong_count(&self.0) != 1 {
                panic!("Only one strong pointer is allowed when mutating DummyCallManager")
            }
            &mut *(Arc::as_ptr(&self.0) as *mut InnerDummyCallManager)
        }

        /// the only truly safe way to use this is to deref the returned Weak ref **after or at the same time of DummyCallManager**
        /// see `borrow_mut()`
        fn weak(&self) -> ManuallyDrop<Weak<InnerDummyCallManager>> {
            ManuallyDrop::new(Arc::downgrade(&self.0))
        }
    }

    impl CallManager for DummyCallManager {
        type Machine = DummyMachine;

        fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self {
            Self(Arc::new(InnerDummyCallManager {
                machine,
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
                charge_gas_calls: 0,
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

        fn charge_gas(&mut self, charge: crate::gas::GasCharge) -> crate::kernel::Result<()> {
            unsafe {
                self.borrow_mut().charge_gas_calls += 1;
            }
            self.gas_tracker_mut().apply_charge(charge)?;
            Ok(())
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
        use crate::kernel::{IpldBlockOps, Block};

        type TestingKernel = DefaultKernel<DummyCallManager>;
        type ExternalCallManager = ManuallyDrop<Weak<InnerDummyCallManager>>;

        // TODO gas functions assert calls are being charged properly
        // TODO maybe make util functions

        /// function to reduce a bit of boilerplate
        fn build_inspecting_test() -> anyhow::Result<(TestingKernel, ExternalCallManager)> {
            // call_manager is not dropped till the end of the function
            let call_manager = DummyCallManager::new_stub();
            // variable for value inspection, only upgrade after done mutating to avoid panic
            let refcell = call_manager.weak();

            let kern =
                TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
            Ok((kern, refcell))
        }

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
        fn ipld_ops_roundtrip() -> anyhow::Result<()> {
            let (mut kern, refcell) = build_inspecting_test()?;

            // roundtrip
            let id = kern.block_create(DAG_CBOR, "foo".as_bytes())?;
            let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;
            let stat = kern.block_stat(id)?;
            let (opened_id, opened_stat) = kern.block_open(&cid)?;

            // prevent new mutations
            let kern = &kern;
            // ok to upgrade into strong pointer since kern can't be mutated anymore
            let arc = &mut refcell.upgrade().unwrap();
            let external_call_manager = arc.as_ref();

            // create op should be 1
            assert_eq!(id, 1);
            // open op should be 2
            assert_eq!(opened_id - 1, id);

            // Stat
            assert_eq!(stat.codec, opened_stat.codec);
            assert_eq!(stat.codec, DAG_CBOR);
            assert_eq!(stat.size, opened_stat.size);
            assert_eq!(stat.size, 3);

            // assert gas charge calls
            assert_eq!(
                external_call_manager.charge_gas_calls,
                // open 2 (load charge and per-byte charge)
                // link 1
                // stat 1
                // create 1
                5
            );

            // drop strong ref *before* weak ref
            drop(kern);
            // ok to drop weak ref
            drop(ManuallyDrop::into_inner(refcell));
            Ok(())
        }

        #[test]
        fn ipld_ops_create_ids() -> anyhow::Result<()> {
            let (mut kern, refcell) = build_inspecting_test()?;

            let mut kern1 = TestingKernel::new(
                DummyCallManager::new_stub(),
                BlockRegistry::default(),
                0,
                0,
                0,
                0.into(),
            );

            let block = "foo".as_bytes();
            // make a block
            let id = kern.block_create(DAG_CBOR, block)?;
            let id1 = kern1.block_create(DAG_CBOR, "bar".as_bytes())?;
            
            // TODO are these assumption correct? other ID values could be used although it would be weird
            assert_eq!(id, 1, "first block id should be 1");
            assert_eq!(id, id1, "two blocks of the different content but same order should have the same block id");

            let id = kern1.block_create(DAG_CBOR, "baz".as_bytes())?;
            assert_eq!(id, 2, "second created block id should be 2");
            
            // prevent new mutations
            let _ = &kern;
            // ok to upgrade into strong pointer since kern can't be mutated anymore
            let arc = &mut refcell.upgrade().unwrap();
            let external_call_manager = arc.as_ref();
            
            {
                assert_eq!(external_call_manager.charge_gas_calls, 1, "charge_gas should called exactly once per block_create");

                let expected_create_price = external_call_manager
                    .machine
                    .context()
                    .price_list
                    .on_block_create(block.len() as usize)
                    .total();
                assert_eq!(external_call_manager.gas_tracker.gas_used(), expected_create_price);
            }
            

            Ok(())
        }
        #[test]
        fn ipld_ops_link() -> anyhow::Result<()> {
            let (mut kern, refcell) = build_inspecting_test()?;
            let (mut kern1, refcell1) = build_inspecting_test()?;

            // setup

            let block = "foo".as_bytes();
            let other_block = "baz".as_bytes();
            // link a block
            let id = kern.block_create(DAG_CBOR, block)?;
            let cid = kern.block_link(id, Code::Blake2b256.into(), 32)?;

            // link a block of the same data inside a different kernel
            let id1 = kern1.block_create(DAG_CBOR, block)?;
            let cid1 = kern1.block_link(id1, Code::Blake2b256.into(), 32)?;

            let other_id = kern1.block_create(DAG_CBOR, other_block)?;
            let other_cid = kern1.block_link(other_id, Code::Blake2b256.into(), 32)?;

            // prevent new mutations
            let kern = &kern;
            // ok to upgrade into strong pointer since kern can't be mutated anymore
            let arc = &mut refcell.upgrade().unwrap();
            let external_call_manager = arc.as_ref();
            // prevent new mutations
            let kern1 = &kern1;
            // ok to upgrade into strong pointer since kern can't be mutated anymore
            let arc1 = &mut refcell1.upgrade().unwrap();
            let _external_call_manager1 = arc1.as_ref();

            // assert

            assert!(external_call_manager.machine.blockstore().has(&cid)?, "block_link was called but CID was not found in the blockstore");
            assert_eq!(cid, cid1, "calling block_link in 2 different kernels of the same data and state should have the same CID");
            assert_ne!(cid, other_cid, "calling block_link with different data should make different CIDs");
            // assert gas
            {
                assert_eq!(external_call_manager.charge_gas_calls-1, 1, "charge_gas should only be called exactly once per block_link");
                
                let expected_block = Block::new(cid.codec(), block);
                let expected_create_price = external_call_manager
                    .machine
                    .context()
                    .price_list
                    .on_block_create(block.len() as usize)
                    .total();
                let expected_link_price = external_call_manager
                    .machine
                    .context()
                    .price_list
                    .on_block_link(expected_block.size() as usize)
                    .total();
                
                assert_eq!(external_call_manager.gas_tracker.gas_used(), expected_create_price + expected_link_price, "cost of creating & linking does not match price list")
            }
            
            // drop strong ref inside kern *before* weak ref
            drop(kern);
            drop(kern1);
            // ok to drop weak ref
            drop(ManuallyDrop::into_inner(refcell));
            drop(ManuallyDrop::into_inner(refcell1));

            Ok(())
        }



        // actor ops broken because origin addr is empty for now
    }
}
