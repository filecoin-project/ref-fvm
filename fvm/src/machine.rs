use std::convert::TryFrom;
use std::rc::Rc;

use actor::{ActorDowncast, BURNT_FUNDS_ACTOR_ADDR, REWARD_ACTOR_ADDR};
use anyhow::anyhow;
use cid::Cid;
use fvm_shared::address::Address;
use num_traits::Zero;
use wasmtime::{Engine, Module};

use blockstore::Blockstore;
use fvm_shared::bigint::{BigInt, Sign};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::error::{ActorError, ExitCode};
use fvm_shared::{actor_error, ActorID};

use crate::externs::Externs;
use crate::gas::{price_list_by_epoch, GasCharge, PriceList};

use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::{ActorState, StateTree};

use crate::Config;

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
    /// Current epoch.
    epoch: ChainEpoch,
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
            epoch,
            config,
            context,
            engine,
            externs,
            blockstore,
            state_tree,
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
        &mut self.state_tree
    }

    /// Creates an uninitialized actor.
    // TODO: Remove
    pub(crate) fn create_actor(
        &mut self,
        addr: &Address,
        act: ActorState,
    ) -> Result<ActorID, ActorError> {
        let state_tree = self.state_tree_mut();

        let addr_id = state_tree
            .register_new_address(addr)
            .map_err(|e| e.downcast_fatal("failed to register new address"))?;

        state_tree
            .set_actor(&Address::new_id(addr_id), act)
            .map_err(|e| e.downcast_fatal("failed to set actor"))?;
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
        // Validate if the message was correct, charge for it, and extract some preliminary data.
        let (sender_id, gas_cost, inclusion_cost) = match self.preflight_message(&msg) {
            Ok(Ok(res)) => res,
            Ok(Err(apply_ret)) => return (Ok(apply_ret), self),
            Err(e) => return (Err(e), self),
        };

        // Apply the message.
        let mut cm = CallManager::new(self, sender_id, msg.gas_limit);
        match cm.charge_gas(inclusion_cost) {
            Err(e) => return (Err(e.into()), cm.finish().1),
            _ => (),
        }

        // Invoke the message.
        // TODO: We need some macro/monad help here.
        let (res, mut cm) = cm.send(msg.to, msg.method_num, &msg.params, &msg.value);

        // Charge for including the result.
        // We shouldn't put this here, but this is where we can still account for gas.
        // TODO: Maybe CallManager::finish() should return the GasTracker?
        if let Ok(ret) = res.as_ref() {
            if let Err(e) =
                cm.charge_gas(cm.context().price_list().on_chain_return_value(ret.len()))
            {
                // No need to charge any gas, we've already charged the _full_ amount.
                // XXX: But we do need to revert here!!
                let (_, s) = cm.finish();
                return (Err(e.into()), s);
            }
        }

        let (gas_used, s) = cm.finish();
        self = s;

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
        (self.finish_message(msg, receipt, err, gas_cost), self)
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
    ) -> anyhow::Result<Result<(ActorID, TokenAmount, GasCharge), ApplyRet>> {
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

        let sender_id = match self.state_tree.lookup_id(&msg.from) {
            Ok(Some(id)) => id,
            Ok(None) => {
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SysErrSenderInvalid,
                    miner_penalty_amount.clone(),
                    Some(actor_error!(SysErrSenderInvalid; "Sender invalid")),
                )))
            }
            Err(e) => return Err(anyhow!("failed to lookup actor {}: {}", &msg.from, e)),
        };

        let sender = match self.state_tree.get_actor(&Address::new_id(sender_id)) {
            Ok(Some(act)) => act,
            Ok(None) => {
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SysErrSenderInvalid,
                    miner_penalty_amount.clone(),
                    Some(actor_error!(SysErrSenderInvalid; "Sender invalid")),
                )))
            }
            Err(e) => return Err(anyhow!("failed to lookup actor {}: {}", &msg.from, e)),
        };

        // If sender is not an account actor, the message is invalid.
        if !actor::is_account_actor(&sender.code) {
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
        act_err: Option<ActorError>,
        gas_cost: BigInt,
    ) -> anyhow::Result<ApplyRet> {
        // Make sure the actor error is sane.
        if let Some(err) = &act_err {
            if err.is_fatal() {
                return Err(anyhow!(
                    "[from={}, to={}, seq={}, m={}, h={}] fatal error: {}",
                    msg.from,
                    msg.to,
                    msg.sequence,
                    msg.method_num,
                    self.epoch,
                    err
                ));
            } else if err.is_ok() {
                return Err(anyhow!(
                    "message invocation errored with an ok status: {}",
                    err
                ));
            }
        }

        // NOTE: we don't support old network versions in the FVM, so we always burn.
        let GasOutputs {
            base_fee_burn,
            miner_tip,
            over_estimation_burn,
            refund,
            miner_penalty,
            ..
        } = compute_gas_outputs(
            receipt.gas_used,
            msg.gas_limit,
            &self.context.base_fee,
            &msg.gas_fee_cap,
            &msg.gas_premium,
        );

        let mut transfer_to_actor = |addr: &Address, amt: &TokenAmount| -> anyhow::Result<()> {
            if amt.sign() == Sign::Minus {
                return Err(anyhow!("attempted to transfer negative value into actor"));
            }
            if amt.is_zero() {
                return Ok(());
            }

            self.state_tree
                .mutate_actor(addr, |act| {
                    act.deposit_funds(amt);
                    Ok(())
                })
                .map_err(|e| anyhow!("failed to lookup actor for transfer: {}", e))
        };

        transfer_to_actor(&*BURNT_FUNDS_ACTOR_ADDR, &base_fee_burn)?;

        transfer_to_actor(&*REWARD_ACTOR_ADDR, &miner_tip)?;

        transfer_to_actor(&*BURNT_FUNDS_ACTOR_ADDR, &over_estimation_burn)?;

        // refund unused gas
        transfer_to_actor(&msg.from, &refund)?;

        if (&base_fee_burn + over_estimation_burn + &refund + &miner_tip) != gas_cost {
            // Sanity check. This could be a fatal error.
            // XXX: this _is_ a fatal error in the FVM, at the moment at least.
            return Err(anyhow!("Gas handling math is wrong"));
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

// TODO: move this to a gas module?

#[derive(Clone, Default)]
struct GasOutputs {
    base_fee_burn: TokenAmount,
    over_estimation_burn: TokenAmount,
    miner_penalty: TokenAmount,
    miner_tip: TokenAmount,
    refund: TokenAmount,

    gas_refund: i64,
    gas_burned: i64,
}

fn compute_gas_outputs(
    gas_used: i64,
    gas_limit: i64,
    base_fee: &TokenAmount,
    fee_cap: &TokenAmount,
    gas_premium: &TokenAmount,
) -> GasOutputs {
    let mut base_fee_to_pay = base_fee;
    let mut out = GasOutputs::default();

    if base_fee > fee_cap {
        base_fee_to_pay = fee_cap;
        out.miner_penalty = (base_fee - fee_cap) * gas_used
    }

    out.base_fee_burn = base_fee_to_pay * gas_used;

    let mut miner_tip = gas_premium.clone();
    if &(base_fee_to_pay + &miner_tip) > fee_cap {
        miner_tip = fee_cap - base_fee_to_pay;
    }
    out.miner_tip = &miner_tip * gas_limit;

    let (out_gas_refund, out_gas_burned) = compute_gas_overestimation_burn(gas_used, gas_limit);
    out.gas_refund = out_gas_refund;
    out.gas_burned = out_gas_burned;

    if out.gas_burned != 0 {
        out.over_estimation_burn = base_fee_to_pay * out.gas_burned;
        out.miner_penalty += (base_fee - base_fee_to_pay) * out.gas_burned;
    }
    let required_funds = fee_cap * gas_limit;
    let refund = required_funds - &out.base_fee_burn - &out.miner_tip - &out.over_estimation_burn;
    out.refund = refund;

    out
}

fn compute_gas_overestimation_burn(gas_used: i64, gas_limit: i64) -> (i64, i64) {
    const GAS_OVERUSE_NUM: i64 = 11;
    const GAS_OVERUSE_DENOM: i64 = 10;

    if gas_used == 0 {
        return (0, gas_limit);
    }

    let mut over = gas_limit - (GAS_OVERUSE_NUM * gas_used) / GAS_OVERUSE_DENOM;
    if over < 0 {
        return (gas_limit - gas_used, 0);
    }

    if over > gas_used {
        over = gas_used;
    }

    let mut gas_to_burn: BigInt = (gas_limit - gas_used).into();
    gas_to_burn *= over;
    gas_to_burn /= gas_used;

    let gas_to_burn = i64::try_from(gas_to_burn).unwrap();
    (gas_limit - gas_used - gas_to_burn, gas_to_burn)
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
