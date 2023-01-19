use arbitrary::Unstructured;

/// State machine tests inspired by [ScalaCheck](https://github.com/typelevel/scalacheck/blob/main/doc/UserGuide.md#stateful-testing)
/// and [quickcheck-state-machine](https://hackage.haskell.org/package/quickcheck-state-machine).
pub trait StateMachine {
    /// System Under Test.
    type System;
    /// The idealised reference state we are testing aginst.
    type State: Clone;
    /// The random commands we can apply on the state in each step.
    type Command;
    /// The return result from command application.
    type Result;

    /// Generate a random initial state.
    fn gen_state(&self, u: &mut Unstructured) -> arbitrary::Result<Self::State>;

    /// Create a new System Under Test reflecting the given initial state.
    ///
    /// The [System] should free all of its resources when it goes out of scope.
    fn new_system(&self, state: &Self::State) -> Self::System;

    /// Generate a random command given the latest state.
    fn gen_command(
        &self,
        u: &mut Unstructured,
        state: &Self::State,
    ) -> arbitrary::Result<Self::Command>;

    /// Apply a command on the System Under Test.
    fn run_command(&self, system: &mut Self::System, cmd: &Self::Command) -> Self::Result;

    /// Use assertions to check that the state transition on the System Under Test was correct, given the model pre-state.
    fn check_post_conditions(
        &self,
        pre_state: &Self::State,
        cmd: &Self::Command,
        result: &Self::Result,
        post_system: &Self::System,
    );

    /// Apply a command on the model state.
    ///
    /// We could use `Cow` here if we wanted to preserve the history of state and
    /// also avoid cloning when there's no change.
    fn next_state(&self, state: Self::State, cmd: &Self::Command) -> Self::State;
}

/// Run a state machine test by generating `max_steps` commands.
///
/// It is expected to panic if some post condition fails.
pub fn run<T: StateMachine>(
    u: &mut Unstructured,
    t: &T,
    max_steps: usize,
) -> arbitrary::Result<()> {
    let mut state = t.gen_state(u)?;
    let mut system = t.new_system(&state);
    for _ in 0..max_steps {
        let cmd = t.gen_command(u, &state)?;
        let res = t.run_command(&mut system, &cmd);
        t.check_post_conditions(&state, &cmd, &res, &system);
        state = t.next_state(state, &cmd);
    }
    Ok(())
}

/// Run a state machine test as a `#[test]`.
///
/// # Example
///
/// ```ignore
/// state_machine_test!(counter, 100 ms, 100 steps, CounterStateMachine { buggy: false });
/// ```
///
/// If the test fails, it will print out the seed which can be used to reproduce the error.
/// One can use [state_machine_seed!] to do that with minimal changes to the parameters.
#[macro_export]
macro_rules! state_machine_test {
    ($name:ident, $ms:literal ms, $steps:literal steps, $smt:expr) => {
        #[test]
        fn $name() {
            arbtest::builder()
                .budget_ms($ms)
                .run(|u| $crate::smt::run(u, &$smt, $steps))
        }
    };
    ($name:ident, $steps:literal steps, $smt:expr) => {
        #[test]
        fn $name() {
            arbtest::builder().run(|u| $crate::smt::run(u, &$smt, $steps))
        }
    };
}

/// Run a state machine test as a `#[test]` with a `seed` to reproduce a failure.
///
/// # Example
///
/// ```ignore
/// state_machine_seed!(counter, 0x001a560e00000020, 100 steps, CounterStateMachine { buggy: true });
/// ```
#[macro_export]
macro_rules! state_machine_seed {
    ($name:ident, $seed:literal, $steps:literal steps, $smt:expr) => {
        paste::paste! {
          #[test]
          fn [<$name _with_seed_ $seed>]() {
              arbtest::builder()
                  .seed($seed)
                  .run(|u| $crate::smt::run(u, &$smt, $steps))
          }
        }
    };
}

#[cfg(test)]
mod tests {
    use arbitrary::{Result, Unstructured};

    use super::StateMachine;

    /// A sample System Under Test.
    struct Counter {
        n: i32,
    }

    impl Counter {
        pub fn new() -> Self {
            Self { n: 0 }
        }
        pub fn get(&self) -> i32 {
            self.n
        }
        pub fn inc(&mut self) {
            self.n += 1;
        }
        pub fn dec(&mut self) {
            self.n -= 1;
        }
        pub fn reset(&mut self) {
            self.n = 0;
        }
    }

    #[derive(Clone, Copy)]
    enum CounterCommand {
        Get,
        Inc,
        Dec,
        Reset,
    }

    struct CounterStateMachine {
        /// Introduce some bug to check the negative case.
        buggy: bool,
    }

    impl StateMachine for CounterStateMachine {
        type System = Counter;
        type State = i32;
        type Command = &'static CounterCommand;
        type Result = Option<i32>;

        fn gen_state(&self, u: &mut Unstructured) -> Result<Self::State> {
            if self.buggy {
                Ok(u.arbitrary::<i32>()?.abs() + 1)
            } else {
                Ok(0)
            }
        }

        fn new_system(&self, _state: &Self::State) -> Self::System {
            Counter::new()
        }

        fn gen_command(&self, u: &mut Unstructured, _state: &Self::State) -> Result<Self::Command> {
            use CounterCommand::*;
            u.choose(&[Get, Inc, Dec, Reset])
        }

        fn run_command(&self, system: &mut Self::System, cmd: &Self::Command) -> Self::Result {
            use CounterCommand::*;
            match cmd {
                Get => return Some(system.get()),
                Inc => system.inc(),
                Dec => system.dec(),
                Reset => system.reset(),
            }
            None
        }

        fn check_post_conditions(
            &self,
            pre_state: &Self::State,
            cmd: &Self::Command,
            result: &Self::Result,
            _post_system: &Self::System,
        ) {
            match cmd {
                CounterCommand::Get => {
                    assert_eq!(result.as_ref(), Some(pre_state))
                }
                _ => {} // We could check the state if we wanted, or we can wait for Get.
            }
        }

        fn next_state(&self, state: Self::State, cmd: &Self::Command) -> Self::State {
            use CounterCommand::*;
            match cmd {
                Inc => state + 1,
                Dec => state - 1,
                Reset => 0,
                Get => state,
            }
        }
    }

    state_machine_test!(counter, 100 steps, CounterStateMachine { buggy: false });

    /// Test the equivalent of:
    ///
    /// ```ignore
    /// state_machine_test!(counter, 100 steps, CounterStateMachine { buggy: true });
    /// ```
    #[test]
    #[should_panic]
    fn counter_with_bug() {
        let t = CounterStateMachine { buggy: true };
        arbtest::builder().run(|u| super::run(u, &t, 100))
    }

    /// Test the equivalent of:
    ///
    /// ```ignore
    /// state_machine_seed!(counter, 0x001a560e00000020, 100 steps, CounterStateMachine { buggy: true });
    /// ```
    #[test]
    #[should_panic]
    fn counter_with_seed() {
        let t = CounterStateMachine { buggy: true };
        arbtest::builder()
            .seed(0x001a560e00000020)
            .run(|u| super::run(u, &t, 100))
    }
}
