use derive_more::{Deref, DerefMut};
use fvm_shared::address::{Address, Protocol};
use fvm_shared::encoding::{RawBytes, DAG_CBOR};
use fvm_shared::error::ExitCode;
use fvm_shared::sys::TokenAmount;
use fvm_shared::{ActorID, MethodNum, METHOD_SEND};
use num_traits::Zero;
use wasmtime::{Linker, Store};

use super::{CallManager, InvocationResult, NO_DATA_BLOCK_ID};
use crate::gas::GasTracker;
use crate::kernel::{ClassifyResult, Kernel, Result, SyscallError};
use crate::machine::{CallError, Machine};
use crate::syscall_error;
use crate::syscalls::bind_syscalls;
use crate::syscalls::error::unwrap_trap;

/// The DefaultCallManager manages a single call stack.
///
/// When a top-level message is executed:
///
/// 1. The machine creates a call manager for that message, giving itself to the call manager.
/// 2. The machine calls the call specified actor/method using the call manager.
/// 3. The call manager then executes the actual actor code.
/// 4. If an actor calls another actor, the kernel will:
///    1. Detach the call manager from itself.
///    2. Call `send` on the call manager to execute the new message.
///    3. Re-attach the call manager.
///    4. Return.
#[repr(transparent)]
pub struct DefaultCallManager<M>(Option<InnerDefaultCallManager<M>>);

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
    backtrace: Vec<CallError>,
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
        DefaultCallManager(Some(InnerDefaultCallManager {
            machine,
            gas_tracker: GasTracker::new(gas_limit, 0),
            origin,
            nonce,
            num_actors_created: 0,
            call_stack_depth: 0,
            backtrace: Vec::new(),
        }))
    }

    fn send<K>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: &RawBytes,
        value: TokenAmount,
    ) -> Result<InvocationResult>
    where
        K: Kernel<CallManager = Self>,
    {
        if self.call_stack_depth > self.machine.config().max_call_depth {
            return Err(
                syscall_error!(SysErrForbidden, "message execution exceeds call depth").into(),
            );
        }
        self.call_stack_depth += 1;
        let result = self.send_unchecked::<K>(from, to, method, params, value);
        self.call_stack_depth -= 1;
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

    fn finish(mut self) -> (i64, Vec<CallError>, Self::Machine) {
        let gas_used = self.gas_tracker.gas_used().max(0);

        let inner = self.0.take().expect("call manager is poisoned");
        // TODO: Having to check against zero here is fishy, but this is what lotus does.
        (gas_used, inner.backtrace, inner.machine)
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

    // Helpers for error tracing.

    fn push_error(&mut self, e: CallError) {
        self.backtrace.push(e);
    }

    fn clear_error(&mut self) {
        self.backtrace.clear();
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
            return Err(SyscallError::new(
                ExitCode::SysErrIllegalArgument,
                "cannot create the bls zero address actor",
            )
            .into());
        }

        // Create the actor in the state tree.
        let act = crate::account_actor::ZERO_STATE.clone();
        let id = self.create_actor(addr, act)?;

        // Now invoke the constructor; first create the parameters, then
        // instantiate a new kernel to invoke the constructor.
        let params = RawBytes::serialize(&addr)
            // TODO(#198) this should be a Sys actor error, but we're copying lotus here.
            .map_err(|e| syscall_error!(ErrSerialization; "failed to serialize params: {}", e))?;

        self.send_resolved::<K>(
            crate::account_actor::SYSTEM_ACTOR_ID,
            id,
            fvm_shared::METHOD_CONSTRUCTOR,
            &params,
            TokenAmount::from(0u64),
        )?;

        Ok(id)
    }

    /// Send without checking the call depth.
    fn send_unchecked<K>(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: &RawBytes,
        value: TokenAmount,
    ) -> Result<InvocationResult>
    where
        K: Kernel<CallManager = Self>,
    {
        // Get the receiver; this will resolve the address.
        // TODO: What kind of errors should we be using here?
        let to = match self.state_tree().lookup_id(&to)? {
            Some(addr) => addr,
            None => match to.protocol() {
                Protocol::BLS | Protocol::Secp256k1 => {
                    // Try to create an account actor if the receiver is a key address.
                    self.create_account_actor::<K>(&to)?
                }
                _ => {
                    return Err(
                        syscall_error!(SysErrInvalidReceiver; "actor does not exist: {}", to)
                            .into(),
                    );
                }
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
        params: &RawBytes,
        value: TokenAmount,
    ) -> Result<InvocationResult>
    where
        K: Kernel<CallManager = Self>,
    {
        // Lookup the actor.
        let state = self
            .state_tree()
            .get_actor_id(to)?
            .ok_or_else(|| syscall_error!(SysErrInvalidReceiver; "actor does not exist: {}", to))?;

        // Charge the method gas. Not sure why this comes second, but it does.
        self.charge_gas(self.price_list().on_method_invocation(value, method))?;

        // Transfer, if necessary.
        if !value.is_zero() {
            self.machine.transfer(from, to, value)?;
        }

        // Abort early if we have a send.
        if method == METHOD_SEND {
            return Ok(InvocationResult::Return(Default::default()));
        }

        // Finally, handle the code.
        let module = self.load_module(&state.code)?;

        // This is a cheap operation as it doesn't actually clone the struct,
        // it returns a referenced copy.
        let engine = self.engine().clone();

        // Create a new linker.
        let mut linker = Linker::new(&engine);
        bind_syscalls(&mut linker).or_fatal()?;

        self.map_mut(|cm| {
            // Make the kernel/store.
            let kernel = K::new(cm, from, to, method, value.clone());
            let mut store = Store::new(&engine, kernel);

            log::trace!("calling {} -> {}::{}", from, to, method);

            let result = (|| {
                // Load parameters, if there are any.
                let param_id = if params.len() > 0 {
                    store.data_mut().block_create(DAG_CBOR, params)?
                } else {
                    super::NO_DATA_BLOCK_ID
                };

                // Instantiate the module.
                let instance = linker.instantiate(&mut store, &module).or_fatal()?;

                // Invoke it.
                let invoke = instance.get_typed_func(&mut store, "invoke").or_fatal()?;
                let return_block_id: u32 = match invoke.call(&mut store, (param_id,)) {
                    Ok((block,)) => block,
                    Err(e) => return unwrap_trap(e),
                };

                // Extract the return value, if there is one.
                let return_value: RawBytes = if return_block_id > NO_DATA_BLOCK_ID {
                    let (code, ret) = store.data().block_get(return_block_id)?;
                    debug_assert_eq!(code, DAG_CBOR);
                    RawBytes::new(ret)
                } else {
                    RawBytes::default()
                };

                Ok(InvocationResult::Return(return_value))
            })();

            match &result {
                Ok(val) => {
                    log::trace!(
                        "returning {}::{} -> {} ({})",
                        to,
                        method,
                        from,
                        val.exit_code()
                    );
                }
                Err(e) => log::trace!("failing {}::{} -> {} (err:{})", to, method, from, e),
            }

            (result, store.into_data().take())
        })
    }

    fn map_mut<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(Self) -> (T, Self),
    {
        replace_with::replace_with_and_return(self, || DefaultCallManager(None), f)
    }
}
