// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::panic::{self, UnwindSafe};
use std::path::PathBuf;

use anyhow::{anyhow, Context as _};
use cid::Cid;
use filecoin_proofs_api::{self as proofs, ProverId, PublicReplicaInfo, SectorId};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{bytes_32, IPLD_RAW};
use fvm_shared::address::Payload;
use fvm_shared::bigint::Zero;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::event::ActorEvent;
use fvm_shared::piece::{zero_piece_commitment, PaddedPieceSize};
use fvm_shared::sector::SectorInfo;
use fvm_shared::sys::out::vm::ContextFlags;
use fvm_shared::{commcid, ActorID};
use lazy_static::lazy_static;
use multihash::MultihashDigest;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rayon::prelude::ParallelDrainRange;

use super::blocks::{Block, BlockRegistry};
use super::error::Result;
use super::hash::SupportedHashes;
use super::*;
use crate::call_manager::{CallManager, InvocationResult, NO_DATA_BLOCK_ID};
use crate::externs::{Chain, Consensus, Rand};
use crate::gas::GasTimer;
use crate::init_actor::INIT_ACTOR_ID;
use crate::machine::{MachineContext, NetworkConfig};
use crate::state_tree::ActorState;
use crate::syscall_error;

lazy_static! {
    static ref NUM_CPUS: usize = num_cpus::get();
    static ref INITIAL_RESERVE_BALANCE: TokenAmount = TokenAmount::from_whole(300_000_000);
}

const BLAKE2B_256: u64 = 0xb220;
const ENV_ARTIFACT_DIR: &str = "FVM_STORE_ARTIFACT_DIR";
const MAX_ARTIFACT_NAME_LEN: usize = 256;
const FINALITY: i64 = 900;

#[cfg(feature = "testing")]
const TEST_ACTOR_ALLOWED_TO_CALL_CREATE_ACTOR: ActorID = 98;

/// The "default" [`Kernel`] implementation.
pub struct DefaultKernel<C> {
    // Fields extracted from the message, except parameters, which have been
    // preloaded into the block registry.
    caller: ActorID,
    actor_id: ActorID,
    method: MethodNum,
    value_received: TokenAmount,
    read_only: bool,

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

    fn into_inner(self) -> (Self::CallManager, BlockRegistry)
    where
        Self: Sized,
    {
        (self.call_manager, self.blocks)
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
        DefaultKernel {
            call_manager: mgr,
            blocks,
            caller,
            actor_id,
            method,
            value_received,
            read_only,
        }
    }

    fn machine(&self) -> &<Self::CallManager as CallManager>::Machine {
        self.call_manager.machine()
    }
}

impl<C> DefaultKernel<C>
where
    C: CallManager,
{
    /// Returns `Some(actor_state)` or `None` if this actor has been deleted.
    fn get_self(&self) -> Result<Option<ActorState>> {
        self.call_manager.get_actor(self.actor_id)
    }
}

impl<C> SelfOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn root(&self) -> Result<Cid> {
        // This can fail during normal operations if the actor has been deleted.
        let cid = self
            .get_self()?
            .context("state root requested after actor deletion")
            .or_error(ErrorNumber::IllegalOperation)?
            .state;

        Ok(cid)
    }

    fn set_root(&mut self, new: Cid) -> Result<()> {
        if self.read_only {
            return Err(
                syscall_error!(ReadOnly; "cannot update the state-root while read-only").into(),
            );
        }
        let mut state = self
            .call_manager
            .get_actor(self.actor_id)?
            .ok_or_else(|| syscall_error!(IllegalOperation; "actor deleted"))?;
        state.state = new;
        self.call_manager.set_actor(self.actor_id, state)?;
        Ok(())
    }

    fn current_balance(&self) -> Result<TokenAmount> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_self_balance())?;

        // If the actor doesn't exist, it has zero balance.
        t.record(Ok(self.get_self()?.map(|a| a.balance).unwrap_or_default()))
    }

    fn self_destruct(&mut self, beneficiary: &Address) -> Result<()> {
        if self.read_only {
            return Err(syscall_error!(ReadOnly; "cannot self-destruct when read-only").into());
        }

        // Idempotentcy: If the actor doesn't exist, this won't actually do anything. The current
        // balance will be zero, and `delete_actor_id` will be a no-op.
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_delete_actor())?;

        let balance = self.current_balance()?;
        if balance != TokenAmount::zero() {
            // Starting from network version v7, the runtime checks if the beneficiary
            // exists; if missing, it fails the self destruct.
            //
            // In FVM we check unconditionally, since we only support nv13+.
            let beneficiary_id = self.resolve_address(beneficiary)?;

            if beneficiary_id == self.actor_id {
                return Err(syscall_error!(Forbidden, "benefactor cannot be beneficiary").into());
            }

            // Transfer the entirety of funds to beneficiary.
            self.call_manager
                .transfer(self.actor_id, beneficiary_id, &balance)?;
        }

        // Delete the executing actor
        t.record(self.call_manager.delete_actor(self.actor_id))
    }
}

