use std::borrow::Borrow;
use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use fvm::call_manager::{Backtrace, CallManager, FinishRet, InvocationResult};
use fvm::externs::{Consensus, Externs, Rand};
use fvm::gas::{Gas, GasCharge, GasTracker};
use fvm::machine::{Engine, Machine, MachineContext, NetworkConfig};
use fvm::state_tree::{ActorState, StateTree};
use fvm::{kernel, Kernel};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::CborStore;
use fvm_shared::actor::builtin::Manifest;
use fvm_shared::address::Address;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use multihash::Code;

pub const STUB_NETWORK_VER: NetworkVersion = NetworkVersion::V15;

/// Unimplemented and empty `Externs` impl
pub struct DummyExterns;

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

/// Minimal *pseudo-functional* implementation of `Machine` for tests
pub struct DummyMachine {
    pub engine: Engine,
    pub state_tree: StateTree<MemoryBlockstore>,
    pub ctx: MachineContext,
    pub builtin_actors: Manifest,
}

impl DummyMachine {
    /// Build a dummy machine with no builtin actors and an empty state-tree.
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

        // add bundle root with version 1 and CID of the empty manifest
        let actors_cid = bs.put_cbor(&(1, manifest_cid), Code::Blake2b256).unwrap();

        // construct state tree from empty root state
        let state_tree = StateTree::new_from_root(bs, &root)?;

        let mut config = NetworkConfig::new(STUB_NETWORK_VER);

        // generate context from the new generated root and override actors with empty list
        let ctx = config.override_actors(actors_cid).for_epoch(0, root);

        Ok(Self {
            ctx,
            engine: Engine::new_default((&config).into())?,
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
        self.state_tree.into_store()
    }

    fn machine_id(&self) -> &str {
        todo!()
    }
}

/// Minimal *pseudo-functional* implementation CallManager
pub struct DummyCallManager {
    pub machine: DummyMachine,
    pub gas_tracker: GasTracker,
    pub origin: Address,
    pub nonce: u64,
    pub test_data: Rc<RefCell<TestData>>,
}

/// Information to be read by external tests
pub struct TestData {
    pub charge_gas_calls: usize,
}

impl DummyCallManager {
    pub fn new_stub() -> (Self, Rc<RefCell<TestData>>) {
        let rc = Rc::new(RefCell::new(TestData {
            charge_gas_calls: 0,
        }));
        let cell_ref = rc.clone();
        (
            Self {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)),
                origin: Address::new_actor(&[]),
                nonce: 0,
                test_data: rc,
            },
            cell_ref,
        )
    }

    pub fn new_with_gas(gas_tracker: GasTracker) -> (Self, Rc<RefCell<TestData>>) {
        let rc = Rc::new(RefCell::new(TestData {
            charge_gas_calls: 0,
        }));
        let cell_ref = rc.clone();
        (
            Self {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker,
                origin: Address::new_actor(&[]),
                nonce: 0,
                test_data: rc,
            },
            cell_ref,
        )
    }
}

impl CallManager for DummyCallManager {
    type Machine = DummyMachine;

    fn new(machine: Self::Machine, _gas_limit: i64, origin: Address, nonce: u64) -> Self {
        let rc = Rc::new(RefCell::new(TestData {
            charge_gas_calls: 0,
        }));
        Self {
            machine,
            gas_tracker: GasTracker::new(Gas::new(i64::MAX), Gas::new(0)),
            origin,
            nonce,
            test_data: rc,
        }
    }

    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        _from: fvm_shared::ActorID,
        _to: Address,
        _method: fvm_shared::MethodNum,
        _params: Option<kernel::Block>,
        _value: &fvm_shared::econ::TokenAmount,
    ) -> kernel::Result<InvocationResult> {
        // Ok(InvocationResult::Return(None))
        todo!()
    }

    fn with_transaction(
        &mut self,
        _f: impl FnOnce(&mut Self) -> kernel::Result<InvocationResult>,
    ) -> kernel::Result<InvocationResult> {
        // Ok(InvocationResult::Return(None))
        todo!()
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
            self.machine,
        )
    }

    fn machine(&self) -> &Self::Machine {
        &self.borrow().machine
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        &mut self.machine
    }

    fn gas_tracker(&self) -> &GasTracker {
        &self.borrow().gas_tracker
    }

    fn gas_tracker_mut(&mut self) -> &mut GasTracker {
        &mut self.gas_tracker
    }

    fn charge_gas(&mut self, charge: GasCharge) -> kernel::Result<()> {
        self.test_data.borrow_mut().charge_gas_calls += 1;
        self.gas_tracker_mut().apply_charge(charge)
    }

    fn origin(&self) -> Address {
        self.origin
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    fn next_actor_idx(&mut self) -> u64 {
        todo!()
    }

    fn invocation_count(&self) -> u64 {
        todo!()
    }
}
