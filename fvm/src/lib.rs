pub use runtime::{Config, DefaultRuntime, Error, InvocationRuntime, IpldRuntime, Runtime};

mod adt;
mod blocks;
mod invocation;
mod machine;
mod node;
mod plumbing;
mod state_tree;
mod syscalls;

pub use syscalls::environment;

#[derive(Copy, Clone)]
pub struct Config {
    /// Initial number of memory pages to allocate for the invocation container.
    pub initial_pages: usize,
    /// Maximum number of memory pages an invocation container's memory
    /// can expand to.
    pub max_pages: usize,
    /// Wasmtime engine configuration.
    pub engine: WasmtimeConfig,
}
