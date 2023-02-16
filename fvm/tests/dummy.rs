// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::borrow::Borrow;
use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Context;
use cid::Cid;
use fvm::call_manager::{Backtrace, CallManager, FinishRet, InvocationResult};
use fvm::engine::Engine;
use fvm::externs::{Chain, Consensus, Externs, Rand};
use fvm::gas::{Gas, GasCharge, GasTimer, GasTracker};
use fvm::machine::limiter::MemoryLimiter;
use fvm::machine::{Machine, MachineContext, Manifest, NetworkConfig};
use fvm::state_tree::StateTree;
use fvm::{kernel, Kernel};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::{CborStore, DAG_CBOR};
use fvm_shared::address::Address;
use fvm_shared::bigint::Zero;
use fvm_shared::econ::TokenAmount;
use fvm_shared::event::StampedEvent;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, IDENTITY_HASH};
use multihash::{Code, Multihash};

pub const STUB_NETWORK_VER: NetworkVersion = NetworkVersion::V18;

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

impl Chain for DummyExterns {
    fn get_tipset_cid(&self, epoch: fvm_shared::clock::ChainEpoch) -> anyhow::Result<Cid> {
        Ok(Cid::new_v1(
            DAG_CBOR,
            Multihash::wrap(IDENTITY_HASH, &epoch.to_be_bytes()).unwrap(),
        ))
    }
}

#[derive(Default)]
pub struct DummyLimiter {
    curr_exec_memory_bytes: usize,
}

impl MemoryLimiter for DummyLimiter {
    fn with_stack_frame<T, G, F, R>(t: &mut T, g: G, f: F) -> R
    where
        G: Fn(&mut T) -> &mut Self,
        F: FnOnce(&mut T) -> R,
    {
        let memory_bytes = g(t).curr_exec_memory_bytes;
        let ret = f(t);
        g(t).curr_exec_memory_bytes = memory_bytes;
        ret
    }

    fn memory_used(&self) -> usize {
        self.curr_exec_memory_bytes
    }

    fn grow_memory(&mut self, bytes: usize) -> bool {
        self.curr_exec_memory_bytes += bytes;
        true
    }
}

/// Minimal *pseudo-functional* implementation of `Machine` for tests
pub struct DummyMachine {
    pub state_tree: StateTree<MemoryBlockstore>,
    pub ctx: MachineContext,
    pub builtin_actors: Manifest,
}

impl DummyMachine {
    /// Build a dummy machine with no builtin actors and an empty state-tree.
    pub fn new_stub() -> anyhow::Result<Self> {
        let bs = MemoryBlockstore::new();

        // generate new state root
        let mut state_tree = StateTree::new(bs, StateTreeVersion::V5)?;
        let root = state_tree.flush()?;
        let bs = state_tree.into_store();

        // Add empty built-in actors manifest to blockstore.
        let manifest = Manifest::dummy();
        let manifest_cid = bs.put_cbor(&Manifest::DUMMY_CODES, Code::Blake2b256)?;

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
        let ctx = config.override_actors(actors_cid).for_epoch(0, 0, root);

        Ok(Self {
            ctx,
            state_tree,
            builtin_actors: manifest,
        })
    }
}

impl Machine for DummyMachine {
    type Blockstore = MemoryBlockstore;
    type Externs = DummyExterns;
    type Limiter = DummyLimiter;

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

    fn into_store(self) -> Self::Blockstore {
        self.state_tree.into_store()
    }

    fn machine_id(&self) -> &str {
        todo!()
    }

    fn new_limiter(&self) -> Self::Limiter {
        DummyLimiter::default()
    }
}

/// Minimal *pseudo-functional* implementation CallManager
pub struct DummyCallManager {
    pub machine: DummyMachine,
    pub gas_tracker: GasTracker,
    pub gas_premium: TokenAmount,
    pub origin: ActorID,
    pub origin_address: Address,
    pub nonce: u64,
    pub test_data: Rc<RefCell<TestData>>,
    limits: DummyLimiter,
}

/// Information to be read by external tests
pub struct TestData {
    pub charge_gas_calls: usize,
}

const BLOCK_GAS_LIMIT: Gas = Gas::new(fvm_shared::BLOCK_GAS_LIMIT);

