// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::event::StampedEvent;
use fvm_shared::ActorID;

use super::{Machine, MachineContext, Manifest};
use crate::kernel::Result;
use crate::state_tree::{ActorState, StateTree};

type Type = MachineContext;

impl<M: Machine> Machine for Box<M> {
    type Blockstore = M::Blockstore;
    type Externs = M::Externs;
    type Limiter = M::Limiter;

    #[inline(always)]
    fn blockstore(&self) -> &Self::Blockstore {
        (**self).blockstore()
    }

    #[inline(always)]
    fn context(&self) -> &Type {
        (**self).context()
    }

    #[inline(always)]
    fn externs(&self) -> &Self::Externs {
        (**self).externs()
    }

    #[inline(always)]
    fn builtin_actors(&self) -> &Manifest {
        (**self).builtin_actors()
    }

    #[inline(always)]
    fn state_tree(&self) -> &StateTree<Self::Blockstore> {
        (**self).state_tree()
    }

    #[inline(always)]
    fn state_tree_mut(&mut self) -> &mut StateTree<Self::Blockstore> {
        (**self).state_tree_mut()
    }

    #[inline(always)]
    fn create_actor(&mut self, addr: &Address, act: ActorState) -> Result<ActorID> {
        (**self).create_actor(addr, act)
    }

    #[inline(always)]
    fn transfer(&mut self, from: ActorID, to: ActorID, value: &TokenAmount) -> Result<()> {
        (**self).transfer(from, to, value)
    }

    #[inline(always)]
    fn flush(&mut self) -> Result<Cid> {
        (**self).flush()
    }

    #[inline(always)]
    fn into_store(self) -> Self::Blockstore {
        (*self).into_store()
    }

    #[inline(always)]
    fn machine_id(&self) -> &str {
        (**self).machine_id()
    }

    #[inline(always)]
    fn new_limiter(&self) -> Self::Limiter {
        (**self).new_limiter()
    }

    #[inline(always)]
    fn commit_events(&self, events: &[StampedEvent]) -> Result<Option<Cid>> {
        (**self).commit_events(events)
    }
}
