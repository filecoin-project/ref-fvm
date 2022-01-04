use crate::{
    call_manager::CallManager,
    kernel::{Kernel, Result},
    machine::Machine,
};

use super::{InterceptKernel, InterceptMachine};

/// A CallManager that wraps kernels in an InterceptKernel.
// NOTE: For now, this _must_ be transparent because we transmute a pointer.
#[repr(transparent)]
pub struct InterceptCallManager<C: CallManager>(pub C);

impl<M, C, D> CallManager for InterceptCallManager<C>
where
    D: 'static,
    M: Machine,
    C: CallManager<Machine = InterceptMachine<M, D>>,
{
    type Machine = C::Machine;

    fn new(
        machine: Self::Machine,
        gas_limit: i64,
        origin: fvm_shared::address::Address,
        nonce: u64,
    ) -> Self {
        InterceptCallManager(C::new(machine, gas_limit, origin, nonce))
    }

    fn send<K: Kernel<CallManager = Self>>(
        &mut self,
        from: fvm_shared::ActorID,
        to: fvm_shared::address::Address,
        method: fvm_shared::MethodNum,
        params: &fvm_shared::encoding::RawBytes,
        value: &fvm_shared::econ::TokenAmount,
    ) -> Result<crate::call_manager::InvocationResult> {
        // K is the kernel specified by the non intercepted kernel.
        // We wrap that here.
        self.0
            .send::<InterceptKernel<K>>(from, to, method, params, value)
    }

    fn with_transaction(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<crate::call_manager::InvocationResult>,
    ) -> Result<crate::call_manager::InvocationResult> {
        // This transmute is _safe_ because this type is "repr transparent".
        let inner_ptr = &mut self.0 as *mut C;
        self.0.with_transaction(|inner: &mut C| unsafe {
            // Make sure that we've got the right pointer. Otherwise, this cast definitely isn't
            // safe.
            assert_eq!(inner_ptr, inner as *mut C);

            // Ok, we got the pointer we expected, casting back to the interceptor is safe.
            f(&mut *(inner as *mut C as *mut Self))
        })
    }

    fn finish(self) -> (i64, Vec<crate::machine::CallError>, Self::Machine) {
        self.0.finish()
    }

    fn machine(&self) -> &Self::Machine {
        self.0.machine()
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        self.0.machine_mut()
    }

    fn gas_tracker(&self) -> &crate::gas::GasTracker {
        self.0.gas_tracker()
    }

    fn gas_tracker_mut(&mut self) -> &mut crate::gas::GasTracker {
        self.0.gas_tracker_mut()
    }

    fn origin(&self) -> fvm_shared::address::Address {
        self.0.origin()
    }

    fn nonce(&self) -> u64 {
        self.0.nonce()
    }

    fn next_actor_idx(&mut self) -> u64 {
        self.0.next_actor_idx()
    }

    fn push_error(&mut self, e: crate::machine::CallError) {
        self.0.push_error(e)
    }

    fn clear_error(&mut self) {
        self.0.clear_error()
    }
}
