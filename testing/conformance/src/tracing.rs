use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fvm::executor::ApplyRet;

/// Timing and result of a message execution.
pub type TestTrace = (Duration, ApplyRet);

/// Closure passed to the runner, to be called with the return values
/// from all messages in the tests, in the order of execution.
pub type TestTraceFun = Box<dyn FnOnce(Vec<TestTrace>)>;

/// Tombstone of a single message execution.
pub struct TestTombstone {
    /// Path to the detailed execution trace.
    ///
    /// The path includes the name of the test, the ID of the variant, and the index of the message.
    pub trace_path: PathBuf,
    /// Overall gas burned.
    pub gas_burned: i64,
    /// Overall time elapsed.
    pub elapsed: Duration,
}

/// Export traces as we complete tests, while collecting tombstones.
pub struct TestTraceExporter {
    /// Root directory of where to put the exports.
    output_dir_path: PathBuf,
    /// Collection of all tombstones, accumulated for the final export.
    tombstones: Mutex<Vec<TestTombstone>>,
}

impl TestTraceExporter {
    pub fn new(output_dir_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            output_dir_path,
            tombstones: Default::default(),
        })
    }

    /// Export the results from a test vector and record the tombstone.
    pub fn export_variant(
        &self,
        input_file_path: PathBuf,
        variant_id: String,
        traces: Vec<TestTrace>,
    ) {
        todo!()
    }

    pub fn export_fun(
        self: Arc<Self>,
        input_file_path: PathBuf,
        variant_id: String,
    ) -> TestTraceFun {
        let f: TestTraceFun =
            Box::new(move |traces| self.export_variant(input_file_path, variant_id, traces));
        f
    }

    pub fn export_tombstones(&self) {
        let tombstones = {
            let mut guard = self.tombstones.lock().unwrap();
            std::mem::take(guard.deref_mut())
        };
        todo!();
    }
}

pub type TestTraceExporterRef = Option<Arc<TestTraceExporter>>;
