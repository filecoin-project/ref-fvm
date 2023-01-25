// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::rc::Rc;

use anyhow::{anyhow, Context};
use cid::Cid;
use derive_more::{Deref, DerefMut};
use fvm_ipld_encoding::{to_vec, RawBytes, CBOR};
use fvm_shared::address::{Address, Payload};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::event::StampedEvent;
use fvm_shared::sys::BlockId;
use fvm_shared::{ActorID, MethodNum, METHOD_SEND};
use num_traits::Zero;

use super::{Backtrace, CallManager, InvocationResult, NO_DATA_BLOCK_ID};
use crate::call_manager::backtrace::Frame;
use crate::call_manager::FinishRet;
use crate::eam_actor::EAM_ACTOR_ID;
use crate::engine::Engine;
use crate::gas::{Gas, GasTimer, GasTracker};
use crate::kernel::{Block, BlockRegistry, ExecutionError, Kernel, Result, SyscallError};
use crate::machine::limiter::MemoryLimiter;
use crate::machine::Machine;
use crate::state_tree::ActorState;
use crate::syscalls::error::Abort;
use crate::syscalls::{charge_for_exec, update_gas_available};
use crate::trace::{ExecutionEvent, ExecutionTrace};
use crate::{syscall_error, system_actor};

/// The default [`CallManager`] implementation.
#[repr(transparent)]
pub struct DefaultCallManager<M: Machine>(Option<Box<InnerDefaultCallManager<M>>>);

#[doc(hidden)]
#[derive(Deref, DerefMut)]
pub struct InnerDefaultCallManager<M: Machine> {
    /// The machine this kernel is attached to.
    #[deref]
    #[deref_mut]
    machine: M,
    /// The engine with which to execute the message.
    engine: Rc<Engine>,
    /// The gas tracker.
    gas_tracker: GasTracker,
    /// The gas premium paid by this message.
    gas_premium: TokenAmount,
    /// The ActorID and the address of the original sender of the chain message that initiated
    /// this call stack.
    origin: ActorID,
    /// The origin address as specified in the message (used to derive new f2 addresses).
    origin_address: Address,
    /// The nonce of the chain message that initiated this call stack.
    nonce: u64,
    /// Number of actors created in this call stack.
    num_actors_created: u64,
    /// Current call-stack depth.
    call_stack_depth: u32,
    /// The current chain of errors, if any.
    backtrace: Backtrace,
    /// The current execution trace.
    exec_trace: ExecutionTrace,
    /// Number of actors that have been invoked in this message execution.
    invocation_count: u64,
    /// Limits on memory throughout the execution.
    limits: M::Limiter,
    /// Accumulator for events emitted in this call stack.
    events: EventsAccumulator,
}

#[doc(hidden)]
impl<M: Machine> std::ops::Deref for DefaultCallManager<M> {
    type Target = InnerDefaultCallManager<M>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("call manager is poisoned")
    }
}

#[doc(hidden)]
impl<M: Machine> std::ops::DerefMut for DefaultCallManager<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("call manager is poisoned")
    }
}