impl<C> IpldBlockOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn block_open(&mut self, cid: &Cid) -> Result<(BlockId, BlockStat)> {
        // TODO(M2): Check for reachability here.

        let _ = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_block_open_base())?;

        let start = GasTimer::start();

        let data = self
            .call_manager
            .blockstore()
            .get(cid)
            // TODO: This is really "super fatal". It means we failed to store state, and should
            // probably abort the entire block.
            .or_fatal()?
            .ok_or_else(|| anyhow!("missing state: {}", cid))
            // Missing state is a fatal error because it means we have a bug. Once we do
            // reachability checking (for user actors) we won't get here unless the block is known
            // to be in the state-tree.
            .or_fatal()?;

        let block = Block::new(cid.codec(), data);

        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_block_open_per_byte(block.size() as usize),
        )?;

        let stat = block.stat();
        let id = self.blocks.put(block)?;
        t.stop_with(start);
        Ok((id, stat))
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId> {
        if data.len() > self.machine().context().max_block_size {
            return Err(syscall_error!(LimitExceeded; "blocks may not be larger than 1MiB").into());
        }

        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_block_create(data.len()))?;

        t.record(Ok(self.blocks.put(Block::new(codec, data))?))
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid> {
        if hash_fun != BLAKE2B_256 || hash_len != 32 {
            return Err(syscall_error!(IllegalCid; "cids must be 32-byte blake2b").into());
        }
        let start = GasTimer::start();
        let block = self.blocks.get(id)?;
        let code = SupportedHashes::try_from(hash_fun)
            .map_err(|_| syscall_error!(IllegalCid; "invalid CID codec"))?;

        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_block_link(code, block.size() as usize),
        )?;

        let hash = code.digest(block.data());
        if u32::from(hash.size()) < hash_len {
            return Err(syscall_error!(IllegalCid; "invalid hash length: {}", hash_len).into());
        }
        let k = Cid::new_v1(block.codec(), hash.truncate(hash_len as u8));
        // TODO(M2): Add the block to the reachable set.
        self.call_manager
            .blockstore()
            .put_keyed(&k, block.data())
            // TODO: This is really "super fatal". It means we failed to store state, and should
            // probably abort the entire block.
            .or_fatal()?;
        t.stop_with(start);
        Ok(k)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<i32> {
        let tstart = GasTimer::start();
        // First, find the end of the _logical_ buffer (taking the offset into account).
        // This must fit into an i32.

        // We perform operations as u64, because we know that the buffer length and offset must fit
        // in a u32.
        let end = i32::try_from((offset as u64) + (buf.len() as u64))
            .map_err(|_|syscall_error!(IllegalArgument; "offset plus buffer length did not fit into an i32"))?;

        // Then get the block.
        let block = self.blocks.get(id)?;
        let data = block.data();

        // We start reading at this offset.
        let start = offset as usize;

        // We read (block_length - start) bytes, or until we fill the buffer.
        let to_read = std::cmp::min(data.len().saturating_sub(start), buf.len());

        // We can now _charge_, because we actually know how many bytes we need to read.
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_block_read(to_read))?;

        // Copy into the output buffer, but only if were're reading. If to_read == 0, start may be
        // past the end of the block.
        if to_read != 0 {
            buf[..to_read].copy_from_slice(&data[start..(start + to_read)]);
        }
        t.stop_with(tstart);
        // Returns the difference between the end of the block, and offset + buf.len()
        Ok((data.len() as i32) - end)
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_block_stat())?;

        t.record(Ok(self.blocks.stat(id)?))
    }
}

