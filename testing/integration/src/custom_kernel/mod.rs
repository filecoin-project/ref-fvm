// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm::call_manager::CallManager;
use fvm::gas::Gas;
use fvm::kernel::filecoin::{DefaultFilecoinKernel, FilecoinKernel};
use fvm::kernel::prelude::*;
use fvm::kernel::Result;
use fvm::kernel::{
    ActorOps, CryptoOps, DebugOps, EventOps, IpldBlockOps, MessageOps, NetworkOps, RandomnessOps,
    SelfOps, SendOps, SyscallHandler, UpgradeOps,
};
use fvm::syscalls::Linker;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::RANDOMNESS_LENGTH;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
};
use fvm_shared::sys::out::network::NetworkContext;
use fvm_shared::sys::out::vm::MessageContext;
use fvm_shared::{address::Address, econ::TokenAmount, ActorID, MethodNum};

use ambassador::Delegate;
use cid::Cid;

// we define a single custom syscall which simply doubles the input
pub trait CustomKernel: Kernel {
    fn my_custom_syscall(&self, doubleme: i32) -> Result<i32>;
}

// our custom kernel extends the filecoin kernel
#[derive(Delegate)]
#[delegate(IpldBlockOps, where = "C: CallManager")]
#[delegate(ActorOps, where = "C: CallManager")]
#[delegate(CryptoOps, where = "C: CallManager")]
#[delegate(DebugOps, where = "C: CallManager")]
#[delegate(EventOps, where = "C: CallManager")]
#[delegate(MessageOps, where = "C: CallManager")]
#[delegate(NetworkOps, where = "C: CallManager")]
#[delegate(RandomnessOps, where = "C: CallManager")]
#[delegate(SelfOps, where = "C: CallManager")]
#[delegate(SendOps<K>, generics = "K", where = "K: CustomKernel")]
#[delegate(UpgradeOps<K>, generics = "K", where = "K: CustomKernel")]
pub struct DefaultCustomKernel<C>(pub DefaultFilecoinKernel<C>);

impl<C> CustomKernel for DefaultCustomKernel<C>
where
    C: CallManager,
    DefaultCustomKernel<C>: Kernel,
{
    fn my_custom_syscall(&self, doubleme: i32) -> Result<i32> {
        Ok(doubleme * 2)
    }
}

impl<C> DefaultCustomKernel<C>
where
    C: CallManager,
{
    fn price_list(&self) -> &PriceList {
        (self.0).0.call_manager.price_list()
    }
}

impl<C> Kernel for DefaultCustomKernel<C>
where
    C: CallManager,
{
    type CallManager = C;
    type Limiter = <DefaultFilecoinKernel<C> as Kernel>::Limiter;

    fn into_inner(self) -> (Self::CallManager, BlockRegistry)
    where
        Self: Sized,
    {
        self.0.into_inner()
    }

    fn new(
        mgr: C,
        blocks: BlockRegistry,
        caller: ActorID,
        actor_id: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
        read_only: bool,
    ) -> Self {
        DefaultCustomKernel(DefaultFilecoinKernel::new(
            mgr,
            blocks,
            caller,
            actor_id,
            method,
            value_received,
            read_only,
        ))
    }

    fn machine(&self) -> &<Self::CallManager as CallManager>::Machine {
        self.0.machine()
    }

    fn limiter_mut(&mut self) -> &mut Self::Limiter {
        self.0.limiter_mut()
    }

    fn gas_available(&self) -> Gas {
        self.0.gas_available()
    }

    fn charge_gas(&self, name: &str, compute: Gas) -> Result<GasTimer> {
        self.0.charge_gas(name, compute)
    }
}

impl<C> FilecoinKernel for DefaultCustomKernel<C>
where
    C: CallManager,
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
            .price_list()
            .on_verify_consensus_fault(h1.len(), h2.len(), extra.len());
        let _ = self.charge_gas(&charge.name, charge.total())?;
        Ok(None)
    }

    // NOT forwarded
    fn verify_aggregate_seals(&self, agg: &AggregateSealVerifyProofAndInfos) -> Result<bool> {
        let charge = self.price_list().on_verify_aggregate_seals(agg);
        let _ = self.charge_gas(&charge.name, charge.total())?;
        Ok(true)
    }

    // NOT forwarded
    fn verify_replica_update(&self, rep: &ReplicaUpdateInfo) -> Result<bool> {
        let charge = self.price_list().on_verify_replica_update(rep);
        let _ = self.charge_gas(&charge.name, charge.total())?;
        Ok(true)
    }

    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        self.0.total_fil_circ_supply()
    }
}

impl<K> SyscallHandler<K> for DefaultCustomKernel<K::CallManager>
where
    K: CustomKernel
        + FilecoinKernel
        + ActorOps
        + SendOps
        + UpgradeOps
        + IpldBlockOps
        + CryptoOps
        + DebugOps
        + EventOps
        + MessageOps
        + NetworkOps
        + RandomnessOps
        + SelfOps,
{
    fn link_syscalls(linker: &mut Linker<K>) -> anyhow::Result<()> {
        DefaultFilecoinKernel::<K::CallManager>::link_syscalls(linker)?;

        linker.link_syscall("my_custom_kernel", "my_custom_syscall", my_custom_syscall)?;

        Ok(())
    }
}

pub fn my_custom_syscall(
    context: fvm::syscalls::Context<'_, impl CustomKernel>,
    doubleme: i32,
) -> Result<i32> {
    context.kernel.my_custom_syscall(doubleme)
}
