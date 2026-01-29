// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::{HashMap, hash_map::Entry};
use std::ops::{Deref, DerefMut};
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use cid::Cid;
use fvm_ipld_encoding::{CBOR, RawBytes};
use fvm_shared::address::{Address, Payload};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::event::StampedEvent;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use fvm_shared::{ActorID, IPLD_RAW, METHOD_SEND};
use num_traits::Zero;

use super::{ApplyFailure, ApplyKind, ApplyRet, Executor, ReservationError};
use crate::call_manager::{Backtrace, CallManager, Entrypoint, InvocationResult, backtrace};
use crate::eam_actor::EAM_ACTOR_ID;
use crate::engine::EnginePool;
use crate::gas::{Gas, GasCharge, GasOutputs};
use crate::kernel::{Block, ClassifyResult, Context as _, ExecutionError, Kernel};
use crate::machine::{BURNT_FUNDS_ACTOR_ID, Machine, REWARD_ACTOR_ID};
use crate::trace::ExecutionTrace;

pub use self::reservation::ReservationSession;

mod reservation {
    use std::collections::HashMap;

    use fvm_shared::ActorID;
    use fvm_shared::econ::TokenAmount;

    use crate::executor::telemetry::ReservationTelemetry;

    /// Tracks the gas reservation ledger for a tipset-scope session.
    ///
    /// The ledger maintains per-actor reservation amounts that are decremented
    /// as messages are processed. All entries must reach zero before the session
    /// can be closed.
    #[derive(Default)]
    pub struct ReservationSession {
        pub reservations: HashMap<ActorID, TokenAmount>,
        pub open: bool,
        pub telemetry: ReservationTelemetry,
    }
}

/// The default [`Executor`].
///
/// # Warning
///
/// Message execution might run out of stack and crash (the entire process) if it doesn't have at
/// least 64MiB of stack space. If you can't guarantee 64MiB of stack space, wrap this executor in
/// a [`ThreadedExecutor`][super::ThreadedExecutor].
pub struct DefaultExecutor<K: Kernel> {
    engine_pool: EnginePool,
    // If the inner value is `None` it means the machine got poisoned and is unusable.
    machine: Option<<K::CallManager as CallManager>::Machine>,
    reservation_session: Arc<Mutex<ReservationSession>>,
}

impl<K: Kernel> Deref for DefaultExecutor<K> {
    type Target = <K::CallManager as CallManager>::Machine;

    fn deref(&self) -> &Self::Target {
        self.machine.as_ref().expect("machine poisoned")
    }
}

impl<K: Kernel> DerefMut for DefaultExecutor<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.machine.as_mut().expect("machine poisoned")
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

        struct MachineExecRet {
            result: crate::kernel::Result<InvocationResult>,
            gas_used: u64,
            backtrace: Backtrace,
            exec_trace: ExecutionTrace,
            events_root: Option<Cid>,
            events: Vec<StampedEvent>, // TODO consider removing if nothing in the client ends up using it.
        }

        // Pre-resolve the message receiver's address, if known.
        let receiver_id = self
            .state_tree()
            .lookup_id(&msg.to)
            .context("failure when looking up message receiver")?;

        // Filecoin caps the premium plus the base-fee at the fee-cap.
        // We expose the _effective_ premium to the user.
        let effective_premium = msg
            .gas_premium
            .clone()
            .min(&msg.gas_fee_cap - &self.context().base_fee)
            .max(TokenAmount::zero());

        // Acquire an engine from the pool. This may block if there are concurrently executing
        // messages inside other executors sharing the same pool.
        let engine = self.engine_pool.acquire();

        // Apply the message.
        let reservation_session = self.reservation_session.clone();
        let ret = self.map_machine(|machine| {
            // We're processing a chain message, so the sender is the origin of the call stack.
            let mut cm = K::CallManager::new(
                machine,
                engine,
                msg.gas_limit,
                sender_id,
                msg.from,
                receiver_id,
                msg.to,
                msg.sequence,
                effective_premium,
                reservation_session.clone(),
            );
            // This error is fatal because it should have already been accounted for inside
            // preflight_message.
            if let Err(e) = cm.charge_gas(inclusion_cost) {
                let (_, machine) = cm.finish();
                return (Err(e), machine);
            }

            let params = (!msg.params.is_empty()).then(|| {
                Block::new(
                    if msg.method_num == METHOD_SEND {
                        // Method zero params are "arbitrary bytes", so we'll just count them as
                        // raw.
                        //
                        // This won't actually affect anything (because no code will see these
                        // parameters), but it's more correct and makes me happier.
                        //
                        // NOTE: this _may_ start to matter once we start _validating_ ipld (m2.2).
                        IPLD_RAW
                    } else {
                        // This is CBOR, not DAG_CBOR, because links sent from off-chain aren't
                        // reachable.
                        CBOR
                    },
                    msg.params.bytes(),
                    // not DAG-CBOR, so we don't have to parse for links.
                    Vec::new(),
                )
            });

            let result = cm.with_transaction(|cm| {
                // Invoke the message. We charge for the return value internally if the call-stack depth
                // is 1.
                cm.call_actor::<K>(
                    sender_id,
                    msg.to,
                    Entrypoint::Invoke(msg.method_num),
                    params,
                    &msg.value,
                    None,
                    false,
                )
            });

            let (res, machine) = match cm.finish() {
                (Ok(res), machine) => (res, machine),
                (Err(err), machine) => return (Err(err), machine),
            };

            (
                Ok(MachineExecRet {
                    result,
                    gas_used: res.gas_used,
                    backtrace: res.backtrace,
                    exec_trace: res.exec_trace,
                    events_root: res.events_root,
                    events: res.events,
                }),
                machine,
            )
        })?;

        let MachineExecRet {
            result: res,
            gas_used,
            mut backtrace,
            exec_trace,
            events_root,
            events,
        } = ret;

        // Extract the exit code and build the result of the message application.
        let receipt = match res {
            Ok(InvocationResult { exit_code, value }) => {
                // Convert back into a top-level return "value". We throw away the codec here,
                // unfortunately.
                let return_data = value
                    .map(|blk| RawBytes::from(blk.data().to_vec()))
                    .unwrap_or_default();

                if exit_code.is_success() {
                    backtrace.clear();
                }
                Receipt {
                    exit_code,
                    return_data,
                    gas_used,
                    events_root,
                }
            }
            Err(ExecutionError::OutOfGas) => Receipt {
                exit_code: ExitCode::SYS_OUT_OF_GAS,
                return_data: Default::default(),
                gas_used,
                events_root,
            },
            Err(ExecutionError::Syscall(err)) => {
                // Errors indicate the message couldn't be dispatched at all
                // (as opposed to failing during execution of the receiving actor).
                // These errors are mapped to exit codes that persist on chain.
                let exit_code = match err.1 {
                    ErrorNumber::InsufficientFunds => ExitCode::SYS_INSUFFICIENT_FUNDS,
                    ErrorNumber::NotFound => ExitCode::SYS_INVALID_RECEIVER,
                    _ => ExitCode::SYS_ASSERTION_FAILED,
                };

                backtrace.begin(backtrace::Cause::from_syscall("send", "send", err));
                Receipt {
                    exit_code,
                    return_data: Default::default(),
                    gas_used,
                    events_root,
                }
            }
            Err(ExecutionError::Fatal(err)) => {
                // We produce a receipt with SYS_ASSERTION_FAILED exit code, and
                // we consume the full gas amount so that, in case of a network-
                // wide fatal errors, all nodes behave deterministically.
                //
                // We set the backtrace from the fatal error to aid diagnosis.
                // Note that we use backtrace#set_cause instead of backtrace#begin
                // because we want to retain the propagation chain that we've
                // accumulated on the way out.
                let err = err.context(format!(
                    "[from={}, to={}, seq={}, m={}, h={}]",
                    msg.from,
                    msg.to,
                    msg.sequence,
                    msg.method_num,
                    self.context().epoch,
                ));
                backtrace.set_cause(backtrace::Cause::from_fatal(err));
                Receipt {
                    exit_code: ExitCode::SYS_ASSERTION_FAILED,
                    return_data: Default::default(),
                    gas_used: msg.gas_limit,
                    events_root,
                }
            }
        };

        let failure_info = if backtrace.is_empty() || receipt.exit_code.is_success() {
            None
        } else {
            Some(ApplyFailure::MessageBacktrace(backtrace))
        };

        match apply_kind {
            ApplyKind::Explicit => self.finish_message(
                sender_id,
                msg,
                receipt,
                failure_info,
                gas_cost,
                exec_trace,
                events,
            ),
            ApplyKind::Implicit => Ok(ApplyRet {
                msg_receipt: receipt,
                penalty: TokenAmount::zero(),
                miner_tip: TokenAmount::zero(),
                base_fee_burn: TokenAmount::zero(),
                over_estimation_burn: TokenAmount::zero(),
                refund: TokenAmount::zero(),
                gas_refund: 0,
                gas_burned: 0,
                failure_info,
                exec_trace,
                events,
            }),
        }
    }

    /// Flush the state-tree to the underlying blockstore.
    fn flush(&mut self) -> anyhow::Result<Cid> {
        let k = (**self).flush()?;
        Ok(k)
    }
}

