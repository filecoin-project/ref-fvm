// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod default;
mod threaded;

use std::fmt::Display;

use cid::Cid;
pub use default::DefaultExecutor;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::StampedEvent;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use num_traits::Zero;
pub use threaded::ThreadedExecutor;

use crate::Kernel;
use crate::call_manager::Backtrace;
use crate::trace::ExecutionTrace;

/// An executor executes messages on the underlying machine/kernel. It's responsible for:
///
/// 1. Validating messages (nonce, sender, etc).
/// 2. Creating message receipts.
/// 3. Charging message inclusion gas, overestimation gas, miner tip, etc.
pub trait Executor {
    /// The [`Kernel`] on which messages will be applied. We specify a [`Kernel`] here, not a
    /// [`Machine`](crate::machine::Machine), because the [`Kernel`] implies the
    /// [`Machine`](crate::machine::Machine).
    type Kernel: Kernel;

    /// This is the entrypoint to execute a message.
    ///
    /// NOTE: The "raw length" is the length of the message as it appears on-chain and is used to
    /// charge message inclusion gas.
    fn execute_message(
        &mut self,
        msg: Message,
        apply_kind: ApplyKind,
        raw_length: usize,
    ) -> anyhow::Result<ApplyRet>;

    /// Flushes the state-tree, returning the new root CID.
    fn flush(&mut self) -> anyhow::Result<Cid>;
}

/// A description of some failure encountered when applying a message.
#[derive(Debug, Clone)]
pub enum ApplyFailure {
    /// The backtrace from a message failure.
    MessageBacktrace(Backtrace),
    /// A message describing a pre-validation failure.
    PreValidation(String),
}

impl Display for ApplyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyFailure::MessageBacktrace(bt) => {
                writeln!(f, "message failed with backtrace:")?;
                write!(f, "{}", bt)?;
            }
            ApplyFailure::PreValidation(msg) => {
                writeln!(f, "pre-validation failed: {}", msg)?;
            }
        }
        Ok(())
    }
}

/// Apply message return data.
#[derive(Clone, Debug)]
pub struct ApplyRet {
    /// Message receipt for the transaction. This data is stored on chain.
    pub msg_receipt: Receipt,
    /// Gas penalty from transaction, if any.
    pub penalty: TokenAmount,
    /// Tip given to miner from message.
    pub miner_tip: TokenAmount,

    // Gas stuffs
    pub base_fee_burn: TokenAmount,
    pub over_estimation_burn: TokenAmount,
    pub refund: TokenAmount,
    pub gas_refund: u64,
    pub gas_burned: u64,

    /// Additional failure information for debugging, if any.
    pub failure_info: Option<ApplyFailure>,
    /// Execution trace information, for debugging.
    pub exec_trace: ExecutionTrace,
    /// Events generated while applying the message.
    pub events: Vec<StampedEvent>,
}

impl ApplyRet {
    #[inline]
    pub fn prevalidation_fail(
        code: ExitCode,
        message: impl Into<String>,
        miner_penalty: TokenAmount,
    ) -> ApplyRet {
        ApplyRet {
            msg_receipt: Receipt {
                exit_code: code,
                return_data: RawBytes::default(),
                gas_used: 0,
                events_root: None,
            },
            penalty: miner_penalty,
            miner_tip: TokenAmount::zero(),
            base_fee_burn: TokenAmount::zero(),
            over_estimation_burn: TokenAmount::zero(),
            refund: TokenAmount::zero(),
            gas_refund: 0,
            gas_burned: 0,
            failure_info: Some(ApplyFailure::PreValidation(message.into())),
            exec_trace: vec![],
            events: vec![],
        }
    }
}

/// The kind of message being applied:
///
/// 1. Explicit messages may only come from account actors and charge the sending account for gas
///    consumed.
/// 2. Implicit messages may come from any actor, ignore the nonce, and charge no gas (but still
///    account for it).
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ApplyKind {
    Explicit,
    Implicit,
}
