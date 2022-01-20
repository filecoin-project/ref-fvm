use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};

use anyhow::{anyhow, Context as _};
use byteorder::{BigEndian, WriteBytesExt};
use cid::Cid;
use filecoin_proofs_api::seal::{
    compute_comm_d, verify_aggregate_seal_commit_proofs, verify_seal as proofs_verify_seal,
};
use filecoin_proofs_api::{self as proofs, post, seal, ProverId, PublicReplicaInfo, SectorId};
use fvm_shared::address::Protocol;
use fvm_shared::blockstore::{Blockstore, CborStore};
use fvm_shared::commcid::{
    cid_to_data_commitment_v1, cid_to_replica_commitment_v1, data_commitment_v1_to_cid,
};
use fvm_shared::encoding::{blake2b_256, bytes_32, to_vec, RawBytes};
use fvm_shared::error::ExitCode;
use fvm_shared::piece::{zero_piece_commitment, PaddedPieceSize};
use fvm_shared::sector::SectorInfo;
use fvm_shared::sys::TokenAmount;
use fvm_shared::{ActorID, FILECOIN_PRECISION};
use lazy_static::lazy_static;
use num_traits::Zero;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::blocks::{Block, BlockRegistry};
use super::error::Result;
use super::*;
use crate::account_actor::is_account_actor;
use crate::builtin::{is_builtin_actor, is_singleton_actor, EMPTY_ARR_CID};
use crate::call_manager::{CallManager, InvocationResult};
use crate::externs::{Consensus, Rand};
use crate::gas::GasCharge;
use crate::machine::CallError;
use crate::market_actor::State as MarketActorState;
use crate::power_actor::State as PowerActorState;
use crate::state_tree::ActorState;
use crate::syscall_error;

pub const BURN_ACTOR_ID: ActorID = 99;
pub const RESERVE_ACTOR_ID: ActorID = 90;

lazy_static! {
    static ref NUM_CPUS: usize = num_cpus::get();
}

/// Tracks data accessed and modified during the execution of a message.
///
/// TODO writes probably ought to be scoped by invocation container.
pub struct DefaultKernel<C> {
    // Fields extracted from the message, except parameters, which have been
    // preloaded into the block registry.
    caller: ActorID,
    actor_id: ActorID,
    method: MethodNum,
    value_received: TokenAmount,

    /// The call manager for this call stack. If this kernel calls another actor, it will
    /// temporarily "give" the call manager to the other kernel before re-attaching it.
    call_manager: C,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    blocks: BlockRegistry,
}

// Even though all children traits are implemented, Rust needs to know that the
// supertrait is implemented too.
impl<C> Kernel for DefaultKernel<C>
where
    C: CallManager,
{
    type CallManager = C;

    fn take(self) -> Self::CallManager
    where
        Self: Sized,
    {
        self.call_manager
    }

    fn new(
        mgr: C,
        from: ActorID,
        to: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
    ) -> Self {
        DefaultKernel {
            call_manager: mgr,
            blocks: BlockRegistry::new(),
            caller: from,
            actor_id: to,
            method,
            value_received,
        }
    }
}

