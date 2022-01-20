use std::convert::{TryFrom, TryInto};

use cid::Cid;
use fvm::call_manager::{CallManager, DefaultCallManager, InvocationResult};
use fvm::gas::GasTracker;
use fvm::kernel::*;
use fvm::machine::{CallError, DefaultMachine, Machine, MachineContext};
use fvm::state_tree::{ActorState, StateTree};
use fvm::{Config, DefaultKernel};
use fvm_shared::address::Address;
use fvm_shared::bigint::{BigInt, ToBigInt};
use fvm_shared::blockstore::MemoryBlockstore;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::error::ExitCode;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::RANDOMNESS_LENGTH;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::sys::TokenAmount;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum, TOTAL_FILECOIN};
use num_traits::Zero;

use crate::externs::TestExterns;
use crate::vector::{MessageVector, Variant};

const DEFAULT_BASE_FEE: u64 = 100;

#[derive(Clone)]
pub struct TestData {
    circ_supply: TokenAmount,
}

pub struct TestMachine<M = Box<DefaultMachine<MemoryBlockstore, TestExterns>>> {
    pub machine: M,
    pub data: TestData,
}

impl TestMachine<Box<DefaultMachine<MemoryBlockstore, TestExterns>>> {
    pub fn new_for_vector(
        v: &MessageVector,
        variant: &Variant,
        blockstore: MemoryBlockstore,
    ) -> TestMachine<Box<DefaultMachine<MemoryBlockstore, TestExterns>>> {
        let network_version =
            NetworkVersion::try_from(variant.nv).expect("unrecognized network version");
        let base_fee = v
            .preconditions
            .basefee
            .map(|i| i.to_bigint().unwrap())
            .unwrap_or_else(|| BigInt::from(DEFAULT_BASE_FEE));
        let epoch = variant.epoch;
        let state_root = v.preconditions.state_tree.root_cid;

        let externs = TestExterns::new(&v.randomness);

        let mut wasm_conf = wasmtime::Config::default();
        wasm_conf
            .cache_config_load_default()
            .expect("failed to load cache config");

        let machine = DefaultMachine::new(
            Config {
                max_call_depth: 4096,
                initial_pages: 0,
                max_pages: 1024,
                engine: wasm_conf,
                debug: true, // Enable debug mode by default.
            },
            epoch,
            base_fee.try_into().unwrap(),
            TokenAmount::zero(),
            network_version,
            state_root,
            blockstore,
            externs,
        )
        .unwrap();

        TestMachine::<Box<DefaultMachine<_, _>>> {
            machine: Box::new(machine),
            data: TestData {
                circ_supply: v
                    .preconditions
                    .circ_supply
                    .map(|i| i.to_bigint().unwrap())
                    .unwrap_or_else(|| TOTAL_FILECOIN.clone())
                    .try_into()
                    .unwrap(),
            },
        }
    }
}