impl<K> DefaultExecutor<K>
where
    K: Kernel,
{
    /// Create a new [`DefaultExecutor`] for executing messages on the [`Machine`].
    pub fn new(
        engine_pool: EnginePool,
        machine: <K::CallManager as CallManager>::Machine,
    ) -> anyhow::Result<Self> {
        // Skip preloading all builtin actors when testing.
        #[cfg(not(any(test, feature = "testing")))]
        {
            // Preload any uncached modules.
            // This interface works for now because we know all actor CIDs
            // ahead of time, but with user-supplied code, we won't have that
            // guarantee.
            engine_pool.acquire().preload_all(
                machine.blockstore(),
                machine.builtin_actors().builtin_actor_codes(),
            )?;
        }
        Ok(Self {
            engine_pool,
            machine: Some(machine),
            reservation_session: Arc::new(Mutex::new(ReservationSession::default())),
        })
    }

    /// Consume consumes the executor and returns the Machine. If the Machine had
    /// been poisoned during execution, the Option will be None.
    pub fn into_machine(self) -> Option<<K::CallManager as CallManager>::Machine> {
        self.machine
    }

    /// Assert that the current reservation session, if any, fully covers the gas cost for the
    /// specified sender. A coverage violation indicates a host/engine invariant breach and is
    /// treated as a fatal error by callers.
    fn reservation_assert_coverage(
        &self,
        sender: ActorID,
        gas_cost: &TokenAmount,
    ) -> StdResult<(), ReservationError> {
        let session = self
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned");

        if !session.open {
            return Ok(());
        }

        let reserved = session
            .reservations
            .get(&sender)
            .cloned()
            .unwrap_or_else(TokenAmount::zero);

        if reserved < *gas_cost {
            return Err(ReservationError::ReservationInvariant(format!(
                "reserved total for sender {} ({}) below gas cost ({})",
                sender, reserved, gas_cost
            )));
        }

        Ok(())
    }

    /// Decrement the reservation for the given sender on a prevalidation failure. This keeps the
    /// session ledger consistent even when the message never executes.
    fn reservation_prevalidation_decrement(
        &mut self,
        sender: ActorID,
        gas_cost: &TokenAmount,
    ) -> StdResult<(), ReservationError> {
        let mut session = self
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned");

        if !session.open {
            return Ok(());
        }

        match session.reservations.entry(sender) {
            Entry::Occupied(mut entry) => {
                let current = entry.get().clone();
                if current < *gas_cost {
                    return Err(ReservationError::Overflow);
                }
                let remaining = current - gas_cost.clone();
                if remaining.is_zero() {
                    entry.remove();
                } else {
                    *entry.get_mut() = remaining.clone();
                }

                // Keep the reserved_remaining_gauge{sender} telemetry in sync with the ledger.
                session
                    .telemetry
                    .reservation_remaining_update(sender, &remaining);
                Ok(())
            }
            Entry::Vacant(_) => Err(ReservationError::ReservationInvariant(format!(
                "no reservation entry for sender {} on prevalidation failure",
                sender
            ))),
        }
    }

    // TODO: The return type here is very strange because we have three cases:
    //  1. Continue: Return sender ID, & gas.
    //  2. Short-circuit: Return ApplyRet.
    //  3. Fail: Return an error.
    //  We could use custom types, but that would be even more annoying.
    fn preflight_message(
        &mut self,
        msg: &Message,
        apply_kind: ApplyKind,
        raw_length: usize,
    ) -> Result<StdResult<(ActorID, TokenAmount, GasCharge), ApplyRet>> {
        msg.check().or_fatal()?;

        // TODO We don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = &self.context().price_list;
        let reservation_mode = self
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned")
            .open;

        let (inclusion_cost, inclusion_total, miner_penalty_amount) = match apply_kind {
            ApplyKind::Implicit => (
                GasCharge::new("none", Gas::zero(), Gas::zero()),
                None,
                Default::default(),
            ),
            ApplyKind::Explicit => {
                let inclusion_cost = pl.on_chain_message(raw_length);
                let inclusion_total = inclusion_cost.total().round_up();
                let miner_penalty_amount = &self.context().base_fee * msg.gas_limit;
                (inclusion_cost, Some(inclusion_total), miner_penalty_amount)
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
                // If we can't resolve the sender address to an actor ID, this is a
                // prevalidation failure. Reservation sessions (when active) are keyed
                // by ActorID, so this case should have been rejected when building the
                // reservation plan.
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SYS_SENDER_INVALID,
                    "Sender invalid",
                    miner_penalty_amount,
                )));
            }
        };

        if apply_kind == ApplyKind::Implicit {
            return Ok(Ok((sender_id, TokenAmount::zero(), inclusion_cost)));
        }

        // Compute the maximum gas cost this message can charge. This uses big-int arithmetic and
        // is expected not to overflow; a negative result would indicate an arithmetic bug.
        let gas_cost: TokenAmount = msg.gas_fee_cap.clone() * msg.gas_limit;
        if gas_cost.is_negative() {
            return Err(ReservationError::Overflow.into());
        }

        // Verify the cost of the message is not over the message gas limit. In reservation mode we
        // must also decrement the reservation for this message so the session can end at zero.
        if let Some(inclusion_total) = inclusion_total {
            if inclusion_total > msg.gas_limit {
                if reservation_mode {
                    self.reservation_prevalidation_decrement(sender_id, &gas_cost)?;
                }
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SYS_OUT_OF_GAS,
                    format!("Out of gas ({} > {})", inclusion_total, msg.gas_limit),
                    &self.context().base_fee * inclusion_total,
                )));
            }
        }

        let mut sender_state = match self
            .state_tree()
            .get_actor(sender_id)
            .with_context(|| format!("failed to lookup actor {}", &msg.from))?
        {
            Some(act) => act,
            None => {
                if reservation_mode {
                    self.reservation_prevalidation_decrement(sender_id, &gas_cost)?;
                }
                return Ok(Err(ApplyRet::prevalidation_fail(
                    ExitCode::SYS_SENDER_INVALID,
                    "Sender invalid",
                    miner_penalty_amount,
                )));
            }
        };

        // Sender is valid if it is:
        // - an account actor
        // - an Ethereum Externally Owned Address
        // - a placeholder actor that has an f4 address in the EAM's namespace

        let mut sender_is_valid = self.builtin_actors().is_account_actor(&sender_state.code)
            || self
                .builtin_actors()
                .is_ethaccount_actor(&sender_state.code);

        if self.builtin_actors().is_placeholder_actor(&sender_state.code) &&
            sender_state.sequence == 0 &&
            sender_state
                .delegated_address
                .map(|a| matches!(a.payload(), Payload::Delegated(da) if da.namespace() == EAM_ACTOR_ID))
                .unwrap_or(false) {
            sender_is_valid = true;
            sender_state.code = *self.builtin_actors().get_ethaccount_code();
        }

        if !sender_is_valid {
            if reservation_mode {
                self.reservation_prevalidation_decrement(sender_id, &gas_cost)?;
            }
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SYS_SENDER_INVALID,
                "Send not from valid sender",
                miner_penalty_amount,
            )));
        };

        // Check sequence is correct
        if msg.sequence != sender_state.sequence {
            if reservation_mode {
                self.reservation_prevalidation_decrement(sender_id, &gas_cost)?;
            }
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SYS_SENDER_STATE_INVALID,
                format!(
                    "Actor sequence invalid: {} != {}",
                    msg.sequence, sender_state.sequence
                ),
                miner_penalty_amount,
            )));
        };

        // At this point the message is syntactically valid and has the correct nonce.
        if reservation_mode {
            // In reservation mode we assert that the ledger fully covers the maximum gas cost, but
            // we _do not_ deduct funds here; settlement is handled in finish_message.
            self.reservation_assert_coverage(sender_id, &gas_cost)?;

            sender_state.sequence += 1;
            self.state_tree_mut().set_actor(sender_id, sender_state);

            return Ok(Ok((sender_id, gas_cost, inclusion_cost)));
        }

        // Legacy behavior: ensure from actor has enough balance to cover the gas cost of the
        // message and pre-deduct it from the sender balance.
        sender_state.sequence += 1;

        if sender_state.balance < gas_cost {
            return Ok(Err(ApplyRet::prevalidation_fail(
                ExitCode::SYS_SENDER_STATE_INVALID,
                format!(
                    "Actor balance less than needed: {} < {}",
                    sender_state.balance, gas_cost
                ),
                miner_penalty_amount,
            )));
        }

        sender_state.deduct_funds(&gas_cost)?;

        // Update the actor in the state tree
        self.state_tree_mut().set_actor(sender_id, sender_state);

        Ok(Ok((sender_id, gas_cost, inclusion_cost)))
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_message(
        &mut self,
        sender_id: ActorID,
        msg: Message,
        receipt: Receipt,
        failure_info: Option<ApplyFailure>,
        gas_cost: TokenAmount,
        exec_trace: ExecutionTrace,
        events: Vec<StampedEvent>,
    ) -> anyhow::Result<ApplyRet> {
        let reservation_mode = self
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned")
            .open;

        // NOTE: we don't support old network versions in the FVM, so we always burn.
        let GasOutputs {
            base_fee_burn,
            over_estimation_burn,
            miner_penalty,
            miner_tip,
            refund,
            gas_refund,
            gas_burned,
        } = GasOutputs::compute(
            receipt.gas_used,
            msg.gas_limit,
            &self.context().base_fee,
            &msg.gas_fee_cap,
            &msg.gas_premium,
        );

        let mut transfer_to_actor = |addr: ActorID, amt: &TokenAmount| -> anyhow::Result<()> {
            if amt.is_negative() {
                return Err(anyhow!("attempted to transfer negative value into actor"));
            }
            if amt.is_zero() {
                return Ok(());
            }

            self.state_tree_mut()
                .mutate_actor(addr, |act| act.deposit_funds(amt).or_fatal())
                .context("failed to lookup actor for transfer")?;
            Ok(())
        };

        // Pay base-fee burn, miner tip, and over-estimation burn as today.
        transfer_to_actor(BURNT_FUNDS_ACTOR_ID, &base_fee_burn)?;

        transfer_to_actor(REWARD_ACTOR_ID, &miner_tip)?;

        transfer_to_actor(BURNT_FUNDS_ACTOR_ID, &over_estimation_burn)?;

        if reservation_mode {
            // In reservation mode we net-charge the sender for the actual gas consumption and
            // realize the refund by releasing the reservation instead of depositing it.
            let consumption = &base_fee_burn + &over_estimation_burn + &miner_tip;

            self.state_tree_mut()
                .mutate_actor(sender_id, |act| act.deduct_funds(&consumption).or_fatal())
                .context("failed to lookup sender actor for settlement")?;

            // Decrement this message's reservation; underflow or a missing entry indicates a fatal
            // reservation invariant breach.
            self.reservation_prevalidation_decrement(sender_id, &gas_cost)?;

            // Track settlement metrics, including the virtual refund realized via reservation
            // release.
            self.reservation_session
                .lock()
                .expect("reservation session mutex poisoned")
                .telemetry
                .settlement_record(
                    &base_fee_burn,
                    &miner_tip,
                    &over_estimation_burn,
                    Some(&refund),
                );
        } else {
            // Legacy behavior: refund unused gas directly to the sender.
            transfer_to_actor(sender_id, &refund)?;

            // Track settlement metrics in legacy mode as well, without a virtual refund component.
            self.reservation_session
                .lock()
                .expect("reservation session mutex poisoned")
                .telemetry
                .settlement_record(&base_fee_burn, &miner_tip, &over_estimation_burn, None);
        }

        if (&base_fee_burn + &over_estimation_burn + &refund + &miner_tip) != gas_cost {
            // Sanity check. This could be a fatal error.
            return Err(anyhow!("Gas handling math is wrong"));
        }
        Ok(ApplyRet {
            msg_receipt: receipt,
            penalty: miner_penalty,
            miner_tip,
            base_fee_burn,
            over_estimation_burn,
            refund,
            gas_refund,
            gas_burned,
            failure_info,
            exec_trace,
            events,
        })
    }

    fn map_machine<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(
            <K::CallManager as CallManager>::Machine,
        ) -> (T, <K::CallManager as CallManager>::Machine),
    {
        replace_with::replace_with_and_return(
            &mut self.machine,
            || None,
            |m| {
                let (ret, machine) = f(m.unwrap());
                (ret, Some(machine))
            },
        )
    }

    /// Begin a tipset-scope gas reservation session from a per-sender plan.
    ///
    /// The plan is keyed by address; this method resolves each address to an ActorID and
    /// aggregates per-actor totals before checking affordability.
    pub fn begin_reservation_session(
        &mut self,
        plan: &[(Address, TokenAmount)],
    ) -> StdResult<(), ReservationError> {
        // Empty plan is a no-op and must not enter reservation mode.
        if plan.is_empty() {
            return Ok(());
        }

        const MAX_SENDERS: usize = 65_536;
        if plan.len() > MAX_SENDERS {
            self.reservation_session
                .lock()
                .expect("reservation session mutex poisoned")
                .telemetry
                .reservation_begin_failed();
            return Err(ReservationError::PlanTooLarge);
        }

        let session_arc = self.reservation_session.clone();
        let record_failure = || {
            session_arc
                .lock()
                .expect("reservation session mutex poisoned")
                .telemetry
                .reservation_begin_failed();
        };

        // Aggregate per-actor reservations.
        let mut reservations: HashMap<ActorID, TokenAmount> = HashMap::new();

        for (addr, amount) in plan {
            // Resolve address to ActorID via the state tree.
            let sender_id = match self.state_tree().lookup_id(addr) {
                Ok(Some(id)) => id,
                Ok(None) => {
                    record_failure();
                    return Err(ReservationError::ReservationInvariant(format!(
                        "failed to resolve address {addr} to actor ID"
                    )));
                }
                Err(e) => {
                    record_failure();
                    return Err(ReservationError::ReservationInvariant(format!(
                        "failed to lookup actor {addr}: {e}"
                    )));
                }
            };

            if amount.is_negative() {
                record_failure();
                return Err(ReservationError::ReservationInvariant(format!(
                    "negative reservation amount for {addr}: {amount}"
                )));
            }
            if amount.is_zero() {
                continue;
            }

            reservations
                .entry(sender_id)
                .and_modify(|total| *total += amount.clone())
                .or_insert_with(|| amount.clone());
        }

        // Check affordability per sender: Σ(plan) ≤ actor.balance.
        for (actor_id, reserved) in &reservations {
            let actor_state = match self.state_tree().get_actor(*actor_id) {
                Ok(Some(state)) => state,
                Ok(None) => {
                    record_failure();
                    return Err(ReservationError::ReservationInvariant(format!(
                        "reservation plan includes unknown actor {actor_id}"
                    )));
                }
                Err(e) => {
                    record_failure();
                    return Err(ReservationError::ReservationInvariant(format!(
                        "failed to load actor {actor_id}: {e}"
                    )));
                }
            };

            if &actor_state.balance < reserved {
                record_failure();
                return Err(ReservationError::InsufficientFundsAtBegin { sender: *actor_id });
            }
        }

        let mut session = self
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned");

        if session.open {
            session.telemetry.reservation_begin_failed();
            return Err(ReservationError::SessionOpen);
        }

        session.telemetry.reservation_begin_succeeded(&reservations);

        session.reservations = reservations;
        session.open = true;
        Ok(())
    }

    /// End the active reservation session, ensuring the ledger has returned to zero.
    pub fn end_reservation_session(&mut self) -> StdResult<(), ReservationError> {
        let mut session = self
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned");

        if !session.open {
            return Err(ReservationError::SessionClosed);
        }

        let has_non_zero = session.reservations.values().any(|amt| !amt.is_zero());

        if has_non_zero {
            return Err(ReservationError::NonZeroRemainder);
        }

        session.reservations.clear();
        session.open = false;

        session.telemetry.reservation_end_succeeded();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cid::Cid;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_ipld_encoding::{CborStore, DAG_CBOR, RawBytes};
    use fvm_shared::IDENTITY_HASH;
    use fvm_shared::address::Address;
    use fvm_shared::error::ExitCode;
    use fvm_shared::state::{ActorState, StateTreeVersion};
    use multihash_codetable::{Code, Multihash};

    use crate::call_manager::DefaultCallManager;
    use crate::call_manager::NO_DATA_BLOCK_ID;
    use crate::engine::EnginePool;
    use crate::externs::{Chain, Consensus, Externs, Rand};
    use crate::kernel::default::DefaultKernel;
    use crate::kernel::filecoin::DefaultFilecoinKernel;
    use crate::kernel::{BlockRegistry, SelfOps, SendOps};
    use crate::machine::{DefaultMachine, Manifest, NetworkConfig};
    use crate::state_tree::StateTree;
    use fvm_shared::sys::SendFlags;

    struct DummyExterns;

    impl Externs for DummyExterns {}

    impl Rand for DummyExterns {
        fn get_chain_randomness(
            &self,
            round: fvm_shared::clock::ChainEpoch,
        ) -> anyhow::Result<[u8; 32]> {
            let msg = "reservation-test".as_bytes();
            let mut out = [0u8; 32];
            out[..msg.len()].copy_from_slice(msg);
            // Make the randomness depend on the round so tests can distinguish calls.
            out[31] ^= (round as u8).wrapping_mul(31);
            Ok(out)
        }

        fn get_beacon_randomness(
            &self,
            _round: fvm_shared::clock::ChainEpoch,
        ) -> anyhow::Result<[u8; 32]> {
            Ok([0u8; 32])
        }
    }

    impl Consensus for DummyExterns {
        fn verify_consensus_fault(
            &self,
            _h1: &[u8],
            _h2: &[u8],
            _extra: &[u8],
        ) -> anyhow::Result<(Option<fvm_shared::consensus::ConsensusFault>, i64)> {
            Ok((None, 0))
        }
    }

    impl Chain for DummyExterns {
        fn get_tipset_cid(&self, epoch: fvm_shared::clock::ChainEpoch) -> anyhow::Result<Cid> {
            Ok(Cid::new_v1(
                DAG_CBOR,
                Multihash::wrap(IDENTITY_HASH, &epoch.to_be_bytes()).unwrap(),
            ))
        }
    }

    type TestMachine = Box<DefaultMachine<MemoryBlockstore, DummyExterns>>;
    type TestKernel = DefaultFilecoinKernel<DefaultCallManager<TestMachine>>;
    type TestCallManager = <TestKernel as Kernel>::CallManager;
    type TestExecutor = DefaultExecutor<TestKernel>;

    fn new_executor_with_base_fee(base_fee: TokenAmount) -> TestExecutor {
        // Construct an empty state-tree and machine, mirroring the lib.rs constructor test, but
        // overriding the base-fee so settlement paths exercise non-trivial gas outputs.
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V5).unwrap();
        let root = st.flush().unwrap();
        bs = st.into_store();

        // An empty built-in actors manifest.
        let manifest_cid = bs
            .put_cbor(&Manifest::DUMMY_CODES, Code::Blake2b256)
            .unwrap();
        let actors_cid = bs.put_cbor(&(1, manifest_cid), Code::Blake2b256).unwrap();

        let mut net_cfg = NetworkConfig::new(fvm_shared::version::NetworkVersion::V21);
        net_cfg.override_actors(actors_cid);
        let mut mc = net_cfg.for_epoch(0, 0, root);
        mc.set_base_fee(base_fee);

        let machine = DefaultMachine::new(&mc, bs, DummyExterns).unwrap();
        let engine = EnginePool::new((&mc.network).into()).unwrap();

        TestExecutor::new(engine, Box::new(machine)).unwrap()
    }

    fn new_executor() -> TestExecutor {
        // Construct an empty state-tree and machine, mirroring the lib.rs constructor test.
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V5).unwrap();
        let root = st.flush().unwrap();
        bs = st.into_store();

        // An empty built-in actors manifest.
        let manifest_cid = bs
            .put_cbor(&Manifest::DUMMY_CODES, Code::Blake2b256)
            .unwrap();
        let actors_cid = bs.put_cbor(&(1, manifest_cid), Code::Blake2b256).unwrap();

        let mc = NetworkConfig::new(fvm_shared::version::NetworkVersion::V21)
            .override_actors(actors_cid)
            .for_epoch(0, 0, root);

        let machine = DefaultMachine::new(&mc, bs, DummyExterns).unwrap();
        let engine = EnginePool::new((&mc.network).into()).unwrap();

        TestExecutor::new(engine, Box::new(machine)).unwrap()
    }

    fn new_executor_with_actor(id: ActorID, balance: TokenAmount) -> TestExecutor {
        let mut exec = new_executor();

        let account_code = *exec.builtin_actors().get_account_code();
        let mut actor = ActorState::new_empty(account_code, None);
        actor.balance = balance;
        exec.state_tree_mut().set_actor(id, actor);

        exec
    }

    #[test]
    fn begin_empty_plan_is_noop() {
        let mut exec = new_executor();

        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.open);
            assert!(session.reservations.is_empty());
        }

        exec.begin_reservation_session(&[]).unwrap();

        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.open);
            assert!(session.reservations.is_empty());
        }

        // Ending without an open session yields SessionClosed.
        assert_eq!(
            exec.end_reservation_session().unwrap_err(),
            ReservationError::SessionClosed
        );
    }

    #[test]
    fn begin_and_end_with_zero_remainder_succeeds() {
        let sender: ActorID = 1000;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));

        let plan = vec![(Address::new_id(sender), TokenAmount::from_atto(500u64))];

        exec.begin_reservation_session(&plan).unwrap();
        {
            let mut session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(session.open);
            assert_eq!(
                session.reservations.get(&sender).cloned().unwrap(),
                TokenAmount::from_atto(500u64)
            );

            // Simulate full consumption of all reservations so the session can end cleanly.
            for amt in session.reservations.values_mut() {
                *amt = TokenAmount::zero();
            }
        }

        exec.end_reservation_session().unwrap();
        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.open);
            assert!(session.reservations.is_empty());
        }
    }

    #[test]
    fn begin_twice_errors_with_session_open() {
        let sender: ActorID = 42;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));
        let plan = vec![(Address::new_id(sender), TokenAmount::from_atto(100u64))];

        exec.begin_reservation_session(&plan).unwrap();
        let err = exec.begin_reservation_session(&plan).unwrap_err();
        assert_eq!(err, ReservationError::SessionOpen);
    }

    #[test]
    fn end_with_non_zero_remainder_errors() {
        let sender: ActorID = 7;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));
        let plan = vec![(Address::new_id(sender), TokenAmount::from_atto(1234u64))];

        exec.begin_reservation_session(&plan).unwrap();
        // Reservations are still non-zero, so ending should fail.
        let err = exec.end_reservation_session().unwrap_err();
        assert_eq!(err, ReservationError::NonZeroRemainder);
    }

    #[test]
    fn plan_too_large_by_sender_count() {
        let mut exec = new_executor();

        const MAX_SENDERS: usize = 65_536;
        let mut plan = Vec::with_capacity(MAX_SENDERS + 1);
        for i in 0..=MAX_SENDERS {
            plan.push((Address::new_id(i as u64), TokenAmount::from_atto(1u64)));
        }

        let err = exec.begin_reservation_session(&plan).unwrap_err();
        assert_eq!(err, ReservationError::PlanTooLarge);
    }

    #[test]
    fn insufficient_funds_at_begin() {
        let sender: ActorID = 5;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(10u64));
        let plan = vec![(Address::new_id(sender), TokenAmount::from_atto(11u64))];

        let err = exec.begin_reservation_session(&plan).unwrap_err();
        assert_eq!(err, ReservationError::InsufficientFundsAtBegin { sender });
    }

    #[test]
    fn unknown_actor_in_plan_yields_reservation_invariant() {
        let mut exec = new_executor();
        let sender: ActorID = 9999;
        let plan = vec![(Address::new_id(sender), TokenAmount::from_atto(1u64))];

        let err = exec.begin_reservation_session(&plan).unwrap_err();
        match err {
            ReservationError::ReservationInvariant(msg) => {
                assert!(msg.contains(&format!("unknown actor {}", sender)));
            }
            other => panic!(
                "expected ReservationInvariant for unknown actor, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn preflight_with_reservations_does_not_deduct_balance() {
        let sender: ActorID = 1001;
        let initial_balance = TokenAmount::from_atto(1_000_000u64);
        let mut exec = new_executor_with_actor(sender, initial_balance.clone());

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        let gas_limit = inclusion_total + 100;

        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;

        let plan = vec![(Address::new_id(sender), gas_cost.clone())];
        exec.begin_reservation_session(&plan).unwrap();

        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 0,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let res = exec
            .preflight_message(&msg, ApplyKind::Explicit, raw_length)
            .unwrap();

        let (seen_sender, seen_gas_cost, _inclusion) =
            res.expect("expected successful preflight under reservations");
        assert_eq!(seen_sender, sender);
        assert_eq!(seen_gas_cost, gas_cost);

        // Balance is untouched in reservation mode; only the nonce is incremented.
        let actor = exec
            .state_tree()
            .get_actor(sender)
            .unwrap()
            .expect("actor must exist");
        assert_eq!(actor.balance, initial_balance);
        assert_eq!(actor.sequence, 1);

        // Reservation ledger is unchanged; consumption happens during settlement.
        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert_eq!(
                session.reservations.get(&sender).cloned().unwrap(),
                gas_cost
            );
        }
    }

    #[test]
    fn preflight_prevalidation_failure_decrements_reservation_and_allows_zero_remainder() {
        let sender: ActorID = 2000;
        let initial_balance = TokenAmount::from_atto(1_000_000u64);
        let mut exec = new_executor_with_actor(sender, initial_balance.clone());

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        let gas_limit = inclusion_total + 100;

        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;

        exec.begin_reservation_session(&[(Address::new_id(sender), gas_cost.clone())])
            .unwrap();

        // Use an incorrect nonce to trigger prevalidation failure.
        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 42,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let res = exec
            .preflight_message(&msg, ApplyKind::Explicit, raw_length)
            .unwrap();

        let apply_ret = res.expect_err("expected prevalidation failure for bad nonce");
        assert_eq!(
            apply_ret.msg_receipt.exit_code,
            ExitCode::SYS_SENDER_STATE_INVALID
        );

        // Actor state is unchanged on prevalidation failure.
        let actor = exec
            .state_tree()
            .get_actor(sender)
            .unwrap()
            .expect("actor must exist");
        assert_eq!(actor.balance, initial_balance);
        assert_eq!(actor.sequence, 0);

        // Reservation for this sender is fully released, allowing the session to end with zero
        // remainder.
        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.reservations.contains_key(&sender));
        }
        exec.end_reservation_session().unwrap();
        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.open);
        }
    }

    #[test]
    fn preflight_negative_fee_cap_yields_overflow() {
        let sender: ActorID = 2500;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        let gas_limit = inclusion_total + 100;

        // Construct a message with a negative gas_fee_cap so that the
        // computed gas_cost is negative and triggers the overflow guard.
        let gas_fee_cap = TokenAmount::from_atto(-1i64);

        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 0,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let res = exec.preflight_message(&msg, ApplyKind::Explicit, raw_length);

        match res {
            Ok(_) => panic!("expected fatal overflow error for negative gas fee cap"),
            Err(err) => {
                let reservation_err = err
                    .downcast_ref::<ReservationError>()
                    .expect("expected ReservationError");
                assert_eq!(reservation_err, &ReservationError::Overflow);
            }
        }
    }

    #[test]
    fn preflight_inclusion_too_low_decrements_reservation() {
        let sender: ActorID = 3000;
        let initial_balance = TokenAmount::from_atto(1_000_000u64);
        let mut exec = new_executor_with_actor(sender, initial_balance.clone());

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        assert!(inclusion_total > 0);
        let gas_limit = inclusion_total - 1;

        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;

        exec.begin_reservation_session(&[(Address::new_id(sender), gas_cost.clone())])
            .unwrap();

        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 0,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let res = exec
            .preflight_message(&msg, ApplyKind::Explicit, raw_length)
            .unwrap();

        let apply_ret = res.expect_err("expected prevalidation failure for inclusion gas too low");
        assert_eq!(apply_ret.msg_receipt.exit_code, ExitCode::SYS_OUT_OF_GAS);

        // Actor state is unchanged on prevalidation failure.
        let actor = exec
            .state_tree()
            .get_actor(sender)
            .unwrap()
            .expect("actor must exist");
        assert_eq!(actor.balance, initial_balance);
        assert_eq!(actor.sequence, 0);

        // Reservation is released, so the session can end with zero remainder.
        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.reservations.contains_key(&sender));
        }
        exec.end_reservation_session().unwrap();
    }

    #[test]
    fn reservation_coverage_violation_yields_reservation_invariant() {
        let sender: ActorID = 4000;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        let gas_limit = inclusion_total + 100;

        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;
        let under_reserved = &gas_cost - &TokenAmount::from_atto(1u64);

        exec.begin_reservation_session(&[(Address::new_id(sender), under_reserved.clone())])
            .unwrap();

        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 0,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let res = exec.preflight_message(&msg, ApplyKind::Explicit, raw_length);

        match res {
            Ok(_) => panic!("expected fatal error for reservation coverage violation"),
            Err(err) => {
                let reservation_err = err
                    .downcast_ref::<ReservationError>()
                    .expect("expected ReservationError");
                match reservation_err {
                    ReservationError::ReservationInvariant(msg) => {
                        assert!(msg.contains("below gas cost"));
                    }
                    other => panic!("expected ReservationInvariant, got {:?}", other),
                }
            }
        }
    }

    #[test]
    fn reservation_prevalidation_decrement_underflow_yields_overflow() {
        let sender: ActorID = 5000;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));

        // Manually open a reservation session with an under-sized reservation to trigger
        // arithmetic underflow when we attempt to decrement it.
        {
            let mut session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            session.open = true;
            session
                .reservations
                .insert(sender, TokenAmount::from_atto(1u64));
        }

        let raw_length = 100usize;
        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_limit = 2u64;

        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 1,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let res = exec.preflight_message(&msg, ApplyKind::Explicit, raw_length);

        match res {
            Ok(_) => panic!("expected fatal error from reservation underflow"),
            Err(err) => {
                let reservation_err = err
                    .downcast_ref::<ReservationError>()
                    .expect("expected ReservationError");
                assert_eq!(reservation_err, &ReservationError::Overflow);
            }
        }
    }

    #[test]
    fn transfer_enforces_reservations_for_message_send() {
        let sender: ActorID = 6000;
        let receiver: ActorID = 6001;
        let mut exec = new_executor();

        let account_code = *exec.builtin_actors().get_account_code();

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        let gas_limit = inclusion_total * 10 + 1_000;

        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;

        // Choose value and balance such that:
        // - balance >= gas_cost (reservation begin succeeds).
        // - value + gas_cost > balance (free < value, so transfer must fail).
        let value = TokenAmount::from_atto(10u64);
        let balance = &gas_cost + &value - &TokenAmount::from_atto(1u64);

        let mut sender_state = ActorState::new_empty(account_code, None);
        sender_state.balance = balance.clone();
        exec.state_tree_mut().set_actor(sender, sender_state);

        let mut receiver_state = ActorState::new_empty(account_code, None);
        receiver_state.balance = TokenAmount::zero();
        exec.state_tree_mut().set_actor(receiver, receiver_state);

        exec.begin_reservation_session(&[(Address::new_id(sender), gas_cost.clone())])
            .unwrap();

        let engine = exec.engine_pool.acquire();
        let reservation_session = exec.reservation_session.clone();

        let res = exec.map_machine(|machine| {
            let mut cm = TestCallManager::new(
                machine,
                engine,
                gas_limit,
                sender,
                Address::new_id(sender),
                Some(receiver),
                Address::new_id(receiver),
                0,
                TokenAmount::zero(),
                reservation_session,
            );

            let transfer_res = cm.transfer(sender, receiver, &value);
            let (_, machine) = cm.finish();
            (transfer_res, machine)
        });

        match res {
            Ok(()) => panic!("expected transfer to fail with insufficient funds"),
            Err(ExecutionError::Syscall(err)) => {
                assert_eq!(err.1, ErrorNumber::InsufficientFunds);
            }
            Err(other) => panic!("unexpected error from transfer: {:?}", other),
        }
    }

    #[test]
    fn send_enforces_reservations_for_existing_actor() {
        let sender: ActorID = 8000;
        let receiver: ActorID = 8001;
        let mut exec = new_executor();

        let account_code = *exec.builtin_actors().get_account_code();

        let raw_length = 100usize;
        let pl = &exec.context().price_list;
        let inclusion_cost = pl.on_chain_message(raw_length);
        let inclusion_total = inclusion_cost.total().round_up();
        let gas_limit = inclusion_total * 10 + 1_000;

        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;

        let value = TokenAmount::from_atto(10u64);
        let balance = &gas_cost + &value - &TokenAmount::from_atto(1u64);

        let mut sender_state = ActorState::new_empty(account_code, None);
        sender_state.balance = balance.clone();
        exec.state_tree_mut().set_actor(sender, sender_state);

        let mut receiver_state = ActorState::new_empty(account_code, None);
        receiver_state.balance = TokenAmount::zero();
        exec.state_tree_mut().set_actor(receiver, receiver_state);

        exec.begin_reservation_session(&[(Address::new_id(sender), gas_cost.clone())])
            .unwrap();

        let engine = exec.engine_pool.acquire();
        let reservation_session = exec.reservation_session.clone();

        type SendKernel = DefaultKernel<TestCallManager>;

        let res = exec.map_machine(|machine| {
            let cm = TestCallManager::new(
                machine,
                engine,
                gas_limit,
                sender,
                Address::new_id(sender),
                Some(sender),
                Address::new_id(sender),
                0,
                TokenAmount::zero(),
                reservation_session,
            );

            let blocks = BlockRegistry::new();
            let mut kernel = <SendKernel as Kernel>::new(
                cm,
                blocks,
                sender,
                sender,
                METHOD_SEND,
                TokenAmount::zero(),
                false,
            );

            let send_res = SendOps::<SendKernel>::send(
                &mut kernel,
                &Address::new_id(receiver),
                METHOD_SEND,
                NO_DATA_BLOCK_ID,
                &value,
                None,
                SendFlags::empty(),
            );

            let (cm, _blocks) = kernel.into_inner();
            let (_, machine) = cm.finish();
            (send_res, machine)
        });

        match res {
            Ok(_) => panic!("expected send to fail with insufficient funds"),
            Err(ExecutionError::Syscall(err)) => {
                assert_eq!(err.1, ErrorNumber::InsufficientFunds);
            }
            Err(other) => panic!("unexpected error from send: {:?}", other),
        }
    }

    #[test]
    fn self_destruct_enforces_reservations() {
        let sender: ActorID = 8100;
        let mut exec = new_executor();

        let account_code = *exec.builtin_actors().get_account_code();
        let initial_balance = TokenAmount::from_atto(1_000_000u64);

        let mut sender_state = ActorState::new_empty(account_code, None);
        sender_state.balance = initial_balance.clone();
        exec.state_tree_mut().set_actor(sender, sender_state);

        let mut burnt_state = ActorState::new_empty(account_code, None);
        burnt_state.balance = TokenAmount::zero();
        exec.state_tree_mut()
            .set_actor(BURNT_FUNDS_ACTOR_ID, burnt_state);

        let reserved = TokenAmount::from_atto(100u64);
        exec.begin_reservation_session(&[(Address::new_id(sender), reserved.clone())])
            .unwrap();

        let engine = exec.engine_pool.acquire();
        let reservation_session = exec.reservation_session.clone();

        type SelfDestructKernel = DefaultKernel<TestCallManager>;

        let res = exec.map_machine(|machine| {
            let cm = TestCallManager::new(
                machine,
                engine,
                1_000_000,
                sender,
                Address::new_id(sender),
                Some(sender),
                Address::new_id(sender),
                0,
                TokenAmount::zero(),
                reservation_session,
            );

            let blocks = BlockRegistry::new();
            let mut kernel = <SelfDestructKernel as Kernel>::new(
                cm,
                blocks,
                sender,
                sender,
                METHOD_SEND,
                TokenAmount::zero(),
                false,
            );

            let sd_res = SelfOps::self_destruct(&mut kernel, true);

            let (cm, _blocks) = kernel.into_inner();
            let (_, machine) = cm.finish();
            (sd_res, machine)
        });

        match res {
            Ok(()) => panic!("expected self_destruct to fail with insufficient funds"),
            Err(ExecutionError::Syscall(err)) => {
                assert_eq!(err.1, ErrorNumber::InsufficientFunds);
            }
            Err(other) => panic!("unexpected error from self_destruct: {:?}", other),
        }

        // Actor is still present and balance unchanged because the transaction was reverted.
        let actor = exec
            .state_tree()
            .get_actor(sender)
            .unwrap()
            .expect("sender actor must exist");
        assert_eq!(actor.balance, initial_balance);
    }

    #[test]
    fn settlement_under_reservations_net_charges_and_clears_ledger() {
        let sender: ActorID = 7000;
        let initial_balance = TokenAmount::from_atto(1_000_000u64);
        let base_fee = TokenAmount::from_atto(10u64);
        let mut exec = new_executor_with_base_fee(base_fee);

        // Install sender, burnt-funds, and reward actors so settlement can move funds.
        let account_code = *exec.builtin_actors().get_account_code();

        let mut sender_state = ActorState::new_empty(account_code, None);
        sender_state.balance = initial_balance.clone();
        exec.state_tree_mut().set_actor(sender, sender_state);

        let mut burnt_state = ActorState::new_empty(account_code, None);
        burnt_state.balance = TokenAmount::zero();
        exec.state_tree_mut()
            .set_actor(BURNT_FUNDS_ACTOR_ID, burnt_state);

        let mut reward_state = ActorState::new_empty(account_code, None);
        reward_state.balance = TokenAmount::zero();
        exec.state_tree_mut()
            .set_actor(REWARD_ACTOR_ID, reward_state);

        let gas_limit = 1_000u64;
        let gas_fee_cap = TokenAmount::from_atto(1u64);
        let gas_cost = gas_fee_cap.clone() * gas_limit;

        exec.begin_reservation_session(&[(Address::new_id(sender), gas_cost.clone())])
            .unwrap();

        let msg = Message {
            version: 0,
            from: Address::new_id(sender),
            to: Address::new_id(1),
            sequence: 0,
            value: TokenAmount::zero(),
            method_num: 0,
            params: RawBytes::default(),
            gas_limit,
            gas_fee_cap: gas_fee_cap.clone(),
            gas_premium: TokenAmount::zero(),
        };

        let gas_used = gas_limit / 2;

        let receipt = Receipt {
            exit_code: ExitCode::OK,
            return_data: RawBytes::default(),
            gas_used,
            events_root: None,
        };

        let apply_ret = exec
            .finish_message(
                sender,
                msg,
                receipt,
                None,
                gas_cost.clone(),
                vec![],
                Vec::new(),
            )
            .expect("finish_message must succeed");

        let base_fee_burn = apply_ret.base_fee_burn.clone();
        let over_estimation_burn = apply_ret.over_estimation_burn.clone();
        let miner_tip = apply_ret.miner_tip.clone();
        let refund = apply_ret.refund.clone();

        // GasOutputs invariants: base_fee_burn + over_estimation_burn + refund + miner_tip ==
        // gas_cost.
        assert_eq!(
            &base_fee_burn + &over_estimation_burn + &refund + &miner_tip,
            gas_cost
        );

        let consumption = &base_fee_burn + &over_estimation_burn + &miner_tip;

        // Net sender balance delta equals the gas consumption.
        let actor = exec
            .state_tree()
            .get_actor(sender)
            .unwrap()
            .expect("sender actor must exist");
        assert_eq!(actor.balance, initial_balance - consumption.clone());

        // Burns and tips are deposited to the appropriate actors.
        let burnt_actor = exec
            .state_tree()
            .get_actor(BURNT_FUNDS_ACTOR_ID)
            .unwrap()
            .expect("burnt funds actor must exist");
        assert_eq!(burnt_actor.balance, &base_fee_burn + &over_estimation_burn);

        let reward_actor = exec
            .state_tree()
            .get_actor(REWARD_ACTOR_ID)
            .unwrap()
            .expect("reward actor must exist");
        assert_eq!(reward_actor.balance, miner_tip);

        // The reservation ledger is fully cleared for this sender so the session can end with zero
        // remainder.
        {
            let session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            assert!(!session.reservations.contains_key(&sender));
        }

        exec.end_reservation_session().unwrap();
    }

    #[test]
    #[ignore]
    fn reservation_begin_end_performance_smoke() {
        use std::time::Instant;

        let sender_base: ActorID = 10_000;
        let num_senders: u64 = 10_000;
        let mut exec = new_executor();

        let account_code = *exec.builtin_actors().get_account_code();
        let balance = TokenAmount::from_atto(1_000_000u64);

        for offset in 0..num_senders {
            let id = sender_base + offset;
            let mut actor = ActorState::new_empty(account_code, None);
            actor.balance = balance.clone();
            exec.state_tree_mut().set_actor(id, actor);
        }

        let reservation = TokenAmount::from_atto(1_000u64);
        let mut plan = Vec::with_capacity(num_senders as usize);
        for offset in 0..num_senders {
            let id = sender_base + offset;
            plan.push((Address::new_id(id), reservation.clone()));
        }

        let begin_start = Instant::now();
        exec.begin_reservation_session(&plan).unwrap();
        let begin_duration = begin_start.elapsed();

        {
            let mut session = exec
                .reservation_session
                .lock()
                .expect("reservation session mutex poisoned");
            for amt in session.reservations.values_mut() {
                *amt = TokenAmount::zero();
            }
        }

        let end_start = Instant::now();
        exec.end_reservation_session().unwrap();
        let end_duration = end_start.elapsed();

        println!(
            "reservation_begin_end_performance_smoke: begin_ms={} end_ms={} senders={}",
            begin_duration.as_secs_f64() * 1000.0,
            end_duration.as_secs_f64() * 1000.0,
            num_senders
        );
    }

    #[cfg(feature = "arb")]
    #[test]
    fn gas_outputs_quickcheck_invariants_hold() {
        use quickcheck::{QuickCheck, TestResult};

        fn prop(
            gas_limit: u64,
            gas_used_seed: u64,
            fee_cap: TokenAmount,
            premium: TokenAmount,
        ) -> TestResult {
            if gas_limit == 0 {
                return TestResult::discard();
            }

            // Ensure 0 <= gas_used <= gas_limit without overflowing when gas_limit == u64::MAX.
            let gas_used = gas_used_seed % gas_limit.saturating_add(1);

            // Constrain fee_cap and premium to be non-negative to match protocol assumptions.
            let fee_cap = if fee_cap.is_negative() {
                -fee_cap
            } else {
                fee_cap
            };
            let premium = if premium.is_negative() {
                -premium
            } else {
                premium
            };

            let base_fee = TokenAmount::from_atto(10u64);

            let outputs = GasOutputs::compute(gas_used, gas_limit, &base_fee, &fee_cap, &premium);

            // All gas accounting components must be non-negative.
            if outputs.base_fee_burn.is_negative()
                || outputs.over_estimation_burn.is_negative()
                || outputs.miner_penalty.is_negative()
                || outputs.miner_tip.is_negative()
                || outputs.refund.is_negative()
            {
                return TestResult::failed();
            }

            // Gas outputs must conserve the total required funds.
            let gas_cost = fee_cap.clone() * gas_limit;
            if (&outputs.base_fee_burn
                + &outputs.over_estimation_burn
                + &outputs.refund
                + &outputs.miner_tip)
                != gas_cost
            {
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .quickcheck(prop as fn(u64, u64, TokenAmount, TokenAmount) -> TestResult);
    }

    #[test]
    fn negative_reservation_amount_fails() {
        let sender: ActorID = 50;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1000u64));
        // Construct a negative amount.
        // TokenAmount is a wrapper around BigInt.
        let negative_amt = TokenAmount::from_atto(100u64) - TokenAmount::from_atto(200u64);
        assert!(negative_amt.is_negative());

        let plan = vec![(Address::new_id(sender), negative_amt.clone())];

        let err = exec.begin_reservation_session(&plan).unwrap_err();
        match err {
            ReservationError::ReservationInvariant(msg) => {
                assert!(msg.contains("negative reservation amount"));
            }
            other => panic!("expected ReservationInvariant, got {:?}", other),
        }

        // Verify telemetry recorded the failure
        let session = exec
            .reservation_session
            .lock()
            .expect("reservation session mutex poisoned");
        assert_eq!(session.telemetry.reservation_begin_failed, 1);
    }

    #[test]
    fn telemetry_state_updates() {
        let sender: ActorID = 60;
        let mut exec = new_executor_with_actor(sender, TokenAmount::from_atto(1_000_000u64));
        let plan = vec![(Address::new_id(sender), TokenAmount::from_atto(500u64))];

        // 1. Test failure increment
        // Use a too-large plan to force failure
        {
            let poor_sender = 61;
            let account_code = *exec.builtin_actors().get_account_code();
            exec.state_tree_mut()
                .set_actor(poor_sender, ActorState::new_empty(account_code, None));
            let fail_plan = vec![(Address::new_id(poor_sender), TokenAmount::from_atto(10u64))];

            exec.begin_reservation_session(&fail_plan).unwrap_err();

            let session = exec.reservation_session.lock().unwrap();
            assert_eq!(session.telemetry.reservation_begin_failed, 1);
            assert_eq!(session.telemetry.reservations_open, 0);
        }

        // 2. Test success increment
        exec.begin_reservation_session(&plan).unwrap();
        {
            let session = exec.reservation_session.lock().unwrap();
            assert_eq!(session.telemetry.reservation_begin_failed, 1); // unchanged
            assert_eq!(session.telemetry.reservations_open, 1);
            assert_eq!(session.telemetry.reservation_total_per_sender.len(), 1);
            assert_eq!(session.telemetry.reserved_remaining_per_sender.len(), 1);
        }

        // 3. Test end session decrement
        // We must clear reservations manually to allow end_session (simulating consumption)
        {
            let mut session = exec.reservation_session.lock().unwrap();
            session.reservations.clear();
        }

        exec.end_reservation_session().unwrap();
        {
            let session = exec.reservation_session.lock().unwrap();
            assert_eq!(session.telemetry.reservations_open, 0);
            assert!(session.telemetry.reservation_total_per_sender.is_empty());
            assert!(session.telemetry.reserved_remaining_per_sender.is_empty());
        }
    }
}
