use quickcheck::{Arbitrary, Gen, TestResult, Testable};

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
    fn gen_init_state(&self, g: &mut Gen) -> Self::State;

    /// Create a new System Under Test reflecting the given initial state.
    ///
    /// The [System] should free all of its resources when it goes out of scope.
    fn new_system(&self, state: &Self::State) -> Self::System;

    /// Generate a random command given the latest state.
    fn gen_command(&self, g: &mut Gen, state: &Self::State) -> Self::Command;

    /// Apply a command on the System Under Test.
    fn run_command(&self, system: &mut Self::System, cmd: &Self::Command) -> Self::Result;

    /// Check that the state transition on the System Under Test was correct, given the model pre-state.
    fn post_condition(
        &self,
        pre_state: &Self::State,
        cmd: &Self::Command,
        result: &Self::Result,
        post_system: &Self::System,
    ) -> TestResult;

    /// Apply a command on the model state.
    ///
    /// We could use `Cow` here if we wanted to preserve the history of state and
    /// also avoid cloning when there's no change.
    fn next_state(&self, state: &mut Self::State, cmd: &Self::Command);
}

/// Adapter for [quickcheck::Testable].
pub struct QuickCheckStateMachine<T>(pub T);

impl<T: StateMachine + 'static> Testable for QuickCheckStateMachine<T> {
    fn result(&self, g: &mut Gen) -> TestResult {
        let n = usize::arbitrary(g) % g.size();
        let mut state = self.0.gen_init_state(g);
        let mut system = self.0.new_system(&state);
        for _ in 0..n {
            let cmd = self.0.gen_command(g, &state);
            let res = self.0.run_command(&mut system, &cmd);
            let res = self.0.post_condition(&state, &cmd, &res, &system);
            if res.is_failure() {
                return res;
            }
            self.0.next_state(&mut state, &cmd);
        }
        return TestResult::passed();
    }
}

/// Run quickcheck on a state machine.
pub fn state_machine_test<T: StateMachine + 'static>(t: T) {
    // Sadly `QuickCheck` and the `Gen` it uses are not seedable. There is an open PR of it,
    // but it's been there for so long I doubt that it will be merged:`
    // https://github.com/BurntSushi/quickcheck/pull/278
    // Without that, an error in this test says nothing about how to reproduce it.
    // We could print all the commands, but that could be unreadable, and unportable as well.
    quickcheck::QuickCheck::new().quickcheck(QuickCheckStateMachine(t))
}

/// Run a state machine test with QuickCheck as a `#[test]`.
///
/// # Example
///
/// ```ignore
/// state_machine_test!(counter, CounterStateMachine { buggy: false });
/// ```
#[macro_export]
macro_rules! state_machine_test {
    ($name:ident, $smt:expr) => {
        #[test]
        fn $name() {
            $crate::smt::state_machine_test($smt)
        }
    };
}

#[cfg(test)]
mod tests {
    use quickcheck::{Arbitrary, TestResult};

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
        type Command = CounterCommand;
        type Result = Option<i32>;

        fn gen_init_state(&self, g: &mut quickcheck::Gen) -> Self::State {
            if self.buggy {
                i32::arbitrary(g).abs() + 1
            } else {
                0
            }
        }

        fn new_system(&self, _state: &Self::State) -> Self::System {
            Counter::new()
        }

        fn gen_command(&self, g: &mut quickcheck::Gen, _state: &Self::State) -> Self::Command {
            use CounterCommand::*;
            *g.choose(&[Get, Inc, Dec, Reset]).unwrap()
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

        fn post_condition(
            &self,
            pre_state: &Self::State,
            cmd: &Self::Command,
            result: &Self::Result,
            _post_system: &Self::System,
        ) -> quickcheck::TestResult {
            use CounterCommand::*;
            match cmd {
                Get => {
                    if Some(pre_state) == result.as_ref() {
                        TestResult::passed()
                    } else {
                        TestResult::error(format!("expected {pre_state}, got {result:?}"))
                    }
                }
                _ => TestResult::passed(), // We could check the state if we wanted, or we can wait for Get.
            }
        }

        fn next_state(&self, state: &mut Self::State, cmd: &Self::Command) {
            use CounterCommand::*;
            match cmd {
                Inc => *state += 1,
                Dec => *state -= 1,
                Reset => *state = 0,
                Get => {}
            }
        }
    }

    state_machine_test!(counter, CounterStateMachine { buggy: false });

    #[test]
    #[should_panic]
    fn buggy_counter() {
        super::state_machine_test(CounterStateMachine { buggy: true })
    }
}
