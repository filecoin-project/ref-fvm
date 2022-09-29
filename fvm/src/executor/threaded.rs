use anyhow::anyhow;
use cid::Cid;
use fvm_shared::message::Message;
use lazy_static::lazy_static;

use super::{ApplyKind, ApplyRet, Executor};

lazy_static! {
    static ref EXEC_POOL: yastl::Pool = yastl::Pool::with_config(
        8,
        yastl::ThreadConfig::new()
            .prefix("fvm-executor")
            // fvm needs more than the deafault available stack (2MiB):
            // - Max 2048 wasm stack elements, which is 16KiB of 64bit entries
            // - Roughly 20KiB overhead per actor call
            // - max 1024 nested calls, which means that in the worst case we need ~36MiB of stack
            // We also want some more space just to be conservative, so 64MiB seems like a reasonable choice
            .stack_size(64 << 20),
    );
}

/// An executor that executes messages on a separate thread with a 64MiB stack. If you can guarantee
/// at least 64MiB of stack space, you don't need this executor.
pub struct ThreadedExecutor<E>(pub E);

impl<E> Executor for ThreadedExecutor<E>
where
    E: Executor + Send,
{
    type Kernel = E::Kernel;

    /// This is the entrypoint to execute a message.
    fn execute_message(
        &mut self,
        msg: Message,
        apply_kind: ApplyKind,
        raw_length: usize,
    ) -> anyhow::Result<ApplyRet> {
        let mut ret = Err(anyhow!("failed to execute"));

        EXEC_POOL.scoped(|scope| {
            scope.execute(|| ret = self.0.execute_message(msg, apply_kind, raw_length));
        });

        ret
    }

    fn flush(&mut self) -> anyhow::Result<Cid> {
        self.0.flush()
    }
}
