// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use cid::Cid;
use futures::executor::block_on;
use fvm::call_manager::{CallManager, DefaultCallManager, FinishRet, InvocationResult};
use fvm::engine::Engine;
use fvm::gas::{price_list_by_network_version, Gas, GasTimer, GasTracker, PriceList};
use fvm::kernel::*;
use fvm::machine::limiter::MemoryLimiter;
use fvm::machine::{DefaultMachine, Machine, MachineContext, Manifest, NetworkConfig};
use fvm::state_tree::{ActorState, StateTree};
use fvm::DefaultKernel;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_car::load_car_unchecked;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::signature::{
    SignatureType, SECP_PUB_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
};
use fvm_shared::econ::TokenAmount;
use fvm_shared::event::StampedEvent;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::RANDOMNESS_LENGTH;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
    WindowPoStVerifyInfo,
};
use fvm_shared::sys::SendFlags;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum, TOTAL_FILECOIN};
use multihash::MultihashGeneric;

use crate::externs::TestExterns;
use crate::vector::{MessageVector, Variant};

const DEFAULT_BASE_FEE: u64 = 100;

#[derive(Clone)]
pub struct TestData {
    circ_supply: TokenAmount,
    price_list: PriceList,
}

/// Statistics about the resources used by test vector executions.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestStats {
    pub min_instance_memory_bytes: usize,
    pub max_instance_memory_bytes: usize,
    pub max_table_elements: u32,
    pub min_table_elements: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TestStatsGlobal {
    /// Min/Max for the initial memory.
    pub init: TestStats,
    /// Min/Max of the overall memory.
    pub exec: TestStats,
}

impl TestStatsGlobal {
    pub fn new_ref() -> TestStatsRef {
        Some(Arc::new(Mutex::new(Self::default())))
    }
}

/// Global statistics about all test vector executions.
pub type TestStatsRef = Option<Arc<Mutex<TestStatsGlobal>>>;

pub struct TestMachine<M = Box<DefaultMachine<MemoryBlockstore, TestExterns>>> {
    pub machine: M,
    pub data: TestData,
    stats: TestStatsRef,
}

impl TestMachine<Box<DefaultMachine<MemoryBlockstore, TestExterns>>> {
    pub fn new_for_vector(
        v: &MessageVector,
        variant: &Variant,
        blockstore: MemoryBlockstore,
        stats: TestStatsRef,
        tracing: bool,
        price_network_version: Option<NetworkVersion>,
    ) -> anyhow::Result<TestMachine<Box<DefaultMachine<MemoryBlockstore, TestExterns>>>> {
        let network_version = NetworkVersion::try_from(variant.nv)
            .map_err(|_| anyhow!("unrecognized network version"))?;

        let base_fee = v
            .preconditions
            .basefee
            .map(TokenAmount::from_atto)
            .unwrap_or_else(|| TokenAmount::from_atto(DEFAULT_BASE_FEE));
        let epoch = variant.epoch;
        let state_root = v.preconditions.state_tree.root_cid;

        let externs = TestExterns::new(&v.randomness);

        // Load the builtin actors bundles into the blockstore.
        let nv_actors = TestMachine::import_actors(&blockstore);

        // Get the builtin actors index for the concrete network version.
        let builtin_actors = *nv_actors
            .get(&network_version)
            .ok_or_else(|| anyhow!("no builtin actors index for NV {network_version}"))?;

        let mut nc = NetworkConfig::new(network_version);
        nc.override_actors(builtin_actors);
        let mut mc = nc.for_epoch(epoch, (epoch * 30) as u64, state_root);
        // Allow overriding prices to some other network version.
        if let Some(nv) = price_network_version {
            nc.price_list = price_list_by_network_version(nv);
        }
        mc.set_base_fee(base_fee);
        mc.tracing = tracing;

        let machine = DefaultMachine::new(&mc, blockstore, externs).unwrap();

        let price_list = machine.context().price_list.clone();

        let machine = TestMachine::<Box<DefaultMachine<_, _>>> {
            machine: Box::new(machine),
            data: TestData {
                circ_supply: v
                    .preconditions
                    .circ_supply
                    .map(TokenAmount::from_atto)
                    .unwrap_or_else(|| TOTAL_FILECOIN.clone()),
                price_list,
            },
            stats,
        };

        Ok(machine)
    }

