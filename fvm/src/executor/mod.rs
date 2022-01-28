mod default;

use std::fmt::Display;

pub use default::DefaultExecutor;
use fvm_shared::bigint::{BigInt, Sign};
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use num_traits::Zero;

use crate::call_manager::Backtrace;
use crate::Kernel;

pub trait Executor {
    type Kernel: Kernel;

    /// This is the entrypoint to execute a message.
    fn execute_message(
        &mut self,
        msg: Message,
        _: ApplyKind,
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
    /// Additional failure information for debugging, if any.
    pub failure_info: Option<ApplyFailure>,
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
            failure_info: Some(ApplyFailure::PreValidation(message.into())),
            miner_tip: BigInt::zero(),
        }
    }

    pub fn assign_from_slice(&mut self, sign: Sign, slice: &[u32]) {
        self.miner_tip.assign_from_slice(sign, slice)
    }
}

pub enum ApplyKind {
    Explicit,
    Implicit,
}
