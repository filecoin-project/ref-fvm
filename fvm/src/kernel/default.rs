use std::collections::VecDeque;
use std::convert::TryFrom;

use anyhow::{anyhow, Result};
use cid::Cid;
use derive_getters::Getters;
use wasmtime::{Linker, Module, Store};

use blockstore::Blockstore;
use fvm_shared::encoding::DAG_CBOR;
use fvm_shared::error::ActorError;
use fvm_shared::ActorID;

use crate::externs::Externs;
use crate::gas::GasTracker;
use crate::machine::Machine;
use crate::message::Message;
use crate::state_tree::StateTree;
use crate::syscalls::bind_syscalls;

use super::blocks::{Block, BlockRegistry};
use super::*;

/// Tracks data accessed and modified during the execution of a message.
///
/// TODO writes probably ought to be scoped by invocation container.
pub struct DefaultKernel<B: 'static, E: 'static> {
    /// The environment attachments to this kernel. If Some, this kernel is
    /// considered attached and active. If None, this kernel is inactive and
    /// cannot be used.
    ///
    /// As kernels are spun up and unwound, the attachment travels with them.
    attachment: MapCell<KernelAttachment<B, E>>,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    blocks: BlockRegistry,
    /// Return stack where values returned by syscalls are stored for consumption.
    return_stack: VecDeque<Vec<u8>>,
}

#[derive(Getters)]
pub struct KernelAttachment<B: 'static, E: 'static> {
    /// The machine this kernel is attached to.
    machine: Box<Machine<B, E>>,
    /// The gas tracker.
    gas_tracker: Box<GasTracker>,
    /// The message being processed by the invocation container to which this
    /// kernel is bound.
    message: Message,
}

impl<B, E> KernelAttachment<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    fn state_tree(&self) -> &StateTree<'static, B> {
        self.machine.state_tree()
    }

    fn state_tree_mut(&mut self) -> &mut StateTree<'static, B> {
        self.machine.state_tree_mut()
    }
}

pub struct InvocationResult {
    return_bytes: Vec<u8>,
    error: Option<ActorError>,
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
    pub fn unattached() -> Self {
        DefaultKernel {
            attachment: MapCell::empty(),
            blocks: BlockRegistry::new(),
            return_stack: Default::default(),
        }
    }

    /// Execute initiates a call stack to handle a message, using the provided
    /// Machine and pre-initialized GasTracker. This method will not charge
    /// the cost of message inclusion. So for correctness, the caller must have
    /// charged that cost before calling this method.
    ///
    /// This method consumes all arguments provided, including itself. This
    /// means that a DefaultKernel can be used only once.
    ///
    /// Because it's likely that the application will want to apply more than
    /// one message, ownership of the Machine is relinquished and returned once
    /// the call stack concludes.
    pub fn execute(
        mut self,
        machine: Box<Machine<B, E>>,
        gas_tracker: GasTracker,
        bytecode: &[u8],
        mut msg: Message,
    ) -> (Result<InvocationResult>, Box<Machine<B, E>>) {
        // TODO check that not reentrant into the same kernel (i.e. we can't run
        //  another invocation container without stashing and recursing).

        assert!(self.attachment.is_empty());

        // This is a cheap operation as it doesn't actually clone the struct,
        // it returns a referenced copy.
        let engine = machine.engine().clone();

        msg = self.replace_id_addrs(msg);

        // Inject the message parameters as a block in the block registry.
        let params_block_id = match self.block_create(DAG_CBOR, msg.params.bytes()) {
            Ok(v) => v,
            Err(e) => return (Err(e.into()), machine),
        };

        let attachment = KernelAttachment {
            machine,
            gas_tracker: Box::new(gas_tracker),
            message: msg,
        };

        self.attachment.set(attachment);
        let mut store = Store::new(&engine, self);

        let result = || -> Result<InvocationResult> {
            // Instantiate the module with the supplied bytecode.
            let module = Module::new(&engine, bytecode)?;

            // Create a new linker.
            // TODO: move this to arguments so it can be reused and supplied by the machine?
            let mut linker = Linker::new(&engine);
            bind_syscalls(&mut linker);

            let instance = linker.instantiate(&mut store, &module)?;
            let invoke = instance.get_typed_func(&mut store, "invoke")?;
            let (return_block_id,): (u32,) = invoke.call(&mut store, (params_block_id))?;

            Ok(InvocationResult {
                return_bytes: vec![],
                error: None,
            })
        }();

        // Destroy the store by consuming it, we're done with it; get the Machine back out.
        let k = store.into_data();
        let machine = k.attachment.take().machine;

        (result, machine)
    }

