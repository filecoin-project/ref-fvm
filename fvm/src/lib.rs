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
    use anyhow::{anyhow, Context};
    use cid::Cid;
    use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
    use fvm_ipld_encoding::CborStore;
    use fvm_shared::actor::builtin::{load_manifest, Manifest};
    use fvm_shared::address::Address;
    use fvm_shared::consensus::ConsensusFault;
    use fvm_shared::state::StateTreeVersion;
    use fvm_shared::version::NetworkVersion;
    use log::debug;
    use multihash::Code;

    use crate::blockstore::BufferedBlockstore;
    use crate::call_manager::{CallManager, DefaultCallManager, InvocationResult, FinishRet, Backtrace};
    use crate::executor::DefaultExecutor;
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
            anyhow::Result::Ok((Some(ConsensusFault { target: todo!(), epoch: todo!(), fault_type: todo!() }), 0))
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

    struct DummyEngine;

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

        /// build self with no context as a stub
        pub fn new_stub() -> anyhow::Result<Self> {
            
            let mut bs = MemoryBlockstore::new();
            // generate new state root
            let mut state_tree = StateTree::new(bs, StateTreeVersion::V4)?;
            let root = state_tree.flush()?;
            let bs = state_tree.into_store();

            
            // An empty built-in actors manifest.
            let manifest = Manifest::new();
            let manifest_cid = bs.put_cbor(&manifest, Code::Blake2b256)?;

            // sanity check 
            bs
                .has(&root)
                .context("failed to load initial state-root")?;

                
            let actors_cid = bs.put_cbor(&(0, manifest_cid), Code::Blake2b256).unwrap();
            
            // construct state tree from root state
            let mut state_tree = StateTree::new_from_root(bs, &root)?; 
            
            let ctx = NetworkConfig::new(fvm_shared::version::NetworkVersion::V16)
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

    struct DummyCallManager {
        machine: DummyMachine,
        gas_tracker: GasTracker,
    }

    impl DummyCallManager {
        fn new_stub() -> Self {
            Self {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
            }
        }
    }

    impl CallManager for DummyCallManager {
        type Machine = DummyMachine;

        fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self {
            Self {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
            }
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
                    backtrace: Backtrace { frames: Vec::new(), cause: None },
                    exec_trace: Vec::new(),
                },
                self.machine
            )
        }

        fn machine(&self) -> &Self::Machine {
            &self.machine
        }

        fn machine_mut(&mut self) -> &mut Self::Machine {
            &mut self.machine
        }

        fn gas_tracker(&self) -> &crate::gas::GasTracker {
            &self.gas_tracker
        }

        fn gas_tracker_mut(&mut self) -> &mut crate::gas::GasTracker {
            &mut self.gas_tracker
        }

        fn origin(&self) -> Address {
            Address::new_actor(&[])
        }

        fn nonce(&self) -> u64 {
            0
        }

        fn next_actor_idx(&mut self) -> u64 {
            0
        }
    }

    mod default_kernel {
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
            let mut bs = MemoryBlockstore::default();

            let cid = bs.put_cbor(&"foo", multihash::Code::Blake2b256)?; // should this be actually hashed beforehand or does this hash automatically?

            let mut kern = TestingKernel::new(
                DummyCallManager::new_stub(),
                BlockRegistry::default(),
                0,
                0,
                0,
                0.into(),
            );

            kern.block_open(&cid)?; // this is incorrect

            // assert gas charge

            // assert block ID

            // assert block stat

            Ok(())
        }
    }
}
