use std::time::Instant;

use super::GasCharge;

/// Type alias so that we can disable this with a compiler flag.
pub type GasInstant = Instant;

/// A handle returned by `charge_gas` which must be used to mark the end of
/// the execution associated with that gas.
#[must_use]
#[derive(Debug)]
pub struct GasTimer(Option<GasTimerInner>);

#[derive(Debug)]
struct GasTimerInner {
    start: GasInstant,
}

impl GasTimer {
    /// Create a timer that doesn't measure anything.
    pub fn empty() -> Self {
        GasTimer(None)
    }

    /// Convenience method to start measuring time before the charge is made.
    ///
    /// Use the return value with [GasTimer::finish_with] to override the internal
    /// instant that the timer was started with.
    pub fn start() -> GasInstant {
        GasInstant::now()
    }

    /// Create a new timer that will update the elapsed time of a charge when it's finished.
    pub fn new(charge: &GasCharge) -> Self {
        todo!()
    }

    /// Record the elapsed time since the charge was made.
    pub fn stop(self) {
        todo!()
    }

    /// Record the elapsed time based on an instant taken before the charge was made.
    pub fn stop_with(self, start: GasInstant) {
        todo!()
    }

    /// Convenience method to record the elapsed time only if some execution was successful.
    pub fn record<R, E>(self, result: Result<R, E>) -> Result<R, E> {
        if result.is_ok() {
            self.stop()
        }
        result
    }
}
