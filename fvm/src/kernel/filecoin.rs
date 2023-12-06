// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::panic::{self, UnwindSafe};

use ambassador::Delegate;
use filecoin_proofs_api::{self as proofs, ProverId, PublicReplicaInfo, SectorId};

use fvm_ipld_encoding::bytes_32;
use fvm_shared::econ::TokenAmount;
use fvm_shared::piece::{zero_piece_commitment, PaddedPieceSize};
use fvm_shared::sector::{RegisteredPoStProof, SectorInfo};
use fvm_shared::{commcid, ActorID};
use lazy_static::lazy_static;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, ParallelDrainRange, ParallelIterator,
};

use super::blocks::BlockRegistry;
use super::error::Result;
use super::*;
use crate::call_manager::CallManager;
use crate::externs::Consensus;
use crate::*;

lazy_static! {
    static ref NUM_CPUS: usize = num_cpus::get();
    static ref INITIAL_RESERVE_BALANCE: TokenAmount = TokenAmount::from_whole(300_000_000);
}

pub trait FilecoinKernel: Kernel {
    /// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid>;

    /// Verifies a window proof of spacetime.
    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<bool>;

    /// Verifies that two block headers provide proof of a consensus fault:
    /// - both headers mined by the same actor
    /// - headers are different
    /// - first header is of the same or lower epoch as the second
    /// - at least one of the headers appears in the current chain at or after epoch `earliest`
    /// - the headers provide evidence of a fault (see the spec for the different fault types).
    /// The parameters are all serialized block headers. The third "extra" parameter is consulted only for
    /// the "parent grinding fault", in which case it must be the sibling of h1 (same parent tipset) and one of the
    /// blocks in the parent of h2 (i.e. h2's grandparent).
    /// Returns nil and an error if the headers don't prove a fault.
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>>;

    /// Verifies a batch of seals. This is a privledged syscall, may _only_ be called by the
    /// power actor during cron.
    ///
    /// Gas: This syscall intentionally _does not_ charge any gas (as said gas would be charged to
    /// cron). Instead, gas is pre-paid by the storage provider on pre-commit.
    fn batch_verify_seals(&self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>>;

    /// Verify aggregate seals verifies an aggregated batch of prove-commits.
    fn verify_aggregate_seals(&self, aggregate: &AggregateSealVerifyProofAndInfos) -> Result<bool>;

    /// Verify replica update verifies a snap deal: an upgrade from a CC sector to a sector with
    /// deals.
    fn verify_replica_update(&self, replica: &ReplicaUpdateInfo) -> Result<bool>;

    /// Returns the total token supply in circulation at the beginning of the current epoch.
    /// The circulating supply is the sum of:
    /// - rewards emitted by the reward actor,
    /// - funds vested from lock-ups in the genesis state,
    /// less the sum of:
    /// - funds burnt,
    /// - pledge collateral locked in storage miner actors (recorded in the storage power actor)
    /// - deal collateral locked by the storage market actor
    fn total_fil_circ_supply(&self) -> Result<TokenAmount>;
}

#[derive(Delegate)]
#[delegate(IpldBlockOps)]
#[delegate(ActorOps)]
#[delegate(CryptoOps)]
#[delegate(DebugOps)]
#[delegate(EventOps)]
#[delegate(GasOps)]
#[delegate(MessageOps)]
#[delegate(NetworkOps)]
#[delegate(RandomnessOps)]
#[delegate(SelfOps)]
#[delegate(LimiterOps)]
pub struct DefaultFilecoinKernel<K>(pub K)
where
    K: Kernel;

impl<C> FilecoinKernel for DefaultFilecoinKernel<DefaultKernel<C>>
where
    C: CallManager,
    DefaultFilecoinKernel<DefaultKernel<C>>: Kernel,
{
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        let t = self.0.call_manager.charge_gas(
            self.0
                .call_manager
                .price_list()
                .on_compute_unsealed_sector_cid(proof_type, pieces),
        )?;

        t.record(catch_and_log_panic("computing unsealed sector CID", || {
            compute_unsealed_sector_cid(proof_type, pieces)
        }))
    }

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

    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>> {
        let t = self.0.call_manager.charge_gas(
            self.0.call_manager.price_list().on_verify_consensus_fault(
                h1.len(),
                h2.len(),
                extra.len(),
            ),
        )?;

        // This syscall cannot be resolved inside the FVM, so we need to traverse
        // the node boundary through an extern.
        let (fault, _) = t.record(
            self.0
                .call_manager
                .externs()
                .verify_consensus_fault(h1, h2, extra)
                .or_illegal_argument(),
        )?;

        Ok(fault)
    }

