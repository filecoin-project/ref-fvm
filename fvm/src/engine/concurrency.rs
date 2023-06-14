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
    engine_count: u64,
    limit: u32,
}

impl EngineConcurrency {
    pub fn new(concurrency: u32) -> Self {
        EngineConcurrency {
            inner: Mutex::new(EngineConcurrencyInner {
                engine_count: 0,
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
        let id = guard.engine_count;

        guard.limit -= 1;
        guard.engine_count += 1;

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
