use super::error::PostgresResult;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;

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
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            return Poll::Ready(result);
        }
        *this.shared.waker.lock().unwrap_or_else(|e| e.into_inner()) = Some(cx.waker().clone());
        if !this.started {
            this.started = true;
            let shared = Arc::clone(&this.shared);
            let operation = this.operation.take().expect("postgres future operation");
            thread::spawn(move || {
                let result = operation();
                *shared.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                if let Some(waker) = shared
                    .waker
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                {
                    waker.wake();
                }
            });
        }
        Poll::Pending
    }
}

pub type PostgresPrepareFuture = PostgresFuture<super::connection::PostgresStatement>;
pub type PostgresExecuteFuture = PostgresFuture<super::connection::PostgresExecutionResult>;
pub type PostgresQueryFuture = PostgresFuture<super::connection::PostgresExecutionResult>;
