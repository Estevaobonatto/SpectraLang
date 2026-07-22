use super::async_ops::{
    PostgresExecuteFuture, PostgresFuture, PostgresPrepareFuture, PostgresQueryFuture,
};
use super::error::{PostgresError, PostgresResult};
use super::value::PostgresValue;
use crate::query::{CompiledQuery, PostgresDialect, Query, QueryError};
use crate::sqlite::SqliteValue;
use crate::{ConnectionFactory, ConnectionPool, PoolConfig};
use native_tls::TlsConnector;
use postgres::{Client, NoTls};
use postgres_native_tls::MakeTlsConnector;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
}

#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: SecretString,
    pub ssl_mode: SslMode,
    pub connect_timeout: Duration,
    pub statement_timeout: Option<Duration>,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 5432,
            database: "postgres".into(),
            user: "postgres".into(),
            password: "".into(),
            ssl_mode: SslMode::Disable,
            connect_timeout: Duration::from_secs(5),
            statement_timeout: None,
        }
    }
}

impl PostgresConfig {
    pub fn from_url(url: &str) -> PostgresResult<Self> {
        let parsed = url
            .strip_prefix("postgres://")
            .or_else(|| url.strip_prefix("postgresql://"))
            .ok_or_else(|| {
                PostgresError::invalid_argument("PostgreSQL URL must use postgres://")
            })?;
        let (authority, database) = parsed
            .split_once('/')
            .ok_or_else(|| PostgresError::invalid_argument("PostgreSQL URL requires a database"))?;
        let (credentials, host_port) = authority.rsplit_once('@').ok_or_else(|| {
            PostgresError::invalid_argument("PostgreSQL URL requires user credentials")
        })?;
        let (user, password) = credentials.split_once(':').unwrap_or((credentials, ""));
        let (host, port) = host_port
            .rsplit_once(':')
            .map_or((host_port, 5432), |(host, port)| {
                (host, port.parse().unwrap_or(0))
            });
        if user.is_empty() || host.is_empty() || database.is_empty() || port == 0 {
            return Err(PostgresError::invalid_argument("invalid PostgreSQL URL"));
        }
        Ok(Self {
            host: host.to_owned(),
            port,
            database: database.split('?').next().unwrap_or(database).to_owned(),
            user: user.to_owned(),
            password: password.into(),
            ..Self::default()
        })
    }

    fn validate(&self) -> PostgresResult<()> {
        if self.host.is_empty()
            || self.database.is_empty()
            || self.user.is_empty()
            || self.port == 0
        {
            return Err(PostgresError::invalid_argument(
                "host, port, database and user are required",
            ));
        }
        Ok(())
    }

    fn connect(&self) -> PostgresResult<Client> {
        self.validate()?;
        let mut config = postgres::Config::new();
        config
            .host(&self.host)
            .port(self.port)
            .dbname(&self.database)
            .user(&self.user)
            .connect_timeout(self.connect_timeout);
        if !self.password.expose_secret().is_empty() {
            config.password(self.password.expose_secret());
        }
        match self.ssl_mode {
            SslMode::Disable => config.connect(NoTls).map_err(PostgresError::from),
            SslMode::Prefer | SslMode::Require => {
                let tls = TlsConnector::new()
                    .map_err(|e| PostgresError::new("DB2505_TLS", e.to_string()))?;
                config
                    .connect(MakeTlsConnector::new(tls))
                    .map_err(PostgresError::from)
            }
        }
    }
}

enum ClientKind {
    Plain(Client),
    Tls(Client),
}

impl ClientKind {
    fn client(&mut self) -> &mut Client {
        match self {
            Self::Plain(c) | Self::Tls(c) => c,
        }
    }
}

#[derive(Clone)]
pub struct PostgresConnection {
    state: Arc<Mutex<Option<ClientKind>>>,
    config: PostgresConfig,
}

impl std::fmt::Debug for PostgresConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresConnection")
            .field("config", &self.config)
            .finish()
    }
}

impl PostgresConnection {
    pub fn open(config: PostgresConfig) -> PostgresResult<Self> {
        let mut client = config.connect()?;
        if let Some(timeout) = config.statement_timeout {
            client
                .batch_execute(&format!("SET statement_timeout = {}", timeout.as_millis()))
                .map_err(PostgresError::from)?;
        }
        let kind = match config.ssl_mode {
            SslMode::Disable => ClientKind::Plain(client),
            _ => ClientKind::Tls(client),
        };
        let connection = Self {
            state: Arc::new(Mutex::new(Some(kind))),
            config,
        };
        connection.health_check()?;
        Ok(connection)
    }

    pub fn health_check(&self) -> PostgresResult<()> {
        self.execute("SELECT 1", &[]).map(|_| ())
    }

