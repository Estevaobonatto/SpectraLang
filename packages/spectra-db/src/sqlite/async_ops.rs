use super::connection::SqlitePool;
use super::error::{SqliteError, SqliteResult};
use super::statement::SqliteStatement;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;

struct AsyncState<T> {
    result: Option<SqliteResult<T>>,
    waker: Option<Waker>,
}

pub struct SqliteExecuteFuture {
    pool: Arc<SqlitePool>,
    sql: String,
    state: Arc<Mutex<AsyncState<usize>>>,
    cancelled: Arc<AtomicBool>,
    started: bool,
}

impl SqliteExecuteFuture {
    pub fn new(pool: Arc<SqlitePool>, sql: impl Into<String>) -> Self {
        Self {
            pool,
            sql: sql.into(),
            state: Arc::new(Mutex::new(AsyncState {
                result: None,
                waker: None,
            })),
            cancelled: Arc::new(AtomicBool::new(false)),
            started: false,
        }
    }
}

impl Future for SqliteExecuteFuture {
    type Output = SqliteResult<usize>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut state = this.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(result) = state.result.take() {
            return Poll::Ready(result);
        }
        state.waker = Some(cx.waker().clone());
        if !this.started {
            this.started = true;
            let pool = this.pool.clone();
            let sql = this.sql.clone();
            let shared = this.state.clone();
            let cancelled = this.cancelled.clone();
            thread::spawn(move || {
                let result = (|| {
                    if cancelled.load(Ordering::Acquire) {
                        return Err(SqliteError::cancelled());
                    }
                    let lease = pool
                        .acquire_blocking()
                        .map_err(|error| SqliteError::new("DB2504_POOL", error.to_string()))?;
                    if cancelled.load(Ordering::Acquire) {
                        return Err(SqliteError::cancelled());
                    }
                    let mut statement = SqliteStatement::prepare(
                        lease
                            .connection()
                            .map_err(|_| SqliteError::invalid_handle())?
                            .clone(),
                        sql,
                    )?;
                    if cancelled.load(Ordering::Acquire) {
                        return Err(SqliteError::cancelled());
                    }
                    statement.step()?;
                    statement.affected_rows()
                })();
                let mut state = shared.lock().unwrap_or_else(|e| e.into_inner());
                state.result = Some(result);
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            });
        }
        Poll::Pending
    }
}

impl Drop for SqliteExecuteFuture {
    fn drop(&mut self) {
        if self.started {
            self.cancelled.store(true, Ordering::Release);
        }
    }
}