impl DummyCallManager {
    pub fn new_stub() -> (Self, Rc<RefCell<TestData>>) {
        let rc = Rc::new(RefCell::new(TestData {
            charge_gas_calls: 0,
        }));
        let cell_ref = rc.clone();
        (
            Self {
                machine: DummyMachine::new_stub().unwrap(),
                gas_tracker: GasTracker::new(BLOCK_GAS_LIMIT, Gas::new(0), false),
                origin: 0,
                nonce: 0,
                test_data: rc,
                limits: DummyLimiter::default(),
                origin_address: Address::new_id(0),
                gas_premium: TokenAmount::zero(),
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
                origin: 0,
                nonce: 0,
                test_data: rc,
                limits: DummyLimiter::default(),
                origin_address: Address::new_id(0),
                gas_premium: TokenAmount::zero(),
            },
            cell_ref,
        )
    }
}

impl CallManager for DummyCallManager {
    type Machine = DummyMachine;

    fn new(
        machine: Self::Machine,
        _engine: Engine,
        _gas_limit: u64,
        origin: ActorID,
        origin_address: Address,
        _receiver: Option<ActorID>,
        _receiver_address: Address,
        nonce: u64,
        gas_premium: TokenAmount,
    ) -> Self {
        let rc = Rc::new(RefCell::new(TestData {
            charge_gas_calls: 0,
        }));
        let limits = machine.new_limiter();
        Self {
            machine,
            gas_tracker: GasTracker::new(BLOCK_GAS_LIMIT, Gas::new(0), false),
            gas_premium,
            origin,
            origin_address,
            nonce,
            test_data: rc,
            limits,
        }
    }

    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        _from: fvm_shared::ActorID,
        _to: Address,
        _method: fvm_shared::MethodNum,
        _params: Option<kernel::Block>,
        _value: &fvm_shared::econ::TokenAmount,
        _gas_limit: Option<Gas>,
        _read_only: bool,
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

    fn finish(self) -> (kernel::Result<FinishRet>, Self::Machine) {
        (
            Ok(FinishRet {
                gas_used: 0,
                backtrace: Backtrace {
                    frames: Vec::new(),
                    cause: None,
                },
                exec_trace: Vec::new(),
                events: Vec::new(),
                events_root: None,
            }),
            self.machine,
        )
    }

    fn machine(&self) -> &Self::Machine {
        &self.borrow().machine
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        &mut self.machine
    }

    fn engine(&self) -> &Engine {
        todo!()
    }

    fn gas_tracker(&self) -> &GasTracker {
        &self.borrow().gas_tracker
    }

    fn charge_gas(&self, charge: GasCharge) -> kernel::Result<GasTimer> {
        self.test_data.borrow_mut().charge_gas_calls += 1;
        self.gas_tracker().apply_charge(charge)
    }

    fn origin(&self) -> ActorID {
        self.origin
    }

    fn gas_premium(&self) -> &TokenAmount {
        &self.gas_premium
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    fn next_actor_address(&self) -> Address {
        todo!()
    }

    fn create_actor(
        &mut self,
        _code_id: Cid,
        _actor_id: ActorID,
        _delegated_address: Option<Address>,
    ) -> kernel::Result<()> {
        todo!()
    }

    fn invocation_count(&self) -> u64 {
        todo!()
    }

    fn limiter_mut(&mut self) -> &mut <Self::Machine as Machine>::Limiter {
        &mut self.limits
    }

    fn append_event(&mut self, _evt: StampedEvent) {
        todo!()
    }

    fn resolve_address(&self, address: &Address) -> fvm::kernel::Result<Option<ActorID>> {
        self.machine.state_tree().lookup_id(address)
    }

    fn get_actor(&self, id: ActorID) -> fvm::kernel::Result<Option<fvm::state_tree::ActorState>> {
        self.machine.state_tree().get_actor(id)
    }

    fn set_actor(
        &mut self,
        id: ActorID,
        state: fvm::state_tree::ActorState,
    ) -> fvm::kernel::Result<()> {
        self.machine.state_tree_mut().set_actor(id, state);
        Ok(())
    }

    fn delete_actor(&mut self, id: ActorID) -> fvm::kernel::Result<()> {
        self.machine.state_tree_mut().delete_actor(id);
        Ok(())
    }

    fn transfer(
        &mut self,
        _from: ActorID,
        _to: ActorID,
        _value: &TokenAmount,
    ) -> fvm::kernel::Result<()> {
        todo!()
    }
}