    pub fn prepare(&self, sql: impl Into<String>) -> PostgresResult<PostgresStatement> {
        let sql = sql.into();
        if sql.trim().is_empty() {
            return Err(PostgresError::invalid_argument("SQL cannot be empty"));
        }
        let mut state = self.lock()?;
        state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client()
            .prepare(&sql)
            .map_err(PostgresError::from)?;
        Ok(PostgresStatement {
            connection: self.clone(),
            sql,
            params: Vec::new(),
            executed: false,
            rows: Vec::new(),
            columns: Vec::new(),
            cursor: 0,
        })
    }

    pub fn execute_query(
        &self,
        query: CompiledQuery<PostgresValue>,
    ) -> PostgresResult<PostgresExecutionResult> {
        let params = query
            .params
            .iter()
            .map(PostgresValue::as_param)
            .collect::<Vec<_>>();
        let refs = params
            .iter()
            .map(|p| p.as_ref() as &(dyn postgres::types::ToSql + Sync))
            .collect::<Vec<_>>();
        let normalized = query.sql.trim_start().to_ascii_uppercase();
        if normalized.starts_with("SELECT ")
            || normalized.starts_with("SELECT\n")
            || normalized.starts_with("SHOW ")
            || normalized.starts_with("VALUES ")
            || normalized.contains(" RETURNING ")
        {
            self.query(&query.sql, &refs)
        } else {
            self.execute(&query.sql, &refs)
        }
    }

    pub fn execute_builder<Q: Query>(&self, query: &Q) -> PostgresResult<PostgresExecutionResult> {
        let compiled = query.compile(&PostgresDialect).map_err(query_error)?;
        let params = compiled
            .params
            .into_iter()
            .map(sqlite_to_postgres)
            .collect();
        self.execute_query(CompiledQuery {
            sql: compiled.sql,
            params,
        })
    }

    pub fn query_async(&self, query: CompiledQuery<PostgresValue>) -> PostgresQueryFuture {
        let connection = self.clone();
        PostgresFuture::new(move || connection.execute_query(query))
    }

    pub fn prepare_async(&self, sql: impl Into<String>) -> PostgresPrepareFuture {
        let connection = self.clone();
        let sql = sql.into();
        PostgresFuture::new(move || connection.prepare(sql))
    }

    pub fn execute_async(&self, query: CompiledQuery<PostgresValue>) -> PostgresExecuteFuture {
        let connection = self.clone();
        PostgresFuture::new(move || connection.execute_query(query))
    }

    pub fn begin(&self) -> PostgresResult<PostgresTransaction> {
        self.execute("BEGIN", &[])?;
        Ok(PostgresTransaction {
            connection: self.clone(),
            active: true,
        })
    }

    pub fn execute_batch(&self, sql: &str) -> PostgresResult<()> {
        let mut state = self.lock()?;
        state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client()
            .batch_execute(sql)
            .map_err(PostgresError::from)
    }

    pub fn copy_in_rows(
        &self,
        sql: &str,
        rows: impl IntoIterator<Item = Vec<PostgresValue>>,
    ) -> PostgresResult<u64> {
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY IN requires a COPY statement",
            ));
        }
        let mut state = self.lock()?;
        let mut writer = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client()
            .copy_in(sql)
            .map_err(PostgresError::from)?;
        let mut count = 0u64;
        for row in rows {
            let encoded = row
                .iter()
                .map(encode_copy_value)
                .collect::<Vec<_>>()
                .join("\t");
            writer
                .write_all(encoded.as_bytes())
                .map_err(|e| PostgresError::new("DB2505_COPY_IN", e.to_string()))?;
            writer
                .write_all(b"\n")
                .map_err(|e| PostgresError::new("DB2505_COPY_IN", e.to_string()))?;
            count += 1;
        }
        writer.finish().map_err(PostgresError::from)?;
        Ok(count)
    }

    pub fn copy_out_bytes(&self, sql: &str) -> PostgresResult<Vec<u8>> {
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY OUT requires a COPY statement",
            ));
        }
        let mut state = self.lock()?;
        let mut reader = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client()
            .copy_out(sql)
            .map_err(PostgresError::from)?;
        let mut output = Vec::new();
        reader
            .read_to_end(&mut output)
            .map_err(|e| PostgresError::new("DB2505_COPY_OUT", e.to_string()))?;
        Ok(output)
    }

    pub fn listen(&self, channel: &str) -> PostgresResult<NotificationListener> {
        validate_name(channel)?;
        self.execute(&format!("LISTEN \"{channel}\""), &[])?;
        Ok(NotificationListener {
            connection: self.clone(),
            channel: channel.to_owned(),
        })
    }

    pub fn close(&self) -> PostgresResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PostgresError::new("DB2505_LOCK", "connection lock poisoned"))?;
        state.take();
        Ok(())
    }

    pub(crate) fn execute(
        &self,
        sql: &str,
        params: &[&(dyn postgres::types::ToSql + Sync)],
    ) -> PostgresResult<PostgresExecutionResult> {
        let mut state = self.lock()?;
        let affected = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client()
            .execute(sql, params)
            .map_err(PostgresError::from)?;
        Ok(PostgresExecutionResult {
            rows: Vec::new(),
            affected_rows: affected as usize,
            columns: Vec::new(),
        })
    }

    pub(crate) fn query(
        &self,
        sql: &str,
        params: &[&(dyn postgres::types::ToSql + Sync)],
    ) -> PostgresResult<PostgresExecutionResult> {
        let mut state = self.lock()?;
        let rows = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client()
            .query(sql, params)
            .map_err(PostgresError::from)?;
        let columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|c| PostgresColumn {
                        name: c.name().to_owned(),
                        ty: c.type_().name().to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let values = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| PostgresValue::from_cell(row, i))
                    .collect()
            })
            .collect();
        Ok(PostgresExecutionResult {
            rows: values,
            affected_rows: rows.len(),
            columns,
        })
    }

    fn lock(&self) -> PostgresResult<std::sync::MutexGuard<'_, Option<ClientKind>>> {
        let guard = self
            .state
            .lock()
            .map_err(|_| PostgresError::new("DB2505_LOCK", "connection lock poisoned"))?;
        if guard.is_none() {
            return Err(PostgresError::invalid_handle());
        }
        Ok(guard)
    }
}

