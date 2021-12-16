use anyhow::anyhow;
use anyhow::Context;
use fvm_shared::error::ExitCode;
use std::collections::VecDeque;
use std::convert::{TryFrom, TryInto};

use cid::Cid;
use num_traits::Signed;

use blockstore::Blockstore;
use fvm_shared::bigint::Zero;
use fvm_shared::commcid::{
    cid_to_data_commitment_v1, cid_to_replica_commitment_v1, data_commitment_v1_to_cid,
};
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{blake2b_256, bytes_32, CborStore, RawBytes};
use fvm_shared::error::ActorError;
use fvm_shared::error::ExitCode::SysErrIllegalArgument;
use fvm_shared::{actor_error, ActorID};

use crate::call_manager::CallManager;
use crate::externs::Externs;
use crate::init_actor::State;
use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::StateTree;

use filecoin_proofs_api::seal::compute_comm_d;
use filecoin_proofs_api::{self as proofs, seal, ProverId, SectorId};
use filecoin_proofs_api::{
    post, seal::verify_aggregate_seal_commit_proofs, seal::verify_seal as proofs_verify_seal,
    PublicReplicaInfo,
};
use fvm_shared::address::Protocol;
use fvm_shared::consensus::ConsensusFaultType;
use fvm_shared::piece::{zero_piece_commitment, PaddedPieceSize};

