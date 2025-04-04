use cid::Cid;
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::state::ActorState;
use fvm_shared::{ActorID, MethodNum};

use crate::gas::GasCharge;
use crate::kernel::SyscallError;

/// Execution Trace, only for informational and debugging purposes.
pub type ExecutionTrace = Vec<ExecutionEvent>;

/// The type of operation being performed in an Ipld ExecutionEvent.
#[derive(Clone, Debug, PartialEq)]
pub enum IpldOperation {
    Get, // open
    Put, // link
}

/// An "event" that happened during execution.
///
/// This is marked as `non_exhaustive` so we can introduce additional event types later.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ExecutionEvent {
    GasCharge(GasCharge),
    /// Emitted on each send call regardless whether we actually end up invoking the
    /// actor or not (e.g. if we don't have enough gas or if the actor does not exist)
    Call {
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: Option<IpldBlock>,
        value: TokenAmount,
        gas_limit: u64,
        read_only: bool,
    },
    CallReturn(ExitCode, Option<IpldBlock>),
    CallError(SyscallError),
    /// Emitted every time an actor is successfully invoked.
    InvokeActor {
        id: ActorID,
        state: ActorState,
    },
    Log(String),
    Ipld {
        op: IpldOperation,
        cid: Cid,
        size: usize,
    },
}
