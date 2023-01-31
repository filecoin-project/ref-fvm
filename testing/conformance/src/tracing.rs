// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::fs::{create_dir_all, File};
use std::io::{Result as IoResult, Write};
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fvm::executor::ApplyRet;
use fvm::trace::ExecutionEvent;
use serde::{Deserialize, Serialize};

/// Timing and result of a message execution.
pub type TestTrace = (Duration, ApplyRet);

/// Closure passed to the runner, to be called with the return values
/// from all messages in the tests, in the order of execution.
pub type TestTraceFun = Box<dyn FnOnce(Vec<TestTrace>) -> IoResult<()>>;

/// Tombstone of a single message execution.
#[derive(Serialize)]
pub struct TestMessageTombstone {
    /// Path to the detailed execution trace.
    ///
    /// The path includes the name of the test, the ID of the variant, and the index of the message.
    pub trace_path: PathBuf,
    /// Overall gas burned.
    pub gas_burned: u64,
    /// Overall time elapsed.
    pub elapsed_nanos: u128,
}

/// Information we export about gas charges.
///
/// Probably only the compute part of gas can have a relation to time,
/// but it contains both so we can differentiate and see what happened.
#[derive(Serialize, Deserialize)]
pub struct TestGasCharge {
    pub name: String,
    pub compute_gas: u64,
    pub other_gas: u64,
    pub elapsed_nanos: Option<u128>,
}

/// Export gas traces as we complete tests, while collecting tombstones.
pub struct TestTraceExporter {
    /// Root directory of where to put the exports.
    output_dir_path: PathBuf,
    /// Collection of all tombstones, accumulated for the final export.
    tombstones: Mutex<Vec<TestMessageTombstone>>,
}

impl TestTraceExporter {
    pub fn new(output_dir_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            output_dir_path,
            tombstones: Default::default(),
        })
    }

    /// Return a closure that exports a variant.
    pub fn export_fun(
        self: Arc<Self>,
        input_file_path: PathBuf,
        variant_id: String,
    ) -> TestTraceFun {
        let f: TestTraceFun =
            Box::new(move |traces| self.export_variant(input_file_path, variant_id, traces));
        f
    }

    /// Export the gas charges from a test vector and record the tombstone.
    ///
    /// Each message in the test vector will be a separate entry in the traces.
    pub fn export_variant(
        &self,
        input_file_path: PathBuf,
        variant_id: String,
        traces: Vec<TestTrace>,
    ) -> IoResult<()> {
        let results = traces
            .into_iter()
            .enumerate()
            .map(|(i, (elapsed, ret))| {
                let mut trace_path = self.output_dir_path.clone();
                trace_path.push(input_file_path.clone());
                trace_path.set_extension(format!("{variant_id}.{i}.jsonline"));

                let ts = TestMessageTombstone {
                    trace_path,
                    gas_burned: ret.gas_burned,
                    elapsed_nanos: elapsed.as_nanos(),
                };

                let charges = ret
                    .exec_trace
                    .into_iter()
                    .filter_map(|event| match event {
                        ExecutionEvent::GasCharge(charge) => {
                            let elapsed_nanos = charge.elapsed.get().map(|e| e.as_nanos());
                            Some(TestGasCharge {
                                name: charge.name.into(),
                                compute_gas: charge.compute_gas.as_milligas(),
                                other_gas: charge.other_gas.as_milligas(),
                                elapsed_nanos,
                            })
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                (ts, charges)
            })
            .collect::<Vec<_>>();

        let mut ts = Vec::new();

        for (t, cs) in results {
            Self::export_json(&t.trace_path, cs)?;
            ts.push(t);
        }

        let mut guard = self.tombstones.lock().unwrap();
        guard.append(&mut ts);

        Ok(())
    }

    pub fn export_tombstones(&self) -> IoResult<()> {
        let tombstones = {
            let mut guard = self.tombstones.lock().unwrap();
            std::mem::take(guard.deref_mut())
        };

        let mut path = self.output_dir_path.clone();
        path.push("traces.jsonline");

        Self::export_json(&path, tombstones)
    }

    fn export_json<T: Serialize>(path: &PathBuf, values: Vec<T>) -> IoResult<()> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        let mut output = File::create(path)?;

        for value in values {
            let line = serde_json::to_string(&value).unwrap();
            writeln!(&mut output, "{}", line)?;
        }

        Ok(())
    }
}

pub type TestTraceExporterRef = Option<Arc<TestTraceExporter>>;
