use std::borrow::BorrowMut;

use std::rc::Rc;

use actor::CallErrorConversions;
use anyhow::anyhow;
use cid::Cid;
use fvm_shared::address::Address;
use num_traits::Zero;
use wasmtime::{Engine, Module};

use blockstore::Blockstore;
use fvm_shared::bigint::BigInt;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::error::{CallError, ExitCode};
use fvm_shared::{call_error, ActorID};

use crate::externs::Externs;
use crate::gas::{price_list_by_epoch, GasCharge, GasTracker, PriceList};

use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::{ActorState, StateTree};

use crate::{Config, DefaultKernel};

use crate::call_manager::CallManager;

/// The core of the FVM.
///
/// ## Generic types
/// * B => Blockstore.
/// * E => Externs.
/// * K => Kernel.
pub struct Machine<B: 'static, E: 'static> {
    config: Config,
    /// The context for the execution.
    context: MachineContext,
    /// The wasmtime engine is created on construction of the Machine, and
    /// is dropped when the Machine is dropped.
    engine: Engine,
    /// Blockstore to use for this machine instance and all kernels
    /// constructed under it.
    blockstore: &'static B,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin node.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    ///
    /// Owned.
    state_tree: StateTree<'static, B>,
    /// The buffer of blocks to be committed to the blockstore after
    /// execution concludes.
    /// TODO @steb needs to figure out how all of this is going to work.
    commit_buffer: (),
}

