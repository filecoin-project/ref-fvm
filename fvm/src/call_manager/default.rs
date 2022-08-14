use anyhow::{anyhow, Context};
use derive_more::{Deref, DerefMut};
use fvm_ipld_encoding::{to_vec, RawBytes, DAG_CBOR};
use fvm_shared::actor::builtin::Type;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::sys::BlockId;
use fvm_shared::{ActorID, MethodNum, METHOD_SEND};
use num_traits::Zero;

use super::{Backtrace, CallManager, InvocationResult, NO_DATA_BLOCK_ID};
use crate::call_manager::backtrace::Frame;
use crate::call_manager::FinishRet;
use crate::gas::{Gas, GasTracker};
use crate::kernel::{Block, BlockRegistry, ExecutionError, Kernel, Result, SyscallError};
use crate::machine::Machine;
use crate::syscalls::error::Abort;
use crate::syscalls::{charge_for_exec, update_gas_available};
use crate::trace::{ExecutionEvent, ExecutionTrace};
use crate::{account_actor, syscall_error};

/// The default [`CallManager`] implementation.
#[repr(transparent)]
pub struct DefaultCallManager<M>(Option<Box<InnerDefaultCallManager<M>>>);

#[doc(hidden)]
#[derive(Deref, DerefMut)]
pub struct InnerDefaultCallManager<M> {
    /// The machine this kernel is attached to.
    #[deref]
    #[deref_mut]
    machine: M,
    /// The gas tracker.
    gas_tracker: GasTracker,
    /// The original sender of the chain message that initiated this call stack.
    origin: Address,
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
}

#[doc(hidden)]
impl<M> std::ops::Deref for DefaultCallManager<M> {
    type Target = InnerDefaultCallManager<M>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("call manager is poisoned")
    }
}

#[doc(hidden)]
impl<M> std::ops::DerefMut for DefaultCallManager<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("call manager is poisoned")
    }
}

