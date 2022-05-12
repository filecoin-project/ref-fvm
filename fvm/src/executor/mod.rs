mod default;

use std::fmt::Display;

pub use default::DefaultExecutor;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::bigint::{BigInt, Sign};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use num_traits::Zero;

use crate::call_manager::Backtrace;
use crate::trace::ExecutionTrace;
use crate::Kernel;

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
    pub penalty: BigInt,
    /// Tip given to miner from message.
    pub miner_tip: BigInt,

    // Gas stuffs
    pub base_fee_burn: TokenAmount,
    pub over_estimation_burn: TokenAmount,
    pub refund: TokenAmount,
    pub gas_refund: i64,
    pub gas_burned: i64,

    /// Additional failure information for debugging, if any.
    pub failure_info: Option<ApplyFailure>,
    /// Execution trace information, for debugging.
    pub exec_trace: ExecutionTrace,
}

impl ApplyRet {
    #[inline]
    pub fn prevalidation_fail(
        code: ExitCode,
        message: impl Into<String>,
        miner_penalty: BigInt,
    ) -> ApplyRet {
        ApplyRet {
            msg_receipt: Receipt {
                exit_code: code,
                return_data: RawBytes::default(),
                gas_used: 0,
            },
            penalty: miner_penalty,
            miner_tip: BigInt::zero(),
            base_fee_burn: TokenAmount::from(0),
            over_estimation_burn: TokenAmount::from(0),
            refund: TokenAmount::from(0),
            gas_refund: 0,
            gas_burned: 0,
            failure_info: Some(ApplyFailure::PreValidation(message.into())),
            exec_trace: vec![],
        }
    }

    pub fn assign_from_slice(&mut self, sign: Sign, slice: &[u32]) {
        self.miner_tip.assign_from_slice(sign, slice)
    }
}

/// The kind of message being applied:
///
/// 1. Explicit messages may only come from account actors and charge the sending account for gas
/// consumed.
/// 2. Implicit messages may come from any actor, ignore the nonce, and charge no gas (but still
/// account for it).
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ApplyKind {
    Explicit,
    Implicit,
}