impl<C> MessageOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn msg_context(&self) -> Result<MessageContext> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_message_context())?;

        let ctx = MessageContext {
            caller: self.caller,
            origin: self.call_manager.origin(),
            receiver: self.actor_id,
            method_number: self.method,
            value_received: (&self.value_received)
                .try_into()
                .or_fatal()
                .context("invalid token amount")?,
            gas_premium: self
                .call_manager
                .gas_premium()
                .try_into()
                .or_fatal()
                .context("invalid gas premium")?,
            flags: if self.read_only {
                ContextFlags::READ_ONLY
            } else {
                ContextFlags::empty()
            },
            nonce: self.call_manager.nonce(),
        };
        t.stop();
        Ok(ctx)
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
        params_id: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<SendResult> {
        let from = self.actor_id;
        let read_only = self.read_only || flags.read_only();

        if read_only && !value.is_zero() {
            return Err(syscall_error!(ReadOnly; "cannot transfer value when read-only").into());
        }

        // Load parameters.
        let params = if params_id == NO_DATA_BLOCK_ID {
            None
        } else {
            Some(self.blocks.get(params_id)?.clone())
        };

        // Make sure we can actually store the return block.
        if self.blocks.is_full() {
            return Err(syscall_error!(LimitExceeded; "cannot store return block").into());
        }

        // Send.
        let result = self.call_manager.with_transaction(|cm| {
            cm.send::<Self>(
                from, *recipient, method, params, value, gas_limit, read_only,
            )
        })?;

        // Store result and return.
        Ok(match result {
            InvocationResult {
                exit_code,
                value: Some(blk),
            } => {
                let block_stat = blk.stat();
                let block_id = self
                    .blocks
                    .put(blk)
                    .or_fatal()
                    .context("failed to store a valid return value")?;
                SendResult {
                    block_id,
                    block_stat,
                    exit_code,
                }
            }
            InvocationResult {
                exit_code,
                value: None,
            } => SendResult {
                block_id: NO_DATA_BLOCK_ID,
                block_stat: BlockStat { codec: 0, size: 0 },
                exit_code,
            },
        })
    }
}

impl<C> CircSupplyOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn total_fil_circ_supply(&self) -> Result<TokenAmount> {
        // From v15 and onwards, Filecoin mainnet was fixed to use a static circ supply per epoch.
        // The value reported to the FVM from clients is now the static value,
        // the FVM simply reports that value to actors.
        Ok(self.call_manager.context().circ_supply.clone())
    }
}

