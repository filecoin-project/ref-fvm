use std::error::Error;
use wasmtime::Trap;

/// Converts any standard error into a Trap by boxing it and using the From trait.
pub fn into_trap<E: Error + Sync + Send + 'static>(e: E) -> Trap {
    Trap::from(Box::from(e))
}
