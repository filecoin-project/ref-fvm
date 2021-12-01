use super::Config;
use crate::invocation::InvocationContainer;
use crate::externs::{CircSupplyCalc, Externs, LookbackStateGetter, Rand};
use crate::state_tree::StateTree;
use blockstore::Blockstore;
use cid::Cid;
use fvm_shared::bigint::BigInt;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use wasmtime::{Engine, Instance, Linker, Store};
use fvm_shared::encoding::Cbor;
use crate::exit_code::ExitCode;
use crate::gas::price_list_by_epoch;
use crate::kernel::{DefaultKernel, Kernel};
use crate::message::Message;
use crate::receipt::Receipt;
use crate::syscalls::bind_syscalls;

/// The core of the FVM.
pub struct Machine<'a, B: Blockstore, E: Externs<B>, K: Kernel> {
    config: Config,
    /// The wasmtime engine is created on construction of the Machine, and
    /// is dropped when the Machine is dropped.
    engine: Engine,
    /// The epoch at which the Machine runs.
    epoch: ChainEpoch,
    /// The base fee that's in effect when the Machine runs.
    base_fee: BigInt,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin node.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    state_tree: StateTree<'a, B>,
    /// The buffer of blocks to be committed to the blockstore after
    /// execution concludes.
    /// TODO @steb needs to figure out how all of this is going to work.
    commit_buffer: (),
    /// Placeholder to maybe keep a reference to FullVerifier (Forest) here.
    /// The FullVerifier is the gateway to filecoin-proofs-api.
    /// TODO these likely go in the kernel, as they are syscalls that can be
    /// resolved inside the FVM without traversing Boundary A.
    verifier: PhantomData<V>,
    /// The kernel template
    /// TODO likely will need to be cloned and "connected" to the context with every invocation container
    kernel: K,
    /// The currently active call stack.
    /// TODO I don't think we need to store this in the state; it can probably
    /// be a stack variable in execute_message.
    call_stack: CallStack<'a, B>,
}

pub struct CallStack<'a, B: Blockstore> {
    /// The buffer of blocks that that a given message execution has written.
    /// Reachable blocks from the updated state roots of actors touched by the
    /// call stack will probably need to be transferred to the Machine's
    /// commit_buffer.
    /// TODO @steb needs to figure out how all of this is going to work.
    write_buffer: (),
    /// The invocation container stack.
    /// TODO likely don't need to retain it in state!
    // instances: VecDeque<InvocationContainer>,
    /// A state tree stacked on top of the Machine state tree, tracking state
    /// changes performed by actors throughout a call stack.
    state_tree: StateTree<'a, B>,

    /// TODO figure out what else needs to be here.
}

#[derive(Default)]
impl CallStack<B> {
    fn call_next(&self, msg: Message) -> thiserror::Result {
        // TODO TBD signature is not complete.
    }

    // TODO need accessors to check the outcome, and merge this state tree onto
    // the machine's state tree.
}

pub enum ApplyKind {
    Explicit,
    Implicit,
}

impl<'a, B, E, K> Machine<'a, B, E, K> where
    B: Blockstore,
    E: Externs<B>,
    K: Kernel,
{
    // TODO add all constructor arguments.
    pub fn new(
        config: Config,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        blockstore: B,
        externs: E,
        state_root: Cid,
    ) -> Machine<'a, B, E, K> {
        let mut engine = Engine::new(&config.engine)?;
        let mut linker = Linker::new(&engine);
        bind_syscalls(linker); // TODO turn into a trait so we can do Linker::new(&engine).with_bound_syscalls();

        // TODO initialize the engine
        // TODO instantiate state tree with root and blockstore.
        // TODO load the gas_list for this epoch, and give it to the kernel.
        // TODO instantiate the Kernel template.

        Machine {
            config,
            epoch,
            base_fee,
            engine,
            externs,
            state_tree: StateTree::new_from_root(store, &state_root)?,
            commit_buffer: Default::default(), // @stebalien TBD
            verifier: Default::default(),
            kernel: Default::default(), // TODO implement constructor.
            call_stack: Default::default(), // TODO implement constructor.
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn config(&self) -> Config {
        self.config
    }

    /// This is the main entrypoint from the node into the VM. The node
    /// requests the VM to apply the provided message.
    pub fn execute_message(self, msg: Message, kind: ApplyKind) -> thiserror::Result<(Receipt)> {
        // TODO sanity check on message, copied from Forest, needs adaptation.
        check_message(msg.message())?;

        // TODO I don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = price_list_by_epoch(self.epoch());
        let ser_msg = msg.marshal_cbor().map_err(|e| e.to_string())?;
        let msg_gas_cost = pl.on_chain_message(ser_msg.len());
        let cost_total = msg_gas_cost.total();

        // Verify the cost of the message is not over the message gas limit.
        // TODO handle errors properly
        if cost_total > msg.gas_limit() {
            return Ok(ApplyRet {
                msg_receipt: MessageReceipt {
                    return_data: Serialized::default(),
                    exit_code: ExitCode::SysErrOutOfGas,
                    gas_used: 0,
                },
                act_error: Some(actor_error!(SysErrOutOfGas;
                    "Out of gas ({} > {})", cost_total, msg.gas_limit())),
                penalty: &self.base_fee * cost_total,
                miner_tip: BigInt::zero(),
            });
        }

        // TODO instantiate a CallStack and make it run.
        // TODO once the CallStack finishes running, copy over the resulting state tree layer to the Machine's state tree
        // TODO pull the receipt from the CallStack and return it.
    }
}
