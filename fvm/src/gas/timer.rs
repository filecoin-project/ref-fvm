use std::fmt;
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use minstant::Instant;

/// Shared reference between the duration and the timer.
type DurationCell = Arc<OnceLock<Duration>>;

/// Data structure to encapsulate the optional duration which is set by the `GasTimer`.
///
/// This is normally created with an empty inner, because at the point of creation
/// we don't know if tracing is on or not. It will be filled in later by `GasTimer`.
#[derive(Default, Clone)]
pub struct GasDuration(GasDurationInner);

#[derive(Default, Clone)]
pub enum GasDurationInner {
    #[default]
    None,
    Atomic(DurationCell),
    Constant(Duration),
}

impl GasDuration {
    pub fn get(&self) -> Option<&Duration> {
        match &self.0 {
            GasDurationInner::None => None,
            GasDurationInner::Atomic(d) => d.get(),
            GasDurationInner::Constant(d) => Some(d),
        }
    }
}

impl From<Duration> for GasDuration {
    fn from(d: Duration) -> Self {
        GasDuration(GasDurationInner::Constant(d))
    }
}

impl fmt::Debug for GasDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GasDuration")
            .field(&self.get() as &dyn fmt::Debug)
            .finish()
    }
}

/// Type alias so that we can disable this with a compiler flag.
pub type GasInstant = Instant;

/// A handle returned by `charge_gas` which must be used to mark the end of
/// the execution associated with that gas.
#[derive(Debug)]
pub struct GasTimer(Option<GasTimerInner>);

#[derive(Debug)]
struct GasTimerInner {
    start: GasInstant,
    elapsed: DurationCell,
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
    ///
    /// As a side effect it will establish the cell in the `GasDuration`, if it has been empty so far.
    ///
    /// When compiled in debug mode, passing a "filled" duration will panic.
    pub fn new(duration: &mut GasDuration) -> Self {
        debug_assert!(duration.get().is_none(), "GasCharge::elapsed already set!");
        let cell = match &duration.0 {
            GasDurationInner::None => {
                let cell = DurationCell::default();
                duration.0 = GasDurationInner::Atomic(cell.clone());
                cell
            }
            GasDurationInner::Atomic(cell) if cell.get().is_none() => cell.clone(),
            // If the duration has already been set, the timer is a no-op.
            _ => return Self(None),
        };

        Self(Some(GasTimerInner {
            start: Self::start(),
            elapsed: cell,
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

    fn set_elapsed(elapsed: Arc<OnceLock<Duration>>, start: GasInstant) {
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
