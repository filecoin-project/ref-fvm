use fvm_shared::ActorID;

use crate::{call_manager::CallManager, kernel::*, machine::Machine};

use super::{InterceptCallManager, InterceptMachine};

/// A kernel for intercepting syscalls.
pub struct InterceptKernel<K>(pub K);

impl<M, C, K, D> Kernel for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
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
        from: fvm_shared::ActorID,
        to: fvm_shared::ActorID,
        method: fvm_shared::MethodNum,
        value_received: fvm_shared::econ::TokenAmount,
    ) -> Self
    where
        Self: Sized,
    {
        // NOTE: You can access the "data" field on the machine here with
        // mgr.machine().data
        //
        // You can then embed any data you want inside the intercept kernel.
        InterceptKernel(K::new(
            InterceptCallManager(mgr),
            from,
            to,
            method,
            value_received,
        ))
    }
}

impl<M, C, K, D> ActorOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn resolve_address(
        &self,
        address: &fvm_shared::address::Address,
    ) -> Result<Option<fvm_shared::ActorID>> {
        self.0.resolve_address(address)
    }

    fn get_actor_code_cid(&self, addr: &fvm_shared::address::Address) -> Result<Option<cid::Cid>> {
        self.0.get_actor_code_cid(addr)
    }

    fn new_actor_address(&mut self) -> Result<fvm_shared::address::Address> {
        self.0.new_actor_address()
    }

    fn create_actor(&mut self, code_id: cid::Cid, actor_id: ActorID) -> Result<()> {
        self.0.create_actor(code_id, actor_id)
    }
}
impl<M, C, K, D> BlockOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn block_open(&mut self, cid: &cid::Cid) -> Result<BlockId> {
        self.0.block_open(cid)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId> {
        self.0.block_create(codec, data)
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<cid::Cid> {
        self.0.block_link(id, hash_fun, hash_len)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32> {
        self.0.block_read(id, offset, buf)
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat> {
        self.0.block_stat(id)
    }
}
impl<M, C, K, D> CircSupplyOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn total_fil_circ_supply(&self) -> Result<fvm_shared::econ::TokenAmount> {
        self.0.total_fil_circ_supply()
    }
}
impl<M, C, K, D> CryptoOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn verify_signature(
        &mut self,
        signature: &fvm_shared::crypto::signature::Signature,
        signer: &fvm_shared::address::Address,
        plaintext: &[u8],
    ) -> Result<bool> {
        self.0.verify_signature(signature, signer, plaintext)
    }

    fn hash_blake2b(&mut self, data: &[u8]) -> Result<[u8; 32]> {
        self.0.hash_blake2b(data)
    }

    fn compute_unsealed_sector_cid(
        &mut self,
        proof_type: fvm_shared::sector::RegisteredSealProof,
        pieces: &[fvm_shared::piece::PieceInfo],
    ) -> Result<cid::Cid> {
        self.0.compute_unsealed_sector_cid(proof_type, pieces)
    }

    fn verify_seal(&mut self, vi: &fvm_shared::sector::SealVerifyInfo) -> Result<bool> {
        self.0.verify_seal(vi)
    }

    fn verify_post(
        &mut self,
        verify_info: &fvm_shared::sector::WindowPoStVerifyInfo,
    ) -> Result<bool> {
        self.0.verify_post(verify_info)
    }

    fn verify_consensus_fault(
        &mut self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<fvm_shared::consensus::ConsensusFault>> {
        self.0.verify_consensus_fault(h1, h2, extra)
    }

    fn batch_verify_seals(
        &mut self,
        vis: &[(
            &fvm_shared::address::Address,
            &[fvm_shared::sector::SealVerifyInfo],
        )],
    ) -> Result<std::collections::HashMap<fvm_shared::address::Address, Vec<bool>>> {
        self.0.batch_verify_seals(vis)
    }

    fn verify_aggregate_seals(
        &mut self,
        aggregate: &fvm_shared::sector::AggregateSealVerifyProofAndInfos,
    ) -> Result<bool> {
        self.0.verify_aggregate_seals(aggregate)
    }
}
impl<M, C, K, D> DebugOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn push_syscall_error(&mut self, e: SyscallError) {
        self.0.push_syscall_error(e)
    }

    fn push_actor_error(&mut self, code: fvm_shared::error::ExitCode, message: String) {
        self.0.push_actor_error(code, message)
    }

    fn clear_error(&mut self) {
        self.0.clear_error()
    }
}
impl<M, C, K, D> GasOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn charge_gas(&mut self, name: &str, compute: i64) -> Result<()> {
        self.0.charge_gas(name, compute)
    }
}
impl<M, C, K, D> MessageOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn msg_caller(&self) -> fvm_shared::ActorID {
        self.0.msg_caller()
    }

    fn msg_receiver(&self) -> fvm_shared::ActorID {
        self.0.msg_receiver()
    }

    fn msg_method_number(&self) -> fvm_shared::MethodNum {
        self.0.msg_method_number()
    }

    fn msg_value_received(&self) -> fvm_shared::econ::TokenAmount {
        self.0.msg_value_received()
    }
}
impl<M, C, K, D> NetworkOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn network_epoch(&self) -> fvm_shared::clock::ChainEpoch {
        self.0.network_epoch()
    }

    fn network_version(&self) -> fvm_shared::version::NetworkVersion {
        self.0.network_version()
    }

    fn network_base_fee(&self) -> &fvm_shared::econ::TokenAmount {
        self.0.network_base_fee()
    }
}
impl<M, C, K, D> RandomnessOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn get_randomness_from_tickets(
        &self,
        personalization: fvm_shared::crypto::randomness::DomainSeparationTag,
        rand_epoch: fvm_shared::clock::ChainEpoch,
        entropy: &[u8],
    ) -> Result<fvm_shared::randomness::Randomness> {
        self.0
            .get_randomness_from_tickets(personalization, rand_epoch, entropy)
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: fvm_shared::crypto::randomness::DomainSeparationTag,
        rand_epoch: fvm_shared::clock::ChainEpoch,
        entropy: &[u8],
    ) -> Result<fvm_shared::randomness::Randomness> {
        self.0
            .get_randomness_from_beacon(personalization, rand_epoch, entropy)
    }
}
impl<M, C, K, D> SelfOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn root(&self) -> cid::Cid {
        self.0.root()
    }

    fn set_root(&mut self, root: cid::Cid) -> Result<()> {
        self.0.set_root(root)
    }

    fn current_balance(&self) -> Result<fvm_shared::econ::TokenAmount> {
        self.0.current_balance()
    }

    fn self_destruct(&mut self, beneficiary: &fvm_shared::address::Address) -> Result<()> {
        self.0.self_destruct(beneficiary)
    }
}
impl<M, C, K, D> SendOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn send(
        &mut self,
        recipient: &fvm_shared::address::Address,
        method: u64,
        params: &fvm_shared::encoding::RawBytes,
        value: &fvm_shared::econ::TokenAmount,
    ) -> Result<crate::call_manager::InvocationResult> {
        self.0.send(recipient, method, params, value)
    }
}
impl<M, C, K, D> ValidationOps for InterceptKernel<K>
where
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
    K: Kernel<CallManager = InterceptCallManager<C>>,
{
    fn validate_immediate_caller_accept_any(&mut self) -> Result<()> {
        self.0.validate_immediate_caller_accept_any()
    }

    fn validate_immediate_caller_addr_one_of(
        &mut self,
        allowed: &[fvm_shared::address::Address],
    ) -> Result<()> {
        self.0.validate_immediate_caller_addr_one_of(allowed)
    }

    fn validate_immediate_caller_type_one_of(&mut self, allowed: &[cid::Cid]) -> Result<()> {
        self.0.validate_immediate_caller_type_one_of(allowed)
    }
}
