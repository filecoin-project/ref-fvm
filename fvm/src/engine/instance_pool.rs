// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::sync::{Condvar, Mutex};

/// An instance pool manages the available pool of engine instances.
///
/// - When there are enough instances to execute an entire message (a full call stack), requests to
///   take an instance will succeed immediately.
/// - When the number of available instances drops below the number required to execute a single
///   message, the executor that took that last instance will get an exclusive "lock" on the
///   instance pool. This lock will be released when enough instances become available to execute an
///   entire message.
pub(super) struct InstancePool {
    inner: Mutex<InstancePoolInner>,
    condv: Condvar,
}

struct InstancePoolInner {
    /// The number of instances available in the pool.
    available: u32,
    /// The maximum number of instances that can be in-use by any given engine. If available drops
    /// to this limit, we'll "lock" the pool to the current executor and refuse to lend out any more
    /// instances to any _other_ engine until we go back above this number.
    per_engine_limit: u32,
    /// The ID of the engine currently "locking" the instance pool.
    locked: Option<u64>,
}

impl InstancePool {
    /// Create a new instance pool.
    pub fn new(available: u32, per_engine_limit: u32) -> InstancePool {
        InstancePool {
            inner: Mutex::new(InstancePoolInner {
                available,
                per_engine_limit,
                locked: None,
            }),
            condv: Condvar::new(),
        }
    }

    /// Put back an instance into the pool, signaling any engines waiting on an instance if
    /// applicable.
    pub fn put(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.available += 1;

        // If we're above the limit, unlock and notify one.
        if guard.available >= guard.per_engine_limit {
            guard.locked = None;
            self.condv.notify_one();
        }
    }

    /// Take an instance out of the instance pool (where `id` is the engine's ID). This function
    /// will block if the instance pool is locked to another engine.
    ///
    /// Panics if any engine tries to allocate more than the configured `per_engine_limit`.
    pub fn take(&self, id: u64) {
        let mut guard = self.inner.lock().unwrap();

        // Wait until we have an instance available. Either:
        // 1. We own the executor lock.
        // 2. We _could_ own the executor lock.
        guard = self
            .condv
            .wait_while(guard, |p| p.locked.unwrap_or(id) != id)
            .unwrap();

        // We either have, or could, lock the executor. So there should be instances available.
        assert!(
            guard.available > 0,
            "no instances available: we must have exceeded our stack depth"
        );

        // Take our instance and lock the executor if we're below the reservation limit.
        guard.available -= 1;
        if guard.available < guard.per_engine_limit {
            guard.locked = Some(id);
        }
    }
}

#[test]
fn test_instance_pool() {
    let pool = InstancePool::new(12, 10);
    std::thread::scope(|scope| {
        pool.take(1);
        pool.take(2);
        assert_eq!(pool.inner.lock().unwrap().locked, None);
        pool.take(1);
        assert_eq!(pool.inner.lock().unwrap().locked, Some(1));
        let t1 = scope.spawn(|| {
            // Take 9 more for engine 2.
            for _ in 0..9 {
                pool.take(2)
            }
        });
        // give the other thread a chance...
        std::thread::sleep(std::time::Duration::from_millis(10));
        // Take the remaining 8 for engine 1 (we're allowed 10).
        // If this is working, we should make progress.
        for _ in 0..8 {
            pool.take(1);
        }
        assert_eq!(pool.inner.lock().unwrap().available, 1);
        assert_eq!(pool.inner.lock().unwrap().locked, Some(1));
        // Put them all back for engine 1.
        for _ in 0..10 {
            pool.put();
        }
        t1.join().unwrap();
        assert_eq!(pool.inner.lock().unwrap().locked, Some(2));

        // We should have two available.
        assert_eq!(pool.inner.lock().unwrap().available, 2);

        // Put back enough to unlock.
        for _ in 0..8 {
            pool.put();
        }
        assert_eq!(pool.inner.lock().unwrap().locked, None);
    });
}
