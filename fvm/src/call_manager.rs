use blockstore::Blockstore;
use derive_more::{Deref, DerefMut};
use fvm_shared::{
    actor_error,
    address::{Address, Protocol},
    econ::TokenAmount,
    encoding::{RawBytes, DAG_CBOR},
    error::ActorError,
    ActorID,
};
use num_traits::Zero;
use wasmtime::{Linker, Module, Store};

use crate::{
    externs::Externs,
    gas::{GasCharge, GasTracker},
    kernel::{BlockOps, MethodId},
    machine::Machine,
    syscalls::bind_syscalls,
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
#[derive(Deref, DerefMut)]
pub struct CallManager<B: 'static, E: 'static> {
    from: ActorID,
    /// The machine this kernel is attached to.
    #[deref]
    #[deref_mut]
    machine: Box<Machine<B, E>>,
    /// The gas tracker.
    gas_tracker: GasTracker,
}

impl<B: 'static, E: 'static> CallManager<B, E>
where
    B: Blockstore,
    E: Externs,
{
    /// Construct a new call manager. This should be called by the machine.
    pub(crate) fn new(machine: Box<Machine<B, E>>, from: ActorID, gas_limit: i64) -> Self {
        Self {
            from,
            machine,
            gas_tracker: GasTracker::new(gas_limit, 0),
        }
    }

    fn create_account_actor(mut self, addr: &Address) -> (Result<ActorID, ActorError>, Self) {
        macro_rules! t {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(e) => return (Err(e.into()), self),
                }
            };
        }
        t!(self.charge_gas(self.context().price_list().on_create_actor()));

        if addr.is_bls_zero_address() {
            return (
                Err(
                    actor_error!(SysErrIllegalArgument; "cannot create the bls zero address actor"),
                ),
                self,
            );
        }

        // Create the actor in the state tree.
        let act = crate::account_actor::ZERO_STATE.clone();
        let id = t!(self.create_actor(addr, act));

        // Now invoke the constructor; first create the parameters, then
        // instantiate a new kernel to invoke the constructor.
        let params = t!(RawBytes::serialize(&addr).map_err(|e| {
            actor_error!(fatal(
                "couldn't serialize params for actor construction: {:?}",
                e
            ))
        }));

        let (res, s) = self.send_explicit(
            crate::account_actor::SYSTEM_ACTOR_ID,
            id,
            fvm_shared::METHOD_CONSTRUCTOR,
            params,
            TokenAmount::from(0u32),
        );

        (res.map(|_| id), s)
    }

    /// Send a message to an actor.
    ///
    /// Creates a state tree snapshot prior to executing the send. Discards the
    /// snapshot if the destination actor aborts.
    pub fn send(
        mut self,
        to: Address,
        method: MethodId,
        // TODO: in the future, we'll need to pass more than one block as params.
        params: RawBytes,
        value: TokenAmount,
    ) -> (Result<RawBytes, ActorError>, Self) {
        // TODO: lotus doesn't do snapshots inside send, it does them outside. I prefer it this way,
        // but there may be reasons...?
        self.state_tree_mut().snapshot();

        // Call the target actor; revert the state tree changes if the call fails.
        let (res, s) = self.send_inner(to, method, params, value);
        self = s;
        match if res.is_ok() {
            self.state_tree_mut().commit_snapshot()
        } else {
            self.state_tree_mut().revert_snapshot()
        } {
            Ok(()) => (res, self),
            Err(e) => (Err(actor_error!(fatal(e))), self),
        }
    }

    /// The inner send function that doesn't snapshot.
    fn send_inner(
        mut self,
        to: Address,
        method: MethodId,
        // TODO: in the future, we'll need to pass more than one block as params.
        params: RawBytes,
        value: TokenAmount,
    ) -> (Result<RawBytes, ActorError>, Self) {
        // Get the receiver; this will resolve the address.
        // TODO: What kind of errors should we be using here?
        let to = match self
            .state_tree()
            .lookup_id(&to)
            .map_err(|e| actor_error!(fatal(e)))
        {
            Ok(Some(addr)) => addr,
            Ok(None) => match to.protocol() {
                Protocol::BLS | Protocol::Secp256k1 => {
                    // Try to create an account actor if the receiver is a key address.
                    let id_addr = match self.create_account_actor(&to) {
                        (Ok(res), s) => {
                            self = s;
                            res
                        }
                        (Err(e), s) => return (Err(e), s),
                    };
                    id_addr
                }
                _ => return (Err(actor_error!(fatal("actor not found: {}", to))), self),
            },
            Err(e) => return (Err(e.into()), self),
        };

        // Do the actual send.

        self.send_resolved(to, method, params, value)
    }

    /// Send with an explicit from. Used when we need to do an internal send with a different
    /// "from".
    ///
    /// NOTE: DOES NOT SNAPSHOT!
    fn send_explicit(
        mut self,
        from: ActorID,
        to: ActorID,
        method: MethodId,
        params: RawBytes,
        value: TokenAmount,
    ) -> (Result<RawBytes, ActorError>, Self) {
        let prev_from = self.from;
        self.from = from;
        let (res, mut s) = self.send_resolved(to, method, params, value);
        s.from = prev_from;
        (res, s)
    }

    /// Send with resolved addresses.
    ///
    /// NOTE: DOES NOT SNAPSHOT!
    fn send_resolved(
        mut self,
        to: ActorID,
        method: MethodId,
        params: RawBytes,
        value: TokenAmount,
    ) -> (Result<RawBytes, ActorError>, Self) {
        macro_rules! t {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(e) => return (Err(e.into()), self),
                }
            };
        }

        // 1. Setup the engine/linker. TODO: move these into the machine?

        // This is a cheap operation as it doesn't actually clone the struct,
        // it returns a referenced copy.
        let engine = self.engine().clone();

        // Create a new linker.
        let mut linker = Linker::new(&engine);
        t!(bind_syscalls(&mut linker).map_err(|e| actor_error!(fatal(e))));

        let to_addr = Address::new_id(to);

        // 2. Lookup the actor.
        // TODO: should we let the kernel do this? We could _ask_ the kernel for the code to
        //  execute?
        let mut state = t!(t!(self.state_tree().get_actor(&to_addr))
            .ok_or_else(|| actor_error!(fatal("actor does not exist: {}", to))));

        let module = t!(self
            .machine
            .load_module(&state.code)
            .map_err(|e| actor_error!(fatal(e))));

        // 2. Update balance.
        if !value.is_zero() {
            state.balance += value.clone();
            t!(self.state_tree_mut().set_actor(&to_addr, state));
        }

        // 3. Construct a kernel.

        // TODO: Make the kernel pluggable.
        let from = self.from.clone();
        let mut kernel = DefaultKernel::new(self, from, to, method, value);

        // 4. Load parameters.

        // TODO: This copies the block. Ideally, we'd give ownership.
        let param_id = match kernel.block_create(DAG_CBOR, &params) {
            Ok(id) => id,
            Err(e) => return (Err(actor_error!(fatal(e))), kernel.take()),
        };

        // TODO: BELOW ERROR HANDLING IS BROKEN.
        // We should put it in a new function.

        // 3. Instantiate the module.
        let mut store = Store::new(&engine, kernel);
        // TODO error handling.
        let instance = linker.instantiate(&mut store, &module).unwrap();

        // 4. Invoke it.
        // TODO error handling.
        let invoke = instance.get_typed_func(&mut store, "invoke").unwrap();
        // TODO error handling.
        let (return_block_id,): (u32,) = invoke.call(&mut store, (param_id)).unwrap();

        // 5. Recover return value.
        let kernel = store.into_data();

        // TODO: this is a nasty API. We should have a nicer way to just "get a block".
        // TODO error handling.
        let ret_stat = kernel.block_stat(return_block_id).unwrap();
        let mut ret = vec![0; ret_stat.size as usize];
        // TODO error handling.
        let read = kernel.block_read(return_block_id, 0, &mut ret).unwrap();
        ret.truncate(read as usize);

        (Ok(RawBytes::new(ret)), kernel.take())
    }

    /// Finishes execution, returning the gas used and the machine.
    pub fn finish(self) -> (i64, Box<Machine<B, E>>) {
        (self.gas_used(), self.machine)
    }

    /// Charge gas.
    pub fn charge_gas(&mut self, charge: GasCharge) -> Result<(), ActorError> {
        self.gas_tracker.charge_gas(charge)
    }

    /// Returns the available gas.
    pub fn gas_available(&self) -> i64 {
        self.gas_tracker.gas_available()
    }

    /// Getter for gas used.
    pub fn gas_used(&self) -> i64 {
        self.gas_tracker.gas_used()
    }
}