    fn batch_verify_seals(&self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>> {
        // NOTE: gas has already been charged by the power actor when the batch verify was enqueued.
        // Lotus charges "virtual" gas here for tracing only.
        let mut items = Vec::new();
        for vi in vis {
            let t = self
                .0
                .call_manager
                .charge_gas(self.0.call_manager.price_list().on_verify_seal(vi))?;
            items.push((vi, t));
        }
        log::debug!("batch verify seals start");
        let out = items.par_drain(..)
            .with_min_len(vis.len() / *NUM_CPUS)
            .map(|(seal, timer)| {
                let start = GasTimer::start();
                let verify_seal_result = std::panic::catch_unwind(|| verify_seal(seal));
                let ok = match verify_seal_result {
                    Ok(res) => {
                        match res {
                            Ok(correct) => {
                                if !correct {
                                    log::debug!(
                                        "seal verify in batch failed (miner: {}) (err: Invalid Seal proof)",
                                        seal.sector_id.miner
                                    );
                                }
                                correct // all ok
                            }
                            Err(err) => {
                                log::debug!(
                                    "seal verify in batch failed (miner: {}) (err: {})",
                                    seal.sector_id.miner,
                                    err
                                );
                                false
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("seal verify internal fail (miner: {}) (err: {:?})", seal.sector_id.miner, e);
                        false
                    }
                };
                timer.stop_with(start);
                ok
            })
            .collect();
        log::debug!("batch verify seals end");
        Ok(out)
    }

    fn verify_aggregate_seals(&self, aggregate: &AggregateSealVerifyProofAndInfos) -> Result<bool> {
        let t = self.0.call_manager.charge_gas(
            self.0
                .call_manager
                .price_list()
                .on_verify_aggregate_seals(aggregate),
        )?;
        t.record(catch_and_log_panic("verifying aggregate seals", || {
            verify_aggregate_seals(aggregate)
        }))
    }

    fn verify_replica_update(&self, replica: &ReplicaUpdateInfo) -> Result<bool> {
        let t = self.0.call_manager.charge_gas(
            self.0
                .call_manager
                .price_list()
                .on_verify_replica_update(replica),
        )?;
        t.record(catch_and_log_panic("verifying replica update", || {
            verify_replica_update(replica)
        }))
    }

    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        // From v15 and onwards, Filecoin mainnet was fixed to use a static circ supply per epoch.
        // The value reported to the FVM from clients is now the static value,
        // the FVM simply reports that value to actors.
        Ok(self.0.machine().context().circ_supply.clone())
    }
}

impl<C> Kernel for DefaultFilecoinKernel<DefaultKernel<C>>
where
    C: CallManager,
{
    type CallManager = C;

    fn into_inner(self) -> (Self::CallManager, BlockRegistry)
    where
        Self: Sized,
    {
        self.0.into_inner()
    }

    fn machine(&self) -> &<Self::CallManager as CallManager>::Machine {
        self.0.machine()
    }

    fn send<K: Kernel<CallManager = C>>(
        &mut self,
        recipient: &Address,
        method: u64,
        params: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<CallResult> {
        self.0
            .send::<Self>(recipient, method, params, value, gas_limit, flags)
    }

    fn upgrade_actor<K: Kernel<CallManager = Self::CallManager>>(
        &mut self,
        new_code_cid: Cid,
        params_id: BlockId,
    ) -> Result<CallResult> {
        self.0.upgrade_actor::<Self>(new_code_cid, params_id)
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

fn get_required_padding(
    old_length: PaddedPieceSize,
    new_piece_length: PaddedPieceSize,
) -> (Vec<PaddedPieceSize>, PaddedPieceSize) {
    let mut sum = 0;

    let mut to_fill = 0u64.wrapping_sub(old_length.0) % new_piece_length.0;
    let n = to_fill.count_ones();
    let mut pad_pieces = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let next = to_fill.trailing_zeros();
        let p_size = 1 << next;
        to_fill ^= p_size;

        let padded = PaddedPieceSize(p_size);
        pad_pieces.push(padded);
        sum += padded.0;
    }

    (pad_pieces, PaddedPieceSize(sum))
}

fn verify_seal(vi: &SealVerifyInfo) -> Result<bool> {
    let commr = commcid::cid_to_replica_commitment_v1(&vi.sealed_cid).or_illegal_argument()?;
    let commd = commcid::cid_to_data_commitment_v1(&vi.unsealed_cid).or_illegal_argument()?;
    let prover_id = prover_id_from_u64(vi.sector_id.miner);

    proofs::seal::verify_seal(
        vi.registered_proof
            .try_into()
            .or_illegal_argument()
            .context(format_args!("invalid proof type {:?}", vi.registered_proof))?,
        commr,
        commd,
        prover_id,
        SectorId::from(vi.sector_id.number),
        bytes_32(&vi.randomness.0),
        bytes_32(&vi.interactive_randomness.0),
        &vi.proof,
    )
    .or_illegal_argument()
    // There are probably errors here that should be fatal, but it's hard to tell so I'm sticking
    // with illegal argument for now.
    //
    // Worst case, _some_ node falls out of sync. Better than the network halting.
    .context("failed to verify seal proof")
}

fn verify_aggregate_seals(aggregate: &AggregateSealVerifyProofAndInfos) -> Result<bool> {
    if aggregate.infos.is_empty() {
        return Err(syscall_error!(IllegalArgument; "no seal verify infos").into());
    }
    let spt: proofs::RegisteredSealProof = aggregate.seal_proof.try_into().or_illegal_argument()?;
    let prover_id = prover_id_from_u64(aggregate.miner);
    struct AggregationInputs {
        // replica
        commr: [u8; 32],
        // data
        commd: [u8; 32],
        sector_id: SectorId,
        ticket: [u8; 32],
        seed: [u8; 32],
    }
    let inputs: Vec<AggregationInputs> = aggregate
        .infos
        .iter()
        .map(|info| {
            let commr = commcid::cid_to_replica_commitment_v1(&info.sealed_cid)?;
            let commd = commcid::cid_to_data_commitment_v1(&info.unsealed_cid)?;
            Ok(AggregationInputs {
                commr,
                commd,
                ticket: bytes_32(&info.randomness.0),
                seed: bytes_32(&info.interactive_randomness.0),
                sector_id: SectorId::from(info.sector_number),
            })
        })
        .collect::<core::result::Result<Vec<_>, &'static str>>()
        .or_illegal_argument()?;

    let inp: Vec<Vec<_>> = inputs
        .par_iter()
        .map(|input| {
            proofs::seal::get_seal_inputs(
                spt,
                input.commr,
                input.commd,
                prover_id,
                input.sector_id,
                input.ticket,
                input.seed,
            )
        })
        .try_reduce(Vec::new, |mut acc, current| {
            acc.extend(current);
            Ok(acc)
        })
        .or_illegal_argument()?;

    let commrs: Vec<[u8; 32]> = inputs.iter().map(|input| input.commr).collect();
    let seeds: Vec<[u8; 32]> = inputs.iter().map(|input| input.seed).collect();

    proofs::seal::verify_aggregate_seal_commit_proofs(
        spt,
        aggregate.aggregate_proof.try_into().or_illegal_argument()?,
        aggregate.proof.clone(),
        &commrs,
        &seeds,
        inp,
    )
    .or_illegal_argument()
}

fn verify_replica_update(replica: &ReplicaUpdateInfo) -> Result<bool> {
    let up: proofs::RegisteredUpdateProof =
        replica.update_proof_type.try_into().or_illegal_argument()?;

    let commr_old =
        commcid::cid_to_replica_commitment_v1(&replica.old_sealed_cid).or_illegal_argument()?;
    let commr_new =
        commcid::cid_to_replica_commitment_v1(&replica.new_sealed_cid).or_illegal_argument()?;
    let commd =
        commcid::cid_to_data_commitment_v1(&replica.new_unsealed_cid).or_illegal_argument()?;

    proofs::update::verify_empty_sector_update_proof(
        up,
        &replica.proof,
        commr_old,
        commr_new,
        commd,
    )
    .or_illegal_argument()
}

fn compute_unsealed_sector_cid(
    proof_type: RegisteredSealProof,
    pieces: &[PieceInfo],
) -> Result<Cid> {
    let ssize = proof_type.sector_size().or_illegal_argument()? as u64;

    let mut all_pieces = Vec::<proofs::PieceInfo>::with_capacity(pieces.len());

    let pssize = PaddedPieceSize(ssize);
    if pieces.is_empty() {
        all_pieces.push(proofs::PieceInfo {
            size: pssize.unpadded().into(),
            commitment: zero_piece_commitment(pssize),
        })
    } else {
        // pad remaining space with 0 piece commitments
        let mut sum = PaddedPieceSize(0);
        let pad_to = |pads: Vec<PaddedPieceSize>,
                      all_pieces: &mut Vec<proofs::PieceInfo>,
                      sum: &mut PaddedPieceSize| {
            for p in pads {
                all_pieces.push(proofs::PieceInfo {
                    size: p.unpadded().into(),
                    commitment: zero_piece_commitment(p),
                });

                sum.0 += p.0;
            }
        };
        for p in pieces {
            let (ps, _) = get_required_padding(sum, p.size);
            pad_to(ps, &mut all_pieces, &mut sum);

            all_pieces.push(proofs::PieceInfo::try_from(p).or_illegal_argument()?);
            sum.0 += p.size.0;
        }

        let (ps, _) = get_required_padding(sum, pssize);
        pad_to(ps, &mut all_pieces, &mut sum);
    }

    let comm_d =
        proofs::seal::compute_comm_d(proof_type.try_into().or_illegal_argument()?, &all_pieces)
            .or_illegal_argument()?;

    commcid::data_commitment_v1_to_cid(&comm_d).or_illegal_argument()
}
