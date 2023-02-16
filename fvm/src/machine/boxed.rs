// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;

use super::{Machine, MachineContext, Manifest};
use crate::kernel::Result;
use crate::state_tree::StateTree;

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
}
