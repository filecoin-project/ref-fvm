use std::ops::{Deref, DerefMut};
use std::result::Result as StdResult;

use anyhow::{anyhow, Result};
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::bigint::{BigInt, Sign};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use fvm_shared::ActorID;
use num_traits::Zero;

use super::{ApplyFailure, ApplyKind, ApplyRet, Executor};
use crate::account_actor::is_account_actor;
use crate::call_manager::{backtrace, CallManager, InvocationResult};
use crate::gas::{GasCharge, GasOutputs};
use crate::kernel::{ClassifyResult, Context as _, ExecutionError, Kernel};
use crate::machine::{Machine, BURNT_FUNDS_ACTOR_ADDR, REWARD_ACTOR_ADDR};

/// The core of the FVM.
///
/// ## Generic types
/// * B => Blockstore.
/// * E => Externs.
/// * K => Kernel.
//
// If the inner value is `None` it means the machine got poisend and is unusable.
#[repr(transparent)]
pub struct DefaultExecutor<K: Kernel>(Option<<K::CallManager as CallManager>::Machine>);

impl<K: Kernel> Deref for DefaultExecutor<K> {
    type Target = <K::CallManager as CallManager>::Machine;

    fn deref(&self) -> &Self::Target {
        &*self.0.as_ref().expect("machine poisoned")
    }
}

impl<K: Kernel> DerefMut for DefaultExecutor<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0.as_mut().expect("machine poisoned")
    }
}

impl<K> Executor for DefaultExecutor<K>
where
    K: Kernel,
{
    type Kernel = K;
    /// This is the entrypoint to execute a message.
    fn execute_message(
        &mut self,
        msg: Message,
        apply_kind: ApplyKind,
        raw_length: usize,
    ) -> anyhow::Result<ApplyRet> {
        // Validate if the message was correct, charge for it, and extract some preliminary data.
        let (sender_id, gas_cost, inclusion_cost) =
            match self.preflight_message(&msg, apply_kind, raw_length)? {
                Ok(res) => res,
                Err(apply_ret) => return Ok(apply_ret),
            };

        // Apply the message.
        let (res, gas_used, stats, mut backtrace) = self.map_machine(|machine| {
            let mut cm = K::CallManager::new(machine, msg.gas_limit, msg.from, msg.sequence);
            // This error is fatal because it should have already been acounted for inside
            // preflight_message.
            if let Err(e) = cm.charge_gas(inclusion_cost) {
                return (Err(e), cm.finish().3);
            }

            let result = cm.with_transaction(|cm| {
                // Invoke the message.
                let ret =
                    cm.send::<K>(sender_id, msg.to, msg.method_num, &msg.params, &msg.value)?;

                // Charge for including the result (before we end the transaction).
                if let InvocationResult::Return(data) = &ret {
                    cm.charge_gas(cm.context().price_list.on_chain_return_value(data.len()))?;
                }

                Ok(ret)
            });
            let (gas_used, backtrace, stats, machine) = cm.finish();
            (Ok((result, gas_used, stats, backtrace)), machine)
        })?;

        // Extract the exit code and build the result of the message application.
        let receipt = match res {
            Ok(InvocationResult::Return(return_data)) => {
                backtrace.clear();
                Receipt {
                    exit_code: ExitCode::Ok,
                    return_data,
                    gas_used,
                }
            }
            Ok(InvocationResult::Failure(exit_code)) => {
                if exit_code.is_success() {
                    return Err(anyhow!("actor failed with status OK"));
                }
                Receipt {
                    exit_code,
                    return_data: Default::default(),
                    gas_used,
                }
            }
            Err(ExecutionError::OutOfGas) => Receipt {
                exit_code: ExitCode::SysErrOutOfGas,
                return_data: Default::default(),
                gas_used,
            },
            Err(ExecutionError::Syscall(err)) => {
                let exit_code = match err.1 {
                    ErrorNumber::IllegalOperation => ExitCode::SysErrIllegalActor,
                    ErrorNumber::AssertionFailed => ExitCode::SysErrIllegalArgument,
                    ErrorNumber::InsufficientFunds => ExitCode::SysErrInsufficientFunds,
                    ErrorNumber::NotFound => ExitCode::SysErrInvalidReceiver,
                    code => {
                        return Err(anyhow!(
                            "unexpected syscall error when processing message: {} ({})",
                            code,
                            code as u32
                        ))
                    }
                };

                backtrace.set_cause(backtrace::Cause::new("send", "send", err));
                Receipt {
                    exit_code,
                    return_data: Default::default(),
                    gas_used,
                }
            }
            Err(ExecutionError::Fatal(e)) => {
                return Err(e.context(format!(
                    "[from={}, to={}, seq={}, m={}, h={}] fatal error",
                    msg.from,
                    msg.to,
                    msg.sequence,
                    msg.method_num,
                    self.context().epoch
                )))
            }
        };

        let failure_info = if backtrace.is_empty() || receipt.exit_code.is_success() {
            None
        } else {
            Some(ApplyFailure::MessageBacktrace(backtrace))
        };

        let mut ret = match apply_kind {
            ApplyKind::Explicit => self.finish_message(msg, receipt, gas_cost)?,
            ApplyKind::Implicit => ApplyRet::from(receipt),
        };

        ret.failure_info = failure_info;
        ret.wasm_stats = Some(stats);
        Ok(ret)
    }
}

