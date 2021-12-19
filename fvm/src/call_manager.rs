use blockstore::Blockstore;
use derive_more::{Deref, DerefMut};
use fvm_shared::{
    address::{Address, Protocol},
    econ::TokenAmount,
    encoding::{RawBytes, DAG_CBOR},
    error::ExitCode,
    ActorID, MethodNum, METHOD_SEND,
};
use num_traits::Zero;
use wasmtime::{Linker, Store};

use crate::{
    externs::Externs,
    gas::{GasCharge, GasTracker},
    kernel::{BlockOps, ClassifyResult, Result, SyscallError},
    machine::Machine,
    receipt::Receipt,
    syscall_error,
    syscalls::{bind_syscalls, error::unwrap_trap},
    DefaultKernel,
};

/// The CallManager manages a single call stack.
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
pub struct CallManager<B: 'static, E: 'static>(Option<InnerCallManager<B, E>>);

#[doc(hidden)]
#[derive(Deref, DerefMut)]
pub struct InnerCallManager<B: 'static, E: 'static> {
    /// The machine this kernel is attached to.
    #[deref]
    #[deref_mut]
    machine: Machine<B, E>,
    /// The gas tracker.
    gas_tracker: GasTracker,
    /// The original sender of the chain message that initiated this call stack.
    origin: Address,
    /// The nonce of the chain message that initiated this call stack.
    nonce: u64,
    /// Number of actors created in this call stack.
    num_actors_created: u64,
}

#[doc(hidden)]
impl<B: 'static, E: 'static> std::ops::Deref for CallManager<B, E> {
    type Target = InnerCallManager<B, E>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("call manager is poisoned")
    }
}

#[doc(hidden)]
impl<B: 'static, E: 'static> std::ops::DerefMut for CallManager<B, E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("call manager is poisoned")
    }
}

impl<B: 'static, E: 'static> CallManager<B, E>
where
    B: Blockstore,
    E: Externs,
{
    /// Construct a new call manager. This should be called by the machine.
    pub(crate) fn new(machine: Machine<B, E>, gas_limit: i64, origin: Address, nonce: u64) -> Self {
        CallManager(Some(InnerCallManager {
            machine,
            gas_tracker: GasTracker::new(gas_limit, 0),
            origin,
            nonce,
            num_actors_created: 0,
        }))
    }

    fn create_account_actor(&mut self, addr: &Address) -> Result<ActorID> {
        self.charge_gas(self.context().price_list().on_create_actor())?;

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
            // TODO this should be a Sys actor error, but we're copying ltous here.
            .map_err(|e| syscall_error!(ErrSerialization; "failed to serialize params: {}", e))?;

        self.send_resolved(
            crate::account_actor::SYSTEM_ACTOR_ID,
            id,
            fvm_shared::METHOD_CONSTRUCTOR,
            &params,
            &TokenAmount::from(0u32),
        )?;

        Ok(id)
    }

    /// Send a message to an actor.
    ///
    /// This method does not create any transactions, that's the caller's responsibility.
    pub fn send(
        &mut self,
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: &RawBytes,
        value: &TokenAmount,
    ) -> Result<Receipt> {
        // Get the receiver; this will resolve the address.
        // TODO: What kind of errors should we be using here?
        let to = match self.state_tree().lookup_id(&to)? {
            Some(addr) => addr,
            None => match to.protocol() {
                Protocol::BLS | Protocol::Secp256k1 => {
                    // Try to create an account actor if the receiver is a key address.
                    self.create_account_actor(&to)?
                }
                _ => {
                    return Err(
                        syscall_error!(SysErrInvalidReceiver; "actor does not exist: {}", to)
                            .into(),
                    )
                }
            },
        };

        // Do the actual send.

        self.send_resolved(from, to, method, params, value)
    }

    /// Send with resolved addresses.
    fn send_resolved(
        &mut self,
        from: ActorID,
        to: ActorID,
        method: MethodNum,
        params: &RawBytes,
        value: &TokenAmount,
    ) -> Result<Receipt> {
        // 1. Lookup the actor.
        let state = self
            .state_tree()
            .get_actor_id(to)?
            .ok_or_else(|| syscall_error!(SysErrInvalidReceiver; "actor does not exist: {}", to))?;

        // 2. Charge the method gas. Not sure why this comes second, but it does.
        self.charge_gas(
            self.context()
                .price_list()
                .on_method_invocation(value, method),
        )?;

        // 3. Transfer, if necessary.
        if !value.is_zero() {
            self.machine.transfer(from, to, value)?;
        }

        // 4. Abort early if we have a send.
        if method == METHOD_SEND {
            return Ok(Receipt {
                exit_code: ExitCode::Ok,
                return_data: Default::default(),
                gas_used: 0,
            });
        }

        // 3. Finally, handle the code.

        let module = self.load_module(&state.code)?;

        // This is a cheap operation as it doesn't actually clone the struct,
        // it returns a referenced copy.
        let engine = self.engine().clone();

        // Create a new linker.
        let mut linker = Linker::new(&engine);
        bind_syscalls(&mut linker).or_fatal()?;

        self.map_mut(|cm| {
            // Make the kernel/store.
            let kernel = DefaultKernel::new(cm, from, to, method, value.clone());
            let mut store = Store::new(&engine, kernel);

            let result = (|| {
                // Load parameters.
                let param_id = store.data_mut().block_create(DAG_CBOR, params)?;

                // Instantiate the module.
                let instance = linker.instantiate(&mut store, &module).or_fatal()?;

                // Invoke it.
                let invoke = instance.get_typed_func(&mut store, "invoke").or_fatal()?;
                let return_block_id: u32 = match invoke.call(&mut store, (param_id,)) {
                    Ok((block,)) => block,
                    Err(e) => return unwrap_trap(e),
                };

                let (code, ret) = store.data().block_get(return_block_id)?;
                debug_assert_eq!(code, DAG_CBOR);
                Ok(Receipt {
                    return_data: RawBytes::new(ret),
                    exit_code: ExitCode::Ok,
                    gas_used: 0,
                })
            })();

            (result, store.into_data().take())
        })
    }

    /// Finishes execution, returning the gas used and the machine.
    pub fn finish(mut self) -> (i64, Machine<B, E>) {
        let gas_used = self.gas_used().max(0);

        let inner = self.0.take().expect("call manager is poisoned");
        // TODO: Having to check against zero here is fishy, but this is what lotus does.
        (gas_used, inner.machine)
    }

    /// Charge gas.
    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<()> {
        self.gas_tracker.charge_gas(charge)?;
        Ok(())
    }

    /// Returns the available gas.
    pub fn gas_available(&self) -> i64 {
        self.gas_tracker.gas_available()
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> i64 {
        self.gas_tracker.gas_used()
    }

    /// Getter for origin actor.
    pub fn origin(&self) -> Address {
        self.origin
    }

    /// Getter for message nonce.
    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    /// Gets and increment the call-stack actor creation index.
    pub fn next_actor_idx(&mut self) -> u64 {
        let ret = self.num_actors_created;
        self.num_actors_created += 1;
        ret
    }

    fn map_mut<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(Self) -> (T, Self),
    {
        replace_with::replace_with_and_return(self, || CallManager(None), f)
    }
}
