use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use anyhow::{anyhow, Context};
use cid::Cid;
use num_traits::Zero;
use wasmtime::{Engine, Module};

use blockstore::Blockstore;
use fvm_shared::address::Address;
use fvm_shared::bigint::{BigInt, Sign};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::error::{ActorError, ExitCode};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{actor_error, ActorID};

use crate::account_actor::is_account_actor;
use crate::call_manager::CallManager;
use crate::externs::Externs;
use crate::gas::{price_list_by_epoch, GasCharge, GasOutputs, PriceList};
use crate::kernel::ExecutionError;
use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::{ActorState, StateTree};
use crate::Config;

pub const REWARD_ACTOR_ADDR: Address = Address::new_id(2);
/// Distinguished AccountActor that is the destination of all burnt funds.
pub const BURNT_FUNDS_ACTOR_ADDR: Address = Address::new_id(99);

/// The core of the FVM.
///
/// ## Generic types
/// * B => Blockstore.
/// * E => Externs.
/// * K => Kernel.
pub struct Machine<B: 'static, E: 'static>(Box<MachineState<B, E>>);

#[doc(hidden)]
pub struct MachineState<B: 'static, E: 'static> {
    config: Config,
    /// The context for the execution.
    context: MachineContext,
    /// The wasmtime engine is created on construction of the Machine, and
    /// is dropped when the Machine is dropped.
    engine: Engine,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin node.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    ///
    /// Owned.
    state_tree: StateTree<B>,
}

// These deref impls exist only for internal usage. THere are no public methods or fields on
// MachineState anyways.

