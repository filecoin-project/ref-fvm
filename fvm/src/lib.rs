pub use runtime::{Config, DefaultRuntime, Error, InvocationRuntime, IpldRuntime, Runtime};

mod adt;
mod blocks;
mod exit_code;
mod externs;
mod invocation;
mod kernel;
mod machine;
mod message;
mod receipt;
mod state_tree;
mod syscalls;

#[derive(Copy, Clone)]
pub struct Config {
    /// Initial number of memory pages to allocate for the invocation container.
    pub initial_pages: usize,
    /// Maximum number of memory pages an invocation container's memory
    /// can expand to.
    pub max_pages: usize,
    /// Wasmtime engine configuration.
    pub engine: wasmtime::Config,
}
