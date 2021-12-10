use std::collections::VecDeque;
use std::convert::{TryFrom, TryInto};

use actor::ActorDowncast;
use anyhow::{anyhow, Result};
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
    from: ActorID,
    to: ActorID,
    method: MethodId,
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

pub struct InvocationResult {
    pub return_bytes: Vec<u8>,
    pub error: Option<ActorError>,
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
        method: MethodId,
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

impl<B, E> ActorOps for DefaultKernel<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
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

        state_tree
            .mutate_actor(&addr, |actor_state| {
                actor_state.state = new;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))
    }
}

impl<B, E> BlockOps for DefaultKernel<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, BlockError> {
        let data = self
            .call_manager
            .blockstore()
            .get(cid)
            .map_err(|e| BlockError::Internal(e.into()))?
            .ok_or_else(|| BlockError::MissingState(Box::new(*cid)))?;

        let block = Block::new(cid.codec(), data);
        self.blocks.put(block)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, BlockError> {
        self.blocks.put(Block::new(codec, data))
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, BlockError> {
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

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, BlockError> {
        let data = &self.blocks.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat, BlockError> {
        self.blocks.get(id).map(|b| BlockStat {
            codec: b.codec(),
            size: b.size(),
        })
    }
}

impl<B, E> InvocationOps for DefaultKernel<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    fn method_number(&self) -> MethodId {
        self.method_number()
    }

    // TODO: Remove this? We're currently passing it to invoke.
    fn method_params(&self) -> BlockId {
        // TODO
        0
    }

    fn caller(&self) -> ActorID {
        self.from
    }

    fn receiver(&self) -> ActorID {
        self.to
    }

    fn value_received(&self) -> u128 {
        // TODO: we shouldn't have to do this conversion here.
        self.value_received
            .clone()
            .try_into()
            .expect("value received exceeds max filecoin")
    }
}

impl<B, E> ReturnOps for DefaultKernel<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
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
    fn send(&mut self, message: Message) -> anyhow::Result<()> {
        // self.call_manager.map_mut(|cm| {
        //     let (res, cm) = cm.send(
        //         message.to,
        //         message.method_num,
        //         message.params,
        //         message.value,
        //     );
        //     // Do something with the result.
        //     todo!();
        //     cm
        // })
        todo!()
    }
}

// TODO provisional, remove once we fix https://github.com/filecoin-project/fvm/issues/107
impl Into<ActorError> for BlockError {
    fn into(self) -> ActorError {
        ActorError::new_fatal(self.to_string())
    }
}
