// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::machine::NetworkConfig;

/// Execution level memory tracking and adjustment.
pub trait MemoryLimiter: Sized {
    /// Get a snapshot of the total memory required by the callstack (in bytes). This currently
    /// includes:
    ///
    /// - Memory used by tables (8 bytes per element).
    /// - Memory used by wasmtime instances.
    ///
    /// In the future, this will likely be extended to include IPLD blocks, actor code, etc.
    fn memory_used(&self) -> usize;

    /// Returns `true` if growing by `delta` bytes is allowed. Implement this memory to track and
    /// limit memory usage.
    fn grow_memory(&mut self, delta: usize) -> bool;

    /// Push a new frame onto the call stack, and keep tallying up the current execution memory,
    /// then restore it to the current value when the frame is finished.
    fn with_stack_frame<T, G, F, R>(t: &mut T, g: G, f: F) -> R
    where
        G: Fn(&mut T) -> &mut Self,
        F: FnOnce(&mut T) -> R;

    /// Grows an instance's memory from `from` to `to`. There's no need to manually implement this
    /// unless you need to track instance metrics.
    fn grow_instance_memory(&mut self, from: usize, to: usize) -> bool {
        self.grow_memory(to.saturating_sub(from))
    }

    /// Grows an instance's table from `from` to `to` elements. There's no need to manually
    /// implement this unless you need to track table metrics.
    fn grow_instance_table(&mut self, from: u32, to: u32) -> bool {
        // we charge 8 bytes per table element
        self.grow_memory(to.saturating_sub(from).saturating_mul(8) as usize)
    }
}

/// Limit resources throughout the whole message execution,
/// across all Wasm instances.
pub struct DefaultMemoryLimiter {
    max_memory_bytes: usize,
    curr_memory_bytes: usize,
}

impl DefaultMemoryLimiter {
    pub fn new(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            curr_memory_bytes: 0,
        }
    }

    pub fn for_network(config: &NetworkConfig) -> Self {
        Self::new(config.max_memory_bytes as usize)
    }
}

impl MemoryLimiter for DefaultMemoryLimiter {
    fn memory_used(&self) -> usize {
        self.curr_memory_bytes
    }

    fn grow_memory(&mut self, bytes: usize) -> bool {
        let total_desired = self.curr_memory_bytes.saturating_add(bytes);

        if total_desired > self.max_memory_bytes {
            return false;
        }

        self.curr_memory_bytes = total_desired;
        true
    }

    fn with_stack_frame<T, G, F, R>(t: &mut T, g: G, f: F) -> R
    where
        G: Fn(&mut T) -> &mut Self,
        F: FnOnce(&mut T) -> R,
    {
        let memory_bytes = g(t).curr_memory_bytes;
        let ret = f(t);
        // This method is part of the trait so that a setter like this
        // doesn't have to be made public.
        g(t).curr_memory_bytes = memory_bytes;
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::DefaultMemoryLimiter;
    use crate::machine::limiter::MemoryLimiter;

    #[test]
    fn basics() {
        let mut limits = DefaultMemoryLimiter::new(4);
        assert!(limits.grow_memory(3));
        assert!(limits.grow_memory(1)); // Ok, just at memory limit.
        assert!(!limits.grow_memory(1)); // Fail, over memory limit.

        let mut limits = DefaultMemoryLimiter::new(6);
        assert!(limits.grow_memory(1));
        DefaultMemoryLimiter::with_stack_frame(
            &mut limits,
            |x| x,
            |limits| {
                assert!(limits.grow_memory(3)); // Ok, within memory limit.
                DefaultMemoryLimiter::with_stack_frame(
                    limits,
                    |x| x,
                    |limits| {
                        assert!(!limits.grow_memory(3)); // Fail, 1+3+3 would be over the limit of 6.
                        assert!(limits.grow_memory(2)); // Ok, just at the call stack limit (although we should used a seen a push as well.)
                        assert_eq!(limits.memory_used(), 1 + 3 + 2);
                    },
                );
                assert_eq!(limits.memory_used(), 4);
            },
        );
        assert_eq!(limits.memory_used(), 1);
    }

    #[test]
    fn table() {
        let mut limits = DefaultMemoryLimiter::new(10);
        assert!(limits.grow_instance_table(0, 1)); // 8 bytes
        assert!(limits.grow_memory(2)); // 2 bytes
        assert!(!limits.grow_memory(1));
    }
}