    pub fn import_actors(blockstore: &MemoryBlockstore) -> BTreeMap<NetworkVersion, Cid> {
        let bundles = [(NetworkVersion::V18, actors_v10::BUNDLE_CAR)];
        bundles
            .into_iter()
            .map(|(nv, car)| {
                let roots = block_on(async { load_car_unchecked(blockstore, car).await.unwrap() });
                assert_eq!(roots.len(), 1);
                (nv, roots[0])
            })
            .collect()
    }
}

impl<M> Machine for TestMachine<M>
where
    M: Machine,
{
    type Blockstore = M::Blockstore;
    type Externs = M::Externs;
    type Limiter = TestLimiter<M::Limiter>;

    fn blockstore(&self) -> &Self::Blockstore {
        self.machine.blockstore()
    }

    fn context(&self) -> &MachineContext {
        self.machine.context()
    }

    fn externs(&self) -> &Self::Externs {
        self.machine.externs()
    }

    fn builtin_actors(&self) -> &Manifest {
        self.machine.builtin_actors()
    }

    fn state_tree(&self) -> &StateTree<Self::Blockstore> {
        self.machine.state_tree()
    }

    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
        self.machine.state_tree_mut()
    }

    fn into_store(self) -> Self::Blockstore {
        self.machine.into_store()
    }

    fn flush(&mut self) -> Result<Cid> {
        self.machine.flush()
    }

    fn machine_id(&self) -> &str {
        self.machine.machine_id()
    }

    fn new_limiter(&self) -> Self::Limiter {
        TestLimiter {
            inner: self.machine.new_limiter(),
            global_stats: self.stats.clone(),
            local_stats: TestStats::default(),
        }
    }
}

/// A CallManager that wraps kernels in an InterceptKernel.
// NOTE: For now, this _must_ be transparent because we transmute a pointer.
#[repr(transparent)]
pub struct TestCallManager<C: CallManager = DefaultCallManager<TestMachine>>(pub C);

impl<M, C> CallManager for TestCallManager<C>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
{
    type Machine = C::Machine;

    fn new(
        machine: Self::Machine,
        engine: Engine,
        gas_limit: u64,
        origin: ActorID,
        origin_address: Address,
        receiver: Option<ActorID>,
        receiver_address: Address,
        nonce: u64,
        gas_premium: TokenAmount,
    ) -> Self {
        TestCallManager(C::new(
            machine,
            engine,
            gas_limit,
            origin,
            origin_address,
            receiver,
            receiver_address,
            nonce,
            gas_premium,
        ))
    }

    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: Option<Block>,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        read_only: bool,
    ) -> Result<InvocationResult> {
        // K is the kernel specified by the non intercepted kernel.
        // We wrap that here.
        self.0
            .send::<TestKernel<K>>(from, to, method, params, value, gas_limit, read_only)
    }

    fn with_transaction(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<InvocationResult>,
    ) -> Result<InvocationResult> {
        // This transmute is _safe_ because this type is "repr transparent".
        let inner_ptr = &mut self.0 as *mut C;
        self.0.with_transaction(|inner: &mut C| unsafe {
            // Make sure that we've got the right pointer. Otherwise, this cast definitely isn't
            // safe.
            assert_eq!(inner_ptr, inner as *mut C);

            // Ok, we got the pointer we expected, casting back to the interceptor is safe.
            f(&mut *(inner as *mut C as *mut Self))
        })
    }

    fn finish(self) -> (Result<FinishRet>, Self::Machine) {
        self.0.finish()
    }

    fn machine(&self) -> &Self::Machine {
        self.0.machine()
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        self.0.machine_mut()
    }

    fn engine(&self) -> &Engine {
        self.0.engine()
    }

    fn gas_tracker(&self) -> &GasTracker {
        self.0.gas_tracker()
    }

    fn gas_premium(&self) -> &TokenAmount {
        self.0.gas_premium()
    }

    fn origin(&self) -> ActorID {
        self.0.origin()
    }

    fn nonce(&self) -> u64 {
        self.0.nonce()
    }

    fn next_actor_address(&self) -> Address {
        self.0.next_actor_address()
    }

    fn create_actor(
        &mut self,
        code_id: Cid,
        actor_id: ActorID,
        delegated_address: Option<Address>,
    ) -> Result<()> {
        self.0.create_actor(code_id, actor_id, delegated_address)
    }

    fn price_list(&self) -> &fvm::gas::PriceList {
        self.0.price_list()
    }

    fn context(&self) -> &MachineContext {
        self.0.context()
    }

    fn blockstore(&self) -> &<Self::Machine as Machine>::Blockstore {
        self.0.blockstore()
    }

    fn externs(&self) -> &<Self::Machine as Machine>::Externs {
        self.0.externs()
    }

    fn charge_gas(&self, charge: fvm::gas::GasCharge) -> Result<GasTimer> {
        self.0.charge_gas(charge)
    }

    fn invocation_count(&self) -> u64 {
        self.0.invocation_count()
    }

    fn limiter_mut(&mut self) -> &mut <Self::Machine as Machine>::Limiter {
        self.0.limiter_mut()
    }

    fn append_event(&mut self, evt: StampedEvent) {
        self.0.append_event(evt)
    }

    fn resolve_address(&self, address: &Address) -> Result<Option<ActorID>> {
        self.0.resolve_address(address)
    }

    fn get_actor(&self, id: ActorID) -> Result<Option<ActorState>> {
        self.0.get_actor(id)
    }

    fn set_actor(&mut self, id: ActorID, state: ActorState) -> Result<()> {
        self.0.set_actor(id, state)
    }

    fn delete_actor(&mut self, id: ActorID) -> Result<()> {
        self.0.delete_actor(id)
    }

    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
        self.0.transfer(from, to, value)
    }
}

