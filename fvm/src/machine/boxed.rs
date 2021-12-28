use cid::Cid;
use fvm_shared::{address::Address, econ::TokenAmount, ActorID};
use wasmtime::{Engine, Module};

use crate::{
    state_tree::{ActorState, StateTree},
    Config,
};

use super::{Machine, MachineContext};
use crate::kernel::Result;

impl<M: Machine> Machine for Box<M> {
    type Blockstore = M::Blockstore;
    type Externs = M::Externs;

    #[inline(always)]
    fn engine(&self) -> &Engine {
        (&**self).engine()
    }

    #[inline(always)]
    fn config(&self) -> &Config {
        (&**self).config()
    }

    #[inline(always)]
    fn blockstore(&self) -> &Self::Blockstore {
        (&**self).blockstore()
    }

    #[inline(always)]
    fn context(&self) -> &MachineContext {
        (&**self).context()
    }

    #[inline(always)]
    fn externs(&self) -> &Self::Externs {
        (&**self).externs()
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
    fn load_module(&self, code: &Cid) -> Result<Module> {
        (&**self).load_module(code)
    }

    #[inline(always)]
    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
        (&mut **self).transfer(from, to, value)
    }
}
