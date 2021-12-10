// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use ipld_blockstore::BlockStore;

use fvm_shared::encoding::RawBytes;
use fvm_shared::error::CallError;
use fvm_shared::MethodNum;

use crate::Runtime;

/// Interface for invoking methods on an Actor
pub(crate) trait ActorCode {
    /// Invokes method with runtime on the actor's code. Method number will match one
    /// defined by the Actor, and parameters will be serialized and used in execution
    fn invoke_method<BS, RT>(
        rt: &mut RT,
        method: MethodNum,
        params: &RawBytes,
    ) -> Result<RawBytes, CallError>
    where
        BS: BlockStore,
        RT: Runtime<BS>;
}
