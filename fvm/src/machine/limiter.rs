use wasmtime::ResourceLimiter;

use crate::machine::NetworkConfig;

/// Execution level memory tracking and adjustment.
pub trait ExecMemory
where
    Self: Sized,
{
    /// Get a snapshot of the total memory required by the modules on the call stack so far.
    fn curr_exec_memory_bytes(&self) -> usize;

    /// Push a new frame onto the call stack, and keep tallying up the current execution memory,
    /// then restore it to the current value when the frame is finished.
    fn with_stack_frame<T, G, F, R>(t: &mut T, g: G, f: F) -> R
    where
        G: Fn(&mut T) -> &mut Self,
        F: FnOnce(&mut T) -> R;
}

/// Limit resources throughout the whole message execution,
/// across all Wasm instances.
pub struct ExecResourceLimiter {
    /// Maximum bytes that a single Wasm instance can use.
    max_inst_memory_bytes: usize,
    /// Maximum bytes that can be used at any point in time during an execution.
    max_exec_memory_bytes: usize,
    /// Total bytes desired so far by all the instances including the currently executing instance on the call stack.
    curr_exec_memory_bytes: usize,
}

impl ExecResourceLimiter {
    pub fn new(max_inst_memory_bytes: usize, max_exec_memory_bytes: usize) -> Self {
        Self {
            max_inst_memory_bytes,
            max_exec_memory_bytes,
            curr_exec_memory_bytes: 0,
        }
    }

    pub fn for_network(config: &NetworkConfig) -> Self {
        Self::new(
            config.max_inst_memory_bytes as usize,
            config.max_exec_memory_bytes as usize,
        )
    }
}

impl ResourceLimiter for ExecResourceLimiter {
    fn memory_growing(&mut self, current: usize, desired: usize, maximum: Option<usize>) -> bool {
        if desired > min(self.max_inst_memory_bytes, maximum) {
            return false;
        }

        let delta_desired = desired.saturating_sub(current);
        let total_desired = self.curr_exec_memory_bytes.saturating_add(delta_desired);

        if total_desired > self.max_exec_memory_bytes {
            return false;
        }

        self.curr_exec_memory_bytes = total_desired;
        true
    }

    /// No limit on table elements.
    fn table_growing(&mut self, _current: u32, desired: u32, maximum: Option<u32>) -> bool {
        maximum.map_or(true, |m| desired <= m)
    }
}

impl ExecMemory for ExecResourceLimiter {
    fn curr_exec_memory_bytes(&self) -> usize {
        self.curr_exec_memory_bytes
    }

    fn with_stack_frame<T, G, F, R>(t: &mut T, g: G, f: F) -> R
    where
        G: Fn(&mut T) -> &mut Self,
        F: FnOnce(&mut T) -> R,
    {
        let memory_bytes = g(t).curr_exec_memory_bytes;
        let ret = f(t);
        // This method is part of the trait so that a setter like this
        // doesn't have to be made public.
        g(t).curr_exec_memory_bytes = memory_bytes;
        ret
    }
}

fn min(a: usize, b: Option<usize>) -> usize {
    b.map_or(a, |b| std::cmp::min(a, b))
}

#[cfg(test)]
mod tests {
    use wasmtime::ResourceLimiter;

    use super::ExecResourceLimiter;
    use crate::machine::limiter::ExecMemory;

    #[test]
    fn basics() {
        let mut limits = ExecResourceLimiter::new(4, 10);
        assert!(limits.memory_growing(0, 3, None));
        assert!(!limits.memory_growing(3, 4, Some(2))); // The maximum in the args takes precedence.
        assert!(limits.memory_growing(3, 4, None)); // Ok, just at instance limit.
        assert!(!limits.memory_growing(4, 5, None)); // Fail, over instance limit.
        ExecResourceLimiter::with_stack_frame(
            &mut limits,
            |x| x,
            |limits| {
                assert!(limits.memory_growing(0, 4, None)); // Ok, within instance limit.
                ExecResourceLimiter::with_stack_frame(
                    limits,
                    |x| x,
                    |limits| {
                        assert!(!limits.memory_growing(0, 3, None)); // Fail, 4+4+3 would be over the call stack limit of 10.
                        assert!(limits.memory_growing(0, 2, None)); // Ok, just at the call stack limit (although we should used a seen a push as well.)
                        assert_eq!(limits.curr_exec_memory_bytes(), 4 + 4 + 2);
                    },
                );
                assert_eq!(limits.curr_exec_memory_bytes(), 4 + 4);
            },
        );
        assert_eq!(limits.curr_exec_memory_bytes(), 4);
    }

    #[test]
    fn table() {
        let mut limits = ExecResourceLimiter::new(1, 1);
        assert!(limits.table_growing(0, 100, None));
        assert!(!limits.table_growing(0, 100, Some(10)));
    }
}
