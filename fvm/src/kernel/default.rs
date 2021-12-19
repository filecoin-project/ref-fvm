use anyhow::anyhow;
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};

use cid::Cid;

use blockstore::Blockstore;
use byteorder::{BigEndian, WriteBytesExt};
use fvm_shared::bigint::Zero;
use fvm_shared::commcid::{
    cid_to_data_commitment_v1, cid_to_replica_commitment_v1, data_commitment_v1_to_cid,
};
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{blake2b_256, bytes_32, to_vec, CborStore};
use fvm_shared::error::ExitCode;
use fvm_shared::ActorID;

use crate::builtin::{is_builtin_actor, is_singleton_actor, EMPTY_ARR_CID};
use crate::call_manager::CallManager;
use crate::externs::Externs;
use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::ActorState;
use crate::syscall_error;

use filecoin_proofs_api::seal::compute_comm_d;
use filecoin_proofs_api::{self as proofs, seal, ProverId, SectorId};
use filecoin_proofs_api::{
    post, seal::verify_aggregate_seal_commit_proofs, seal::verify_seal as proofs_verify_seal,
    PublicReplicaInfo,
};
use fvm_shared::address::Protocol;
use fvm_shared::piece::{zero_piece_commitment, PaddedPieceSize};
use lazy_static::lazy_static;

use super::blocks::{Block, BlockRegistry};
use super::error::Result;
use super::*;

use crate::gas::GasCharge;
use fvm_shared::sector::SectorInfo;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

lazy_static! {
    static ref NUM_CPUS: usize = num_cpus::get();
}

/// Tracks data accessed and modified during the execution of a message.
///
/// TODO writes probably ought to be scoped by invocation container.
pub struct DefaultKernel<B: 'static, E: 'static> {
    // Fields extracted from the message, except parameters, which have been
    // preloaded into the block registry.
    from: ActorID,
    to: ActorID,
    method: MethodNum,
    value_received: TokenAmount,

    /// The call manager for this call stack. If this kernel calls another actor, it will
    /// temporarily "give" the call manager to the other kernel before re-attaching it.
    call_manager: CallManager<B, E>,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    blocks: BlockRegistry,
    caller_validated: bool,
}

// Even though all children traits are implemented, Rust needs to know that the
// supertrait is implemented too.
impl<B, E> Kernel for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs + 'static,
{
}

impl<B, E> DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs + 'static,
{
    /// Starts an unattached kernel.
    // TODO: combine the gas tracker and the machine into some form of "call stack context"?
    pub fn new(
        mgr: CallManager<B, E>,
        from: ActorID,
        to: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
    ) -> Self {
        DefaultKernel {
            call_manager: mgr,
            blocks: BlockRegistry::new(),
            from,
            to,
            method,
            value_received,
            caller_validated: false,
        }
    }

    pub fn take(self) -> CallManager<B, E> {
        self.call_manager
    }

    fn assert_not_validated(&mut self) -> Result<()> {
        if self.caller_validated {
            return Err(syscall_error!(
                SysErrIllegalActor,
                "Method must validate caller identity exactly once"
            )
            .into());
        }
        self.caller_validated = true;
        Ok(())
    }

    pub fn resolve_to_key_addr(&self, addr: &Address) -> Result<Address> {
        if addr.protocol() == Protocol::BLS || addr.protocol() == Protocol::Secp256k1 {
            return Ok(*addr);
        }

        let state_tree = self.call_manager.state_tree();
        let act = state_tree
            .get_actor(addr)?
            .ok_or(anyhow!("state tree doesn't contain actor"))
            .or_error(ExitCode::ErrIllegalArgument)?;

        let state: crate::account_actor::State = state_tree
            .store()
            .get_cbor(&act.state)
            .or_fatal()?
            .ok_or(anyhow!("account actor state not found"))
            .or_fatal()?;

        Ok(state.address)
    }
}

impl<B, E> SelfOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: 'static + Externs,
{
    fn root(&self) -> Cid {
        let addr = Address::new_id(self.to);
        let state_tree = self.call_manager.state_tree();

        state_tree
            .get_actor(&addr)
            .unwrap()
            .expect("expected actor to exist")
            .state
    }

    fn set_root(&mut self, new: Cid) -> Result<()> {
        let addr = Address::new_id(self.to);
        let state_tree = self.call_manager.state_tree_mut();

        state_tree.mutate_actor(&addr, |actor_state| {
            actor_state.state = new;
            Ok(())
        })?;

        Ok(())
    }

