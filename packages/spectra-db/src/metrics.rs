use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default)]
pub(crate) struct MetricsState {
    pub created: AtomicUsize,
    pub active: AtomicUsize,
    pub idle: AtomicUsize,
    pub discarded: AtomicUsize,
    pub failures: AtomicUsize,
    pub acquisition_timeouts: AtomicUsize,
    pub waiters: AtomicUsize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolMetrics {
    pub created: usize,
    pub active: usize,
    pub idle: usize,
    pub discarded: usize,
    pub failures: usize,
    pub acquisition_timeouts: usize,
    pub waiters: usize,
}

impl MetricsState {
    pub fn snapshot(&self) -> PoolMetrics {
        let load = |value: &AtomicUsize| value.load(Ordering::Acquire);
        PoolMetrics {
            created: load(&self.created),
            active: load(&self.active),
            idle: load(&self.idle),
            discarded: load(&self.discarded),
            failures: load(&self.failures),
            acquisition_timeouts: load(&self.acquisition_timeouts),
            waiters: load(&self.waiters),
        }
    }
}