use super::blocks::{Block, BlockRegistry};
use super::error::Result;
use super::*;

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
    /// Return stack where values returned by syscalls are stored for consumption.
    return_stack: VecDeque<Vec<u8>>,
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
            return_stack: Default::default(),
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
            return Err(actor_error!(SysErrIllegalActor;
                    "Method must validate caller identity exactly once")
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
            .ok_or(anyhow!("state tree doesn't contain actor"))?;

        let state: crate::account_actor::State = state_tree
            .store()
            .get_cbor(&act.state)?
            .ok_or(anyhow!("account actor state not found"))?;

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
            .clone()
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
            .get_actor(&addr)?
            .ok_or(anyhow!("state tree doesn't contain current actor"))?
            .balance;
        Ok(balance)
    }

    fn self_destruct(&mut self, beneficiary: &Address) -> Result<()> {
        let gas_charge = self.call_manager.context().price_list().on_delete_actor();
        // TODO abort with internal error instead of returning.
        self.call_manager.charge_gas(gas_charge)?;

        let balance = self.current_balance()?;
        if balance != TokenAmount::zero() {
            // Starting from network version v7, the runtime checks if the beneficiary
            // exists; if missing, it fails the self destruct.
            //
            // In FVM we check unconditionally, since we only support nv13+.
            let beneficiary_id = self.resolve_address(beneficiary)?.ok_or_else(||
                // TODO this should not be an actor error, but a system error with an exit code.
                actor_error!(SysErrIllegalArgument, "beneficiary doesn't exist"))?;

            if beneficiary_id == self.to {
                return Err(actor_error!(
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
            .map_err(|e| anyhow!(e))?
            .ok_or_else(|| BlockError::MissingState(Box::new(*cid)))?;

        let block = Block::new(cid.codec(), data);
        Ok(self.blocks.put(block)?)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId> {
        Ok(self.blocks.put(Block::new(codec, data))?)
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid> {
        // TODO: check hash function & length against allow list.

        use multihash::MultihashDigest;
        let block = self.blocks.get(id)?;
        let code =
            multihash::Code::try_from(hash_fun)
                .ok()
                .ok_or(BlockError::InvalidMultihashSpec {
                    code: hash_fun,
                    length: hash_len,
                })?;

        let hash = code.digest(&block.data());
        if u32::from(hash.size()) < hash_len {
            return Err(BlockError::InvalidMultihashSpec {
                code: hash_fun,
                length: hash_len,
            }
            .into());
        }
        let k = Cid::new_v1(block.codec, hash.truncate(hash_len as u8));
        // TODO: for now, we _put_ the block here. In the future, we should put it into a write
        // cache, then flush it later.
        self.call_manager
            .blockstore()
            .put_keyed(&k, block.data())
            .map_err(|e| anyhow!(e))?;
        Ok(k)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32> {
        let data = &self.blocks.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat> {
        let b = self.blocks.get(id)?;
        Ok(BlockStat {
            codec: b.codec(),
            size: b.size(),
        })
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
        self.msg_method_number()
    }

    fn msg_value_received(&self) -> TokenAmount {
        self.value_received.clone()
    }
}

impl<B, E> ReturnOps for DefaultKernel<B, E> {
    fn return_size(&self) -> u64 {
        self.return_stack.back().map(Vec::len).unwrap_or(0) as u64
    }

    fn return_discard(&mut self) {
        self.return_stack.pop_back();
    }

    fn return_pop(&mut self, into: &mut [u8]) -> u64 {
        let ret: Vec<u8> = self.return_stack.pop_back().unwrap_or(Vec::new());
        let len = into.len().min(ret.len());
        into.copy_from_slice(&ret[..len]);
        len as u64
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

        // We convert the error into a receipt because we _dont'_ want to trap.
        // TODO: we need to log the error message int he machine somehow.
        res.map(|v| Receipt {
            exit_code: ExitCode::Ok,
            return_data: v,
            gas_used: 0, // fill in?
        })
        .or_else(|e| match e {
            ExecutionError::Actor(e) => {
                // These cases shouldn't be possible, but we can't yet statically rule them out and
                // I don't trust auto-conversion magic.
                if e.is_fatal() {
                    Err(ExecutionError::SystemError(anyhow!(e.msg().to_string())))
                } else if e.exit_code().is_success() {
                    Err(ExecutionError::SystemError(anyhow!(
                        "got an error with a success code"
                    )))
                } else {
                    Ok(Receipt {
                        exit_code: e.exit_code(),
                        return_data: RawBytes::default(),
                        gas_used: 0,
                    })
                }
            }
            err => Err(err),
        })
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
    ) -> Result<()> {
        let charge = self
            .call_manager
            .context()
            .price_list()
            .on_verify_signature(signature.signature_type());
        self.call_manager.charge_gas(charge)?;

        // Resolve to key address before verifying signature.
        let signing_addr = self.resolve_to_key_addr(signer)?;
        Ok(signature
            .verify(plaintext, &signing_addr)
            // TODO raising as a system error but this is NOT a fatal error;
            //  this should be a SyscallError type with no associated exit code.
            .map_err(|s| ExecutionError::SystemError(anyhow!(s)))?)
    }

    fn hash_blake2b(&mut self, data: &[u8]) -> Result<[u8; 32]> {
        let charge = self
            .call_manager
            .context()
            .price_list()
            .on_hashing(data.len());
        self.call_manager.charge_gas(charge)?;

        Ok(blake2b_256(data))
    }

    fn compute_unsealed_sector_cid(
        &mut self,
        reg: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid> {
        todo!()
    }

    /// Verify seal proof for sectors. This proof verifies that a sector was sealed by the miner.
    fn verify_seal(&mut self, vi: &SealVerifyInfo) -> Result<()> {
        todo!()
    }

    fn verify_post(&mut self, verify_info: &WindowPoStVerifyInfo) -> Result<()> {
        todo!()
    }

    fn verify_consensus_fault(
        &mut self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>> {
        todo!()
    }

    fn batch_verify_seals(
        &mut self,
        vis: &[(&Address, &[SealVerifyInfo])],
    ) -> Result<HashMap<Address, Vec<bool>>> {
        todo!()
    }

    fn verify_aggregate_seals(
        &mut self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<()> {
        todo!()
    }
}

impl<B, E> GasOps for DefaultKernel<B, E> {
    fn charge_gas(&mut self, name: &str, compute: i64) -> Result<()> {
        todo!()
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
    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness> {
        todo!()
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness> {
        todo!()
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
            return Err(actor_error!(SysErrForbidden;
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
            .ok_or_else(|| actor_error!(fatal("failed to lookup code cid for caller")))?;

        if !allowed.iter().any(|c| *c == caller_cid) {
            return Err(actor_error!(SysErrForbidden;
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
            .get_actor(addr)?
            // TODO fatal error
            //.map_err(|e| e.downcast_fatal("failed to get actor"))?
            .map(|act| act.code))
    }

    fn new_actor_address(&mut self) -> Result<Address> {
        todo!()
    }

    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Result<()> {
        todo!()
    }
}

// TODO provisional, remove once we fix https://github.com/filecoin-project/fvm/issues/107
impl Into<ActorError> for BlockError {
    fn into(self) -> ActorError {
        ActorError::new_fatal(self.to_string())
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
