mod default;

pub use default::DefaultExecutor;
use fvm_shared::encoding::RawBytes;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use fvm_shared::sys::TokenAmount;
use num_traits::Zero;

use crate::kernel::SyscallError;
use crate::machine::CallError;
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

/// Apply message return data.
#[derive(Clone, Debug)]
pub struct ApplyRet {
    /// Message receipt for the transaction. This data is stored on chain.
    pub msg_receipt: Receipt,
    /// A backtrace for the transaction, if it failed.
    pub backtrace: Vec<CallError>,
    /// Gas penalty from transaction, if any.
    pub penalty: TokenAmount,
    /// Tip given to miner from message.
    pub miner_tip: TokenAmount,
}

impl ApplyRet {
    #[inline]
    pub fn prevalidation_fail(error: SyscallError, miner_penalty: TokenAmount) -> ApplyRet {
        ApplyRet {
            msg_receipt: Receipt {
                exit_code: error.1,
                return_data: RawBytes::default(),
                gas_used: 0,
            },
            penalty: miner_penalty,
            backtrace: vec![CallError {
                source: 0,
                code: error.1,
                message: error.0,
            }],
            miner_tip: TokenAmount::zero(),
        }
    }
    // review: was this used?
    // pub fn assign_from_slice(&mut self, sign: Sign, slice: &[u32]) {
    //     self.miner_tip.assign_from_slice(sign, slice)
    // }
}

pub enum ApplyKind {
    Explicit,
    Implicit,
}