#[doc(hidden)]
impl<B: 'static, E: 'static> Deref for Machine<B, E> {
    type Target = MachineState<B, E>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[doc(hidden)]
impl<B: 'static, E: 'static> DerefMut for Machine<B, E> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<B, E> Machine<B, E>
where
    B: Blockstore,
    E: Externs,
{
    pub fn new(
        config: Config,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        network_version: NetworkVersion,
        state_root: Cid,
        blockstore: B,
        externs: E,
    ) -> Result<Machine<B, E>, ExecutionError> {
        let context = MachineContext::new(
            epoch,
            base_fee,
            state_root,
            price_list_by_epoch(epoch),
            network_version,
        );

        // Initialize the WASM engine.
        let engine = Engine::new(&config.engine)?;

        // TODO: fix the error handling to use anyhow up and down the stack, or at least not use
        //  non-send errors in the state-tree.
        let state_tree = StateTree::new_from_root(blockstore, &context.initial_state_root)
            .map_err(|e| anyhow!(e.to_string()))?;

        Ok(Machine::wrap(Box::new(MachineState {
            config,
            context,
            engine,
            externs,
            state_tree,
        })))
    }

    fn wrap(state: Box<MachineState<B, E>>) -> Self {
        Machine(state)
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn config(&self) -> Config {
        self.config.clone()
    }

    pub fn blockstore(&self) -> &B {
        &self.state_tree.store()
    }

    pub fn state_tree(&self) -> &StateTree<B> {
        &self.state_tree
    }

    pub fn state_tree_mut(&mut self) -> &mut StateTree<B> {
        &mut self.state_tree
    }

    /// Creates an uninitialized actor.
    // TODO: Remove
    pub(crate) fn create_actor(
        &mut self,
        addr: &Address,
        act: ActorState,
    ) -> Result<ActorID, ExecutionError> {
        let state_tree = self.state_tree_mut();

        let addr_id = state_tree
            .register_new_address(addr)
            .context("failed to register new address")?;

        state_tree
            .set_actor(&Address::new_id(addr_id), act)
            .context("failed to set actor")?;
        Ok(addr_id)
    }

    pub fn load_module(&self, k: &Cid) -> Result<Module, ExecutionError> {
        // TODO: cache compiled code, and modules?
        todo!("get the actual code");
        let bytecode = &[];
        let module = Module::new(&self.engine, bytecode)?;
        Ok(module)
    }

    /// This is the entrypoint to execute a message.
    pub fn execute_message(
        &mut self,
        msg: Message,
        _: ApplyKind,
    ) -> Result<ApplyRet, ExecutionError> {
        // Validate if the message was correct, charge for it, and extract some preliminary data.
        let (sender_id, gas_cost, inclusion_cost) = match self.preflight_message(&msg)? {
            Ok(res) => res,
            Err(apply_ret) => return Ok(apply_ret),
        };

        // Apply the message.
        let (res, gas_used) = self.map_mut(|machine| {
            let mut cm = CallManager::new(machine, sender_id, msg.gas_limit);
            if let Err(e) = cm.charge_gas(inclusion_cost) {
                return (Err(e), cm.finish().1);
            }

            // BEGIN CRITICAL SECTION: Do not return an error after this line
            cm.state_tree.begin_transaction();

            // Invoke the message.
            let mut res = cm.send(msg.to, msg.method_num, &msg.params, &msg.value);

            // Charge for including the result.
            // We shouldn't put this here, but this is where we can still account for gas.
            // TODO: Maybe CallManager::finish() should return the GasTracker?
            let result = res.and_then(|ret| {
                cm.charge_gas(cm.context().price_list().on_chain_return_value(ret.len()))
                    .map(|_| ret)
                    .map_err(Into::into)
            });

            let (gas_used, machine) = cm.finish();
            (Ok((result, gas_used)), machine)
        })?;

        // Abort or commit the transaction.
        self.state_tree
            .end_transaction(res.is_err())
            .context("failed to end transaction")?;

        // END CRITICAL SECTION

        // Extract the exit code and build the result of the message application.
        let (ret_data, exit_code, err) = match res {
            Ok(ret) => (ret, ExitCode::Ok, None),
            Err(err) => (Default::default(), err.exit_code(), Some(err)),
        };

        let receipt = Receipt {
            exit_code,
            return_data: ret_data,
            gas_used,
        };

        // Finish processing.
        self.finish_message(msg, receipt, err, gas_cost)
    }

    // TODO: The return type here is very strange because we have three cases:
    // 1. Continue (return actor ID & gas).
    // 2. Short-circuit (return ApplyRet).
    // 3. Fail (return an error).
    //
    // We could use custom types, but that would be even more annoying.
    fn preflight_message(
        &mut self,
        msg: &Message,
    ) -> Result<Result<(ActorID, TokenAmount, GasCharge), ApplyRet>, ExecutionError> {
        // TODO sanity check on message, copied from Forest, needs adaptation.
        msg.check()?;

        // TODO I don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = &self.context.price_list;
        let ser_msg = msg.marshal_cbor()?;
        let inclusion_cost = pl.on_chain_message(ser_msg.len());
        let inclusion_total = inclusion_cost.total();

        // Verify the cost of the message is not over the message gas limit.
        if inclusion_total > msg.gas_limit {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrOutOfGas,
                &self.context.base_fee * inclusion_total,
                Some(
                    actor_error!(SysErrOutOfGas; "Out of gas ({} > {})", inclusion_total, msg.gas_limit),
                ),
            )));
        }

        // Load sender actor state.
        let miner_penalty_amount = &self.context.base_fee * msg.gas_limit;