impl<K> DefaultExecutor<K>
where
    K: Kernel,
{
    pub fn new(m: <K::CallManager as CallManager>::Machine) -> Self {
        Self(Some(m))
    }

    /// Flush the state-tree to the underlying blockstore.
    pub fn flush(&mut self) -> anyhow::Result<Cid> {
        let k = (&mut **self).flush()?;
        Ok(k)
    }

    /// Consume consumes the executor and returns the Machine. If the Machine had
    /// been poisoned during execution, the Option will be None.
    pub fn consume(self) -> Option<<K::CallManager as CallManager>::Machine> {
        self.0
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
        apply_kind: ApplyKind,
        raw_length: usize,
    ) -> Result<StdResult<(ActorID, TokenAmount, GasCharge<'static>), ApplyRet>> {
        // TODO sanity check on message, copied from Forest, needs adaptation.
        msg.check().or_fatal()?;

        // TODO I don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = &self.context().price_list;

        let (inclusion_cost, miner_penalty_amount) = match apply_kind {
            ApplyKind::Implicit => (GasCharge::new("none", 0, 0), Default::default()),
            ApplyKind::Explicit => {
                let inclusion_cost = pl.on_chain_message(raw_length);
                let inclusion_total = inclusion_cost.total();

                // Verify the cost of the message is not over the message gas limit.
                if inclusion_total > msg.gas_limit {
                    return Ok(Err(ApplyRet::prevalidation_fail(
                        ExitCode::SysErrOutOfGas,
                        format!("Out of gas ({} > {})", inclusion_total, msg.gas_limit),
                        &self.context().base_fee * inclusion_total,
                    )));
                }

                let miner_penalty_amount = &self.context().base_fee * msg.gas_limit;
                (inclusion_cost, miner_penalty_amount)
            }
        };

        // Load sender actor state.
        let sender_id = match self
            .state_tree()
            .lookup_id(&msg.from)
            .with_context(|| format!("failed to lookup actor {}", &msg.from))?
        {
            Some(id) => id,
            None => {
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SysErrSenderInvalid,
                    "Sender invalid",
                    miner_penalty_amount,
                )))
            }
        };

        if apply_kind == ApplyKind::Implicit {
            return Ok(Ok((sender_id, TokenAmount::zero(), inclusion_cost)));
        }

        let sender = match self
            .state_tree()
            .get_actor(&Address::new_id(sender_id))
            .with_context(|| format!("failed to lookup actor {}", &msg.from))?
        {
            Some(act) => act,
            None => {
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SysErrSenderInvalid,
                    "Sender invalid",
                    miner_penalty_amount,
                )))
            }
        };

        // If sender is not an account actor, the message is invalid.
        if !is_account_actor(&sender.code) {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrSenderInvalid,
                "Send not from account actor",
                miner_penalty_amount,
            )));
        };

        // Check sequence is correct
        if msg.sequence != sender.sequence {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrSenderStateInvalid,
                format!(
                    "Actor sequence invalid: {} != {}",
                    msg.sequence, sender.sequence
                ),
                miner_penalty_amount,
            )));
        };

        // Ensure from actor has enough balance to cover the gas cost of the message.
        let gas_cost: TokenAmount = msg.gas_fee_cap.clone() * msg.gas_limit;
        if sender.balance < gas_cost {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SysErrSenderStateInvalid,
                format!(
                    "Actor balance less than needed: {} < {}",
                    sender.balance, gas_cost
                ),
                miner_penalty_amount,
            )));
        }

        // Deduct message inclusion gas cost and increment sequence.
        self.state_tree_mut().mutate_actor_id(sender_id, |act| {
            act.deduct_funds(&gas_cost)?;
            act.sequence += 1;
            Ok(())
        })?;

        Ok(Ok((sender_id, gas_cost, inclusion_cost)))
    }

    fn finish_message(
        &mut self,
        msg: Message,
        receipt: Receipt,
        gas_cost: BigInt,
    ) -> anyhow::Result<ApplyRet> {
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
            &self.context().base_fee,
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

            self.state_tree_mut()
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
            return Err(anyhow!("Gas handling math is wrong"));
        }
        Ok(ApplyRet {
            msg_receipt: receipt,
            failure_info: None,
            wasm_stats: None,
            penalty: miner_penalty,
            miner_tip,
        })
    }

    fn map_machine<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(
            <K::CallManager as CallManager>::Machine,
        ) -> (T, <K::CallManager as CallManager>::Machine),
    {
        replace_with::replace_with_and_return(
            &mut self.0,
            || None,
            |m| {
                let (ret, machine) = f(m.unwrap());
                (ret, Some(machine))
            },
        )
    }
}
