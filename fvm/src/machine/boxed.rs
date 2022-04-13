use cid::Cid;
use fvm_shared::actor::builtin::Manifest;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::ActorID;

use super::{Engine, Machine, MachineContext};
use crate::kernel::Result;
use crate::state_tree::{ActorState, StateTree};

type Type = MachineContext;

impl<M: Machine> Machine for Box<M> {
    type Blockstore = M::Blockstore;
    type Externs = M::Externs;

    #[inline(always)]
    fn engine(&self) -> &Engine {
        (&**self).engine()
    }

    #[inline(always)]
    fn blockstore(&self) -> &Self::Blockstore {
        (&**self).blockstore()
    }

    #[inline(always)]
    fn context(&self) -> &Type {
        (&**self).context()
    }

    #[inline(always)]
    fn externs(&self) -> &Self::Externs {
        (&**self).externs()
    }

    #[inline(always)]
    fn builtin_actors(&self) -> &Manifest {
        (&**self).builtin_actors()
    }

    #[inline(always)]
    fn state_tree(&self) -> &StateTree<Self::Blockstore> {
        (&**self).state_tree()
    }

    #[inline(always)]
    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
        (&mut **self).state_tree_mut()
    }

    #[inline(always)]
    fn create_actor(&mut self, addr: &Address, act: ActorState) -> Result<ActorID> {
        (&mut **self).create_actor(addr, act)
    }

    #[inline(always)]
    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
        (&mut **self).transfer(from, to, value)
    }

    #[inline(always)]
    fn consume(self) -> Self::Blockstore {
        (*self).consume()
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<Cid> {
        (**self).flush()
    }
}
