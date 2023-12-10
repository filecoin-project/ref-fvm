// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::convert::{TryFrom, TryInto};
use std::panic::{self, UnwindSafe};
use std::path::PathBuf;

use anyhow::{anyhow, Context as _};
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::IPLD_RAW;
use fvm_shared::address::Payload;
use fvm_shared::crypto::signature;
use fvm_shared::error::ErrorNumber;
use fvm_shared::event::{ActorEvent, Entry, Flags};
use fvm_shared::sys::out::vm::ContextFlags;
use fvm_shared::upgrade::UpgradeInfo;
use multihash::MultihashDigest;

use super::blocks::{Block, BlockRegistry};
use super::error::Result;
use super::hash::SupportedHashes;
use super::*;
use crate::call_manager::{
    CallManager, Entrypoint, InvocationResult, INVOKE_FUNC_NAME, NO_DATA_BLOCK_ID,
    UPGRADE_FUNC_NAME,
};
use crate::externs::{Chain, Rand};
use crate::gas::GasTimer;
use crate::init_actor::INIT_ACTOR_ID;
use crate::machine::{MachineContext, NetworkConfig, BURNT_FUNDS_ACTOR_ID};
use crate::state_tree::ActorState;
use crate::{ipld, syscall_error};

const BLAKE2B_256: u64 = 0xb220;
const ENV_ARTIFACT_DIR: &str = "FVM_STORE_ARTIFACT_DIR";
const MAX_ARTIFACT_NAME_LEN: usize = 256;

#[cfg(feature = "testing")]
const TEST_ACTOR_ALLOWED_TO_CALL_CREATE_ACTOR: ActorID = 98;

/// The "default" [`Kernel`] implementation.
pub struct DefaultKernel<C> {
    // Fields extracted from the message, except parameters, which have been
    // preloaded into the block registry.
    pub caller: ActorID,
    pub actor_id: ActorID,
    pub method: MethodNum,
    pub value_received: TokenAmount,
    pub read_only: bool,

    /// The call manager for this call stack. If this kernel calls another actor, it will
    /// temporarily "give" the call manager to the other kernel before re-attaching it.
    pub call_manager: C,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    pub blocks: BlockRegistry,
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

    fn send<K: Kernel<CallManager = C>>(
        &mut self,
        recipient: &Address,
        method: MethodNum,
        params_id: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<CallResult> {
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
            cm.call_actor::<K>(
                from,
                *recipient,
                Entrypoint::Invoke(method),
                params,
                value,
                gas_limit,
                read_only,
            )
        })?;