impl<C> CryptoOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn verify_signature(
        &self,
        sig_type: SignatureType,
        signature: &[u8],
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool> {
        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_verify_signature(sig_type, plaintext.len()),
        )?;

        // We only support key addresses (f1/f3). This change does not require a FIP, because no
        // actors invoke this method with non-key addresses.
        let signing_addr = match signer.payload() {
            Payload::BLS(_) | Payload::Secp256k1(_) => *signer,
            // Not a key address.
            _ => {
                return Err(syscall_error!(IllegalArgument; "address protocol {} not supported", signer.protocol()).into());
            }
        };

        // Verify signature, catching errors. Signature verification can include some complicated
        // math.
        t.record(catch_and_log_panic("verifying signature", || {
            Ok(signature::verify(sig_type, signature, plaintext, &signing_addr).is_ok())
        }))
    }

    fn recover_secp_public_key(
        &self,
        hash: &[u8; SECP_SIG_MESSAGE_HASH_SIZE],
        signature: &[u8; SECP_SIG_LEN],
    ) -> Result<[u8; SECP_PUB_LEN]> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_recover_secp_public_key())?;

        t.record(
            signature::ops::recover_secp_public_key(hash, signature)
                .map(|pubkey| pubkey.serialize())
                .map_err(|e| {
                    syscall_error!(IllegalArgument; "public key recovery failed: {}", e).into()
                }),
        )
    }

    fn hash(&self, code: u64, data: &[u8]) -> Result<MultihashGeneric<64>> {
        let hasher = SupportedHashes::try_from(code).map_err(|e| {
            if let multihash::Error::UnsupportedCode(code) = e {
                syscall_error!(IllegalArgument; "unsupported hash code {}", code)
            } else {
                syscall_error!(AssertionFailed; "hash expected unsupported code, got {}", e)
            }
        })?;

        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_hashing(hasher, data.len()),
        )?;

        t.record(Ok(hasher.digest(data)))
    }

    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_compute_unsealed_sector_cid(proof_type, pieces),
        )?;

        t.record(catch_and_log_panic("computing unsealed sector CID", || {
            compute_unsealed_sector_cid(proof_type, pieces)
        }))
    }

    /// Verify seal proof for sectors. This proof verifies that a sector was sealed by the miner.
    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<bool> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_verify_seal(vi))?;

        // It's probably _fine_ to just let these turn into fatal errors, but seal verification is
        // pretty self contained, so catching panics here probably doesn't hurt.
        t.record(catch_and_log_panic("verifying seal", || verify_seal(vi)))
    }

    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<bool> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_verify_post(verify_info))?;

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
        let t = self.call_manager.charge_gas(
            self.call_manager.price_list().on_verify_consensus_fault(
                h1.len(),
                h2.len(),
                extra.len(),
            ),
        )?;

        // This syscall cannot be resolved inside the FVM, so we need to traverse
        // the node boundary through an extern.
        let (fault, _) = t.record(
            self.call_manager
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
                .call_manager
                .charge_gas(self.call_manager.price_list().on_verify_seal(vi))?;
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
        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_verify_aggregate_seals(aggregate),
        )?;
        t.record(catch_and_log_panic("verifying aggregate seals", || {
            verify_aggregate_seals(aggregate)
        }))
    }

    fn verify_replica_update(&self, replica: &ReplicaUpdateInfo) -> Result<bool> {
        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_verify_replica_update(replica),
        )?;
        t.record(catch_and_log_panic("verifying replica update", || {
            verify_replica_update(replica)
        }))
    }
}

impl<C> GasOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn gas_used(&self) -> Gas {
        self.call_manager.gas_tracker().gas_used()
    }

    fn gas_available(&self) -> Gas {
        self.call_manager.gas_tracker().gas_available()
    }

    fn charge_gas(&self, name: &str, compute: Gas) -> Result<GasTimer> {
        self.call_manager.gas_tracker().charge_gas(name, compute)
    }

    fn price_list(&self) -> &PriceList {
        self.call_manager.price_list()
    }
}

impl<C> NetworkOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn network_context(&self) -> Result<NetworkContext> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_network_context())?;

        let MachineContext {
            epoch,
            timestamp,
            base_fee,
            network:
                NetworkConfig {
                    network_version,
                    chain_id,
                    ..
                },
            ..
        } = self.call_manager.context();

        let ctx = NetworkContext {
            chain_id: (*chain_id).into(),
            epoch: *epoch,
            network_version: *network_version,
            timestamp: *timestamp,
            base_fee: base_fee
                .try_into()
                .or_fatal()
                .context("base-fee exceeds u128 limit")?,
        };

        t.stop();
        Ok(ctx)
    }

    fn tipset_cid(&self, epoch: ChainEpoch) -> Result<Cid> {
        use std::cmp::Ordering::*;

        if epoch < 0 {
            return Err(syscall_error!(IllegalArgument; "epoch is negative").into());
        }
        let offset = self.call_manager.context().epoch - epoch;

        // Can't lookup the current tipset CID, or a future tipset CID>
        match offset.cmp(&0) {
            Less => return Err(syscall_error!(IllegalArgument; "epoch {} is in the future", epoch).into()),
            Equal => return Err(syscall_error!(IllegalArgument; "cannot lookup the tipset cid for the current epoch").into()),
            Greater => {},
        }

        // Can't lookup tipset CIDs beyond finality.
        if offset >= FINALITY {
            return Err(
                syscall_error!(IllegalArgument; "epoch {} is too far in the past", epoch).into(),
            );
        }

        let _ = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_tipset_cid(offset > 1));

        self.call_manager.externs().get_tipset_cid(epoch).or_fatal()
    }
}