impl<C> DefaultKernel<C>
where
    C: CallManager,
{
    pub fn resolve_to_key_addr(&self, addr: &Address) -> Result<Address> {
        if addr.protocol() == Protocol::BLS || addr.protocol() == Protocol::Secp256k1 {
            return Ok(*addr);
        }

        let state_tree = self.call_manager.state_tree();
        let act = state_tree
            .get_actor(addr)?
            .ok_or(anyhow!("state tree doesn't contain actor"))
            .or_illegal_argument()?;

        if !is_account_actor(&act.code) {
            return Err(
                syscall_error!(SysErrIllegalArgument; "target actor is not an account").into(),
            );
        }

        let state: crate::account_actor::State = state_tree
            .store()
            .get_cbor(&act.state)
            .context("failed to decode actor state as an account")
            .or_fatal()? // because we've checked and this should be an account.
            .ok_or(anyhow!("account actor state not found"))
            .or_fatal()?; // because the state should exist.

        Ok(state.address)
    }

    fn get_burnt_funds(&self) -> Result<TokenAmount> {
        Ok(self
            .call_manager
            .state_tree()
            .get_actor_id(BURN_ACTOR_ID)?
            .ok_or_else(|| anyhow!("burn actor state couldn't be loaded"))
            .or_fatal()?
            .balance)
    }

    fn get_reserve_disbursed(&self) -> Result<TokenAmount> {
        let initial_reserve_balance = TokenAmount::from(330_000_000 as u64) * FILECOIN_PRECISION;
        initial_reserve_balance
            .checked_sub(
                &self
                    .call_manager
                    .state_tree()
                    .get_actor_id(RESERVE_ACTOR_ID)?
                    .ok_or_else(|| anyhow!("reserve actor state couldn't be loaded"))
                    .or_fatal()?
                    .balance,
            )
            .ok_or_else(|| anyhow!("failed to subtract"))
            .or_fatal()
    }

    fn power_locked(&self) -> Result<TokenAmount> {
        let (power_state, _) = PowerActorState::load(self.call_manager.state_tree())?;
        Ok(power_state.total_locked().try_into().unwrap())
    }

    fn market_locked(&self) -> Result<TokenAmount> {
        let (market_state, _) = MarketActorState::load(self.call_manager.state_tree())?;
        Ok(market_state.total_locked().try_into().unwrap())
    }

    /// Returns `Some(actor_state)` or `None` if this actor has been deleted.
    fn get_self(&self) -> Result<Option<ActorState>> {
        self.call_manager
            .state_tree()
            .get_actor_id(self.actor_id)
            .or_fatal()
            .context("error when finding current actor")
    }

    /// Mutates this actor's state, returning a syscall error if this actor has been deleted.
    fn mutate_self<F>(&mut self, mutate: F) -> Result<()>
    where
        F: FnOnce(&mut ActorState) -> Result<()>,
    {
        self.call_manager
            .state_tree_mut()
            .maybe_mutate_actor_id(self.actor_id, mutate)
            .context("failed to mutate self")
            .and_then(|found| {
                if found {
                    Ok(())
                } else {
                    Err(syscall_error!(SysErrIllegalActor; "actor deleted").into())
                }
            })
    }
}

impl<C> SelfOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn root(&self) -> Result<Cid> {
        // This can fail during normal operations if the actor has been deleted.
        Ok(self
            .get_self()?
            .context("state root requested after actor deletion")
            .or_error(ExitCode::SysErrIllegalActor)?
            .state)
    }

    fn set_root(&mut self, new: Cid) -> Result<()> {
        self.mutate_self(|actor_state| {
            actor_state.state = new;
            Ok(())
        })
    }

    fn current_balance(&self) -> Result<TokenAmount> {
        // If the actor doesn't exist, it has zero balance.
        Ok(self
            .get_self()?
            .map(|a| a.balance)
            .unwrap_or_else(|| TokenAmount::zero()))
    }

    fn self_destruct(&mut self, beneficiary: &Address) -> Result<()> {
        // Idempotentcy: If the actor doesn't exist, this won't actually do anything. The current
        // balance will be zero, and `delete_actor_id` will be a no-op.
        self.call_manager
            .charge_gas(self.call_manager.price_list().on_delete_actor())?;

        let balance = self.current_balance()?;
        if balance != TokenAmount::zero() {
            // Starting from network version v7, the runtime checks if the beneficiary
            // exists; if missing, it fails the self destruct.
            //
            // In FVM we check unconditionally, since we only support nv13+.
            let beneficiary_id = self
                .resolve_address(beneficiary)?
                .context("beneficiary doesn't exist")
                .or_error(ExitCode::SysErrIllegalArgument)?;

            if beneficiary_id == self.actor_id {
                return Err(syscall_error!(
                    SysErrIllegalArgument,
                    "benefactor cannot be beneficiary"
                )
                .into());
            }

            // Transfer the entirety of funds to beneficiary.
            self.call_manager
                .machine_mut()
                .transfer(self.actor_id, beneficiary_id, balance)?;
        }

        // Delete the executing actor
        self.call_manager
            .state_tree_mut()
            .delete_actor_id(self.actor_id)
    }
}

impl<C> BlockOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn block_open(&mut self, cid: &Cid) -> Result<(BlockId, BlockStat)> {
        self.call_manager
            .charge_gas(self.call_manager.price_list().on_ipld_get())?;

        let data = self
            .call_manager
            .blockstore()
            .get(cid)
            .or_fatal()?
            .ok_or_else(|| anyhow!("missing state: {}", cid))
            // Missing state is a fatal error because it means we have a bug. Once we do
            // reachability checking (for user actors) we won't get here unless the block is known
            // to be in the state-tree.
            .or_fatal()?;