        // Store result and return.
        Ok(match result {
            InvocationResult {
                exit_code,
                value: Some(blk),
            } => {
                let block_stat = blk.stat();
                // This can't fail because:
                // 1. We've already charged for gas.
                // 2. We've already checked that we have space for a return block.
                // 3. This block has already been validated by the kernel that returned it.
                let block_id = self
                    .blocks
                    .put_reachable(blk)
                    .or_fatal()
                    .context("failed to store a valid return value")?;
                CallResult {
                    block_id,
                    block_stat,
                    exit_code,
                }
            }
            InvocationResult {
                exit_code,
                value: None,
            } => CallResult {
                block_id: NO_DATA_BLOCK_ID,
                block_stat: BlockStat { codec: 0, size: 0 },
                exit_code,
            },
        })
    }

    fn upgrade_actor<K: Kernel<CallManager = C>>(
        &mut self,
        new_code_cid: Cid,
        params_id: BlockId,
    ) -> Result<CallResult> {
        if self.read_only {
            return Err(
                syscall_error!(ReadOnly, "upgrade_actor cannot be called while read-only").into(),
            );
        }

        // check if this actor is already on the call stack
        //
        // We first find the first position of this actor on the call stack, and then make sure that
        // no other actor appears on the call stack after 'position' (unless its a recursive upgrade
        // call which is allowed)
        let mut iter = self.call_manager.get_call_stack().iter();
        let position = iter.position(|&tuple| tuple == (self.actor_id, INVOKE_FUNC_NAME));
        if position.is_some() {
            for tuple in iter {
                if tuple.0 != self.actor_id || tuple.1 != UPGRADE_FUNC_NAME {
                    return Err(syscall_error!(
                        Forbidden,
                        "calling upgrade on actor already on call stack is forbidden"
                    )
                    .into());
                }
            }
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

        let result = self.call_manager.with_transaction(|cm| {
            let state = cm
                .get_actor(self.actor_id)?
                .ok_or_else(|| syscall_error!(IllegalOperation; "actor deleted"))?;

            // store the code cid of the calling actor before running the upgrade entrypoint
            // in case it was changed (which could happen if the target upgrade entrypoint
            // sent a message to this actor which in turn called upgrade)
            let code = state.code;

            // update the code cid of the actor to new_code_cid
            cm.set_actor(
                self.actor_id,
                ActorState::new(
                    new_code_cid,
                    state.state,
                    state.balance,
                    state.sequence,
                    None,
                ),
            )?;

            // run the upgrade entrypoint
            let result = cm.call_actor::<K>(
                self.caller,
                Address::new_id(self.actor_id),
                Entrypoint::Upgrade(UpgradeInfo { old_code_cid: code }),
                params,
                &TokenAmount::from_whole(0),
                None,
                false,
            )?;

            Ok(result)
        });

        match result {
            Ok(InvocationResult { exit_code, value }) => {
                let (block_stat, block_id) = match value {
                    None => (BlockStat { codec: 0, size: 0 }, NO_DATA_BLOCK_ID),
                    Some(block) => (block.stat(), self.blocks.put_reachable(block)?),
                };
                Ok(CallResult {
                    block_id,
                    block_stat,
                    exit_code,
                })
            }
            Err(err) => Err(err),
        }
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
    fn root(&mut self) -> Result<Cid> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_get_root())?;

        // This can fail during normal operations if the actor has been deleted.
        let cid = self
            .get_self()?
            .context("state root requested after actor deletion")
            .or_error(ErrorNumber::IllegalOperation)?
            .state;

        self.blocks.mark_reachable(&cid);

        t.stop();

        Ok(cid)
    }

    fn set_root(&mut self, new: Cid) -> Result<()> {
        if self.read_only {
            return Err(
                syscall_error!(ReadOnly; "cannot update the state-root while read-only").into(),
            );
        }

        let _ = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_set_root())?;

        if !self.blocks.is_reachable(&new) {
            return Err(syscall_error!(NotFound; "new root cid not reachable: {new}").into());
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

    fn self_destruct(&mut self, burn_unspent: bool) -> Result<()> {
        if self.read_only {
            return Err(syscall_error!(ReadOnly; "cannot self-destruct when read-only").into());
        }

        // Idempotent: If the actor doesn't exist, this won't actually do anything. The current
        // balance will be zero, and `delete_actor_id` will be a no-op.
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_delete_actor())?;

        // If there are remaining funds, burn them. We do this instead of letting the user to
        // specify the beneficiary as:
        //
        // 1. This lets the user handle transfer failure cases themselves. The only way _this_ can
        //    fail is for the caller to run out of gas.
        // 2. If we ever decide to allow code on method 0, allowing transfers here would be
        //    unfortunate.
        let balance = self.current_balance()?;
        if !balance.is_zero() {
            if !burn_unspent {
                return Err(
                    syscall_error!(IllegalOperation; "self-destruct with unspent funds").into(),
                );
            }
            self.call_manager
                .transfer(self.actor_id, BURNT_FUNDS_ACTOR_ID, &balance)
                .or_fatal()?;
        }

        // Delete the executing actor.
        t.record(self.call_manager.delete_actor(self.actor_id))
    }
}