    fn current_balance(&self) -> Result<TokenAmount> {
        let addr = Address::new_id(self.to);
        let balance = self
            .call_manager
            .state_tree()
            .get_actor(&addr)
            .or_fatal()
            .context("error when finding current actor")?
            .ok_or(anyhow!("state tree doesn't contain current actor"))
            .or_fatal()?
            .balance;
        Ok(balance)
    }

    fn self_destruct(&mut self, beneficiary: &Address) -> Result<()> {
        // TODO abort with internal error instead of returning.
        self.call_manager
            .charge_gas(self.call_manager.context().price_list().on_delete_actor())?;

        let balance = self.current_balance()?;
        if balance != TokenAmount::zero() {
            // Starting from network version v7, the runtime checks if the beneficiary
            // exists; if missing, it fails the self destruct.
            //
            // In FVM we check unconditionally, since we only support nv13+.
            let beneficiary_id = self.resolve_address(beneficiary)?.ok_or_else(||
                // TODO this should not be an actor error, but a system error with an exit code.
                syscall_error!(SysErrIllegalArgument, "beneficiary doesn't exist"))?;

            if beneficiary_id == self.to {
                return Err(syscall_error!(
                    SysErrIllegalArgument,
                    "benefactor cannot be beneficiary"
                )
                .into());
            }

            // Transfer the entirety of funds to beneficiary.
            self.call_manager
                .transfer(self.from, beneficiary_id, &balance)?;
        }

        // Delete the executing actor
        // TODO errors here are FATAL errors
        self.call_manager.state_tree_mut().delete_actor_id(self.to)
    }
}