        // We charge on open, not read, to emulate the current gas model.
        let block = Block::new(cid.codec(), data);
        let stat = block.stat();

        // TODO: I mean, this means you put 4M blocks in a single message. That's not actually possible?
        let id = self.blocks.put(block).or_illegal_argument()?;
        Ok((id, stat))
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId> {
        self.blocks
            .put(Block::new(codec, data))
            .or_illegal_argument()
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid> {
        // TODO: check hash function & length against allow list.

        use multihash::MultihashDigest;
        let block = self.blocks.get(id).or_illegal_argument()?;
        let code = multihash::Code::try_from(hash_fun)
            .or_illegal_argument()
            .context(format_args!("invalid hash code: {}", hash_fun))?;

        // We charge on link, not create, to emulate the current gas model.
        self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_ipld_put(block.size().try_into().or_illegal_argument()?),
        )?;

        let hash = code.digest(block.data());
        if u32::from(hash.size()) < hash_len {
            return Err(
                syscall_error!(SysErrIllegalArgument; "invalid hash length: {}", hash_len).into(),
            );
        }
        let k = Cid::new_v1(block.codec, hash.truncate(hash_len as u8));
        // TODO: for now, we _put_ the block here. In the future, we should put it into a write
        // cache, then flush it later.
        self.call_manager
            .blockstore()
            .put_keyed(&k, block.data())
            .or_fatal()?;
        Ok(k)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32> {
        let data = &self.blocks.get(id).or_illegal_argument()?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat> {
        self.blocks.stat(id).or_illegal_argument()
    }
}

impl<C> MessageOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn msg_caller(&self) -> ActorID {
        self.caller
    }

    fn msg_receiver(&self) -> ActorID {
        self.actor_id
    }

    fn msg_method_number(&self) -> MethodNum {
        self.method
    }

    fn msg_value_received(&self) -> TokenAmount {
        self.value_received.clone()
    }
}

impl<C> SendOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn send(
        &mut self,
        recipient: &Address,
        method: MethodNum,
        params: &RawBytes,
        value: TokenAmount,
    ) -> Result<InvocationResult> {
        let from = self.actor_id;
        self.call_manager
            .with_transaction(|cm| cm.send::<Self>(from, *recipient, method, params, value))
    }
}

impl<C> CircSupplyOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        self.call_manager
            .context()
            .base_circ_supply
            .checked_add(&self.get_reserve_disbursed()?)
            .ok_or(anyhow!(
                "overflow when adding reserve to base circulating supply"
            ))
            .or_fatal()?
            .checked_sub(&self.get_burnt_funds()?)
            .ok_or(anyhow!("underflow when subtracting burnt funds"))
            .or_fatal()?
            .checked_sub(&self.power_locked()?)
            .ok_or(anyhow!("underflow when subtracting power locked funds"))
            .or_fatal()?
            .checked_sub(&self.market_locked()?)
            .ok_or(anyhow!("underflow when subtracting market locked funds"))
            .or_fatal()
    }
}