impl<M> CallManager for DefaultCallManager<M>
where
    M: Machine,
{
    type Machine = M;

    fn new(machine: M, gas_limit: i64, origin: Address, nonce: u64) -> Self {
        DefaultCallManager(Some(Box::new(InnerDefaultCallManager {
            machine,
            gas_tracker: GasTracker::new(Gas::new(gas_limit), Gas::zero()),
            origin,
            nonce,
            num_actors_created: 0,
            call_stack_depth: 0,
            backtrace: Backtrace::default(),
            exec_trace: vec![],
            invocation_count: 0,
        })))
    }

    fn send<K>(
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
        if self.machine.context().tracing {
            self.exec_trace.push(ExecutionEvent::Call {
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

        // We check _then_ set because we don't count the top call. This effectivly allows a
        // call-stack depth of `max_call_depth + 1` (or `max_call_depth` sub-calls). While this is
        // likely a bug, this is how NV15 behaves so we mimic that behavior here.
        //
        // By example:
        //
        // 1. If the max depth is 0, call_stack_depth will be 1 and the top-level message won't be
        //    able to make sub-calls (1 > 0).
        // 2. If the max depth is 1, the call_stack_depth will be 1 in the top-level message, 2 in
        //    sub-calls, and said sub-calls will not be able to make further subcalls (2 > 1).
        //
        // NOTE: Unlike the FVM, Lotus adds _then_ checks. It does this because the
        // `call_stack_depth` in lotus is 0 for the top-level call, unlike in the FVM where it's 1.
        if self.call_stack_depth > self.machine.context().max_call_depth {
            let sys_err = syscall_error!(LimitExceeded, "message execution exceeds call depth");
            if self.machine.context().tracing {
                self.exec_trace
                    .push(ExecutionEvent::CallError(sys_err.clone()))
            }
            return Err(sys_err.into());
        }
        self.call_stack_depth += 1;
        let result = self.send_unchecked::<K>(from, to, method, params, value);
        self.call_stack_depth -= 1;

        if self.machine.context().tracing {
            self.exec_trace.push(match &result {
                Ok(InvocationResult::Return(v)) => ExecutionEvent::CallReturn(
                    v.as_ref()
                        .map(|blk| RawBytes::from(blk.data().to_vec()))
                        .unwrap_or_default(),
                ),
                Ok(InvocationResult::Failure(code)) => ExecutionEvent::CallAbort(*code),

                Err(ExecutionError::OutOfGas) => ExecutionEvent::CallError(SyscallError::new(
                    ErrorNumber::Forbidden,
                    "out of gas",
                )),
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
        f: impl FnOnce(&mut Self) -> Result<InvocationResult>,
    ) -> Result<InvocationResult> {
        self.state_tree_mut().begin_transaction();
        let (revert, res) = match f(self) {
            Ok(v) => (!v.exit_code().is_success(), Ok(v)),
            Err(e) => (true, Err(e)),
        };
        self.state_tree_mut().end_transaction(revert)?;
        res
    }

    fn finish(mut self) -> (FinishRet, Self::Machine) {
        // TODO: Having to check against zero here is fishy, but this is what lotus does.
        let gas_used = self.gas_tracker.gas_used().max(Gas::zero()).round_up();

        let inner = self.0.take().expect("call manager is poisoned");
        (
            FinishRet {
                gas_used,
                backtrace: inner.backtrace,
                exec_trace: inner.exec_trace,
            },
            inner.machine,
        )
    }

    // Accessor methods so the trait can implement some common methods by default.

    fn machine(&self) -> &Self::Machine {
        &self.machine
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        &mut self.machine
    }

    fn gas_tracker(&self) -> &GasTracker {
        &self.gas_tracker
    }

    fn gas_tracker_mut(&mut self) -> &mut GasTracker {
        &mut self.gas_tracker
    }

    // Other accessor methods

    fn origin(&self) -> Address {
        self.origin
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    // Helper for creating actors. This really doesn't belong on this trait.

    fn next_actor_idx(&mut self) -> u64 {
        let ret = self.num_actors_created;
        self.num_actors_created += 1;
        ret
    }

    fn invocation_count(&self) -> u64 {
        self.invocation_count
    }
}

impl<M> DefaultCallManager<M>
where
    M: Machine,
{
    fn create_account_actor<K>(&mut self, addr: &Address) -> Result<ActorID>
    where
        K: Kernel<CallManager = Self>,
    {
        self.charge_gas(self.price_list().on_create_actor())?;

        if addr.is_bls_zero_address() {
            return Err(
                syscall_error!(IllegalArgument; "cannot create the bls zero address actor").into(),
            );
        }

        // Create the actor in the state tree.
        let id = {
            let code_cid = self
                .builtin_actors()
                .get_by_right(&Type::Account)
                .expect("failed to determine account actor CodeCID");
            let state = account_actor::zero_state(*code_cid);
            self.create_actor(addr, state)?
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

        self.send_resolved::<K>(
            account_actor::SYSTEM_ACTOR_ID,
            id,
            fvm_shared::METHOD_CONSTRUCTOR,
            Some(Block::new(DAG_CBOR, params)),
            &TokenAmount::from(0u32),
        )?;

        Ok(id)
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
            None => match to.protocol() {
                Protocol::BLS | Protocol::Secp256k1 => {
                    // Try to create an account actor if the receiver is a key address.
                    self.create_account_actor::<K>(&to)?
                }
                _ => return Err(syscall_error!(NotFound; "actor does not exist: {}", to).into()),
            },
        };

        // Do the actual send.

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
            .get_actor_id(to)?
            .ok_or_else(|| syscall_error!(NotFound; "actor does not exist: {}", to))?;

        // Charge the method gas. Not sure why this comes second, but it does.
        self.charge_gas(self.price_list().on_method_invocation(value, method))?;

        // Transfer, if necessary.
        if !value.is_zero() {
            self.machine.transfer(from, to, value)?;
        }

        // Abort early if we have a send.
        if method == METHOD_SEND {
            log::trace!("sent {} -> {}: {}", from, to, &value);
            return Ok(InvocationResult::Return(Default::default()));
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

        // This is a cheap operation as it doesn't actually clone the struct,
        // it returns a referenced copy.
        let engine = self.engine().clone();

        log::trace!("calling {} -> {}::{}", from, to, method);
        self.map_mut(|cm| {
            // Make the kernel.
            let kernel = K::new(cm, block_registry, from, to, method, value.clone());

            // Make a store.
            let mut store = engine.new_store(kernel);

            // From this point on, there are no more syscall errors, only aborts.
            let result: std::result::Result<BlockId, Abort> = (|| {
                // Instantiate the module.
                let instance = engine
                    .get_instance(&mut store, &state.code)
                    .and_then(|i| i.context("actor code not found"))
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

                // Charge for any remaining uncharged execution gas, returning an error if we run
                // out.
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
                        )
                    })?)
                })
            });

            // Process the result, updating the backtrace if necessary.
            let ret = match result {
                Ok(ret) => Ok(InvocationResult::Return(ret.cloned())),
                Err(abort) => {
                    if let Some(err) = last_error {
                        cm.backtrace.begin(err);
                    }

                    let (code, message, res) = match abort {
                        Abort::Exit(code, message) => {
                            (code, message, Ok(InvocationResult::Failure(code)))
                        }
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

                    cm.backtrace.push_frame(Frame {
                        source: to,
                        method,
                        message,
                        code,
                    });

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
                        val.exit_code()
                    ),
                    Err(e) => log::trace!("failing {}::{} -> {} (err:{})", to, method, from, e),
                }
            }

            (ret, cm)
        })
    }

    fn map_mut<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(Self) -> (T, Self),
    {
        replace_with::replace_with_and_return(self, || DefaultCallManager(None), f)
    }
}
