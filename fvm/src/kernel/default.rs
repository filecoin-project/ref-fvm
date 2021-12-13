use std::collections::VecDeque;
use std::convert::{TryFrom, TryInto};

use cid::Cid;
use derive_getters::Getters;
use wasmtime::{Engine, Linker, Module, Store};

use blockstore::Blockstore;
use fvm_shared::address::Protocol;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{RawBytes, DAG_CBOR};
use fvm_shared::error::{ActorError, ExitCode};
use fvm_shared::{actor_error, ActorID};

use crate::call_manager::CallManager;
use crate::errors::ActorDowncast;
use crate::externs::Externs;
use crate::gas::GasTracker;
use crate::machine::Machine;
use crate::message::Message;
use crate::state_tree::{ActorState, StateTree};
use crate::syscalls::bind_syscalls;

use super::blocks::{Block, BlockRegistry};
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
    call_manager: MapCell<CallManager<B, E>>,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    blocks: BlockRegistry,
    /// Return stack where values returned by syscalls are stored for consumption.
    return_stack: VecDeque<Vec<u8>>,
}

// Even though all children traits are implemented, Rust needs to know that the
// supertrait is implemented too.
impl<B, E> Kernel for DefaultKernel<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
}

impl<B, E> DefaultKernel<B, E>
where
    B: Blockstore + 'static,
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
            call_manager: MapCell::new(mgr),
            blocks: BlockRegistry::new(),
            return_stack: Default::default(),
            from,
            to,
            method,
            value_received,
        }
    }

    pub fn take(self) -> CallManager<B, E> {
        self.call_manager.take()
    }
}

impl<B, E> SelfOps for DefaultKernel<B, E>
where
    B: 'static + Blockstore,
    E: 'static + Externs,
{
    fn root(&self) -> Infallible<Cid> {
        let addr = Address::new_id(self.to);
        let state_tree = self.call_manager.state_tree();

        state_tree
            .get_actor(&addr)
            .unwrap()
            .expect("expected actor to exist")
            .state
            .clone()
    }

    fn set_root(&mut self, new: Cid) -> Fallible<()> {
        let addr = Address::new_id(self.to);
        let state_tree = self.call_manager.state_tree_mut();

        state_tree
            .mutate_actor(&addr, |actor_state| {
                actor_state.state = new;
                Ok(())
            })
            .map_err(|e| actor_error!(fatal(e.to_string())))
    }

    fn current_balance(&self) -> Fallible<TokenAmount> {
        todo!()
    }

    fn self_destruct(&mut self, beneficiary: &Address) -> Fallible<()> {
        todo!()
    }
}

impl<B, E> BlockOps for DefaultKernel<B, E>
where
    B: 'static + Blockstore,
    E: 'static + Externs,
{
    fn block_open(&mut self, cid: &Cid) -> Fallible<BlockId, BlockError> {
        let data = self
            .call_manager
            .blockstore()
            .get(cid)
            .map_err(|e| BlockError::Internal(e.into()))?
            .ok_or_else(|| BlockError::MissingState(Box::new(*cid)))?;

        let block = Block::new(cid.codec(), data);
        self.blocks.put(block)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Fallible<BlockId, BlockError> {
        self.blocks.put(Block::new(codec, data))
    }

    fn block_link(
        &mut self,
        id: BlockId,
        hash_fun: u64,
        hash_len: u32,
    ) -> Fallible<Cid, BlockError> {
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
            });
        }
        let k = Cid::new_v1(block.codec, hash.truncate(hash_len as u8));
        // TODO: for now, we _put_ the block here. In the future, we should put it into a write
        // cache, then flush it later.
        // self.call_manager
        //     .blockstore()
        //     .put(&k, block.data())
        //     .map_err(|e| BlockError::Internal(Box::new(e)))?;
        Ok(k)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Fallible<u32, BlockError> {
        let data = &self.blocks.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Fallible<BlockStat, BlockError> {
        self.blocks.get(id).map(|b| BlockStat {
            codec: b.codec(),
            size: b.size(),
        })
    }
}

