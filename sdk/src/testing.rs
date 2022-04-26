// Wrapper around the assert macro to have a hand on which exit code we want to give to our failed
// assertion
#[macro_export]
macro_rules! assert {
    ($cond:expr, $(,)?) => {{
        let res = std::panic::catch_unwind(|| {
            core::assert!($cond);
        });
        if res.is_err() {
            let panic_msg = match res.err() {
                Some(err) => match err.downcast::<String>() {
                    Ok(panic_msg_box) => Some(panic_msg_box.as_str()),
                    Err(err) => None,
                },
                None => unreachable!(),
            };

            $crate::vm::abort(
                fvm_shared::error::ExitCode::USR_ASSERTION_FAILED.value(),
                panic_msg,
            );
        }
    }};
}

// Utility macro to generate macro code for assert_eq and assert_ne
macro_rules! assert2 {
    ($assert_macro:ident) => {
        with_dollar_sign! {
            ($d:tt) => {
                #[macro_export]
                macro_rules! $assert_macro {
                    ($d left:expr, $d right:expr $d(,$d arg:tt)*) => {
                        let res = std::panic::catch_unwind(|| {
                            core::$assert_macro!($d left, $d right);
                        });
                        if res.is_err() {
                            let panic_msg = match res.err() {
                                Some(err) => match err.downcast::<String>() {
                                    Ok(panic_msg_box) => Some(panic_msg_box.as_str()),
                                    Err(err) => None,
                                },
                                None => unreachable!(),
                            };

                            $crate::vm::abort(
                                fvm_shared::error::ExitCode::USR_ASSERTION_FAILED.value(),
                                panic_msg,
                            );
                        }
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
assert2!(assert_eq);

// Wrapper around the assert_ne macro to have a hand on which exit code we want to give to our failed
// assertion
assert2!(assert_ne);
