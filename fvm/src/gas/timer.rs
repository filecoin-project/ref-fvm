use std::sync::Arc;
use std::time::{Duration, Instant};

use once_cell::sync::OnceCell;

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
    elapsed: Arc<OnceCell<Duration>>,
}

impl GasTimer {
    /// Convenience method to start measuring time before the charge is made.
    ///
    /// Use the return value with [GasTimer::finish_with] to override the internal
    /// instant that the timer was started with.
    pub fn start() -> GasInstant {
        GasInstant::now()
    }

    /// Create a timer that doesn't measure anything.
    pub fn empty() -> Self {
        GasTimer(None)
    }

    /// Create a new timer that will update the elapsed time of a charge when it's finished.
    pub fn new(charge: &GasCharge) -> Self {
        assert!(
            charge.elapsed.get().is_none(),
            "GasCharge::elapsed already set!"
        );

        Self(Some(GasTimerInner {
            start: Self::start(),
            elapsed: charge.elapsed.clone(),
        }))
    }

    /// Record the elapsed time since the charge was made.
    pub fn stop(self) {
        if let Some(timer) = self.0 {
            Self::set_elapsed(timer.elapsed, timer.start)
        }
    }

    /// Record the elapsed time based on an instant taken before the charge was made.
    pub fn stop_with(self, start: GasInstant) {
        if let Some(timer) = self.0 {
            Self::set_elapsed(timer.elapsed, start)
        }
    }

    fn set_elapsed(elapsed: Arc<OnceCell<Duration>>, start: GasInstant) {
        elapsed
            .set(start.elapsed())
            .expect("GasCharge::elapsed already set!")
    }

    /// Convenience method to record the elapsed time only if some execution was successful.
    ///
    /// There's no need to record the time of unsuccessful executions because we don't know
    /// how they would compare to successful ones; maybe the error arised before the bulk
    /// of the computation could have taken place.
    pub fn record<R, E>(self, result: Result<R, E>) -> Result<R, E> {
        if result.is_ok() {
            self.stop()
        }
        result
    }
}