impl<C> IpldBlockOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn block_open(&mut self, cid: &Cid) -> Result<(BlockId, BlockStat)> {
        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_block_open_base())?;

        if !self.blocks.is_reachable(cid) {
            return Err(syscall_error!(NotFound; "block not reachable: {cid}").into());
        }

        let data = self
            .call_manager
            .blockstore()
            .get(cid)
            // Treat missing blocks as errors as well.
            .and_then(|b| b.ok_or_else(|| anyhow!("missing reachable state: {}", cid)))
            // TODO Any failures here should really be considered "super fatal". It means we're
            // missing state and/or have a corrupted store.
            .or_fatal()?;

        t.stop();

        // This can fail because we can run out of gas.
        let children = ipld::scan_for_reachable_links(
            cid.codec(),
            &data,
            self.call_manager.price_list(),
            self.call_manager.gas_tracker(),
        )?;

        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_block_open(data.len(), children.len()),
        )?;

        let block = Block::new(cid.codec(), data, children);
        let stat = block.stat();
        let id = self.blocks.put_reachable(block)?;
        t.stop();
        Ok((id, stat))
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId> {
        if data.len() > self.machine().context().max_block_size {
            return Err(syscall_error!(LimitExceeded; "blocks may not be larger than 1MiB").into());
        }

        if !ipld::ALLOWED_CODECS.contains(&codec) {
            return Err(syscall_error!(IllegalCodec; "codec {} not allowed", codec).into());
        }

        let children = ipld::scan_for_reachable_links(
            codec,
            data,
            self.call_manager.price_list(),
            self.call_manager.gas_tracker(),
        )?;

        let t = self.call_manager.charge_gas(
            self.call_manager
                .price_list()
                .on_block_create(data.len(), children.len()),
        )?;

        let blk = Block::new(codec, data, children);

        t.record(Ok(self.blocks.put_check_reachable(blk)?))
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
        self.call_manager
            .blockstore()
            .put_keyed(&k, block.data())
            // TODO: This is really "super fatal". It means we failed to store state, and should
            // probably abort the entire block.
            .or_fatal()?;
        self.blocks.mark_reachable(&k);

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
            .map_err(|_| syscall_error!(IllegalArgument; "offset plus buffer length did not fit into an i32"))?;

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

    fn hash(&self, code: u64, data: &[u8]) -> Result<Multihash> {
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
            Greater => {}
        }

        self.call_manager
            .charge_gas(self.call_manager.price_list().on_tipset_cid(offset))?;

        self.call_manager.externs().get_tipset_cid(epoch).or_fatal()
    }
}

