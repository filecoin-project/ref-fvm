use std::mem::ManuallyDrop;
use std::sync::{Arc, Weak};

use anyhow::Context;
use derive_more::{Deref, DerefMut};
use fvm::call_manager::{Backtrace, CallManager, FinishRet, InvocationResult};
use fvm::gas::{Gas, GasCharge, GasTracker};
use fvm::kernel;
use fvm::machine::{Engine, EngineConfig, MachineContext, NetworkConfig};
use fvm::state_tree::{ActorState, StateTree};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::CborStore;
use fvm_shared::actor::builtin::Manifest;
use fvm_shared::address::Address;
use fvm_shared::state::StateTreeVersion;
// use fvm_shared::version::NetworkVersion;
use multihash::Code;

use super::*;
use crate::DummyExterns;

/// this is essentially identical to DefaultMachine, but has pub fields for reaching inside
pub struct DummyMachine {
    pub engine: Engine,
    pub state_tree: StateTree<MemoryBlockstore>,
    pub ctx: MachineContext,
    pub builtin_actors: Manifest,
}

// hardcoded elsewhere till relavant TODOs are solved
// const STUB_NETWORK_VER: NetworkVersion = NetworkVersion::V16;

impl DummyMachine {
    /// build a dummy machine with no builtin actors, and from empty & new state tree for unit tests
    pub fn new_stub() -> anyhow::Result<Self> {
        let bs = MemoryBlockstore::new();

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
                wasm_prices: fvm::__TESTING_FREE_WASM_PRICE,
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

    fn context(&self) -> &fvm::machine::MachineContext {
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
        _addr: &Address,
        _act: ActorState,
    ) -> kernel::Result<fvm_shared::ActorID> {
        todo!()
    }

    fn transfer(
        &mut self,
        _from: fvm_shared::ActorID,
        _to: fvm_shared::ActorID,
        _value: &fvm_shared::econ::TokenAmount,
    ) -> kernel::Result<()> {
        todo!()
    }

    fn into_store(self) -> Self::Blockstore {
        todo!()
    }
}

#[derive(Deref, DerefMut)]
// similar to `DefaultCallManager` but with public variables for modification by unit tests
pub struct InnerDummyCallManager {
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
pub struct DummyCallManager(Arc<InnerDummyCallManager>);

impl DummyCallManager {
    pub fn new_stub() -> Self {
        Self(Arc::new(InnerDummyCallManager {
            machine: DummyMachine::new_stub().unwrap(),
            gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)), // TODO this will need to be modified for gas limit testing
            origin: Address::new_actor(&[]),
            charge_gas_calls: 0,
            nonce: 0,
        }))
    }

    pub fn borrow(&self) -> &InnerDummyCallManager {
        self.0.as_ref()
    }

    // TODO this needs proper safety review
    /// similar to https://doc.rust-lang.org/src/alloc/sync.rs.html#1545
    /// panics if the Arc isn't the only strong pointer that exists.
    /// SAFETY: Any other Arc or Weak pointers to the same allocation must not be dereferenced for the duration of the returned borrow
    pub unsafe fn borrow_mut(&mut self) -> &mut InnerDummyCallManager {
        if Arc::strong_count(&self.0) != 1 {
            panic!("Only one strong pointer is allowed when mutating DummyCallManager")
        }
        &mut *(Arc::as_ptr(&self.0) as *mut InnerDummyCallManager)
    }

    /// the only truly safe way to use this is to deref the returned Weak ref **after or at the same time of DummyCallManager** `ManuallyDrop` helps enforce that the returned ref is either never dropped or explicitly dropped at some point (hopefully after )  
    /// see `borrow_mut()`
    pub fn weak(&self) -> ManuallyDrop<Weak<InnerDummyCallManager>> {
        ManuallyDrop::new(Arc::downgrade(&self.0))
    }
}

impl CallManager for DummyCallManager {
    type Machine = DummyMachine;

    fn new(machine: Self::Machine, _gas_limit: i64, origin: Address, nonce: u64) -> Self {
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
        _from: fvm_shared::ActorID,
        _to: Address,
        _method: fvm_shared::MethodNum,
        _params: Option<kernel::Block>,
        _value: &fvm_shared::econ::TokenAmount,
    ) -> kernel::Result<InvocationResult> {
        // Ok(InvocationResult::Return(None));
        todo!()
    }

    fn with_transaction(
        &mut self,
        _f: impl FnOnce(&mut Self) -> kernel::Result<InvocationResult>,
    ) -> kernel::Result<InvocationResult> {
        Ok(InvocationResult::Return(None))
    }

    fn finish(self) -> (FinishRet, Self::Machine) {
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

    fn gas_tracker(&self) -> &GasTracker {
        &self.borrow().gas_tracker
    }

    fn gas_tracker_mut(&mut self) -> &mut GasTracker {
        unsafe { &mut self.borrow_mut().gas_tracker }
    }

    fn charge_gas(&mut self, charge: GasCharge) -> kernel::Result<()> {
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
