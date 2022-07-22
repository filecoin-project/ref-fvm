// Wrapper around the assert macro to have a hand on which exit code we want to give to our failed
// assertion
#[macro_export]
macro_rules! assert {
    ($cond:expr $(,)?) => ({
        std::panic::set_hook(Box::new(|info| {
            $crate::vm::abort(ExitCode::USR_ASSERTION_FAILED.value(), Some(&format!("{}", info)))
        }));

        core::assert!($cond);
    });
    ($cond:expr, $($arg:tt)+) => {{
        std::panic::set_hook(Box::new(|info| {
            $crate::vm::abort(ExitCode::USR_ASSERTION_FAILED.value(), Some(&format!("{}", info)))
        }));

        core::assert!($cond, "{}", format_args!($($arg)+));
    }};
}

// Utility macro to generate macro code for assert_eq and assert_ne
macro_rules! assert_gen {
    ($assert_macro:ident) => {
        with_dollar_sign! {
            ($d:tt) => {
                #[macro_export]
                macro_rules! $assert_macro {
                    ($d left:expr, $d right:expr $d(,)?) => {
                        std::panic::set_hook(Box::new(|info| {
                            $d crate::vm::abort(ExitCode::USR_ASSERTION_FAILED.value(), Some(&format!("{}", info)))
                        }));

                        core::$assert_macro!($d left, $d right);
                    };
                    ($d left:expr, $d right:expr, $d($d arg:tt)+) => {
                        std::panic::set_hook(Box::new(|info| {
                            $d crate::vm::abort(ExitCode::USR_ASSERTION_FAILED.value(), Some(&format!("{}", info)))
                        }));

                        core::$assert_macro!($d left, $d right, "{}", format_args!($d($d arg)+));
                    };
                }
            }
        }
    };
}

// Utility macro to allow for nested repetition
macro_rules! with_dollar_sign {
    ($($body:tt)*) => {
        macro_rules! __with_dollar_sign { $($body)* }
        __with_dollar_sign!($);
    }
}

// Wrapper around the assert_eq macro to have a hand on which exit code we want to give to our failed
// assertion
assert_gen!(assert_eq);

// Wrapper around the assert_ne macro to have a hand on which exit code we want to give to our failed
// assertion
assert_gen!(assert_ne);
