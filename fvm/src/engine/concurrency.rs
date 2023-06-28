// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::sync::{Condvar, Mutex};

/// An engine concurrency manages the concurrency available for a single engine. It's basically a
/// semaphore that also assigns IDs to new engines.
pub(super) struct EngineConcurrency {
    inner: Mutex<EngineConcurrencyInner>,
    condv: Condvar,
}

struct EngineConcurrencyInner {
    next_id: u64,
    limit: u32,
}

impl EngineConcurrency {
    pub fn new(concurrency: u32) -> Self {
        EngineConcurrency {
            inner: Mutex::new(EngineConcurrencyInner {
                next_id: 0,
                limit: concurrency,
            }),
            condv: Condvar::new(),
        }
    }

    /// Acquire a new engine (well, an engine ID). This function blocks until we're below the
    /// maximum engine concurrency limit.
    pub fn acquire(&self) -> u64 {
        let mut guard = self
            .condv
            .wait_while(self.inner.lock().unwrap(), |inner| inner.limit == 0)
            .unwrap();
        let id = guard.next_id;

        guard.limit -= 1;
        guard.next_id += 1;

        id
    }

    /// Release the engine. After this is called, the caller should not allocate any more instances
    /// or continue to use their engine ID.
    pub fn release(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.limit += 1;
        self.condv.notify_one();
    }
}

#[test]
fn test_engine_concurrency() {
    let concurrency = EngineConcurrency::new(2);
    std::thread::scope(|scope| {
        assert_eq!(concurrency.inner.lock().unwrap().limit, 2);
        assert_eq!(concurrency.acquire(), 0);
        assert_eq!(concurrency.inner.lock().unwrap().limit, 1);
        assert_eq!(concurrency.acquire(), 1);
        assert_eq!(concurrency.inner.lock().unwrap().limit, 0);
        let threads: Vec<_> = std::iter::repeat_with(|| scope.spawn(|| concurrency.acquire()))
            .take(10)
            .collect();
        assert_eq!(concurrency.inner.lock().unwrap().limit, 0);
        for _ in &threads {
            concurrency.release();
        }
        let mut ids: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        ids.sort();
        assert_eq!(ids, (2..12).collect::<Vec<_>>());
        assert_eq!(concurrency.inner.lock().unwrap().limit, 0);
        concurrency.release();
        assert_eq!(concurrency.inner.lock().unwrap().limit, 1);
    });
}