    // TODO: We should be constructing the kernel with pre-looked-up addresses. That'll make this
    // much easier.
    fn replace_id_addrs(&mut self, mut msg: Message) -> Message {
        let state_tree = self.attachment.state_tree();

        msg.from = state_tree
            .lookup_id(&msg.from)
            .expect("failed to convert from address to id address")
            .expect("from address has no id");

        msg.to = state_tree
            .lookup_id(&msg.to)
            .expect("failed to convert to address to id address")
            .expect("to address has no id");

        msg
    }

    //     pub fn try_create_account_actor(
    //         &mut self,
    //         addr: &Address,
    //     ) -> Result<(ActorState, Address), ActorError> {
    //         let attachment = self.attachment.as_mut().expect("unattached kernel");
    //         let machine: &mut Machine<B, E> = attachment.machine.borrow_mut();
    //         let gas_tracker: &mut GasTracker = attachment.gas_tracker.borrow_mut();
    //
    //         let mut state_tree = machine.state_tree_mut();
    //
    //         gas_tracker.charge_gas(machine.context().price_list().on_create_actor())?;
    //
    //         if addr.is_bls_zero_address() {
    //             actor_error!(SysErrIllegalArgument; "cannot create the bls zero address actor");
    //         }
    //
    //         let addr_id = state_tree
    //             .register_new_address(addr)
    //             .map_err(|e| e.downcast_fatal("failed to register new address"))?;
    //
    //         let act = crate::account_actor::ZERO_STATE.clone();
    //
    //         state_tree
    //             .set_actor(&addr_id, act)
    //             .map_err(|e| e.downcast_fatal("failed to set actor"))?;
    //
    //         let params = RawBytes::serialize(&addr).map_err(|e| {
    //             actor_error!(fatal(
    //                 "couldn't serialize params for actor construction: {:?}",
    //                 e
    //             ))
    //         })?;
    //
    //         let msg = Message {
    //             from: *crate::account_actor::SYSTEM_ACTOR_ADDR,
    //             to: addr.clone(),
    //             method_num: fvm_shared::METHOD_CONSTRUCTOR,
    //             value: TokenAmount::from(0_u32),
    //             params,
    //             gas_limit: gas_tracker.gas_available(),
    //             version: Default::default(),
    //             sequence: Default::default(),
    //             gas_fee_cap: Default::default(),
    //             gas_premium: Default::default(),
    //         };
    //
    //         let mut next_kernel = self.stash(msg);
    //         next_kernel.run_invocation_container(&[]); // TODO get bytecode.
    //         *self = *next_kernel.finish().expect("missing previous kernel"); // restore our kernel.
    //
    //         // TODO referencing the old state_tree is safe?
    //         let act = state_tree
    //             .get_actor(&addr_id)
    //             .map_err(|e| e.downcast_fatal("failed to get actor"))?
    //             .ok_or_else(|| actor_error!(fatal("failed to retrieve created actor state")))?;
    //
    //         Ok((act, addr_id))
    //     }
}

impl<B, E> ActorOps for DefaultKernel<B, E>
where
    B: Blockstore + 'static,
    E: Externs + 'static,
{
    fn root(&self) -> Cid {
        let attachment = &self.attachment;
        let addr = attachment.message().to;
        let state_tree = attachment.state_tree();

        state_tree
            .get_actor(&addr)
            .unwrap()
            .expect("expected actor to exist")
            .state
            .clone()
    }

    fn set_root(&mut self, new: Cid) -> Result<()> {
        let addr = self.attachment.message().to;
        let state_tree = self.attachment.state_tree_mut();

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
            .attachment
            .machine()
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
        // self.attachment
        //     .as_ref()
        //     .expect("no attachment")
        //     .machine
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
        self.attachment.message().method_num
    }

    fn method_params(&self) -> BlockId {
        // TODO
        0
    }

    fn caller(&self) -> ActorID {
        self.attachment
            .message()
            .from
            .id()
            .expect("invocation from address was not an ID address")
    }

    fn receiver(&self) -> ActorID {
        self.attachment
            .message()
            .to
            .id()
            .expect("invocation to address was not an ID address")
    }

    fn value_received(&self) -> u128 {
        // TODO @steb
        // self.invocation_msg.value.into()
        0
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
