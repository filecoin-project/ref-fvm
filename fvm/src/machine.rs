use anyhow::anyhow;
use cid::Cid;
use fvm_shared::encoding::{Cbor, RawBytes};
use num_traits::Zero;
use wasmtime::{Engine, Linker};

use blockstore::Blockstore;
use fvm_shared::actor_error;
use fvm_shared::bigint::BigInt;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ActorError, ExitCode};

use crate::externs::Externs;
use crate::gas::price_list_by_epoch;
use crate::kernel::Kernel;
use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::StateTree;
use crate::syscalls::bind_syscalls;
use crate::Config;

/// The core of the FVM.
///
/// ## Generic types
/// * B => Blockstore.
/// * E => Externs.
/// * K => Kernel.
pub struct Machine<'db, B, E, K> {
    config: Config,
    /// The context for the execution.
    context: MachineContext,
    /// The wasmtime engine is created on construction of the Machine, and
    /// is dropped when the Machine is dropped.
    engine: Engine,
    /// The linker used to store wasm functions.
    /// TODO: This probably needs to be per-invocation?
    linker: Linker<K>,
    /// Blockstore to use for this machine instance.
    blockstore: &'db B,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin node.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    state_tree: StateTree<'db, B>,
    /// The buffer of blocks to be committed to the blockstore after
    /// execution concludes.
    /// TODO @steb needs to figure out how all of this is going to work.
    commit_buffer: (),
    /// Placeholder to maybe keep a reference to FullVerifier (Forest) here.
    /// The FullVerifier is the gateway to filecoin-proofs-api.
    /// TODO these likely go in the kernel, as they are syscalls that can be
    /// resolved inside the FVM without traversing Boundary A.
    // verifier: PhantomData<V>,
    /// The kernel template
    /// TODO likely will need to be cloned and "connected" to the context with every invocation container
    kernel: K,
    /// The currently active call stack.
    /// TODO I don't think we need to store this in the state; it can probably
    /// be a stack variable in execute_message.
    /// @steb says we _can't_ store this state.
    call_stack: CallStack<'db, B>,
}

impl<'db, B, E, K> Machine<'db, B, E, K>
where
    B: Blockstore,
    E: Externs,
    K: Kernel + 'static,
{
    pub fn new(
        config: Config,
        context: MachineContext,
        blockstore: &'db B,
        externs: E,
        kernel: K,
    ) -> anyhow::Result<Machine<'db, B, E, K>> {
        let mut engine = Engine::new(&config.engine)?;
        let mut linker = Linker::new(&engine);
        bind_syscalls(&mut linker); // TODO turn into a trait so we can do Linker::new(&engine).with_bound_syscalls();

        // Initialize the WASM engine.
        // TODO initialize the engine
        // TODO instantiate state tree with root and blockstore.
        // TODO load the gas_list for this epoch, and give it to the kernel.
        // TODO instantiate the Kernel template.

        // TODO: fix the error handling to use anyhow up and down the stack, or at least not use
        // non-send errors in the state-tree.
        let state_tree = StateTree::new_from_root(blockstore, &context.state_root)
            .map_err(|e| anyhow!(e.to_string()))?;

        Ok(Machine {
            config,
            linker,
            context,
            engine,
            externs,
            blockstore,
            kernel,
            state_tree,
            commit_buffer: Default::default(), // @stebalien TBD
            call_stack: todo!(),               // TODO implement constructor.
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn config(&self) -> Config {
        self.config.clone()
    }

    /// This is the entrypoint to execute a message.
    pub fn execute_message(self, msg: Message, kind: ApplyKind) -> anyhow::Result<ApplyRet> {
        // TODO sanity check on message, copied from Forest, needs adaptation.
        msg.check()?;

        // TODO I don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = price_list_by_epoch(self.context.epoch);
        let ser_msg = msg.marshal_cbor()?;
        let msg_gas_cost = pl.on_chain_message(ser_msg.len());
        let cost_total = msg_gas_cost.total();

        // Verify the cost of the message is not over the message gas limit.
        // TODO handle errors properly
        if cost_total > msg.gas_limit {
            return Ok(ApplyRet {
                msg_receipt: Receipt {
                    // TODO: Eventually, this will be an arbitrary IPLD block.
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrOutOfGas,
                    gas_used: 0,
                },
                act_error: Some(actor_error!(SysErrOutOfGas;
                    "Out of gas ({} > {})", cost_total, msg.gas_limit)),
                penalty: &self.context.base_fee * cost_total,
                miner_tip: BigInt::zero(),
            });
        }

        // TODO instantiate a CallStack and make it run.
        // TODO once the CallStack finishes running, copy over the resulting state tree layer to the Machine's state tree
        // TODO pull the receipt from the CallStack and return it.
        // Ok(Default::default())
        todo!("return the receipt")
    }
}

/// Apply message return data.
#[derive(Clone, Debug)]
pub struct ApplyRet {
    /// Message receipt for the transaction. This data is stored on chain.
    pub msg_receipt: Receipt,
    /// Actor error from the transaction, if one exists.
    pub act_error: Option<ActorError>,
    /// Gas penalty from transaction, if any.
    pub penalty: BigInt,
    /// Tip given to miner from message.
    pub miner_tip: BigInt,
}

pub struct CallStack<'a, B> {
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
    // TODO figure out what else needs to be here.
}

impl<'a, B> CallStack<'_, B>
where
    B: Blockstore,
{
    fn call_next(&self, msg: Message) -> anyhow::Result<()> {
        // TODO TBD signature is not complete.
        Ok(())
    }

    // TODO need accessors to check the outcome, and merge this state tree onto
    // the machine's state tree.
}

pub enum ApplyKind {
    Explicit,
    Implicit,
}

/// Execution context supplied to the machine. All fields are private.
/// Epoch and base fee cannot be mutated. The state_root corresponds to the
/// initial state root, and gets updated internally with every message execution.
pub struct MachineContext {
    /// The epoch at which the Machine runs.
    epoch: ChainEpoch,
    /// The base fee that's in effect when the Machine runs.
    base_fee: BigInt,
    state_root: Cid,
}

impl MachineContext {
    fn new(epoch: ChainEpoch, base_fee: TokenAmount, state_root: Cid) -> MachineContext {
        MachineContext {
            epoch,
            base_fee,
            state_root,
        }
    }

    pub fn epoch(self) -> ChainEpoch {
        self.epoch
    }

    pub fn base_fee(self) -> TokenAmount {
        self.base_fee
    }

    pub fn state_root(self) -> Cid {
        self.state_root
    }

    fn set_state_root(&mut self, state_root: Cid) {
        self.state_root = state_root
    }
}