        let sender_id = match self
            .state_tree
            .lookup_id(&msg.from)
            .with_context(|| format!("failed to lookup actor {}", &msg.from))?
        {
            Some(id) => id,
            None => {
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SysErrSenderInvalid,
                    miner_penalty_amount.clone(),
                    Some(actor_error!(SysErrSenderInvalid; "Sender invalid")),
                )))
            }
        };

        let sender = match self
            .state_tree
            .get_actor(&Address::new_id(sender_id))
            .with_context(|| format!("failed to lookup actor {}", &msg.from))?
        {
            Some(act) => act,
            None => {
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SysErrSenderInvalid,
                    miner_penalty_amount.clone(),
                    Some(actor_error!(SysErrSenderInvalid; "Sender invalid")),
                )))
            }
        };

        // If sender is not an account actor, the message is invalid.
        if !is_account_actor(&sender.code) {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrSenderInvalid,
                miner_penalty_amount,
                Some(actor_error!(SysErrSenderInvalid; "send not from account actor")),
            )));
        };

        // Check sequence is correct
        if msg.sequence != sender.sequence {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrSenderStateInvalid,
                miner_penalty_amount,
                Some(
                    actor_error!(SysErrSenderStateInvalid; "actor sequence invalid: {} != {}", msg.sequence, sender.sequence),
                ),
            )));
        };

        // Ensure from actor has enough balance to cover the gas cost of the message.
        let gas_cost: TokenAmount = msg.gas_fee_cap.clone() * msg.gas_limit.clone();
        if sender.balance < gas_cost {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrSenderStateInvalid,
                miner_penalty_amount,
                Some(actor_error!(SysErrSenderStateInvalid;
                    "actor balance less than needed: {} < {}", sender.balance, gas_cost)),
            )));
        }

        // Deduct message inclusion gas cost and increment sequence.
        self.state_tree
            .mutate_actor(&Address::new_id(sender_id), |act| {
                act.deduct_funds(&gas_cost)?;
                act.sequence += 1;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))?;

        Ok(Ok((sender_id, gas_cost, inclusion_cost)))
    }

    pub fn finish_message(
        &mut self,
        msg: Message,
        receipt: Receipt,
        act_err: Option<ExecutionError>,
        gas_cost: BigInt,
    ) -> Result<ApplyRet, ExecutionError> {
        // Make sure the actor error is sane.
        let act_err = match act_err {
            Some(ExecutionError::Actor(err)) => {
                if err.is_fatal() {
                    return Err(anyhow!(
                        "[from={}, to={}, seq={}, m={}, h={}] fatal error: {}",
                        msg.from,
                        msg.to,
                        msg.sequence,
                        msg.method_num,
                        self.context.epoch,
                        err
                    )
                    .into());
                } else if err.is_ok() {
                    return Err(
                        anyhow!("message invocation errored with an ok status: {}", err).into(),
                    );
                } else {
                    Some(err)
                }
            }
            Some(e @ ExecutionError::SystemError(_)) => return Err(e),
            None => None,
        };

        // NOTE: we don't support old network versions in the FVM, so we always burn.
        let GasOutputs {
            base_fee_burn,
            miner_tip,
            over_estimation_burn,
            refund,
            miner_penalty,
            ..
        } = GasOutputs::compute(
            receipt.gas_used,
            msg.gas_limit,
            &self.context.base_fee,
            &msg.gas_fee_cap,
            &msg.gas_premium,
        );

        let mut transfer_to_actor =
            |addr: &Address, amt: &TokenAmount| -> Result<(), ExecutionError> {
                if amt.sign() == Sign::Minus {
                    return Err(anyhow!("attempted to transfer negative value into actor").into());
                }
                if amt.is_zero() {
                    return Ok(());
                }

                self.state_tree
                    .mutate_actor(addr, |act| {
                        act.deposit_funds(amt);
                        Ok(())
                    })
                    .context("failed to lookup actor for transfer")?;
                Ok(())
            };

        transfer_to_actor(&BURNT_FUNDS_ACTOR_ADDR, &base_fee_burn)?;

        transfer_to_actor(&REWARD_ACTOR_ADDR, &miner_tip)?;

        transfer_to_actor(&BURNT_FUNDS_ACTOR_ADDR, &over_estimation_burn)?;

        // refund unused gas
        transfer_to_actor(&msg.from, &refund)?;

        if (&base_fee_burn + over_estimation_burn + &refund + &miner_tip) != gas_cost {
            // Sanity check. This could be a fatal error.
            // XXX: this _is_ a fatal error in the FVM, at the moment at least.
            return Err(anyhow!("Gas handling math is wrong").into());
        }
        Ok(ApplyRet {
            msg_receipt: receipt,
            act_error: act_err,
            penalty: miner_penalty,
            miner_tip,
        })
    }

    pub fn context(&self) -> &MachineContext {
        &self.context
    }

    pub fn externs(&self) -> &E {
        &self.externs
    }

    fn map_mut<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(Self) -> (T, Self),
    {
        replace_with::replace_with_or_abort_and_return(self, f)
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

impl ApplyRet {
    #[inline]
    pub fn prevalidation_fail(
        exit_code: ExitCode,
        miner_penalty: BigInt,
        error: Option<ActorError>,
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
    /// The network version at epoch
    network_version: NetworkVersion,
}

impl MachineContext {
    fn new(
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        state_root: Cid,
        price_list: PriceList,
        network_version: NetworkVersion,
    ) -> MachineContext {
        MachineContext {
            epoch,
            base_fee,
            initial_state_root: state_root,
            price_list: Rc::new(price_list),
            network_version,
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