impl<B, E> BlockOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: 'static + Externs,
{
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId> {
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

        let block = Block::new(cid.codec(), data);
        // TODO: I mean, this means you put 4M blocks in a single message. That's not actually possible?
        self.blocks.put(block).or_illegal_argument()
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

impl<B, E> MessageOps for DefaultKernel<B, E> {
    fn msg_caller(&self) -> ActorID {
        self.from
    }

    fn msg_receiver(&self) -> ActorID {
        self.to
    }

    fn msg_method_number(&self) -> MethodNum {
        self.method
    }

    fn msg_value_received(&self) -> TokenAmount {
        self.value_received.clone()
    }
}

impl<B, E> SendOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs + 'static,
{
    /// XXX: is message the right argument? Most of the fields are unused and unchecked.
    /// Also, won't the params be a block ID?
    fn send(&mut self, message: Message) -> Result<Receipt> {
        self.call_manager.state_tree_mut().begin_transaction();

        let res = self.call_manager.send(
            self.from,
            message.to,
            message.method_num,
            &message.params,
            &message.value,
        );
        // TODO Do something with the result.
        self.call_manager
            .state_tree_mut()
            .end_transaction(res.is_err())?;

        res
    }
}

impl<B, E> CircSupplyOps for DefaultKernel<B, E>
where
    E: Externs,
{
    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        todo!()
    }
}

impl<B, E> CryptoOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn verify_signature(
        &mut self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool> {
        self.call_manager.charge_gas(
            self.call_manager
                .context()
                .price_list()
                .on_verify_signature(signature.signature_type()),
        )?;

        // Resolve to key address before verifying signature.
        let signing_addr = self.resolve_to_key_addr(signer)?;
        Ok(signature.verify(plaintext, &signing_addr).is_ok())
    }

    fn hash_blake2b(&mut self, data: &[u8]) -> Result<[u8; 32]> {
        self.call_manager.charge_gas(
            self.call_manager
                .context()
                .price_list()
                .on_hashing(data.len()),
        )?;

        Ok(blake2b_256(data))
    }

    fn compute_unsealed_sector_cid(
        &mut self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        self.call_manager.charge_gas(
            self.call_manager
                .context()
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
        verify_seal(vi)
    }

    fn verify_post(&mut self, verify_info: &WindowPoStVerifyInfo) -> Result<bool> {
        self.call_manager.charge_gas(
            self.call_manager
                .context()
                .price_list()
                .on_verify_post(verify_info),
        )?;

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
        self.call_manager.charge_gas(
            self.call_manager
                .context()
                .price_list()
                .on_verify_consensus_fault(),
        )?;

        // This syscall cannot be resolved inside the FVM, so we need to traverse
        // the node boundary through an extern.
        self.call_manager
            .externs()
            .verify_consensus_fault(h1, h2, extra)
            .or_illegal_argument()
            .context("fault not verified")
    }

    fn batch_verify_seals(
        &mut self,
        vis: &[(&Address, &[SealVerifyInfo])],
    ) -> Result<HashMap<Address, Vec<bool>>> {
        log::debug!("batch verify seals start");
        let out = vis
            .par_iter()
            .with_min_len(vis.len() / *NUM_CPUS)
            .map(|(&addr, seals)| {
                let results = seals
                    .par_iter()
                    .map(|s| {
                        let verify_seal_result = std::panic::catch_unwind(|| verify_seal(s));
                        match verify_seal_result {
                            Ok(res) => {
                                match res {
                                    Ok(correct) => {
                                        if !correct {
                                            log::debug!(
                                            "seal verify in batch failed (miner: {}) (err: Invalid Seal proof)",
                                            addr,
                                            );
                                        }
                                        correct // all ok
                                    }
                                    Err(err) => {
                                        log::debug!(
                                        "seal verify in batch failed (miner: {}) (err: {})",
                                        addr,
                                        err
                                    );
                                        false
                                    }
                                }
                            },
                            Err(_) => {
                                log::error!("seal verify internal fail (miner: {})", addr);
                                false
                            }
                        }
                    })
                    .collect();
                (addr, results)
            })
            .collect();
        log::debug!("batch verify seals end");
        Ok(out)
    }

    fn verify_aggregate_seals(
        &mut self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<bool> {
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

impl<B, E> GasOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn charge_gas(&mut self, name: &str, compute: i64) -> Result<()> {
        let charge = GasCharge::new(name, compute, 0);
        self.call_manager.charge_gas(charge)
    }
}

impl<B, E> NetworkOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn network_epoch(&self) -> ChainEpoch {
        self.call_manager.context().epoch()
    }

    fn network_version(&self) -> NetworkVersion {
        self.call_manager.context().network_version()
    }

    fn network_base_fee(&self) -> &TokenAmount {
        self.call_manager.context().base_fee()
    }
}

impl<B, E> RandomnessOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: 'static + Externs,
{
    #[allow(unused)]
    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness> {
        // TODO: Check error code
        self.call_manager
            .externs()
            .get_chain_randomness_looking_forward(personalization, rand_epoch, entropy)
            .map(|r| Randomness(r.to_vec()))
            .or_illegal_argument()
    }

    #[allow(unused)]
    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness> {
        // TODO: Check error code
        // Hyperdrive and above only.
        self.call_manager
            .externs()
            .get_beacon_randomness_looking_forward(personalization, rand_epoch, entropy)
            .map(|r| Randomness(r.to_vec()))
            .or_illegal_argument()
    }
}

impl<B, E> ValidationOps for DefaultKernel<B, E>
where
    B: 'static + Blockstore,
    E: 'static + Externs,
{
    fn validate_immediate_caller_accept_any(&mut self) -> Result<()> {
        self.assert_not_validated()
    }

    fn validate_immediate_caller_addr_one_of(&mut self, allowed: &[Address]) -> Result<()> {
        self.assert_not_validated()?;

        let caller_addr = Address::new_id(self.from);
        if !allowed.iter().any(|a| *a == caller_addr) {
            return Err(syscall_error!(SysErrForbidden;
                "caller {} is not one of supported", caller_addr
            )
            .into());
        }
        Ok(())
    }

    fn validate_immediate_caller_type_one_of(&mut self, allowed: &[Cid]) -> Result<()> {
        self.assert_not_validated()?;

        let caller_cid = self
            .get_actor_code_cid(&Address::new_id(self.from))?
            .ok_or_else(|| anyhow!("failed to lookup code cid for caller"))
            .or_fatal()?;

        if !allowed.iter().any(|c| *c == caller_cid) {
            return Err(syscall_error!(SysErrForbidden;
                    "caller cid type {} not one of supported", caller_cid)
            .into());
        }
        Ok(())
    }
}

impl<B, E> ActorOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs,
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
    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Result<()> {
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
        if let Ok(Some(_)) = state_tree.get_actor(address) {
            return Err(
                syscall_error!(SysErrIllegalArgument; "Actor address already exists").into(),
            );
        }

        self.call_manager
            .charge_gas(self.call_manager.context().price_list().on_create_actor())?;

        let state_tree = self.call_manager.state_tree_mut();
        state_tree.set_actor(
            address,
            ActorState::new(code_id, *EMPTY_ARR_CID, 0.into(), 0),
        )
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
    .or_fatal()
    .context("failed to verify seal proof") // TODO: Verify that this is actually a fatal error.
}
