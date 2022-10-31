use wasmtime::ResourceLimiter;

use crate::machine::NetworkConfig;

/// Execution level memory tracking and adjustment.
pub trait ExecMemory {
    /// Get a snapshot of the total memory required by the Wasm module so far.
    fn total_exec_memory_bytes(&self) -> usize;

    /// Limit the maximum memory bytes available for the rest of the execution.
    ///
    /// This can only make the maximum smaller than what it currently is, not raise it.
    fn avail_exec_memory_bytes(&mut self, limit: usize);
}

/// Limit resources throughout the whole message execution,
/// across all Wasm instances.
pub struct ExecResourceLimiter {
    /// Maximum bytes that a single Wasm instance can use.
    max_inst_memory_bytes: usize,
    /// Maximum bytes that can be used during an execution, in total.
    max_exec_memory_bytes: usize,
    /// Total bytes desired so far in the whole execution.
    /// This is a constraint for all stores created with the
    /// same call manager.
    total_exec_memory_bytes: usize,
    /// If set, the available memory left for the execution.
    avail_exec_memory_bytes: Option<usize>,
}

impl ExecResourceLimiter {
    pub fn new(max_inst_memory_bytes: usize, max_exec_memory_bytes: usize) -> Self {
        Self {
            max_inst_memory_bytes,
            max_exec_memory_bytes,
            total_exec_memory_bytes: 0,
            avail_exec_memory_bytes: None,
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

        let delta_desired = desired - current;
        let total_desired = self.total_exec_memory_bytes + delta_desired;

        if total_desired > min(self.max_exec_memory_bytes, maximum) {
            return false;
        }

        if let Some(avail_memory) = self.avail_exec_memory_bytes {
            if delta_desired > avail_memory {
                return false;
            }
            self.avail_exec_memory_bytes = Some(avail_memory - delta_desired);
        }

        self.total_exec_memory_bytes = total_desired;

        true
    }

    /// No limit on table elements.
    fn table_growing(&mut self, _current: u32, desired: u32, maximum: Option<u32>) -> bool {
        maximum.map_or(true, |m| desired <= m)
    }
}

impl ExecMemory for ExecResourceLimiter {
    fn total_exec_memory_bytes(&self) -> usize {
        self.total_exec_memory_bytes
    }

    fn avail_exec_memory_bytes(&mut self, limit: usize) {
        self.avail_exec_memory_bytes = Some(limit)
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
        assert!(!limits.memory_growing(3, 4, Some(2)));
        assert!(limits.memory_growing(3, 4, None));
        assert!(!limits.memory_growing(4, 5, None));
        assert!(limits.memory_growing(0, 4, None));
        assert!(!limits.memory_growing(0, 3, None));
        assert!(limits.memory_growing(0, 2, None));
        assert!(!limits.memory_growing(0, 2, None));

        assert!(limits.table_growing(0, 100, None));
        assert!(!limits.table_growing(0, 100, Some(10)));
    }

    #[test]
    fn avail_exec_memory_bytes() {
        let mut limits = ExecResourceLimiter::new(6, 10);
        limits.avail_exec_memory_bytes(9); // budget less than max
        assert!(limits.memory_growing(0, 4, None)); // spend some of it
        limits.avail_exec_memory_bytes(5); // reduce budget by spent amount
        assert!(limits.memory_growing(0, 5, None)); // we should be able to grow by what's left
        assert!(!limits.memory_growing(0, 1, None)); // but by now the budget is exhausted
    }
}
