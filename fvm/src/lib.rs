pub use kernel::{default::DefaultKernel, BlockError, Kernel};

pub mod externs;
pub mod kernel;
pub mod machine;
pub mod message;
pub mod receipt;
pub mod syscalls;

mod account_actor;
mod builtin;
mod call_manager;
mod gas;
mod init_actor;
mod state_tree;

#[derive(Clone)]
pub struct Config {
    /// Initial number of memory pages to allocate for the invocation container.
    pub initial_pages: usize,
    /// Maximum number of memory pages an invocation container's memory
    /// can expand to.
    pub max_pages: usize,
    /// Wasmtime engine configuration.
    pub engine: wasmtime::Config,
}
