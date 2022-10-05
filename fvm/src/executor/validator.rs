use anyhow::anyhow;
use cid::Cid;
use fvm_ipld_encoding::{Cbor, RawBytes, DAG_CBOR};
use fvm_shared::message::params::ValidateParams;
use fvm_shared::message::Message;

use super::{ApplyKind, ApplyRet, DefaultExecutor, Executor, ValidateExecutor};
use crate::call_manager::{CallManager, ExecutionType, InvocationResult};
use crate::executor::GasSpec;
use crate::kernel::{Block, Context, ExecutionError};
use crate::machine::Machine;
use crate::Kernel;

/// TODO try not to be stuck with Default, but it has methods methods i want for validate.
pub struct DefaultValidateExecutor<K: Kernel>(pub DefaultExecutor<K>);

impl<K> Executor for DefaultValidateExecutor<K>
where
    K: Kernel,
{
    type Kernel = K;

    /// This is the entrypoint to execute a message.
    fn execute_message(
        &mut self,
        msg: Message,
        apply_kind: ApplyKind,
        raw_length: usize,
    ) -> anyhow::Result<ApplyRet> {
        self.0.execute_message(msg, apply_kind, raw_length)
    }

    fn flush(&mut self) -> anyhow::Result<Cid> {
        self.0.flush()
    }
}

impl<K> ValidateExecutor for DefaultValidateExecutor<K>
where
    K: Kernel,
{
    type Validator = K;

    /// validate a message from an abstract account with a delegate signature
    fn validate_message(&mut self, msg: Message, sig: Vec<u8>) -> anyhow::Result<super::GasSpec> {
        const VALIDATION_GAS_LIMIT: i64 = i64::MAX; // TODO reasonable gas limit

        // Load sender actor state.
        let sender_id = match self
            .0
            .state_tree()
            .lookup_id(&msg.from)
            .with_context(|| format!("failed to lookup actor {}", &msg.from))?
        {
            Some(id) => id,
            None => {
                return Err(
                    anyhow!("TODO"), // TODO: what to do if no actor found
                );
            }
        };

        // Validate the message.
        let (res, gas_used, mut backtrace, exec_trace) = self.0.map_machine(|machine| {
            // We're processing a chain message, so the sender is the origin of the call stack.
            let mut cm = K::CallManager::new(
                machine,
                VALIDATION_GAS_LIMIT,
                (sender_id, msg.from),
                msg.sequence,
            );
            cm.set_execution_type(ExecutionType::Validator);

            // Dont charge gas inclusion cost depending on where this is called
            // // This error is fatal because it should have already been accounted for inside
            // // preflight_message.
            // if let Err(e) = cm.charge_gas(inclusion_cost) {
            //     return (Err(e), cm.finish().1);
            // }

            let params = {
                let params = ValidateParams::new(msg, sig).marshal_cbor();

                match params {
                    Err(_) => return (Err(ExecutionError::OutOfGas), cm.finish().1),
                    Ok(params) => Some(Block::new(DAG_CBOR, params)),
                }
            };
            let params = params.unwrap(); // TODO err

            let ret = cm.validate::<K>(params, sender_id);
            println!("{ret:?}");
            let (res, machine) = cm.finish();
            (
                Ok((ret, res.gas_used, res.backtrace, res.exec_trace)),
                machine,
            )
        })?;

        // TODO use errors as part of message
        // Extract the exit code and build the result of the message application.
        let result = match res {
            Ok(InvocationResult::Return(return_value)) => {
                // Convert back into a top-level return "value". We throw away the codec here,
                // unfortunately.
                let return_data = return_value
                    .map(|blk| RawBytes::from(blk.data().to_vec()))
                    .unwrap_or_default();

                backtrace.clear();
                Ok(return_data)
            }
            Ok(InvocationResult::Failure(exit_code)) => {
                if exit_code.is_success() {
                    return Err(anyhow!("actor failed with status OK"));
                }
                Err(ExecutionError::Fatal(anyhow!("validation failed")))
            }
            Err(e) => Err(e),
        };

        let ret = result
            .map_err(|e| anyhow!("actor failed to validate: {e}"))?
            .deserialize::<GasSpec>()
            .map_err(|_| anyhow!("failed to unmarshall return data from validate"))?; // TODO better Errs
        Ok(ret)
    }
}