impl<M> CallManager for DefaultCallManager<M>
where
    M: Machine,
{
    type Machine = M;

    fn new(
        machine: M,
        engine: Engine,
        gas_limit: i64,
        origin: ActorID,
        origin_address: Address,
        nonce: u64,
        gas_premium: TokenAmount,
    ) -> Self {
        let limits = machine.new_limiter();
        let gas_tracker =
            GasTracker::new(Gas::new(gas_limit), Gas::zero(), machine.context().tracing);

        DefaultCallManager(Some(Box::new(InnerDefaultCallManager {
            engine: Rc::new(engine),
            machine,
            gas_tracker,
            gas_premium,
            origin,
            origin_address,
            nonce,
            num_actors_created: 0,
            call_stack_depth: 0,
            backtrace: Backtrace::default(),
            exec_trace: vec![],
            invocation_count: 0,
            limits,
            events: Default::default(),
        })))
    }

    fn limiter_mut(&mut self) -> &mut <Self::Machine as Machine>::Limiter {
        &mut self.limits
    }

    fn send<K>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: Option<Block>,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
    ) -> Result<InvocationResult>
    where
        K: Kernel<CallManager = Self>,
    {
        if self.machine.context().tracing {
            self.trace(ExecutionEvent::Call {
                from,
                to,
                method,
                params: params
                    .as_ref()
                    .map(|blk| blk.data().to_owned().into())
                    .unwrap_or_default(),
                value: value.clone(),
            });
        }

        // If a specific gas limit has been requested, push a new limit into the gas tracker.
        if let Some(limit) = gas_limit {
            self.gas_tracker.push_limit(limit);
        }

        let mut result =
            self.with_stack_frame(|s| s.send_unchecked::<K>(from, to, method, params, value));

        // If we pushed a limit, pop it.
        if gas_limit.is_some() {
            self.gas_tracker.pop_limit()?;
        }
        // If we're not out of gas but the error is "out of gas" (e.g., due to a gas limit), replace
        // the error with an explicit exit code.
        if !self.gas_tracker.gas_available().is_zero()
            && matches!(result, Err(ExecutionError::OutOfGas))
        {
            result = Ok(InvocationResult {
                exit_code: ExitCode::SYS_OUT_OF_GAS,
                value: None,
            })
        }

        if self.machine.context().tracing {
            self.trace(match &result {
                Ok(InvocationResult { exit_code, value }) => ExecutionEvent::CallReturn(
                    *exit_code,
                    value
                        .as_ref()
                        .map(|blk| RawBytes::from(blk.data().to_vec()))
                        .unwrap_or_default(),
                ),
                Err(ExecutionError::OutOfGas) => {
                    ExecutionEvent::CallReturn(ExitCode::SYS_OUT_OF_GAS, RawBytes::default())
                }
                Err(ExecutionError::Fatal(_)) => {
                    ExecutionEvent::CallError(SyscallError::new(ErrorNumber::Forbidden, "fatal"))
                }
                Err(ExecutionError::Syscall(s)) => ExecutionEvent::CallError(s.clone()),
            });
        }

        result
    }

    fn with_transaction(
        &mut self,
        read_only: bool,
        f: impl FnOnce(&mut Self) -> Result<InvocationResult>,
    ) -> Result<InvocationResult> {
        self.state_tree_mut().begin_transaction(read_only);
        self.events.begin_transaction(read_only);

        let (revert, res) = match f(self) {
            Ok(v) => (!v.exit_code.is_success(), Ok(v)),
            Err(e) => (true, Err(e)),
        };

        self.state_tree_mut().end_transaction(revert)?;
        self.events.end_transaction(revert)?;

        res
    }

    fn finish(mut self) -> (FinishRet, Self::Machine) {
        let InnerDefaultCallManager {
            machine,
            backtrace,
            gas_tracker,
            mut exec_trace,
            events,
            ..
        } = *self.0.take().expect("call manager is poisoned");

        // TODO: Having to check against zero here is fishy, but this is what lotus does.
        let gas_used = gas_tracker.gas_used().max(Gas::zero()).round_up();

        // Finalize any trace events, if we're tracing.
        if machine.context().tracing {
            exec_trace.extend(gas_tracker.drain_trace().map(ExecutionEvent::GasCharge));
        }

        let events = events.finish();

        (
            FinishRet {
                gas_used,
                backtrace,
                exec_trace,
                events,
            },
            machine,
        )
    }

    // Accessor methods so the trait can implement some common methods by default.

    fn machine(&self) -> &Self::Machine {
        &self.machine
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        &mut self.machine
    }

    fn engine(&self) -> &Engine {
        &self.engine
    }

    fn gas_tracker(&self) -> &GasTracker {
        &self.gas_tracker
    }

    fn gas_premium(&self) -> &TokenAmount {
        &self.gas_premium
    }

    // Other accessor methods

    fn origin(&self) -> ActorID {
        self.origin
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    fn next_actor_address(&self) -> Address {
        // Base the next address on the address specified as the message origin. This lets us use,
        // e.g., an f2 address even if we can't look it up anywhere.
        //
        // Of course, if the user decides to send from an f0 address without waiting for finality,
        // their "stable" address may not be as stable as they'd like. But that's their problem.
        //
        // In case you're wondering: but what if someone _else_ is relying on the stability of this
        // address? They shouldn't be. The sender can always _replace_ a message with a new message,
        // and completely change how f2 addresses are assigned. Only the message sender can rely on
        // an f2 address (before finality).
        let mut b = to_vec(&self.origin_address).expect("failed to serialize address");
        b.extend_from_slice(&self.nonce.to_be_bytes());
        b.extend_from_slice(&self.num_actors_created.to_be_bytes());
        Address::new_actor(&b)
    }

    fn create_actor(
        &mut self,
        code_id: Cid,
        actor_id: ActorID,
        delegated_address: Option<Address>,
    ) -> Result<()> {
        let start = GasTimer::start();

        if self.machine.builtin_actors().is_placeholder_actor(&code_id) {
            return Err(syscall_error!(
                Forbidden,
                "cannot explicitly construct a placeholder actor"
            )
            .into());
        }

        // Check to make sure the actor doesn't exist, or is a placeholder.
        let (actor, is_new) = match self.machine.state_tree().get_actor(actor_id)? {
            // Replace the placeholder
            Some(mut act)
                if self
                    .machine
                    .builtin_actors()
                    .is_placeholder_actor(&act.code) =>
            {
                if act.delegated_address.is_none() {
                    // The FVM made a mistake somewhere.
                    return Err(ExecutionError::Fatal(anyhow!(
                        "placeholder {actor_id} doesn't have a delegated address"
                    )));
                }
                if act.delegated_address != delegated_address {
                    // The Init actor made a mistake?
                    return Err(syscall_error!(
                        Forbidden,
                        "placeholder has a different delegated address"
                    )
                    .into());
                }
                act.code = code_id;
                (act, false)
            }
            // Don't replace anything else.
            Some(_) => {
                return Err(syscall_error!(Forbidden; "Actor address already exists").into());
            }
            // Create a new actor.
            None => (ActorState::new_empty(code_id, delegated_address), true),
        };
        let t = self.charge_gas(self.price_list().on_create_actor(is_new))?;
        self.state_tree_mut().set_actor(actor_id, actor)?;
        self.num_actors_created += 1;
        t.stop_with(start);
        Ok(())
    }

    fn append_event(&mut self, evt: StampedEvent) {
        self.events.append_event(evt)
    }

    // Helper for creating actors. This really doesn't belong on this trait.
    fn invocation_count(&self) -> u64 {
        self.invocation_count
    }
}