/// A kernel for intercepting syscalls.
pub struct TestKernel<K = DefaultKernel<TestCallManager>>(pub K, pub TestData);

impl<M, C, K> Kernel for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    type CallManager = C;

    fn into_inner(self) -> (Self::CallManager, BlockRegistry)
    where
        Self: Sized,
    {
        let (cm, br) = self.0.into_inner();
        (cm.0, br)
    }

    fn new(
        mgr: Self::CallManager,
        blocks: BlockRegistry,
        caller: ActorID,
        actor_id: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
        read_only: bool,
    ) -> Self
    where
        Self: Sized,
    {
        // Extract the test data.
        let data = mgr.machine().data.clone();

        TestKernel(
            K::new(
                TestCallManager(mgr),
                blocks,
                caller,
                actor_id,
                method,
                value_received,
                read_only,
            ),
            data,
        )
    }

    fn machine(&self) -> &<Self::CallManager as CallManager>::Machine {
        self.0.machine()
    }
}

impl<M, C, K> ActorOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn resolve_address(&self, address: &Address) -> Result<ActorID> {
        self.0.resolve_address(address)
    }

    fn get_actor_code_cid(&self, id: ActorID) -> Result<Cid> {
        self.0.get_actor_code_cid(id)
    }

    fn next_actor_address(&self) -> Result<Address> {
        self.0.next_actor_address()
    }

    fn create_actor(
        &mut self,
        code_id: Cid,
        actor_id: ActorID,
        delegated_address: Option<Address>,
    ) -> Result<()> {
        self.0.create_actor(code_id, actor_id, delegated_address)
    }

    fn get_builtin_actor_type(&self, code_cid: &Cid) -> Result<u32> {
        self.0.get_builtin_actor_type(code_cid)
    }

    fn get_code_cid_for_type(&self, typ: u32) -> Result<Cid> {
        self.0.get_code_cid_for_type(typ)
    }

    #[cfg(feature = "m2-native")]
    fn install_actor(&mut self, _code_id: Cid) -> Result<()> {
        Ok(())
    }

    fn balance_of(&self, actor_id: ActorID) -> Result<TokenAmount> {
        self.0.balance_of(actor_id)
    }

    fn lookup_delegated_address(&self, actor_id: ActorID) -> Result<Option<Address>> {
        self.0.lookup_delegated_address(actor_id)
    }
}

