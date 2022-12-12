// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::sync::Arc;
use std::time::Duration;

use minstant::Instant;
use once_cell::sync::OnceCell;

/// Shared reference between the duration and the timer.
type DurationCell = Arc<OnceCell<Duration>>;

/// Data structure to encapsulate the optional duration which is set by the `GasTimer`.
///
/// This is normally created with an empty inner, because at the point of creation
/// we don't know if tracing is on or not. It will be filled in later by `GasTimer`.
#[derive(Default, Debug, Clone)]
pub struct GasDuration(Option<DurationCell>);

impl GasDuration {
    pub fn get(&self) -> Option<&Duration> {
        self.0.as_ref().and_then(|d| d.get())
    }
}

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
    pub fn new(duration: &mut GasDuration) -> Self {
        assert!(duration.get().is_none(), "GasCharge::elapsed already set!");

        let cell = match &duration.0 {
            Some(cell) => cell.clone(),
            None => {
                let cell = DurationCell::default();
                duration.0 = Some(cell.clone());
                cell
            }
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
