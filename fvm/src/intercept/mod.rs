mod call_manager;
mod kernel;
mod machine;

pub use call_manager::*;
pub use kernel::*;
pub use machine::*;

#[cfg(test)]
mod test {
    use cid::Cid;
    use num_traits::Zero;

    use crate::{
        call_manager::DefaultCallManager, executor::DefaultExecutor, externs,
        machine::DefaultMachine, Config, DefaultKernel,
    };

    use super::{machine::InterceptMachine, InterceptCallManager, InterceptKernel};
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
