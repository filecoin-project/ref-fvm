pub use kernel::{default::DefaultKernel, BlockError, Kernel};

pub mod call_manager;
pub mod executor;
pub mod externs;
pub mod kernel;
pub mod machine;
pub mod syscalls;

mod account_actor;
mod builtin;
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

#[cfg(test)]
mod test {
    use cid::Cid;
    use num_traits::Zero;

    use crate::{
        call_manager::DefaultCallManager, executor, externs, machine::DefaultMachine, Config,
        DefaultKernel,
    };
    #[test]
    fn test_constructor() {
        let machine = DefaultMachine::new(
            Config {
                initial_pages: 0,
                max_pages: 1024,
                engine: Default::default(),
            },
            0,
            Zero::zero(),
            fvm_shared::version::NetworkVersion::V14,
            Cid::default(),
            externs::cgo::CgoExterns::new(0),
            externs::cgo::CgoExterns::new(0),
        )
        .unwrap();
        let _ = executor::DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(Box::new(
            machine,
        ));
    }
}