impl<C> RandomnessOps for DefaultKernel<C>
where
    C: CallManager,
{
    fn get_randomness_from_tickets(
        &self,
        rand_epoch: ChainEpoch,
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        let lookback = self
            .call_manager
            .context()
            .epoch
            .checked_sub(rand_epoch)
            .ok_or_else(|| syscall_error!(IllegalArgument; "randomness epoch {} is in the future", rand_epoch)
            )?;

        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_get_randomness(lookback))?;

        t.record(
            self.call_manager
                .externs()
                .get_chain_randomness(rand_epoch)
                .or_illegal_argument(),
        )
    }

    fn get_randomness_from_beacon(
        &self,
        rand_epoch: ChainEpoch,
    ) -> Result<[u8; RANDOMNESS_LENGTH]> {
        let lookback = self
            .call_manager
            .context()
            .epoch
            .checked_sub(rand_epoch)
            .ok_or_else(|| syscall_error!(IllegalArgument; "randomness epoch {} is in the future", rand_epoch))?;

        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_get_randomness(lookback))?;

        t.record(
            self.call_manager
                .externs()
                .get_beacon_randomness(rand_epoch)
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

    fn install_actor(&mut self, code_id: Cid) -> Result<()> {
        let start = GasTimer::start();
        let size = self
            .call_manager
            .engine()
            .preload(self.call_manager.blockstore(), &[code_id])
            .context("failed to install actor")
            .or_illegal_argument()?;

        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_install_actor(size))?;
        t.stop_with(start);

        Ok(())
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
    fn emit_event(
        &mut self,
        event_headers: &[fvm_shared::sys::EventEntry],
        event_keys: &[u8],
        event_values: &[u8],
    ) -> Result<()> {
        const MAX_NR_ENTRIES: usize = 255;
        const MAX_KEY_LEN: usize = 31;
        const MAX_TOTAL_VALUES_LEN: usize = 8 << 10;

        if self.read_only {
            return Err(syscall_error!(ReadOnly; "cannot emit events while read-only").into());
        }

        let t = self
            .call_manager
            .charge_gas(self.call_manager.price_list().on_actor_event(
                event_headers.len(),
                event_keys.len(),
                event_values.len(),
            ))?;

        if event_headers.len() > MAX_NR_ENTRIES {
            return Err(syscall_error!(LimitExceeded; "event exceeded max entries: {} > {MAX_NR_ENTRIES}", event_headers.len()).into());
        }

        if event_values.len() > MAX_TOTAL_VALUES_LEN {
            return Err(syscall_error!(LimitExceeded; "total event value lengths exceeded the max size: {} > {MAX_TOTAL_VALUES_LEN}", event_values.len()).into());
        }

        // We validate utf8 all at once for better performance.
        let event_keys = std::str::from_utf8(event_keys)
            .context("invalid event key")
            .or_illegal_argument()?;

        let mut key_offset: usize = 0;
        let mut val_offset: usize = 0;

        let mut entries: Vec<Entry> = Vec::with_capacity(event_headers.len());
        for header in event_headers {
            // make sure that the fixed parsed values are within bounds before we do any allocation
            let flags = header.flags;
            if Flags::from_bits(flags.bits()).is_none() {
                return Err(
                    syscall_error!(IllegalArgument; "event flags are invalid: {}", flags.bits())
                        .into(),
                );
            }

            if header.key_len > MAX_KEY_LEN as u32 {
                let tmp = header.key_len;
                return Err(syscall_error!(LimitExceeded; "event key exceeded max size: {} > {MAX_KEY_LEN}", tmp).into());
            }

            // We check this here purely to detect/prevent integer overflows below. That's why we
            // return IllegalArgument, not LimitExceeded.
            if header.val_len > MAX_TOTAL_VALUES_LEN as u32 {
                return Err(
                    syscall_error!(IllegalArgument; "event entry value out of range").into(),
                );
            }

            // parse the variable sized fields from the raw_key/raw_val buffers
            let key = &event_keys
                .get(key_offset..key_offset + header.key_len as usize)
                .context("event entry key out of range")
                .or_illegal_argument()?;

            let value = &event_values
                .get(val_offset..val_offset + header.val_len as usize)
                .context("event entry value out of range")
                .or_illegal_argument()?;

            // Check the codec. We currently only allow IPLD_RAW.
            if header.codec != IPLD_RAW {
                let tmp = header.codec;
                return Err(
                    syscall_error!(IllegalCodec; "event codec must be IPLD_RAW, was: {}", tmp)
                        .into(),
                );
            }

            // we have all we need to construct a new Entry
            let entry = Entry {
                flags: header.flags,
                key: key.to_string(),
                codec: header.codec,
                value: value.to_vec(),
            };

            // shift the key/value offsets
            key_offset += header.key_len as usize;
            val_offset += header.val_len as usize;

            entries.push(entry);
        }

        if key_offset != event_keys.len() {
            return Err(syscall_error!(IllegalArgument;
                "event key buffer length is too large: {} < {}",
                key_offset,
                event_keys.len()
            )
            .into());
        }

        if val_offset != event_values.len() {
            return Err(syscall_error!(IllegalArgument;
                "event value buffer length is too large: {} < {}",
                val_offset,
                event_values.len()
            )
            .into());
        }

        let actor_evt = ActorEvent::from(entries);

        let stamped_evt = StampedEvent::new(self.actor_id, actor_evt);
        // Enable this when performing gas calibration to measure the cost of serializing early.
        #[cfg(feature = "gas_calibration")]
        let _ = fvm_ipld_encoding::to_vec(&stamped_evt).unwrap();

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
