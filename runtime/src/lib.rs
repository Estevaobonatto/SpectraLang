use std::sync::OnceLock;
use std::thread::ThreadId;
use std::time::{Duration, Instant, SystemTime};

pub mod memory;

pub use memory::{CollectionOutcome, HybridMemory, MemoryConfig, MemoryStats, TracedStats, ManualStats};

static RUNTIME_STATE: OnceLock<RuntimeState> = OnceLock::new();

/// Captures when and where the runtime became available.
#[derive(Debug)]
pub struct RuntimeState {
    start_instant: Instant,
    start_time: SystemTime,
    init_thread: ThreadId,
    memory: HybridMemory,
}

impl RuntimeState {
    fn with_config(config: MemoryConfig) -> Self {
        Self {
            start_instant: Instant::now(),
            start_time: SystemTime::now(),
            init_thread: std::thread::current().id(),
            memory: HybridMemory::with_config(config),
        }
    }

    /// Returns how long the runtime has been alive using a monotonic clock.
    pub fn uptime(&self) -> Duration {
        self.start_instant.elapsed()
    }

    /// Returns the wall-clock time at which the runtime finished initialisation.
    pub fn started_at(&self) -> SystemTime {
        self.start_time
    }

    /// Returns the identifier of the thread that performed the initialisation.
    pub fn init_thread_id(&self) -> ThreadId {
        self.init_thread
    }

    /// Returns the hybrid memory manager associated with this runtime.
    pub fn memory(&self) -> &HybridMemory {
        &self.memory
    }

    /// Returns current memory statistics.
    pub fn memory_stats(&self) -> MemoryStats {
        self.memory.stats()
    }

    /// Returns the active memory configuration.
    pub fn memory_config(&self) -> MemoryConfig {
        self.memory.config()
    }

    /// Forces a garbage collection cycle via the hybrid memory manager.
    pub fn collect_garbage(&self) -> CollectionOutcome {
        self.memory.collect_garbage()
    }
}

/// Ensures the runtime is initialised exactly once and returns its state.
pub fn initialize() -> &'static RuntimeState {
    initialize_with_config(MemoryConfig::default())
}

/// Initialises the runtime using the specified memory configuration.
pub fn initialize_with_config(config: MemoryConfig) -> &'static RuntimeState {
    RUNTIME_STATE.get_or_init(move || RuntimeState::with_config(config))
}

/// Returns the runtime state if it has already been initialised.
pub fn state() -> Option<&'static RuntimeState> {
    RUNTIME_STATE.get()
}

/// Returns whether the runtime has already been initialised.
pub fn is_initialized() -> bool {
    RUNTIME_STATE.get().is_some()
}

/// Returns the monotonic uptime, initialising the runtime if required.
pub fn uptime() -> Duration {
    initialize().uptime()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn initialise_only_once() {
        let first = initialize() as *const _;
        let second = initialize() as *const _;
        assert_eq!(first, second);
    }

    #[test]
    fn uptime_increases_over_time() {
        initialize();
        thread::sleep(Duration::from_millis(5));
        assert!(uptime() > Duration::from_millis(0));
    }
}