impl<C> CryptoOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn verify_signature(
        &mut self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool> {
        self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_verify_signature(signature.signature_type()),
        )?;

        // Resolve to key address before verifying signature.
        let signing_addr = self.resolve_to_key_addr(signer)?;
        Ok(signature.verify(plaintext, &signing_addr).is_ok())
    }

    fn hash_blake2b(&mut self, data: &[u8]) -> Result<[u8; 32]> {
        self.call_manager
            .charge_gas(self.call_manager.price_list().on_hashing(data.len()))?;

        Ok(blake2b_256(data))
    }

    fn compute_unsealed_sector_cid(
        &mut self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_compute_unsealed_sector_cid(proof_type, pieces),
        )?;

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

        let comm_d = compute_comm_d(proof_type.try_into().or_illegal_argument()?, &all_pieces)
            .or_illegal_argument()?;

        data_commitment_v1_to_cid(&comm_d).or_illegal_argument()
    }

    /// Verify seal proof for sectors. This proof verifies that a sector was sealed by the miner.
    fn verify_seal(&mut self, vi: &SealVerifyInfo) -> Result<bool> {
        self.call_manager
            .charge_gas(self.call_manager.price_list().on_verify_seal(vi))?;
        verify_seal(vi)
    }

    fn verify_post(&mut self, verify_info: &WindowPoStVerifyInfo) -> Result<bool> {
        self.call_manager
            .charge_gas(self.call_manager.price_list().on_verify_post(verify_info))?;

        let WindowPoStVerifyInfo {
            ref proofs,
            ref challenged_sectors,
            prover,
            ..
        } = verify_info;

        let Randomness(mut randomness) = verify_info.randomness.clone();

        // Necessary to be valid bls12 381 element.
        randomness[31] &= 0x3f;

        // Convert sector info into public replica
        let replicas = to_fil_public_replica_infos(challenged_sectors, ProofType::Window)?;

        // Convert PoSt proofs into proofs-api format
        let proofs: Vec<(proofs::RegisteredPoStProof, _)> = proofs
            .iter()
            .map(|p| Ok((p.post_proof.try_into()?, p.proof_bytes.as_ref())))
            .collect::<core::result::Result<_, String>>()
            .or_illegal_argument()?;

        // Generate prover bytes from ID
        let prover_id = prover_id_from_u64(*prover);

        // Verify Proof
        post::verify_window_post(&bytes_32(&randomness), &proofs, &replicas, prover_id)
            .or_illegal_argument()
    }

    fn verify_consensus_fault(
        &mut self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>> {
        self.call_manager
            .charge_gas(self.call_manager.price_list().on_verify_consensus_fault())?;

        // This syscall cannot be resolved inside the FVM, so we need to traverse
        // the node boundary through an extern.
        self.call_manager
            .externs()
            .verify_consensus_fault(h1, h2, extra)
            .or_illegal_argument()
            .context("fault not verified")
    }

    fn batch_verify_seals(&mut self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>> {
        // NOTE: gas has already been charged by the power actor when the batch verify was enqueued.
        // Lotus charges "virtual" gas here for tracing only.
        log::debug!("batch verify seals start");
        let out = vis
            .par_iter()
            .with_min_len(vis.len() / *NUM_CPUS)
            .map(|seal| {
                let verify_seal_result = std::panic::catch_unwind(|| verify_seal(seal));
                match verify_seal_result {
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
                }
            })
            .collect();
        log::debug!("batch verify seals end");
        Ok(out)
    }

    fn verify_aggregate_seals(
        &mut self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<bool> {
        self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_verify_aggregate_seals(aggregate),
        )?;
        if aggregate.infos.is_empty() {
            return Err(syscall_error!(SysErrIllegalArgument; "no seal verify infos").into());
        }
        let spt: proofs::RegisteredSealProof =
            aggregate.seal_proof.try_into().or_illegal_argument()?;
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
                let commr = cid_to_replica_commitment_v1(&info.sealed_cid)?;
                let commd = cid_to_data_commitment_v1(&info.unsealed_cid)?;
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
                seal::get_seal_inputs(
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

        verify_aggregate_seal_commit_proofs(
            spt,
            aggregate.aggregate_proof.try_into().or_illegal_argument()?,
            aggregate.proof.clone(),
            &commrs,
            &seeds,
            inp,
        )
        .or_illegal_argument()
    }
}

impl<C> GasOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn charge_gas(&mut self, name: &str, compute: i64) -> Result<()> {
        let charge = GasCharge::new(name, compute, 0);
        self.call_manager.charge_gas(charge)
    }
}

impl<C> NetworkOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn network_epoch(&self) -> ChainEpoch {
        self.call_manager.context().epoch
    }

    fn network_version(&self) -> NetworkVersion {
        self.call_manager.context().network_version
    }

    fn network_base_fee(&self) -> TokenAmount {
        self.call_manager.context().base_fee
    }
}

impl<C> RandomnessOps for DefaultKernel<C>
where
    C: CallManager,
{
    #[allow(unused)]
    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        // TODO: Check error code
        self.call_manager
            .externs()
            .get_chain_randomness_looking_forward(personalization, rand_epoch, entropy)
            .or_illegal_argument()
    }

    #[allow(unused)]
    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        // TODO: Check error code
        // Hyperdrive and above only.
        self.call_manager
            .externs()
            .get_beacon_randomness_looking_forward(personalization, rand_epoch, entropy)
            .or_illegal_argument()
    }
}