pub struct PostgresStatement {
    connection: PostgresConnection,
    sql: String,
    params: Vec<PostgresValue>,
    executed: bool,
    rows: Vec<Vec<PostgresValue>>,
    columns: Vec<PostgresColumn>,
    cursor: usize,
}

impl PostgresStatement {
    pub fn bind(&mut self, index: usize, value: PostgresValue) -> PostgresResult<()> {
        if index == 0 {
            return Err(PostgresError::invalid_argument(
                "parameter indexes are 1-based",
            ));
        }
        if self.executed {
            return Err(PostgresError::new(
                "DB2505_INVALID_STATE",
                "reset required before binding",
            ));
        }
        if self.params.len() < index {
            self.params.resize(index, PostgresValue::Null);
        }
        self.params[index - 1] = value;
        Ok(())
    }

    pub fn execute(&mut self) -> PostgresResult<PostgresExecutionResult> {
        let query = CompiledQuery {
            sql: self.sql.clone(),
            params: self.params.clone(),
        };
        self.executed = true;
        self.cursor = 0;
        let result = self.connection.execute_query(query)?;
        self.rows = result.rows.clone();
        self.columns = result.columns.clone();
        Ok(result)
    }

    pub fn step(&mut self) -> PostgresResult<i32> {
        if !self.executed {
            self.execute()?;
        }
        if self.cursor < self.rows.len() {
            self.cursor += 1;
            Ok(1)
        } else {
            Ok(2)
        }
    }
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
    pub fn column_type(&self, index: usize) -> PostgresResult<super::value::PostgresType> {
        self.rows
            .first()
            .and_then(|r| r.get(index))
            .map(|v| v.ty())
            .ok_or_else(|| PostgresError::invalid_argument("column index out of range"))
    }
    pub fn column_value(&self, index: usize) -> PostgresResult<PostgresValue> {
        self.rows
            .get(self.cursor.saturating_sub(1))
            .and_then(|r| r.get(index))
            .cloned()
            .ok_or_else(|| PostgresError::invalid_argument("column index out of range"))
    }
    pub fn reset(&mut self) {
        self.executed = false;
        self.params.clear();
        self.rows.clear();
        self.columns.clear();
        self.cursor = 0;
    }
    pub fn finalize(self) {}
}

