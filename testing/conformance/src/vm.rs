// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};

use ambassador::Delegate;
use anyhow::anyhow;
use cid::Cid;
use fvm::kernel::filecoin::{DefaultFilecoinKernel, FilecoinKernel};
use fvm::syscalls::InvocationData;

use fvm::call_manager::{CallManager, DefaultCallManager};
use fvm::gas::{price_list_by_network_version, Gas, GasTimer, PriceList};
use fvm::kernel::*;
use fvm::machine::limiter::MemoryLimiter;
use fvm::machine::{DefaultMachine, Machine, MachineContext, Manifest, NetworkConfig};
use fvm::state_tree::StateTree;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::Address;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::econ::TokenAmount;
use fvm_shared::piece::PieceInfo;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
};
use fvm_shared::sys::SendFlags;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum, TOTAL_FILECOIN};
use wasmtime::Linker;

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

        let mut nc = NetworkConfig::new(network_version);
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

/// A kernel for intercepting syscalls.
// TestKernel is coupled to TestMachine because it needs to use that to plumb the
// TestData through when it's destroyed into a CallManager then recreated by that CallManager.
#[derive(Delegate)]
#[delegate(IpldBlockOps, where = "K: IpldBlockOps", target = "0")]
#[delegate(ActorOps, where = "K: ActorOps", target = "0")]
#[delegate(CryptoOps, where = "K: CryptoOps", target = "0")]
#[delegate(DebugOps, where = "K: DebugOps", target = "0")]
#[delegate(SystemOps, where = "K: SystemOps", target = "0")]
#[delegate(ChainOps, where = "K: ChainOps", target = "0")]
pub struct TestKernel<K = DefaultFilecoinKernel<DefaultCallManager<TestMachine>>>(
    pub K,
    pub TestData,
);

impl<M, C, K> Kernel for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = C> + SyscallHandler<Self>,
{
    type CallManager = K::CallManager;
    type Limiter = K::Limiter;

    fn new(
        mgr: C,
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
                mgr,
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

    fn into_inner(self) -> (Self::CallManager, BlockRegistry)
    where
        Self: Sized,
    {
        self.0.into_inner()
    }

    fn machine(&self) -> &<Self::CallManager as CallManager>::Machine {
        self.0.machine()
    }

    fn limiter_mut(&mut self) -> &mut Self::Limiter {
        self.0.limiter_mut()
    }

    fn price_list(&self) -> &PriceList {
        self.0.price_list()
    }

    fn charge_gas(&self, name: &str, compute: Gas) -> Result<GasTimer> {
        self.0.charge_gas(name, compute)
    }

    fn gas_available(&self) -> Gas {
        self.0.gas_available()
    }

    fn gas_used(&self) -> Gas {
        self.0.gas_available()
    }
}

impl<M, C, K> CallOps<Self> for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = C> + CallOps<Self>,
{
    /// Sends a message to another actor.
    /// The method type parameter K is the type of the kernel to instantiate for
    /// the receiving actor. This is necessary to support wrapping a kernel, so the outer
    /// kernel can specify its Self as the receiver's kernel type, rather than the wrapped
    /// kernel specifying its Self.
    /// This method is part of the Kernel trait so it can refer to the Self::CallManager
    /// associated type necessary to constrain K.
    fn send(
        &mut self,
        recipient: &Address,
        method: u64,
        params: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<CallResult> {
        self.0
            .send(recipient, method, params, value, gas_limit, flags)
    }

    fn upgrade_actor(&mut self, new_code_cid: Cid, params_id: BlockId) -> Result<CallResult> {
        self.0.upgrade_actor(new_code_cid, params_id)
    }
}

impl<M, C, K> SyscallHandler<Self> for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = C> + SyscallHandler<Self>,
{
    fn bind_syscalls(linker: &mut Linker<InvocationData<Self>>) -> anyhow::Result<()> {
        K::bind_syscalls(linker)
    }
}

impl<M, C, K> FilecoinKernel for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: FilecoinKernel<CallManager = C> + SyscallHandler<Self>,
{
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        self.0.compute_unsealed_sector_cid(proof_type, pieces)
    }

    fn verify_post(&self, verify_info: &fvm_shared::sector::WindowPoStVerifyInfo) -> Result<bool> {
        self.0.verify_post(verify_info)
    }

    // NOT forwarded
    fn batch_verify_seals(&self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>> {
        Ok(vec![true; vis.len()])
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

    // Not forwarded. Circulating supply is taken from the TestData.
    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        Ok(self.1.circ_supply.clone())
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
