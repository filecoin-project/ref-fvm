//! (Proper package docs coming shortly; for now this is a holding pen for items
//! we must mention).
//!
//! ## Logging
//!
//! This package emits logs using the log façade. Configure the logging backend
//! of your choice during the initialization of the consuming application.
pub use kernel::default::DefaultKernel;
pub use kernel::{BlockError, Kernel};

pub mod call_manager;
pub mod executor;
pub mod externs;
pub mod kernel;
pub mod machine;
pub mod syscalls;

// TODO Public only for conformance tests.
//  Consider exporting only behind a feature.
pub mod account_actor;
pub mod builtin;
pub mod gas;
pub mod init_actor;
pub mod state_tree;

mod blockstore;
mod market_actor;
mod power_actor;

#[derive(Clone)]
pub struct Config {
    /// The maximum call depth.
    pub max_call_depth: u32,
    /// Initial number of memory pages to allocate for the invocation container.
    pub initial_pages: usize,
    /// Maximum number of memory pages an invocation container's memory
    /// can expand to.
    pub max_pages: usize,
    /// Wasmtime engine configuration.
    pub engine: wasmtime::Config,
    /// Whether debug mode is enabled or not.
    pub debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            initial_pages: 0,
            max_pages: 1024,
            engine: Default::default(),
            max_call_depth: 4096,
            debug: false,
        }
    }
}

#[cfg(test)]
mod test {
    use fvm_shared::blockstore::MemoryBlockstore;
    use fvm_shared::state::StateTreeVersion;
    use fvm_shared::sys::TokenAmount;

    use crate::call_manager::DefaultCallManager;
    use crate::machine::DefaultMachine;
    use crate::state_tree::StateTree;
    use crate::{executor, externs, Config, DefaultKernel};

    #[test]
    fn test_constructor() {
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V4).unwrap();
        let root = st.flush().unwrap();
        bs = st.consume();

        let machine = DefaultMachine::new(
            Config::default(),
            0,
            TokenAmount::zero(),
            TokenAmount::zero(),
            fvm_shared::version::NetworkVersion::V14,
            root,
            bs,
            externs::cgo::CgoExterns::new(0),
        )
        .unwrap();
        let _ = executor::DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(Box::new(
            machine,
        ));
    }
}