impl<C> RandomnessOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn get_randomness_from_tickets(
        &self,
        personalization: i64,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_get_randomness(entropy.len()),
        )?;

        // TODO(M2): Check error code
        // Specifically, lookback length?
        t.record(
            self.call_manager
                .externs()
                .get_chain_randomness(personalization, rand_epoch, entropy)
                .or_illegal_argument(),
        )
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: i64,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_get_randomness(entropy.len()),
        )?;

        // TODO(M2): Check error code
        // Specifically, lookback length?
        t.record(
            self.call_manager
                .externs()
                .get_beacon_randomness(personalization, rand_epoch, entropy)
                .or_illegal_argument(),
        )
    }
}

impl<C> ActorOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn resolve_address(&self, address: &Address) -> Result<ActorID> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_resolve_address())?;

        t.record(Ok(self
            .call_manager
            .resolve_address(address)?
            .ok_or_else(|| syscall_error!(NotFound; "actor not found"))?))
    }

    fn get_actor_code_cid(&self, id: ActorID) -> Result<Cid> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_get_actor_code_cid())?;

        t.record(Ok(self
            .call_manager
            .get_actor(id)?
            .ok_or_else(|| syscall_error!(NotFound; "actor not found"))?
            .code))
    }

    fn next_actor_address(&self) -> Result<Address> {
        Ok(self.call_manager.next_actor_address())
    }

    fn create_actor(
        &mut self,
        code_id: Cid,
        actor_id: ActorID,
        delegated_address: Option<Address>,
    ) -> Result<()> {
        let is_allowed_to_create_actor = self.actor_id == INIT_ACTOR_ID;

        #[cfg(feature = "testing")]
        let is_allowed_to_create_actor =
            is_allowed_to_create_actor || self.actor_id == TEST_ACTOR_ALLOWED_TO_CALL_CREATE_ACTOR;

        if !is_allowed_to_create_actor {
            return Err(syscall_error!(
                Forbidden,
                "create_actor is restricted to InitActor. Called by {}",
                self.actor_id
            )
            .into());
        }

        if self.read_only {
            return Err(
                syscall_error!(ReadOnly, "create_actor cannot be called while read-only").into(),
            );
        }

        self.call_manager
            .create_actor(code_id, actor_id, delegated_address)
    }

    fn get_builtin_actor_type(&self, code_cid: &Cid) -> Result<u32> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_get_builtin_actor_type())?;

        let id = self
            .call_manager
            .machine()
            .builtin_actors()
            .id_by_code(code_cid);

        t.stop();
        Ok(id)
    }

    fn get_code_cid_for_type(&self, typ: u32) -> Result<Cid> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_get_code_cid_for_type())?;

        t.record(
            self.call_manager
                .machine()
                .builtin_actors()
                .code_by_id(typ)
                .cloned()
                .context("tried to resolve CID of unrecognized actor type")
                .or_illegal_argument(),
        )
    }

    #[cfg(feature = "m2-native")]
    fn install_actor(&mut self, code_id: Cid) -> Result<()> {
        let start = GasTimer::start();
        let size = self
            .call_manager
            .engine()
            .preload(self.call_manager.blockstore(), &[code_id])
            .map_err(|_| syscall_error!(IllegalArgument; "failed to load actor code"))?;

        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_install_actor(size))?;
        t.stop_with(start);

        Ok(())
    }

    fn balance_of(&self, actor_id: ActorID) -> Result<TokenAmount> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_balance_of())?;

        Ok(t.record(self.call_manager.get_actor(actor_id))?
            .ok_or_else(|| syscall_error!(NotFound; "actor not found"))?
            .balance)
    }

    fn lookup_delegated_address(&self, actor_id: ActorID) -> Result<Option<Address>> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_lookup_delegated_address())?;

        Ok(t.record(self.call_manager.get_actor(actor_id))?
            .ok_or_else(|| syscall_error!(NotFound; "actor not found"))?
            .delegated_address)
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
        self.call_manager.context().actor_debugging
    }

    fn store_artifact(&self, name: &str, data: &[u8]) -> Result<()> {
        // Ensure well formed artifact name
        {
            if name.len() > MAX_ARTIFACT_NAME_LEN {
                Err("debug artifact name should not exceed 256 bytes")
            } else if name.chars().any(std::path::is_separator) {
                Err("debug artifact name should not include any path separators")
            } else if name
                .chars()
                .next()
                .ok_or("debug artifact name should be at least one character")
                .or_error(fvm_shared::error::ErrorNumber::IllegalArgument)?
                == '.'
            {
                Err("debug artifact name should not start with a decimal '.'")
            } else {
                Ok(())
            }
        }
        .or_error(fvm_shared::error::ErrorNumber::IllegalArgument)?;

        // Write to disk
        if let Ok(dir) = std::env::var(ENV_ARTIFACT_DIR).as_deref() {
            let dir: PathBuf = [
                dir,
                self.call_manager.machine().machine_id(),
                &self.call_manager.origin().to_string(),
                &self.call_manager.nonce().to_string(),
                &self.actor_id.to_string(),
                &self.call_manager.invocation_count().to_string(),
            ]
            .iter()
            .collect();

            if let Err(e) = std::fs::create_dir_all(dir.clone()) {
                log::error!("failed to make directory to store debug artifacts {}", e);
            } else if let Err(e) = std::fs::write(dir.join(name), data) {
                log::error!("failed to store debug artifact {}", e)
            } else {
                log::info!("wrote artifact: {} to {:?}", name, dir);
            }
        } else {
            log::error!(
                "store_artifact was ignored, env var {} was not set",
                ENV_ARTIFACT_DIR
            )
        }
        Ok(())
    }
}

