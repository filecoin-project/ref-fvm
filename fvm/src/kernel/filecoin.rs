// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::panic::{self, UnwindSafe};

use ambassador::Delegate;
use filecoin_proofs_api::{self as proofs, ProverId, PublicReplicaInfo, SectorId};

use fvm_ipld_encoding::bytes_32;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sector::{RegisteredPoStProof, SectorInfo};
use fvm_shared::{commcid, ActorID};

use super::blocks::BlockRegistry;
use super::error::Result;
use super::*;
use crate::call_manager::CallManager;
//use crate::{syscall_error, DefaultKernel, ambassador_impl_ActorOps};
use crate::*;

pub trait FilecoinKernel: Kernel {
    /// Verifies a window proof of spacetime.
    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<bool>;
}

#[derive(Delegate)]
#[delegate(IpldBlockOps)]
#[delegate(ActorOps)]
#[delegate(CircSupplyOps)]
#[delegate(CryptoOps)]
#[delegate(DebugOps)]
#[delegate(EventOps)]
#[delegate(GasOps)]
#[delegate(MessageOps)]
#[delegate(NetworkOps)]
#[delegate(RandomnessOps)]
#[delegate(SelfOps)]
#[delegate(LimiterOps)]
pub struct DefaultFilecoinKernel<KK>(pub KK)
where
    KK: Kernel;

impl<C> FilecoinKernel for DefaultFilecoinKernel<DefaultKernel<C>>
where
    C: CallManager,
    DefaultFilecoinKernel<DefaultKernel<C>>: Kernel,
{
    /// Verifies a window proof of spacetime.
    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<bool> {
        let t = self
            .0
            .call_manager
            .charge_gas(self.0.call_manager.price_list().on_verify_post(verify_info))?;

        // This is especially important to catch as, otherwise, a bad "post" could be undisputable.
        t.record(catch_and_log_panic("verifying post", || {
            verify_post(verify_info)
        }))
    }
}

impl<C> ConstructKernel<C> for DefaultFilecoinKernel<DefaultKernel<C>>
where
    C: CallManager,
{
    fn new(
        mgr: C,
        blocks: BlockRegistry,
        caller: ActorID,
        actor_id: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
        read_only: bool,
    ) -> Self {
        DefaultFilecoinKernel(DefaultKernel::new(
            mgr,
            blocks,
            caller,
            actor_id,
            method,
            value_received,
            read_only,
        ))
    }
}

fn catch_and_log_panic<F: FnOnce() -> Result<R> + UnwindSafe, R>(context: &str, f: F) -> Result<R> {
    match panic::catch_unwind(f) {
        Ok(v) => v,
        Err(e) => {
            log::error!("caught panic when {}: {:?}", context, e);
            Err(syscall_error!(IllegalArgument; "caught panic when {}: {:?}", context, e).into())
        }
    }
}

fn verify_post(verify_info: &WindowPoStVerifyInfo) -> Result<bool> {
    let WindowPoStVerifyInfo {
        ref proofs,
        ref challenged_sectors,
        prover,
        ..
    } = verify_info;

    let Randomness(mut randomness) = verify_info.randomness.clone();

    // Necessary to be valid bls12 381 element.
    randomness[31] &= 0x3f;

    let proof_type = proofs[0].post_proof;

    for proof in proofs {
        if proof.post_proof != proof_type {
            return Err(
                syscall_error!(IllegalArgument; "all proof types must be the same (found both {:?} and {:?})", proof_type, proof.post_proof)
                    .into(),
            );
        }
    }
    // Convert sector info into public replica
    let replicas = to_fil_public_replica_infos(challenged_sectors, proof_type)?;

    // Convert PoSt proofs into proofs-api format
    let proofs: Vec<(proofs::RegisteredPoStProof, _)> = proofs
        .iter()
        .map(|p| Ok((p.post_proof.try_into()?, p.proof_bytes.as_ref())))
        .collect::<core::result::Result<_, String>>()
        .or_illegal_argument()?;

    // Generate prover bytes from ID
    let prover_id = prover_id_from_u64(*prover);

    // Verify Proof
    proofs::post::verify_window_post(&bytes_32(&randomness), &proofs, &replicas, prover_id)
        .or_illegal_argument()
}

fn to_fil_public_replica_infos(
    src: &[SectorInfo],
    typ: RegisteredPoStProof,
) -> Result<BTreeMap<SectorId, PublicReplicaInfo>> {
    let replicas = src
        .iter()
        .map::<core::result::Result<(SectorId, PublicReplicaInfo), String>, _>(
            |sector_info: &SectorInfo| {
                let commr = commcid::cid_to_replica_commitment_v1(&sector_info.sealed_cid)?;
                if !check_valid_proof_type(typ, sector_info.proof) {
                    return Err("invalid proof type".to_string());
                }
                let replica = PublicReplicaInfo::new(typ.try_into()?, commr);
                Ok((SectorId::from(sector_info.sector_number), replica))
            },
        )
        .collect::<core::result::Result<BTreeMap<SectorId, PublicReplicaInfo>, _>>()
        .or_illegal_argument()?;
    Ok(replicas)
}

fn check_valid_proof_type(post_type: RegisteredPoStProof, seal_type: RegisteredSealProof) -> bool {
    if let Ok(proof_type_v1p1) = seal_type.registered_window_post_proof() {
        proof_type_v1p1 == post_type
    } else {
        false
    }
}

fn prover_id_from_u64(id: u64) -> ProverId {
    let mut prover_id = ProverId::default();
    let prover_bytes = Address::new_id(id).payload().to_raw_bytes();
    prover_id[..prover_bytes.len()].copy_from_slice(&prover_bytes);
    prover_id
}
