use cid::Cid;
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::{ActorID, MethodNum};

use crate::gas::GasCharge;
use crate::kernel::{SpanId, SyscallError};

/// Execution Trace, only for informational and debugging purposes.
pub type ExecutionTrace = Vec<ExecutionEvent>;

/// An "event" that happened during execution.
///
/// This is marked as `non_exhaustive` so we can introduce additional event types later.
#[derive(Clone, Debug)]
// TODO This might be a mistake
#[non_exhaustive]
pub enum ExecutionEvent {
    GasCharge(GasCharge),
    Call {
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: Option<IpldBlock>,
        value: TokenAmount,
    },
    SpanBegin(SpanBegin),
    SpanEnd(SpanEnd),
    CallReturn(ExitCode, Option<IpldBlock>),
    CallError(SyscallError),
}

#[derive(Clone, Debug)]
pub struct SpanBegin {
    /// User-supplied label for this span.
    pub label: String,
    /// User-supplied tag for this span.
    pub tag: String,
    /// Parent span.
    pub parent: SpanId,
    /// CID of the currently executing method's code.
    pub code: Cid,
    /// Number of the currently executing method.
    pub method: MethodNum,
    /// The timestamp when this event ocurred, in nanoseconds.
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct SpanEnd {
    /// The ID of the span that is ending.
    pub id: SpanId,
    /// The timestamp when this event ocurred, in nanoseconds.
    pub timestamp: u64,
}

/// A monotonic clock used for generating nanosecond timestamps during tracing.
pub trait TraceClock {
    fn timestamp(&mut self) -> u64;
}

pub struct DefaultTraceClock(std::time::Instant);

impl DefaultTraceClock {
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Default for DefaultTraceClock {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceClock for DefaultTraceClock {
    fn timestamp(&mut self) -> u64 {
        self.0.elapsed().as_nanos() as u64
    }
}
