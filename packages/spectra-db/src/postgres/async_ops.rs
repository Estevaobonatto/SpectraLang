use super::error::PostgresResult;
use std::future::Future;
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct WorkerQueue {
    sender: mpsc::SyncSender<Job>,
}

static WORKERS: OnceLock<WorkerQueue> = OnceLock::new();

fn workers() -> &'static WorkerQueue {
    WORKERS.get_or_init(|| {
        // PostgreSQL calls are blocking at the crate boundary. Keep them off the
        // language reactor, but use a bounded, shared worker queue instead of
        // creating an unbounded OS thread per future.
        let (sender, receiver) = mpsc::sync_channel::<Job>(64);
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..4 {
            let receiver = Arc::clone(&receiver);
            thread::Builder::new()
                .name(format!("spectra-postgres-io-{index}"))
                .spawn(move || loop {
                    let job = receiver
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                })
                .expect("postgres worker thread");
        }
        WorkerQueue { sender }
    })
}

struct Shared<T> {
    result: Mutex<Option<PostgresResult<T>>>,
    waker: Mutex<Option<Waker>>,
}

pub struct PostgresFuture<T> {
    shared: Arc<Shared<T>>,
    started: bool,
    operation: Option<Box<dyn FnOnce() -> PostgresResult<T> + Send + 'static>>,
}

impl<T> PostgresFuture<T> {
    pub(crate) fn new(operation: impl FnOnce() -> PostgresResult<T> + Send + 'static) -> Self {
        Self {
            shared: Arc::new(Shared {
                result: Mutex::new(None),
                waker: Mutex::new(None),
            }),
            started: false,
            operation: Some(Box::new(operation)),
        }
    }
}

impl<T: Send + 'static> Future for PostgresFuture<T> {
    type Output = PostgresResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(result) = this
            .shared
            .result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            return Poll::Ready(result);
        }

        *this
            .shared
            .waker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(cx.waker().clone());

        if !this.started {
            this.started = true;
            let shared = Arc::clone(&this.shared);
            let operation = this.operation.take().expect("postgres future operation");
            // send() is intentionally performed from poll. Dropping an
            // unpolled future therefore cancels it before dispatch.
            let job: Job = Box::new(move || {
                let result = operation();
                *shared
                    .result
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result);
                if let Some(waker) = shared
                    .waker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take()
                {
                    waker.wake();
                }
            });
            if let Err(error) = workers().sender.send(job) {
                // The queue is process-owned and normally cannot close, but
                // preserve a deterministic error rather than losing the waker.
                *this
                    .shared
                    .result
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Err(
                    super::error::PostgresError::new(
                        "DB2505_ASYNC_WORKER_UNAVAILABLE",
                        format!("PostgreSQL worker queue unavailable: {error}"),
                    ),
                ));
                if let Some(waker) = this
                    .shared
                    .waker
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take()
                {
                    waker.wake();
                }
            }
        }
        Poll::Pending
    }
}

pub type PostgresPrepareFuture = PostgresFuture<super::connection::PostgresStatement>;
pub type PostgresExecuteFuture = PostgresFuture<super::connection::PostgresExecutionResult>;
pub type PostgresQueryFuture = PostgresFuture<super::connection::PostgresExecutionResult>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn queue_reuses_bounded_worker_threads() {
        let (tx, rx) = mpsc::sync_channel(1);
        let _ = tx.send(Box::new(|| {}));
        drop(rx);
        let _ = workers();
        thread::sleep(Duration::from_millis(5));
        assert!(WORKERS.get().is_some());
    }
}
