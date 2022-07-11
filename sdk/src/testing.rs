// Utility function to handle call error message formatting and call to abort syscall
fn handle_assert_err(catch_unwind_res: std::thread::Result<()>) {
    if catch_unwind_res.is_err() {
        match catch_unwind_res.err() {
            Some(err) => match err.downcast::<String>() {
                Ok(panic_msg_box) => crate::vm::abort(
                    fvm_shared::error::ExitCode::USR_ASSERTION_FAILED.value(),
                    Some(panic_msg_box.as_str()),
                ),
                Err(err) => crate::vm::abort(
                    fvm_shared::error::ExitCode::USR_ASSERTION_FAILED.value(),
                    None,
                ),
            },
            None => unreachable!(),
        };
    }
}

// Wrapper around the assert macro to have a hand on which exit code we want to give to our failed
// assertion
#[macro_export]
macro_rules! assert {
    ($cond:expr $(,)?) => ({
        let res = std::panic::catch_unwind(|| {
            core::assert!($cond);
        });
        handle_assert_err(res);
    });
    ($cond:expr, $($arg:tt)+) => {{
        let res = std::panic::catch_unwind(|| {
            core::assert!($cond, "{}", format_args!($($arg)+));
        });
        handle_assert_err(res);
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
                        let res = std::panic::catch_unwind(|| {
                            core::$assert_macro!($d left, $d right);
                        });
                        handle_assert_err(res);
                    };
                    ($d left:expr, $d right:expr, $d($d arg:tt)+) => {
                        let res = std::panic::catch_unwind(|| {
                            core::$assert_macro!($d left, $d right, "{}", format_args!($d($d arg)+));
                        });
                        handle_assert_err(res);
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