#[derive(Debug, Clone)]
pub struct PostgresColumn {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct PostgresExecutionResult {
    pub rows: Vec<Vec<PostgresValue>>,
    pub affected_rows: usize,
    pub columns: Vec<PostgresColumn>,
}

pub struct PostgresTransaction {
    connection: PostgresConnection,
    active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub channel: String,
    pub payload: String,
    pub process_id: i32,
}

pub struct NotificationListener {
    connection: PostgresConnection,
    channel: String,
}

impl NotificationListener {
    pub fn channel(&self) -> &str {
        &self.channel
    }
    pub fn next_timeout(&self, timeout: Duration) -> PostgresResult<Option<Notification>> {
        let mut state = self.connection.lock()?;
        let client = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?
            .client();
        let mut notifications = client.notifications();
        let mut iterator = notifications.timeout_iter(timeout);
        use fallible_iterator::FallibleIterator;
        iterator.next().map_err(PostgresError::from).map(|item| {
            item.map(|n| Notification {
                channel: n.channel().to_owned(),
                payload: n.payload().to_owned(),
                process_id: n.process_id(),
            })
        })
    }
}

impl Drop for NotificationListener {
    fn drop(&mut self) {
        let _ = self
            .connection
            .execute(&format!("UNLISTEN \"{}\"", self.channel), &[]);
    }
}

impl PostgresTransaction {
    pub fn execute(&self, sql: &str) -> PostgresResult<PostgresExecutionResult> {
        if !self.active {
            return Err(PostgresError::new(
                "DB2505_TRANSACTION_CLOSED",
                "transaction is closed",
            ));
        }
        self.connection.execute(sql, &[])
    }
    pub fn savepoint(&self, name: &str) -> PostgresResult<()> {
        validate_name(name)?;
        self.execute(&format!("SAVEPOINT \"{name}\"")).map(|_| ())
    }
    pub fn rollback_to(&self, name: &str) -> PostgresResult<()> {
        validate_name(name)?;
        self.execute(&format!("ROLLBACK TO SAVEPOINT \"{name}\""))
            .map(|_| ())
    }
    pub fn release_savepoint(&self, name: &str) -> PostgresResult<()> {
        validate_name(name)?;
        self.execute(&format!("RELEASE SAVEPOINT \"{name}\""))
            .map(|_| ())
    }
    pub fn commit(mut self) -> PostgresResult<()> {
        self.connection.execute("COMMIT", &[])?;
        self.active = false;
        Ok(())
    }
    pub fn rollback(mut self) -> PostgresResult<()> {
        self.connection.execute("ROLLBACK", &[])?;
        self.active = false;
        Ok(())
    }
}

impl Drop for PostgresTransaction {
    fn drop(&mut self) {
        if self.active {
            let _ = self.connection.execute("ROLLBACK", &[]);
        }
    }
}

fn validate_name(name: &str) -> PostgresResult<()> {
    if name.is_empty()
        || name.len() > 63
        || name
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    {
        return Err(PostgresError::invalid_argument("invalid savepoint name"));
    }
    Ok(())
}

fn encode_copy_value(value: &PostgresValue) -> String {
    match value {
        PostgresValue::Null => "\\N".to_owned(),
        PostgresValue::Bool(value) => value.to_string(),
        PostgresValue::Int16(value) => value.to_string(),
        PostgresValue::Int32(value) => value.to_string(),
        PostgresValue::Int64(value) => value.to_string(),
        PostgresValue::Float32(value) => value.to_string(),
        PostgresValue::Float64(value) => value.to_string(),
        PostgresValue::Text(value) => value
            .replace('\\', "\\\\")
            .replace('\t', "\\t")
            .replace('\n', "\\n")
            .replace('\r', "\\r"),
        PostgresValue::Bytes(value) => format!(
            "\\\\x{}",
            value.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ),
        PostgresValue::Uuid(value) => value.to_string(),
        PostgresValue::Timestamp(value) => value.to_rfc3339(),
    }
}

fn query_error(error: QueryError) -> PostgresError {
    PostgresError::new("DB2505_QUERY", error.to_string())
}

fn sqlite_to_postgres(value: SqliteValue) -> PostgresValue {
    match value {
        SqliteValue::Null => PostgresValue::Null,
        SqliteValue::Integer(value) => PostgresValue::Int64(value),
        SqliteValue::Real(value) => PostgresValue::Float64(value),
        SqliteValue::Text(value) => PostgresValue::Text(value),
        SqliteValue::Blob(value) => PostgresValue::Bytes(value),
    }
}

#[derive(Clone)]
pub struct PostgresFactory {
    pub config: PostgresConfig,
}
impl PostgresFactory {
    pub fn new(config: PostgresConfig) -> Self {
        Self { config }
    }
}
impl ConnectionFactory for PostgresFactory {
    type Connection = PostgresConnection;
    type Error = PostgresError;
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        PostgresConnection::open(self.config.clone())
    }
    fn is_valid(&self, connection: &Self::Connection) -> bool {
        connection.health_check().is_ok()
    }
    fn close(&self, connection: Self::Connection) {
        let _ = connection.close();
    }
}
pub type PostgresPool = ConnectionPool<PostgresFactory>;
pub fn open_pool(config: PostgresConfig, pool: PoolConfig) -> PostgresResult<PostgresPool> {
    ConnectionPool::new(PostgresFactory::new(config), pool)
        .map_err(|e| PostgresError::new("DB2505_POOL", e.to_string()))
}
