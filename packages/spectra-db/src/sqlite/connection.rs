use super::error::{SqliteError, SqliteResult};
use super::statement::{SqliteExecutionResult, SqliteStatement, SqliteValue, StepResult};
use crate::query::CompiledQuery;
use crate::{ConnectionFactory, ConnectionPool, PoolConfig};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct ConnectionState {
    pub connection: Connection,
    pub in_transaction: bool,
    pub closed: bool,
}

#[derive(Clone)]
pub struct SqliteConnection {
    pub(crate) state: Arc<Mutex<ConnectionState>>,
}

impl SqliteConnection {
    pub fn open(path: impl AsRef<Path>, busy_timeout: Duration) -> SqliteResult<Self> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(SqliteError::new(
                "DB2504_INVALID_PATH",
                "SQLite path is empty",
            ));
        }
        let connection = Connection::open(path).map_err(SqliteError::from)?;
        connection
            .busy_timeout(busy_timeout)
            .map_err(SqliteError::from)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(SqliteError::from)?;
        Ok(Self {
            state: Arc::new(Mutex::new(ConnectionState {
                connection,
                in_transaction: false,
                closed: false,
            })),
        })
    }

    pub fn execute_batch(&self, sql: &str) -> SqliteResult<()> {
        let state = self.lock()?;
        state
            .connection
            .execute_batch(sql)
            .map_err(SqliteError::from)
    }

    pub fn execute_query(
        &self,
        query: CompiledQuery<SqliteValue>,
    ) -> SqliteResult<SqliteExecutionResult> {
        let mut statement = SqliteStatement::prepare(self.clone(), query.sql)?;
        for (index, value) in query.params.into_iter().enumerate() {
            statement.bind(index + 1, value)?;
        }
        let mut rows = Vec::new();
        while statement.step()? == StepResult::Row {
            let count = statement.column_count()?;
            let mut row = Vec::with_capacity(count);
            for index in 0..count {
                row.push(statement.column_value(index)?);
            }
            rows.push(row);
        }
        let affected_rows = statement.affected_rows()?;
        statement.finalize()?;
        Ok(SqliteExecutionResult {
            rows,
            affected_rows,
        })
    }

    pub fn begin(&self) -> SqliteResult<()> {
        let mut state = self.lock()?;
        if state.in_transaction {
            return Err(SqliteError::invalid_state("transaction already active"));
        }
        state
            .connection
            .execute_batch("BEGIN")
            .map_err(SqliteError::from)?;
        state.in_transaction = true;
        Ok(())
    }

    pub fn commit(&self) -> SqliteResult<()> {
        let mut state = self.lock()?;
        if !state.in_transaction {
            return Err(SqliteError::invalid_state("no active transaction"));
        }
        state
            .connection
            .execute_batch("COMMIT")
            .map_err(SqliteError::from)?;
        state.in_transaction = false;
        Ok(())
    }

    pub fn rollback(&self) -> SqliteResult<()> {
        let mut state = self.lock()?;
        if !state.in_transaction {
            return Err(SqliteError::invalid_state("no active transaction"));
        }
        state
            .connection
            .execute_batch("ROLLBACK")
            .map_err(SqliteError::from)?;
        state.in_transaction = false;
        Ok(())
    }

    pub fn close(&self) -> SqliteResult<()> {
        let mut state = self.lock()?;
        if state.closed {
            return Ok(());
        }
        if state.in_transaction {
            let _ = state.connection.execute_batch("ROLLBACK");
            state.in_transaction = false;
        }
        state.closed = true;
        Ok(())
    }

    pub(crate) fn lock(&self) -> SqliteResult<std::sync::MutexGuard<'_, ConnectionState>> {
        let state = self
            .state
            .lock()
            .map_err(|_| SqliteError::new("DB2504_LOCK", "SQLite connection lock poisoned"))?;
        if state.closed {
            return Err(SqliteError::closed());
        }
        Ok(state)
    }
}

pub struct SqliteFactory {
    pub path: PathBuf,
    pub busy_timeout: Duration,
}

impl SqliteFactory {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            busy_timeout: Duration::from_secs(5),
        }
    }
    pub fn with_busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }
}

impl ConnectionFactory for SqliteFactory {
    type Connection = SqliteConnection;
    type Error = SqliteError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        SqliteConnection::open(&self.path, self.busy_timeout)
    }
    fn is_valid(&self, connection: &Self::Connection) -> bool {
        connection.lock().is_ok()
    }
    fn close(&self, connection: Self::Connection) {
        let _ = connection.close();
    }
}

pub type SqlitePool = ConnectionPool<SqliteFactory>;

pub fn open_pool(path: impl Into<PathBuf>, config: PoolConfig) -> SqliteResult<SqlitePool> {
    ConnectionPool::new(SqliteFactory::new(path), config)
        .map_err(|error| SqliteError::new("DB2504_POOL", error.to_string()))
}
