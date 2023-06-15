// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::sync::{Condvar, Mutex};

use derive_more::{Add, AddAssign, Sub, SubAssign};

/// A shared resource limiter.
///
/// - When available resources are sufficient to execute an entire message (a full call stack),
///   requests to reserve resources will succeed immediately.
/// - When available resources drop below the threshold required to execute a single message, the
///   engine currently requesting resources takes an exclusive lock on all remaining resources. This
///   guarantees that said engine will be able to complete its work, eventually returning resources
///   to the limiter. NOTE: dropping below any resource limit will lock all resources to the engine
///   to prevent deadlock.
/// - When available resources go above the specified limit, the resource limiter will be unlocked.
pub(super) struct SharedResourceLimiter {
    inner: Mutex<SharedResourceLimiterInner>,
    condv: Condvar,
}

struct SharedResourceLimiterInner {
    /// The number of instances available in the pool.
    available: Resources,
    /// The maximum number of instances that can be in-use by any given engine. If available drops
    /// to this limit, we'll "lock" the pool to the current executor and refuse to lend out any more
    /// instances to any _other_ engine until we go back above this number.
    per_engine_limit: Resources,

    /// The ID of the engine currently "locking" the instance pool.
    locked: Option<u64>,
}

/// Resources represents a vector of resources.
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Add, AddAssign, Sub, SubAssign)]
pub(super) struct Resources {
    /// A number of instances.
    pub instances: u32,
    /// An amount of memory requested in pages (see [[wasmtime_environ::WASM_PAGE_SIZE]]).
    pub memory_pages: u32,
}

impl SharedResourceLimiter {
    /// Create a new instance pool.
    pub fn new(available: Resources, per_engine_limit: Resources) -> SharedResourceLimiter {
        SharedResourceLimiter {
            inner: Mutex::new(SharedResourceLimiterInner {
                available,
                per_engine_limit,
                locked: None,
            }),
            condv: Condvar::new(),
        }
    }

    /// Return resources to the pool.
    pub fn return_resource(&self, r: Resources) {
        let mut guard = self.inner.lock().unwrap();

        guard.available += r;

        if guard.available >= guard.per_engine_limit {
            guard.locked = None;
            self.condv.notify_one();
        }
    }

    /// Take resources out of the instance pool (where `id` is the engine's ID). This function
    /// will block if the instance pool is locked to another engine.
    ///
    /// Panics if any engine tries to reserve more resources than the configured per-instance limit.
    pub fn reserve_resource(&self, id: u64, r: Resources) {
        let mut guard = self.inner.lock().unwrap();

        // Wait until we have resources available. Either:
        //
        // 1. We own the executor lock. In that case, we're guaranteed to be able to reserve all the
        //    resources we need as long as we stick to the configured per-engine limits.
        // 2. Nobody owns the executor lock. In that case, the guarantee is that there are at least
        //    enough resources available for a single message to execute.
        guard = self
            .condv
            .wait_while(guard, |p| p.locked.unwrap_or(id) != id)
            .unwrap();

        // We let the user enforce this constraint internally. I'd prefer to return an error, but
        // that gets really annoying to deal with.
        assert!(r >= guard.available, "not enough resources");

        // Reserve our instance and lock the executor if we're below the reservation limit.
        guard.available -= r;
        if guard.available < guard.per_engine_limit {
            guard.locked = Some(id);
        }
    }
}