impl<M> DefaultCallManager<M>
where
    M: Machine,
{
    fn trace(&mut self, trace: ExecutionEvent) {
        // The price of deref magic is that you sometimes need to tell the compiler: no, this is
        // fine.
        let s = &mut **self;

        s.exec_trace
            .extend(s.gas_tracker.drain_trace().map(ExecutionEvent::GasCharge));

        s.exec_trace.push(trace);
    }

    fn create_account_actor<K>(&mut self, addr: &Address) -> Result<ActorID>
    where
        K: Kernel<CallManager = Self>,
    {
        let t = self.charge_gas(self.price_list().on_create_actor(true))?;

        if addr.is_bls_zero_address() {
            return Err(
                syscall_error!(IllegalArgument; "cannot create the bls zero address actor").into(),
            );
        }

        // Create the actor in the state tree.
        let id = {
            let code_cid = self.builtin_actors().get_account_code();
            let state = ActorState::new_empty(*code_cid, None);
            self.machine.create_actor(addr, state)?
        };

        // Now invoke the constructor; first create the parameters, then
        // instantiate a new kernel to invoke the constructor.
        let params = to_vec(&addr).map_err(|e| {
            // This shouldn't happen, but we treat it as an illegal argument error and move on.
            // It _likely_ means that the inputs were invalid in some unexpected way.
            log::error!(
                "failed to serialize address when creating actor, ignoring: {}",
                e
            );
            syscall_error!(IllegalArgument; "failed to serialize params: {}", e)
        })?;

        // The cost of sending the message is measured independently.
        t.stop();

        self.send_resolved::<K>(
            system_actor::SYSTEM_ACTOR_ID,
            id,
            fvm_shared::METHOD_CONSTRUCTOR,
            Some(Block::new(CBOR, params)),
            &TokenAmount::zero(),
        )?;

        Ok(id)
    }

    fn create_placeholder_actor<K>(&mut self, addr: &Address) -> Result<ActorID>
    where
        K: Kernel<CallManager = Self>,
    {
        let t = self.charge_gas(self.price_list().on_create_actor(true))?;

        // Create the actor in the state tree, but don't call any constructor.
        let code_cid = self.builtin_actors().get_placeholder_code();

        let state = ActorState::new_empty(*code_cid, Some(*addr));
        t.record(self.machine.create_actor(addr, state))
    }

    /// Send without checking the call depth.
    fn send_unchecked<K>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: Option<Block>,
        value: &TokenAmount,
    ) -> Result<InvocationResult>
    where
        K: Kernel<CallManager = Self>,
    {
        // Get the receiver; this will resolve the address.
        let to = match self.state_tree().lookup_id(&to)? {
            Some(addr) => addr,
            None => match to.payload() {
                Payload::BLS(_) | Payload::Secp256k1(_) => {
                    // Try to create an account actor if the receiver is a key address.
                    self.create_account_actor::<K>(&to)?
                }
                // Validate that there's an actor at the target ID (we don't care what is there,
                // just that something is there).
                Payload::Delegated(da) if da.namespace() == EAM_ACTOR_ID => {
                    self.create_placeholder_actor::<K>(&to)?
                }
                _ => return Err(
                    syscall_error!(NotFound; "actor does not exist or cannot be created: {}", to)
                        .into(),
                ),
            },
        };

        self.send_resolved::<K>(from, to, method, params, value)
    }

    /// Send with resolved addresses.
    fn send_resolved<K>(
        &mut self,
        from: ActorID,
        to: ActorID,
        method: MethodNum,
        params: Option<Block>,
        value: &TokenAmount,
    ) -> Result<InvocationResult>
    where
        K: Kernel<CallManager = Self>,
    {
        // Lookup the actor.
        let state = self
            .state_tree()
            .get_actor(to)?
            .ok_or_else(|| syscall_error!(NotFound; "actor does not exist: {}", to))?;

        // Charge the method gas. Not sure why this comes second, but it does.
        let _ = self.charge_gas(self.price_list().on_method_invocation(value, method))?;

        // Transfer, if necessary.
        if !value.is_zero() {
            self.machine.transfer(from, to, value)?;
        }

        // Abort early if we have a send.
        if method == METHOD_SEND {
            log::trace!("sent {} -> {}: {}", from, to, &value);
            return Ok(InvocationResult::default());
        }

        // Store the parametrs, and initialize the block registry for the target actor.
        let mut block_registry = BlockRegistry::new();
        let params_id = if let Some(blk) = params {
            block_registry.put(blk)?
        } else {
            NO_DATA_BLOCK_ID
        };

        // Increment invocation count
        self.invocation_count += 1;

        // Ensure that actor's code is loaded and cached in the engine.
        // NOTE: this does not cover the EVM smart contract actor, which is a built-in actor, is
        // listed the manifest, and therefore preloaded during system initialization.
        #[cfg(feature = "m2-native")]
        self.engine
            .prepare_actor_code(&state.code, self.blockstore())
            .map_err(
                |_| syscall_error!(NotFound; "actor code cid does not exist {}", &state.code),
            )?;

        log::trace!("calling {} -> {}::{}", from, to, method);
        self.map_mut(|cm| {
            let engine = cm.engine.clone(); // reference the RC.

            // Make the kernel.
            let kernel = K::new(cm, block_registry, from, to, method, value.clone());

            // Make a store.
            let mut store = engine.new_store(kernel);

            // From this point on, there are no more syscall errors, only aborts.
            let result: std::result::Result<BlockId, Abort> = (|| {
                // Instantiate the module.
                let instance = engine
                    .instantiate(&mut store, &state.code)?
                    .context("actor not found")
                    .map_err(Abort::Fatal)?;

                // Resolve and store a reference to the exported memory.
                let memory = instance
                    .get_memory(&mut store, "memory")
                    .context("actor has no memory export")
                    .map_err(Abort::Fatal)?;

                store.data_mut().memory = memory;

                // Lookup the invoke method.
                let invoke: wasmtime::TypedFunc<(u32,), u32> = instance
                    .get_typed_func(&mut store, "invoke")
                    // All actors will have an invoke method.
                    .map_err(Abort::Fatal)?;

                // Set the available gas.
                update_gas_available(&mut store)?;

                // Invoke it.
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    invoke.call(&mut store, (params_id,))
                }))
                .map_err(|panic| Abort::Fatal(anyhow!("panic within actor: {:?}", panic)))?;

                // Charge for any remaining uncharged execution gas, returning an error if we run out.
                charge_for_exec(&mut store)?;

                // If the invocation failed due to running out of exec_units, we have already
                // detected it and returned OutOfGas above. Any other invocation failure is returned
                // here as an Abort
                Ok(res?)
            })();

            let invocation_data = store.into_data();
            let last_error = invocation_data.last_error;
            let (mut cm, block_registry) = invocation_data.kernel.into_inner();

            // Resolve the return block's ID into an actual block, converting to an abort if it
            // doesn't exist.
            let result = result.and_then(|ret_id| {
                Ok(if ret_id == NO_DATA_BLOCK_ID {
                    None
                } else {
                    Some(block_registry.get(ret_id).map_err(|_| {
                        Abort::Exit(
                            ExitCode::SYS_MISSING_RETURN,
                            String::from("returned block does not exist"),
                            NO_DATA_BLOCK_ID,
                        )
                    })?)
                })
            });

            // Process the result, updating the backtrace if necessary.
            let ret = match result {
                Ok(ret) => Ok(InvocationResult {
                    exit_code: ExitCode::OK,
                    value: ret.cloned(),
                }),
                Err(abort) => {
                    let (code, message, res) = match abort {
                        Abort::Exit(code, message, NO_DATA_BLOCK_ID) => (
                            code,
                            message,
                            Ok(InvocationResult {
                                exit_code: code,
                                value: None,
                            }),
                        ),
                        Abort::Exit(code, message, blk_id) => match block_registry.get(blk_id) {
                            Err(e) => (
                                ExitCode::SYS_MISSING_RETURN,
                                "error getting exit data block".to_owned(),
                                Err(ExecutionError::Fatal(anyhow!(e))),
                            ),
                            Ok(blk) => (
                                code,
                                message,
                                Ok(InvocationResult {
                                    exit_code: code,
                                    value: Some(blk.clone()),
                                }),
                            ),
                        },
                        Abort::OutOfGas => (
                            ExitCode::SYS_OUT_OF_GAS,
                            "out of gas".to_owned(),
                            Err(ExecutionError::OutOfGas),
                        ),
                        Abort::Fatal(err) => (
                            ExitCode::SYS_ASSERTION_FAILED,
                            "fatal error".to_owned(),
                            Err(ExecutionError::Fatal(err)),
                        ),
                    };

                    if !code.is_success() {
                        if let Some(err) = last_error {
                            cm.backtrace.begin(err);
                        }

                        cm.backtrace.push_frame(Frame {
                            source: to,
                            method,
                            message,
                            code,
                        });
                    }

                    res
                }
            };

            // Log the results if tracing is enabled.
            if log::log_enabled!(log::Level::Trace) {
                match &ret {
                    Ok(val) => log::trace!(
                        "returning {}::{} -> {} ({})",
                        to,
                        method,
                        from,
                        val.exit_code
                    ),
                    Err(e) => log::trace!("failing {}::{} -> {} (err:{})", to, method, from, e),
                }
            }

            (ret, cm)
        })
    }

    /// Temporarily replace `self` with a version that contains `None` for the inner part,
    /// to be able to hand over ownership of `self` to a new kernel, while the older kernel
    /// has a reference to the hollowed out version.
    fn map_mut<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(Self) -> (T, Self),
    {
        replace_with::replace_with_and_return(self, || DefaultCallManager(None), f)
    }

    /// Check that we're not violating the call stack depth, then envelope a call
    /// with an increase/decrease of the depth to make sure none of them are missed.
    fn with_stack_frame<F, V>(&mut self, f: F) -> Result<V>
    where
        F: FnOnce(&mut Self) -> Result<V>,
    {
        if self.call_stack_depth >= self.machine.context().max_call_depth {
            let sys_err = syscall_error!(LimitExceeded, "message execution exceeds call depth");
            if self.machine.context().tracing {
                self.trace(ExecutionEvent::CallError(sys_err.clone()));
            }
            return Err(sys_err.into());
        }

        self.call_stack_depth += 1;
        let res = <<<DefaultCallManager<M> as CallManager>::Machine as Machine>::Limiter>::with_stack_frame(
            self,
            |s| s.limiter_mut(),
            f,
        );
        self.call_stack_depth -= 1;
        res
    }
}