impl<C> LimiterOps for DefaultKernel<C>
where
    C: CallManager,
{
    type Limiter = <<C as CallManager>::Machine as Machine>::Limiter;

    fn limiter_mut(&mut self) -> &mut Self::Limiter {
        self.call_manager.limiter_mut()
    }
}

impl<C> EventOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn emit_event(&mut self, raw_evt: &[u8]) -> Result<()> {
        if self.read_only {
            return Err(syscall_error!(ReadOnly; "cannot emit events while read-only").into());
        }
        let len = raw_evt.len() as usize;
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_actor_event_validate(len))?;

        // This is an over-estimation of the maximum event size, for safety. No valid event can even
        // get close to this. We check this first so we don't try to decode a large event.
        const MAX_ENCODED_SIZE: usize = 1 << 20;
        if raw_evt.len() > MAX_ENCODED_SIZE {
            return Err(syscall_error!(IllegalArgument; "event WAY too large").into());
        }

        let actor_evt = {
            let res = match panic::catch_unwind(|| {
                fvm_ipld_encoding::from_slice(raw_evt).or_error(ErrorNumber::IllegalArgument)
            }) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("panic when decoding event cbor from actor: {:?}", e);
                    Err(syscall_error!(IllegalArgument; "panic when decoding event cbor from actor").into())
                }
            };
            t.stop();
            res
        }?;
        validate_actor_event(&actor_evt)?;

        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_actor_event_accept(&actor_evt, len),
        )?;

        let stamped_evt = StampedEvent::new(self.actor_id, actor_evt);
        self.call_manager.append_event(stamped_evt);

        t.stop();

        Ok(())
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

fn validate_actor_event(evt: &ActorEvent) -> Result<()> {
    const MAX_ENTRIES: usize = 256;
    const MAX_DATA: usize = 8 << 10;
    const MAX_KEY_LEN: usize = 32;

    if evt.entries.len() > MAX_ENTRIES {
        return Err(syscall_error!(IllegalArgument; "event exceeded max entries: {} > {MAX_ENTRIES}", evt.entries.len()).into());
    }
    let mut total_value_size: usize = 0;
    for entry in &evt.entries {
        if entry.key.len() > MAX_KEY_LEN {
            return Err(syscall_error!(IllegalArgument; "event key exceeded max size: {} > {MAX_KEY_LEN}", entry.key.len()).into());
        }
        if entry.codec != IPLD_RAW {
            return Err(
                syscall_error!(IllegalCodec; "event codec must be IPLD_RAW, was: {}", entry.codec)
                    .into(),
            );
        }
        total_value_size += entry.value.len();
    }
    if total_value_size > MAX_DATA {
        return Err(
            syscall_error!(IllegalArgument; "event total values exceeded max size: {total_value_size} > {MAX_DATA}").into(),
        );
    }
    Ok(())
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
                let commr = commcid::cid_to_replica_commitment_v1(&sector_info.sealed_cid)?;
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
    proofs::post::verify_window_post(&bytes_32(&randomness), &proofs, &replicas, prover_id)
        .or_illegal_argument()
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