impl<M, C, K> IpldBlockOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn block_open(&mut self, cid: &Cid) -> Result<(BlockId, BlockStat)> {
        self.0.block_open(cid)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId> {
        self.0.block_create(codec, data)
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid> {
        self.0.block_link(id, hash_fun, hash_len)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<i32> {
        self.0.block_read(id, offset, buf)
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat> {
        self.0.block_stat(id)
    }
}

impl<M, C, K> CircSupplyOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    // Not forwarded. Circulating supply is taken from the TestData.
    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        Ok(self.1.circ_supply.clone())
    }
}

impl<M, C, K> CryptoOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    // forwarded
    fn hash(&self, code: u64, data: &[u8]) -> Result<MultihashGeneric<64>> {
        self.0.hash(code, data)
    }

    // forwarded
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        self.0.compute_unsealed_sector_cid(proof_type, pieces)
    }

    // forwarded
    fn verify_signature(
        &self,
        sig_type: SignatureType,
        signature: &[u8],
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool> {
        self.0
            .verify_signature(sig_type, signature, signer, plaintext)
    }

    // forwarded
    fn recover_secp_public_key(
        &self,
        hash: &[u8; SECP_SIG_MESSAGE_HASH_SIZE],
        signature: &[u8; SECP_SIG_LEN],
    ) -> Result<[u8; SECP_PUB_LEN]> {
        self.0.recover_secp_public_key(hash, signature)
    }

    // NOT forwarded
    fn batch_verify_seals(&self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>> {
        Ok(vec![true; vis.len()])
    }

    // NOT forwarded
    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<bool> {
        let charge = self.1.price_list.on_verify_seal(vi);
        let _ = self.0.charge_gas(&charge.name, charge.total())?;
        Ok(true)
    }

    // NOT forwarded
    fn verify_post(&self, vi: &WindowPoStVerifyInfo) -> Result<bool> {
        let charge = self.1.price_list.on_verify_post(vi);
        let _ = self.0.charge_gas(&charge.name, charge.total())?;
        Ok(true)
    }

    // NOT forwarded
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>> {
        let charge = self
            .1
            .price_list
            .on_verify_consensus_fault(h1.len(), h2.len(), extra.len());
        let _ = self.0.charge_gas(&charge.name, charge.total())?;
        Ok(None)
    }

    // NOT forwarded
    fn verify_aggregate_seals(&self, agg: &AggregateSealVerifyProofAndInfos) -> Result<bool> {
        let charge = self.1.price_list.on_verify_aggregate_seals(agg);
        let _ = self.0.charge_gas(&charge.name, charge.total())?;
        Ok(true)
    }

    // NOT forwarded
    fn verify_replica_update(&self, rep: &ReplicaUpdateInfo) -> Result<bool> {
        let charge = self.1.price_list.on_verify_replica_update(rep);
        let _ = self.0.charge_gas(&charge.name, charge.total())?;
        Ok(true)
    }
}

impl<M, C, K> DebugOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn log(&self, msg: String) {
        self.0.log(msg)
    }

    fn debug_enabled(&self) -> bool {
        self.0.debug_enabled()
    }

    fn store_artifact(&self, name: &str, data: &[u8]) -> Result<()> {
        self.0.store_artifact(name, data)
    }
}

impl<M, C, K> GasOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn gas_used(&self) -> Gas {
        self.0.gas_used()
    }

    fn charge_gas(&self, name: &str, compute: Gas) -> Result<GasTimer> {
        self.0.charge_gas(name, compute)
    }

    fn price_list(&self) -> &PriceList {
        self.0.price_list()
    }

    fn gas_available(&self) -> Gas {
        self.0.gas_available()
    }
}

impl<M, C, K> MessageOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn msg_context(&self) -> Result<fvm_shared::sys::out::vm::MessageContext> {
        self.0.msg_context()
    }
}

impl<M, C, K> NetworkOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn network_context(&self) -> Result<fvm_shared::sys::out::network::NetworkContext> {
        self.0.network_context()
    }

    fn tipset_cid(&self, epoch: ChainEpoch) -> Result<Cid> {
        self.0.tipset_cid(epoch)
    }
}

impl<M, C, K> RandomnessOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn get_randomness_from_tickets(
        &self,
        personalization: i64,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        self.0
            .get_randomness_from_tickets(personalization, rand_epoch, entropy)
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: i64,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        self.0
            .get_randomness_from_beacon(personalization, rand_epoch, entropy)
    }
}