impl<C> ActorOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn resolve_address(&self, address: &Address) -> Result<Option<ActorID>> {
        self.call_manager.state_tree().lookup_id(address)
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Result<Option<Cid>> {
        Ok(self
            .call_manager
            .state_tree()
            .get_actor(addr)
            .context("failed to lookup actor to get code CID")
            .or_fatal()?
            .map(|act| act.code))
    }

    fn new_actor_address(&mut self) -> Result<Address> {
        let oa = self
            .resolve_to_key_addr(&self.call_manager.origin())
            // This is already an execution error, but we're _making_ it fatal.
            .or_fatal()?;

        let mut b = to_vec(&oa)
            .or_fatal()
            .context("could not serialize address in new_actor_address")?;
        b.write_u64::<BigEndian>(self.call_manager.nonce())
            .or_fatal()
            .context("writing nonce into a buffer")?;
        b.write_u64::<BigEndian>(self.call_manager.next_actor_idx())
            .or_fatal()
            .context("writing actor index in buffer")?;
        let addr = Address::new_actor(&b);
        Ok(addr)
    }

    // TODO merge new_actor_address and create_actor into a single syscall.
    fn create_actor(&mut self, code_id: Cid, actor_id: ActorID) -> Result<()> {
        if !is_builtin_actor(&code_id) {
            return Err(
                syscall_error!(SysErrIllegalArgument; "Can only create built-in actors").into(),
            );
        }
        if is_singleton_actor(&code_id) {
            return Err(
                syscall_error!(SysErrIllegalArgument; "can only have one instance of singleton actors").into(),
            );
        }

        let state_tree = self.call_manager.state_tree();
        if let Ok(Some(_)) = state_tree.get_actor_id(actor_id) {
            return Err(
                syscall_error!(SysErrIllegalArgument; "Actor address already exists").into(),
            );
        }

        self.call_manager
            .charge_gas(self.call_manager.price_list().on_create_actor())?;

        let state_tree = self.call_manager.state_tree_mut();
        state_tree.set_actor_id(
            actor_id,
            ActorState::new(code_id, *EMPTY_ARR_CID, 0u32.into(), 0),
        )
    }
}

impl<C> DebugOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn log(&self, msg: String) {
        println!("{}", msg)
    }

    fn debug_enabled(&self) -> bool {
        self.call_manager.context().debug
    }

    fn push_syscall_error(&mut self, err: SyscallError) {
        self.call_manager.push_error(CallError {
            source: 0,
            code: err.1,
            message: err.0,
        })
    }
    fn push_actor_error(&mut self, code: ExitCode, message: String) {
        self.call_manager.push_error(CallError {
            source: self.actor_id,
            code,
            message,
        })
    }
    fn clear_error(&mut self) {
        self.call_manager.clear_error();
    }
}

/// PoSt proof variants.
enum ProofType {
    #[allow(unused)]
    Winning,
    Window,
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

fn to_fil_public_replica_infos(
    src: &[SectorInfo],
    typ: ProofType,
) -> Result<BTreeMap<SectorId, PublicReplicaInfo>> {
    let replicas = src
        .iter()
        .map::<core::result::Result<(SectorId, PublicReplicaInfo), String>, _>(
            |sector_info: &SectorInfo| {
                let commr = cid_to_replica_commitment_v1(&sector_info.sealed_cid)?;
                let proof = match typ {
                    ProofType::Winning => sector_info.proof.registered_winning_post_proof()?,
                    ProofType::Window => sector_info.proof.registered_window_post_proof()?,
                };
                let replica = PublicReplicaInfo::new(proof.try_into()?, commr);
                Ok((SectorId::from(sector_info.sector_number), replica))
            },
        )
        .collect::<core::result::Result<BTreeMap<SectorId, PublicReplicaInfo>, _>>()
        .or_illegal_argument()?;
    Ok(replicas)
}

fn verify_seal(vi: &SealVerifyInfo) -> Result<bool> {
    let commr = cid_to_replica_commitment_v1(&vi.sealed_cid).or_illegal_argument()?;
    let commd = cid_to_data_commitment_v1(&vi.unsealed_cid).or_illegal_argument()?;
    let prover_id = prover_id_from_u64(vi.sector_id.miner);

    proofs_verify_seal(
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
    // TODO: There are probably errors here that should be fatal, but it's hard to tell so I'm
    // sticking with illegal argument for now.
    // Worst case, _some_ node falls out of sync. Better than the network halting.
    .context("failed to verify seal proof")
}