impl<B, E> MessageOps for DefaultKernel<B, E> {
    fn msg_caller(&self) -> Infallible<ActorID> {
        self.from
    }

    fn msg_receiver(&self) -> Infallible<ActorID> {
        self.to
    }

    fn msg_method_number(&self) -> Infallible<MethodNum> {
        self.msg_method_number()
    }

    // TODO: Remove this? We're currently passing it to invoke.
    fn msg_method_params(&self) -> Infallible<BlockId> {
        // TODO
        0
    }

    fn msg_value_received(&self) -> Infallible<u128> {
        // TODO: we shouldn't have to do this conversion here.
        self.value_received
            .clone()
            .try_into()
            .expect("value received exceeds max filecoin")
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
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    /// XXX: is message the right argument? Most of the fields are unused and unchecked.
    /// Also, won't the params be a block ID?
    fn send(&mut self, message: Message) -> Fallible<RawBytes> {
        self.call_manager.map_mut(|cm| {
            let (res, cm) = cm.send(
                message.to,
                message.method_num,
                message.params,
                message.value,
            );
            // Do something with the result.
            (cm, res)
        })
    }
}

impl<B, E> CircSupplyOps for DefaultKernel<B, E>
where
    E: Externs,
{
    fn total_fil_circ_supply(&self) -> Fallible<TokenAmount> {
        todo!()
    }
}

impl<B, E> CryptoOps for DefaultKernel<B, E> {
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Fallible<()> {
        todo!()
    }

    fn hash_blake2b(&self, data: &[u8]) -> Fallible<[u8; 32]> {
        todo!()
    }

    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Fallible<Cid> {
        todo!()
    }

    fn verify_seal(&self, vi: &SealVerifyInfo) -> Fallible<()> {
        todo!()
    }

    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Fallible<()> {
        todo!()
    }

    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Fallible<Option<ConsensusFault>> {
        todo!()
    }

    fn batch_verify_seals(
        &self,
        vis: &[(&Address, &[SealVerifyInfo])],
    ) -> Fallible<HashMap<Address, Vec<bool>>> {
        todo!()
    }

    fn verify_aggregate_seals(&self, aggregate: &AggregateSealVerifyProofAndInfos) -> Fallible<()> {
        todo!()
    }
}

impl<B, E> GasOps for DefaultKernel<B, E> {
    fn charge_gas(&mut self, name: &str, compute: i64) -> Fallible<()> {
        todo!()
    }
}

impl<B, E> NetworkOps for DefaultKernel<B, E> {
    fn network_curr_epoch(&self) -> ChainEpoch {
        todo!()
    }

    fn network_version(&self) -> NetworkVersion {
        todo!()
    }

    fn network_base_fee(&self) -> &TokenAmount {
        todo!()
    }
}

impl<B, E> RandomnessOps for DefaultKernel<B, E>
where
    B: 'static + Blockstore,
    E: 'static + Externs,
{
    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Fallible<Randomness> {
        todo!()
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Fallible<Randomness> {
        todo!()
    }
}

impl<B, E> ValidationOps for DefaultKernel<B, E> {
    fn validate_immediate_caller_accept_any(&mut self) -> Fallible<()> {
        todo!()
    }

    fn validate_immediate_caller_addr_one_of(&mut self, allowed: &[Address]) -> Fallible<()> {
        todo!()
    }

    fn validate_immediate_caller_type_one_of(&mut self, allowed: &[Cid]) -> Fallible<()> {
        todo!()
    }
}

impl<B, E> ActorOps for DefaultKernel<B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn resolve_address(&self, address: &Address) -> Fallible<Option<Address>> {
        todo!()
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Fallible<Option<Cid>> {
        todo!()
    }

    fn new_actor_address(&mut self) -> Fallible<Address> {
        todo!()
    }

    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Fallible<()> {
        todo!()
    }
}

// TODO provisional, remove once we fix https://github.com/filecoin-project/fvm/issues/107
impl Into<ActorError> for BlockError {
    fn into(self) -> ActorError {
        ActorError::new_fatal(self.to_string())
    }
}
