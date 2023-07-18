// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::MaybeResolvedAddress;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::ActorID;

use crate::call_manager::Entrypoint;
use crate::gas::GasCharge;
use crate::kernel::SyscallError;

/// Execution Trace, only for informational and debugging purposes.
pub type ExecutionTrace = Vec<ExecutionEvent>;

/// An "event" that happened during execution.
///
/// This is marked as `non_exhaustive` so we can introduce additional event types later.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ExecutionEvent {
    GasCharge(GasCharge),
    Call {
        from: ActorID,
        to: MaybeResolvedAddress,
        entrypoint: Entrypoint,
        params: Option<IpldBlock>,
        value: TokenAmount,
    },
    CallReturn(ExitCode, Option<IpldBlock>),
    CallError(SyscallError),
}