impl<M> Machine for TestMachine<M>
where
    M: Machine,
{
    type Blockstore = M::Blockstore;
    type Externs = M::Externs;

    fn engine(&self) -> &wasmtime::Engine {
        self.machine.engine()
    }

    fn config(&self) -> &Config {
        self.machine.config()
    }

    fn blockstore(&self) -> &Self::Blockstore {
        self.machine.blockstore()
    }

    fn context(&self) -> &MachineContext {
        self.machine.context()
    }

    fn externs(&self) -> &Self::Externs {
        self.machine.externs()
    }

    fn state_tree(&self) -> &StateTree<Self::Blockstore> {
        self.machine.state_tree()
    }

    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
        self.machine.state_tree_mut()
    }

    fn create_actor(&mut self, addr: &Address, act: ActorState) -> Result<ActorID> {
        self.machine.create_actor(addr, act)
    }

    fn load_module(&self, code: &Cid) -> Result<wasmtime::Module> {
        self.machine.load_module(code)
    }

    fn transfer(&mut self, from: ActorID, to: ActorID, value: TokenAmount) -> Result<()> {
        self.machine.transfer(from, to, value)
    }

    fn consume(self) -> Self::Blockstore {
        self.machine.consume()
    }

    fn flush(&mut self) -> Result<Cid> {
        self.machine.flush()
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

    fn new(machine: Self::Machine, gas_limit: i64, origin: Address, nonce: u64) -> Self {
        TestCallManager(C::new(machine, gas_limit, origin, nonce))
    }

    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: &fvm_shared::encoding::RawBytes,
        value: TokenAmount,
    ) -> Result<InvocationResult> {
        // K is the kernel specified by the non intercepted kernel.
        // We wrap that here.
        self.0
            .send::<TestKernel<K>>(from, to, method, params, value)
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

    fn finish(self) -> (i64, Vec<CallError>, Self::Machine) {
        self.0.finish()
    }

    fn machine(&self) -> &Self::Machine {
        self.0.machine()
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        self.0.machine_mut()
    }

    fn gas_tracker(&self) -> &GasTracker {
        self.0.gas_tracker()
    }

    fn gas_tracker_mut(&mut self) -> &mut GasTracker {
        self.0.gas_tracker_mut()
    }

    fn origin(&self) -> Address {
        self.0.origin()
    }

    fn nonce(&self) -> u64 {
        self.0.nonce()
    }

    fn next_actor_idx(&mut self) -> u64 {
        self.0.next_actor_idx()
    }

    fn push_error(&mut self, e: CallError) {
        self.0.push_error(e)
    }

    fn clear_error(&mut self) {
        self.0.clear_error()
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

    fn state_tree(&self) -> &StateTree<<Self::Machine as Machine>::Blockstore> {
        self.0.state_tree()
    }

    fn state_tree_mut(&mut self) -> &mut StateTree<<Self::Machine as Machine>::Blockstore> {
        self.0.state_tree_mut()
    }

    fn charge_gas(&mut self, charge: fvm::gas::GasCharge) -> Result<()> {
        self.0.charge_gas(charge)
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

    fn take(self) -> Self::CallManager
    where
        Self: Sized,
    {
        self.0.take().0
    }

    fn new(
        mgr: Self::CallManager,
        from: ActorID,
        to: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
    ) -> Self
    where
        Self: Sized,
    {
        // Extract the test data.
        let data = mgr.machine().data.clone();

        TestKernel(
            K::new(TestCallManager(mgr), from, to, method, value_received),
            data,
        )
    }
}

impl<M, C, K> ActorOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn resolve_address(&self, address: &Address) -> Result<Option<ActorID>> {
        self.0.resolve_address(address)
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Result<Option<Cid>> {
        self.0.get_actor_code_cid(addr)
    }

    fn new_actor_address(&mut self) -> Result<Address> {
        self.0.new_actor_address()
    }

    fn create_actor(&mut self, code_id: Cid, actor_id: ActorID) -> Result<()> {
        self.0.create_actor(code_id, actor_id)
    }
}

impl<M, C, K> BlockOps for TestKernel<K>
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

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32> {
        self.0.block_read(id, offset, buf)
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat> {
        self.0.block_stat(id)
    }

    fn block_get(&self, id: BlockId) -> Result<(u64, Vec<u8>)> {
        self.0.block_get(id)
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
    fn hash_blake2b(&mut self, data: &[u8]) -> Result<[u8; 32]> {
        self.0.hash_blake2b(data)
    }

    // forwarded
    fn compute_unsealed_sector_cid(
        &mut self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        self.0.compute_unsealed_sector_cid(proof_type, pieces)
    }

    // forwarded
    fn batch_verify_seals(&mut self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>> {
        self.0.batch_verify_seals(vis)
    }

    // NOT forwarded
    fn verify_signature(
        &mut self,
        _signature: &Signature,
        _signer: &Address,
        _plaintext: &[u8],
    ) -> Result<bool> {
        Ok(true)
    }

    // NOT forwarded
    fn verify_seal(&mut self, _vi: &SealVerifyInfo) -> Result<bool> {
        Ok(true)
    }

    // NOT forwarded
    fn verify_post(&mut self, _verify_info: &WindowPoStVerifyInfo) -> Result<bool> {
        Ok(true)
    }

    // NOT forwarded
    fn verify_consensus_fault(
        &mut self,
        _h1: &[u8],
        _h2: &[u8],
        _extra: &[u8],
    ) -> Result<Option<ConsensusFault>> {
        // TODO this seems wrong, should probably be parameterized.
        Ok(None)
    }

    // NOT forwarded
    fn verify_aggregate_seals(&mut self, _agg: &AggregateSealVerifyProofAndInfos) -> Result<bool> {
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

    fn push_syscall_error(&mut self, e: SyscallError) {
        self.0.push_syscall_error(e)
    }

    fn push_actor_error(&mut self, code: ExitCode, message: String) {
        self.0.push_actor_error(code, message)
    }

    fn clear_error(&mut self) {
        self.0.clear_error()
    }
}

impl<M, C, K> GasOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn charge_gas(&mut self, name: &str, compute: i64) -> Result<()> {
        self.0.charge_gas(name, compute)
    }
}

impl<M, C, K> MessageOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn msg_caller(&self) -> ActorID {
        self.0.msg_caller()
    }

    fn msg_receiver(&self) -> ActorID {
        self.0.msg_receiver()
    }

    fn msg_method_number(&self) -> MethodNum {
        self.0.msg_method_number()
    }

    fn msg_value_received(&self) -> TokenAmount {
        self.0.msg_value_received()
    }
}

impl<M, C, K> NetworkOps for TestKernel<K>
where
    M: Machine,
    C: CallManager<Machine = TestMachine<M>>,
    K: Kernel<CallManager = TestCallManager<C>>,
{
    fn network_epoch(&self) -> ChainEpoch {
        self.0.network_epoch()
    }

    fn network_version(&self) -> NetworkVersion {
        self.0.network_version()
    }

    fn network_base_fee(&self) -> TokenAmount {
        self.0.network_base_fee()
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
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        self.0
            .get_randomness_from_tickets(personalization, rand_epoch, entropy)
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
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
        params: &fvm_shared::encoding::RawBytes,
        value: TokenAmount,
    ) -> Result<InvocationResult> {
        self.0.send(recipient, method, params, value)
    }
}