impl<M, C, K> SelfOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn root(&self) -> Result<Cid> {
        self.0.root()
    }

    fn set_root(&mut self, root: Cid) -> Result<()> {
        self.0.set_root(root)
    }

    fn current_balance(&self) -> Result<TokenAmount> {
        self.0.current_balance()
    }

    fn self_destruct(&mut self, beneficiary: &Address) -> Result<()> {
        self.0.self_destruct(beneficiary)
    }
}

impl<M, C, K> SendOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn send(
        &mut self,
        recipient: &Address,
        method: u64,
        params: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<SendResult> {
        self.0
            .send(recipient, method, params, value, gas_limit, flags)
    }
}

impl<K> LimiterOps for TestKernel<K>
where
    K: LimiterOps,
{
    type Limiter = K::Limiter;

    fn limiter_mut(&mut self) -> &mut Self::Limiter {
        self.0.limiter_mut()
    }
}

impl<M, C, K> EventOps for TestKernel<K>
where
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
    M: Machine,
{
    fn emit_event(&mut self, raw_evt: &[u8]) -> Result<()> {
        self.0.emit_event(raw_evt)
    }
}

/// Wrap a `ResourceLimiter` and collect statistics.
pub struct TestLimiter<L> {
    inner: L,
    global_stats: TestStatsRef,
    local_stats: TestStats,
}

/// Store the minimum of the maximums of desired memories in the global stats.
impl<L> Drop for TestLimiter<L> {
    fn drop(&mut self) {
        if let Some(ref stats) = self.global_stats {
            if let Ok(mut stats) = stats.lock() {
                let max_desired = self.local_stats.max_instance_memory_bytes;
                let min_desired = self.local_stats.min_instance_memory_bytes;

                if stats.exec.max_instance_memory_bytes < max_desired {
                    stats.exec.max_instance_memory_bytes = max_desired;
                }

                if stats.exec.min_instance_memory_bytes == 0
                    || stats.exec.min_instance_memory_bytes > max_desired
                {
                    stats.exec.min_instance_memory_bytes = max_desired;
                }

                if stats.init.max_instance_memory_bytes < min_desired {
                    stats.init.max_instance_memory_bytes = min_desired;
                }

                if stats.init.min_instance_memory_bytes == 0
                    || stats.init.min_instance_memory_bytes > min_desired
                {
                    stats.init.min_instance_memory_bytes = min_desired;
                }

                let max_desired = self.local_stats.max_table_elements;
                let min_desired = self.local_stats.min_table_elements;

                if stats.exec.max_table_elements < max_desired {
                    stats.exec.max_table_elements = max_desired;
                }

                if stats.exec.min_table_elements == 0 || stats.exec.min_table_elements > max_desired
                {
                    stats.exec.min_table_elements = max_desired;
                }

                if stats.init.max_table_elements < min_desired {
                    stats.init.max_table_elements = min_desired;
                }

                if stats.init.min_table_elements == 0 || stats.init.min_table_elements > min_desired
                {
                    stats.init.min_table_elements = min_desired;
                }
            }
        }
    }
}

impl<L> MemoryLimiter for TestLimiter<L>
where
    L: MemoryLimiter,
{
    fn memory_used(&self) -> usize {
        self.inner.memory_used()
    }

    fn with_stack_frame<T, G, F, R>(t: &mut T, g: G, f: F) -> R
    where
        G: Fn(&mut T) -> &mut Self,
        F: FnOnce(&mut T) -> R,
    {
        L::with_stack_frame(t, |t| &mut g(t).inner, f)
    }

    fn grow_instance_memory(&mut self, from: usize, to: usize) -> bool {
        if self.local_stats.max_instance_memory_bytes < to {
            self.local_stats.max_instance_memory_bytes = to;
        }

        if from == 0 && to < self.local_stats.min_instance_memory_bytes {
            self.local_stats.min_instance_memory_bytes = to;
        }

        self.inner.grow_instance_memory(from, to)
    }

    fn grow_instance_table(&mut self, from: u32, to: u32) -> bool {
        if self.local_stats.max_table_elements < to {
            self.local_stats.max_table_elements = to;
        }

        if from == 0 && to < self.local_stats.min_table_elements {
            self.local_stats.min_table_elements = to;
        }

        self.inner.grow_instance_table(from, to)
    }

    fn grow_memory(&mut self, _: usize) -> bool {
        // We don't expect this to be called explicitly.
        panic!("explicit call to grow_memory")
    }
}
