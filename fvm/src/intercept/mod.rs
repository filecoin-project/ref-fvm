mod call_manager;
mod kernel;
mod machine;

pub use call_manager::*;
pub use kernel::*;
pub use machine::*;

#[cfg(test)]
mod test {
    use blockstore::MemoryBlockstore;
    use fvm_shared::state::StateTreeVersion;
    use num_traits::Zero;

    use crate::{
        call_manager::DefaultCallManager, executor::DefaultExecutor, externs,
        machine::DefaultMachine, state_tree::StateTree, Config, DefaultKernel,
    };

    use super::{machine::InterceptMachine, InterceptCallManager, InterceptKernel};

    #[test]
    fn test_constructor() {
        let mut bs = MemoryBlockstore::default();
        let mut st = StateTree::new(bs, StateTreeVersion::V4).unwrap();
        let root = st.flush().unwrap();
        bs = st.consume();

        let machine = DefaultMachine::new(
            Config {
                initial_pages: 0,
                max_pages: 1024,
                engine: Default::default(),
            },
            0,
            Zero::zero(),
            fvm_shared::version::NetworkVersion::V14,
            root,
            bs,
            externs::cgo::CgoExterns::new(0),
        )
        .unwrap();
        let _: DefaultExecutor<
            InterceptKernel<
                DefaultKernel<
                    InterceptCallManager<
                        DefaultCallManager<InterceptMachine<Box<DefaultMachine<_, _>>, ()>>,
                    >,
                >,
            >,
        > = DefaultExecutor::new(InterceptMachine {
            machine: Box::new(machine),
            data: (),
        });
    }
}