impl<B, E> Machine<B, E>
where
    B: Blockstore + 'static,
    E: Externs,
{
    pub fn new(
        config: Config,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        state_root: Cid,
        blockstore: &'static B,
        externs: E,
    ) -> anyhow::Result<Machine<B, E>> {
        let context = MachineContext::new(epoch, base_fee, state_root, price_list_by_epoch(epoch));

        // Initialize the WASM engine.
        let engine = Engine::new(&config.engine)?;

        // TODO: fix the error handling to use anyhow up and down the stack, or at least not use
        //  non-send errors in the state-tree.
        let state_tree = StateTree::new_from_root(blockstore, &context.initial_state_root)
            .map_err(|e| anyhow!(e.to_string()))?;

        Ok(Machine {
            config,
            context,
            engine,
            externs,
            blockstore,
            state_tree,
            commit_buffer: Default::default(), // @stebalien TBD
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn config(&self) -> Config {
        self.config.clone()
    }

    pub fn blockstore(&self) -> &B {
        &self.blockstore
    }

    pub fn state_tree(&self) -> &StateTree<'static, B> {
        &self.state_tree
    }

    pub fn state_tree_mut(&mut self) -> &mut StateTree<'static, B> {
        self.state_tree.borrow_mut()
    }

    /// Creates an uninitialized actor.
    // TODO: Remove
    pub(crate) fn create_actor(
        &mut self,
        addr: &Address,
        act: ActorState,
    ) -> Result<ActorID, CallError> {
        let mut state_tree = self.state_tree_mut();

        let addr_id = state_tree
            .register_new_address(addr)
            .map_err(|e| e.convert_fatal("failed to register new address"))?;

        state_tree
            .set_actor(&Address::new_id(addr_id), act)
            .map_err(|e| e.convert_fatal("failed to set actor"))?;
        Ok(addr_id)
    }

    pub fn load_module(&self, k: &Cid) -> anyhow::Result<Module> {
        // TODO: cache compiled code, and modules?
        todo!("get the actual code");
        let bytecode = &[];
        Module::new(&self.engine, bytecode)
    }

    /// This is the entrypoint to execute a message.
    pub fn execute_message(
        mut self: Box<Self>,
        msg: Message,
        _: ApplyKind,
    ) -> (anyhow::Result<ApplyRet>, Box<Self>) {
        macro_rules! t {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(e) => return (Err(e.into()), self),
                }
            };
        }
        // TODO return self.
        // TODO sanity check on message, copied from Forest, needs adaptation.
        t!(msg.check());

        // TODO I don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = &self.context.price_list;
        let ser_msg = t!(msg.marshal_cbor());
        let inclusion_cost = pl.on_chain_message(ser_msg.len());

        // Validate if the message
        // TODO I don't like the Option return value here.
        if let Some(ret) = self.validate_message(&msg, inclusion_cost.total()) {
            return (Ok(ret), self);
        }

        // TODO we calculate this twice, in validate_message and here.
        // Ensure from actor has enough balance to cover the gas cost of the message.
        let gas_cost: TokenAmount = msg.gas_fee_cap * msg.gas_limit;

        // Deduct message inclusion gas cost and increment sequence.
        // XXX: We need to charge the gas for the whole message here. That's base_fee * total cost.
        t!(self
            .state_tree
            .mutate_actor(&msg.from, |act| {
                act.deduct_funds(&gas_cost)?;
                act.sequence += 1;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string())));

        t!(self.state_tree.snapshot().map_err(anyhow::Error::msg));

        todo!("resolve actor ID");

        /// TODO this requires the ActorID; need to resolve it first.
        let mut cm = CallManager::new(self, 0, msg.gas_limit);
        t!(cm.charge_gas(inclusion_cost));

        // Invoke the message.
        // TODO: We need some macro/monad help here.
        let (res, cm) = cm.send(msg.to, msg.method_num, msg.params, msg.value);
        let (gas_used, s) = cm.finish();
        self = s;
        let result = t!(res);

        // XXXX steb left off here

        // Perform state transition
        // // TODO: here is where we start the call stack and the invocation container.
        // let (mut ret_data, rt, mut act_err) = self.send(msg.message(), Some(msg_gas_cost));
        // if let Some(err) = &act_err {
        //     if err.is_fatal() {
        //         return Err(format!(
        //             "[from={}, to={}, seq={}, m={}, h={}] fatal error: {}",
        //             msg.from(),
        //             msg.to(),
        //             msg.sequence(),
        //             msg.method_num(),
        //             self.epoch,
        //             err
        //         ));
        //     } else {
        //         debug!(
        //             "[from={}, to={}, seq={}, m={}] send error: {}",
        //             msg.from(),
        //             msg.to(),
        //             msg.sequence(),
        //             msg.method_num(),
        //             err
        //         );
        //         if !ret_data.is_empty() {
        //             return Err(format!(
        //                 "message invocation errored, but had a return value anyway: {}",
        //                 err
        //             ));
        //         }
        //     }
        // }

        // let gas_used = if let Some(mut rt) = rt {
        //     if !ret_data.is_empty() {
        //         if let Err(e) = rt.charge_gas(rt.price_list().on_chain_return_value(ret_data.len()))
        //         {
        //             act_err = Some(e);
        //             ret_data = Serialized::default();
        //         }
        //     }
        //     if rt.gas_used() < 0 {
        //         0
        //     } else {
        //         rt.gas_used()
        //     }
        // } else {
        //     return Err(format!("send returned None runtime: {:?}", act_err));
        // };
        //
        // let err_code = if let Some(err) = &act_err {
        //     if !err.is_ok() {
        //         // Revert all state changes on error.
        //         self.state.revert_to_snapshot()?;
        //     }
        //     err.exit_code()
        // } else {
        //     ExitCode::Ok
        // };
        //
        // let should_burn = self
        //     .should_burn(self.state(), msg, err_code)
        //     .map_err(|e| format!("failed to decide whether to burn: {}", e))?;
        //
        // let GasOutputs {
        //     base_fee_burn,
        //     miner_tip,
        //     over_estimation_burn,
        //     refund,
        //     miner_penalty,
        //     ..
        // } = compute_gas_outputs(
        //     gas_used,
        //     msg.gas_limit(),
        //     &self.base_fee,
        //     msg.gas_fee_cap(),
        //     msg.gas_premium().clone(),
        //     should_burn,
        // );
        //
        // let mut transfer_to_actor = |addr: &Address, amt: &TokenAmount| -> Result<(), String> {
        //     if amt.sign() == Sign::Minus {
        //         return Err("attempted to transfer negative value into actor".into());
        //     }
        //     if amt.is_zero() {
        //         return Ok(());
        //     }
        //
        //     self.state
        //         .mutate_actor(addr, |act| {
        //             act.deposit_funds(amt);
        //             Ok(())
        //         })
        //         .map_err(|e| e.to_string())?;
        //     Ok(())
        // };
        //
        // transfer_to_actor(&*BURNT_FUNDS_ACTOR_ADDR, &base_fee_burn)?;
        //
        // transfer_to_actor(&**reward::ADDRESS, &miner_tip)?;
        //
        // transfer_to_actor(&*BURNT_FUNDS_ACTOR_ADDR, &over_estimation_burn)?;
        //
        // // refund unused gas
        // transfer_to_actor(msg.from(), &refund)?;
        //
        // if &base_fee_burn + over_estimation_burn + &refund + &miner_tip != gas_cost {
        //     // Sanity check. This could be a fatal error.
        //     return Err("Gas handling math is wrong".to_owned());
        // }
        // self.state.clear_snapshot()?;
        //
        // Ok(ApplyRet {
        //     msg_receipt: MessageReceipt {
        //         return_data: ret_data,
        //         exit_code: err_code,
        //         gas_used,
        //     },
        //     penalty: miner_penalty,
        //     act_error: act_err,
        //     miner_tip,
        // })

        // TODO once the CallStack finishes running, copy over the resulting state tree layer to the Machine's state tree
        // TODO pull the receipt from the CallStack and return it.
        // Ok(Default::default())
        todo!()
    }

    // TODO probably should return a validation failure.
    fn validate_message(&mut self, msg: &Message, cost_total: i64) -> Option<ApplyRet> {
        // Verify the cost of the message is not over the message gas limit.
        // TODO handle errors properly
        if cost_total > msg.gas_limit {
            let err =
                call_error!(SysErrOutOfGas; "Out of gas ({} > {})", cost_total, msg.gas_limit);
            return Some(ApplyRet::prevalidation_fail(
                ExitCode::SysErrOutOfGas,
                &self.context.base_fee * cost_total,
                Some(err),
            ));
        }

        // Load sender actor state.
        let miner_penalty_amount = &self.context.base_fee * msg.gas_limit;
        let sender = match self.state_tree.get_actor(&msg.from) {
            Ok(Some(sender)) => sender,
            _ => {
                return Some(ApplyRet {
                    msg_receipt: Receipt {
                        return_data: RawBytes::default(),
                        exit_code: ExitCode::SysErrSenderInvalid,
                        gas_used: 0,
                    },
                    penalty: miner_penalty_amount,
                    act_error: Some(call_error!(SysErrSenderInvalid; "Sender invalid")),
                    miner_tip: BigInt::zero(),
                });
            }
        };

        // If sender is not an account actor, the message is invalid.
        if !actor::is_account_actor(&sender.code) {
            return Some(ApplyRet {
                msg_receipt: Receipt {
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrSenderInvalid,
                    gas_used: 0,
                },
                penalty: miner_penalty_amount,
                act_error: Some(call_error!(SysErrSenderInvalid; "send not from account actor")),
                miner_tip: BigInt::zero(),
            });
        };

        // Check sequence is correct
        if msg.sequence != sender.sequence {
            return Some(ApplyRet {
                msg_receipt: Receipt {
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrSenderStateInvalid,
                    gas_used: 0,
                },
                penalty: miner_penalty_amount,
                act_error: Some(call_error!(SysErrSenderStateInvalid;
                    "actor sequence invalid: {} != {}", msg.sequence, sender.sequence)),
                miner_tip: BigInt::zero(),
            });
        };

        // Ensure from actor has enough balance to cover the gas cost of the message.
        let gas_cost: TokenAmount = msg.gas_fee_cap.clone() * msg.gas_limit.clone();
        if sender.balance < gas_cost {
            return Some(ApplyRet {
                msg_receipt: Receipt {
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrSenderStateInvalid,
                    gas_used: 0,
                },
                penalty: miner_penalty_amount,
                act_error: Some(call_error!(SysErrSenderStateInvalid;
                    "actor balance less than needed: {} < {}", sender.balance, gas_cost)),
                miner_tip: BigInt::zero(),
            });
        };
        None
    }

    pub fn context(&self) -> &MachineContext {
        &self.context
    }

    pub fn externs(&self) -> &E {
        &self.externs
    }
}

