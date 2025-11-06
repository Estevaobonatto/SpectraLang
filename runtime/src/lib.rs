use std::sync::OnceLock;
use std::thread::ThreadId;
use std::time::{Duration, Instant, SystemTime};

static RUNTIME_STATE: OnceLock<RuntimeState> = OnceLock::new();

/// Captures when and where the runtime became available.
#[derive(Debug)]
pub struct RuntimeState {
    start_instant: Instant,
    start_time: SystemTime,
    init_thread: ThreadId,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            start_instant: Instant::now(),
            start_time: SystemTime::now(),
            init_thread: std::thread::current().id(),
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
}

/// Ensures the runtime is initialised exactly once and returns its state.
pub fn initialize() -> &'static RuntimeState {
    RUNTIME_STATE.get_or_init(RuntimeState::new)
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
