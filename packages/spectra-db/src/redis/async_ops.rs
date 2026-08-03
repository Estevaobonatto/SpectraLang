use super::error::RedisResult;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;

struct Shared<T> {
    result: Mutex<Option<RedisResult<T>>>,
    waker: Mutex<Option<Waker>>,
    cancelled: AtomicBool,
}

pub struct RedisFuture<T> {
    shared: Arc<Shared<T>>,
    started: bool,
    operation: Option<Box<dyn FnOnce() -> RedisResult<T> + Send + 'static>>,
}

impl<T> RedisFuture<T> {
    pub(crate) fn new(operation: impl FnOnce() -> RedisResult<T> + Send + 'static) -> Self {
        Self {
            shared: Arc::new(Shared {
                result: Mutex::new(None),
                waker: Mutex::new(None),
                cancelled: AtomicBool::new(false),
            }),
            started: false,
            operation: Some(Box::new(operation)),
        }
    }
}

impl<T: Send + 'static> Future for RedisFuture<T> {
    type Output = RedisResult<T>;
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
            let operation = this.operation.take().expect("Redis future operation");
            thread::Builder::new()
                .name("spectra-redis-worker".into())
                .spawn(move || {
                    let result = if shared.cancelled.load(Ordering::Acquire) {
                        Err(super::error::RedisError::new(
                            "DB2507_CANCELLED",
                            "Redis operation cancelled before dispatch",
                        ))
                    } else {
                        operation()
                    };
                    *shared.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                    if let Some(waker) = shared
                        .waker
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take()
                    {
                        waker.wake();
                    }
                })
                .expect("spawn Redis worker");
        }
        Poll::Pending
    }
}

impl<T> Drop for RedisFuture<T> {
    fn drop(&mut self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }
}