/// Apply message return data.
#[derive(Clone, Debug)]
pub struct ApplyRet {
    /// Message receipt for the transaction. This data is stored on chain.
    pub msg_receipt: Receipt,
    /// Actor error from the transaction, if one exists.
    pub act_error: Option<CallError>,
    /// Gas penalty from transaction, if any.
    pub penalty: BigInt,
    /// Tip given to miner from message.
    pub miner_tip: BigInt,
}

impl ApplyRet {
    #[inline]
    pub fn prevalidation_fail(
        exit_code: ExitCode,
        miner_penalty: BigInt,
        error: Option<CallError>,
    ) -> ApplyRet {
        ApplyRet {
            msg_receipt: Receipt {
                exit_code,
                return_data: RawBytes::default(),
                gas_used: 0,
            },
            penalty: miner_penalty,
            act_error: error,
            miner_tip: BigInt::zero(),
        }
    }
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
    base_fee: TokenAmount,
    /// The initial state root.
    initial_state_root: Cid,
    /// The price list.
    price_list: Rc<PriceList>,
}

impl MachineContext {
    fn new(
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        state_root: Cid,
        price_list: PriceList,
    ) -> MachineContext {
        MachineContext {
            epoch,
            base_fee,
            initial_state_root: state_root,
            price_list: Rc::new(price_list),
        }
    }

    pub fn epoch(self) -> ChainEpoch {
        self.epoch
    }

    pub fn base_fee(&self) -> &TokenAmount {
        &self.base_fee
    }

    pub fn state_root(&self) -> Cid {
        self.initial_state_root
    }

    pub fn price_list(&self) -> Rc<PriceList> {
        self.price_list.clone()
    }

    pub fn set_state_root(&mut self, state_root: Cid) {
        self.initial_state_root = state_root
    }
}
