use wasmtime::ResourceLimiter;

use crate::machine::NetworkConfig;

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
}

impl ExecResourceLimiter {
    pub fn new(max_inst_memory_bytes: usize, max_exec_memory_bytes: usize) -> Self {
        Self {
            max_inst_memory_bytes,
            max_exec_memory_bytes,
            total_exec_memory_bytes: 0,
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

        let total_desired = self.total_exec_memory_bytes + (desired - current);

        if total_desired > min(self.max_exec_memory_bytes, maximum) {
            return false;
        }

        self.total_exec_memory_bytes = total_desired;
        true
    }

    /// No limit on table elements.
    fn table_growing(&mut self, _current: u32, desired: u32, maximum: Option<u32>) -> bool {
        maximum.map_or(true, |m| desired <= m)
    }
}

fn min(a: usize, b: Option<usize>) -> usize {
    b.map_or(a, |b| std::cmp::min(a, b))
}

#[cfg(test)]
mod tests {
    use wasmtime::ResourceLimiter;

    use super::ExecResourceLimiter;

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
}
