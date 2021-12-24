use crate::machine::Machine;

pub struct InterceptMachine<M, D> {
    pub machine: M,
    pub data: D,
}

impl<M, D: 'static> Machine for InterceptMachine<M, D>
where
    M: Machine,
{
    type Blockstore = M::Blockstore;
    type Externs = M::Externs;

    fn engine(&self) -> &wasmtime::Engine {
        self.machine.engine()
    }

    fn config(&self) -> crate::Config {
        self.machine.config()
    }

    fn blockstore(&self) -> &Self::Blockstore {
        self.machine.blockstore()
    }

    fn context(&self) -> &crate::machine::MachineContext {
        self.machine.context()
    }

    fn externs(&self) -> &Self::Externs {
        self.machine.externs()
    }

    fn state_tree(&self) -> &crate::state_tree::StateTree<Self::Blockstore> {
        self.machine.state_tree()
    }

    fn state_tree_mut(&mut self) -> &mut crate::state_tree::StateTree<Self::Blockstore> {
        self.machine.state_tree_mut()
    }

    fn create_actor(
        &mut self,
        addr: &fvm_shared::address::Address,
        act: crate::state_tree::ActorState,
    ) -> crate::kernel::Result<fvm_shared::ActorID> {
        self.machine.create_actor(addr, act)
    }

    fn load_module(&self, code: &cid::Cid) -> crate::kernel::Result<wasmtime::Module> {
        self.machine.load_module(code)
    }

    fn transfer(
        &mut self,
        from: fvm_shared::ActorID,
        to: fvm_shared::ActorID,
        value: &fvm_shared::econ::TokenAmount,
    ) -> crate::kernel::Result<()> {
        self.machine.transfer(from, to, value)
    }
}