/// Stores events in layers as they are emitted by actors. As the call stack progresses, when an
/// actor exits normally, its events should be merged onto the previous layer (merge_last_layer).
/// If an actor aborts, the last layer should be discarded (discard_last_layer). This will also
/// throw away any events collected from subcalls (and previously merged, as those subcalls returned
/// normally).
#[derive(Default)]
pub struct EventsAccumulator {
    events: Vec<StampedEvent>,
    idxs: Vec<usize>,
    read_only_layers: u32,
}

impl EventsAccumulator {
    fn is_read_only(&self) -> bool {
        self.read_only_layers > 0
    }

    fn append_event(&mut self, evt: StampedEvent) {
        if !self.is_read_only() {
            self.events.push(evt)
        }
    }

    fn begin_transaction(&mut self, read_only: bool) {
        if read_only || self.is_read_only() {
            self.read_only_layers += 1;
        } else {
            self.idxs.push(self.events.len());
        }
    }

    fn end_transaction(&mut self, revert: bool) -> Result<()> {
        if self.is_read_only() {
            self.read_only_layers -= 1;
        } else {
            let idx = self.idxs.pop().ok_or_else(|| {
                ExecutionError::Fatal(anyhow!(
                    "no index in the event accumulator when ending a transaction"
                ))
            })?;
            if revert {
                self.events.truncate(idx);
            }
        }
        Ok(())
    }

    fn finish(self) -> Vec<StampedEvent> {
        // Ideally would assert here, but there's risk of poisoning the Machine.
        // Cannot return a Result because the call site expects infallibility.
        // assert!(self.idxs.is_empty());
        self.events
    }
}
