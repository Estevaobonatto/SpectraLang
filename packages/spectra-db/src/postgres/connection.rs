use super::async_ops::{
    PostgresExecuteFuture, PostgresFuture, PostgresPrepareFuture, PostgresQueryFuture,
};
use super::error::{PostgresError, PostgresResult};
use super::value::PostgresValue;
use crate::query::{CompiledQuery, PostgresDialect, Query, QueryError};
use crate::sqlite::SqliteValue;
use crate::{ConnectionFactory, ConnectionPool, PoolConfig};
use native_tls::TlsConnector;
use postgres::{CancelToken, Client, NoTls};
use postgres_native_tls::MakeTlsConnector;
use spectra_runtime::tracing::{self, SpanKind, SpanStatus};
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex};
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
        let parsed = url::Url::parse(url)
            .map_err(|_| PostgresError::invalid_argument("invalid PostgreSQL URL"))?;
        if parsed.scheme() != "postgres" && parsed.scheme() != "postgresql" {
            return Err(PostgresError::invalid_argument(
                "PostgreSQL URL must use postgres://",
            ));
        }
        let user = percent_encoding::percent_decode_str(parsed.username())
            .decode_utf8()
            .map_err(|_| PostgresError::invalid_argument("invalid PostgreSQL username"))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| PostgresError::invalid_argument("PostgreSQL URL requires a host"))?;
        let database = parsed.path().trim_start_matches('/');
        if user.is_empty() || host.is_empty() || database.is_empty() {
            return Err(PostgresError::invalid_argument("invalid PostgreSQL URL"));
        }
        let mut config = Self {
            host: host.to_owned(),
            port: parsed.port().unwrap_or(5432),
            database: database.to_owned(),
            user: user.into_owned(),
            password: percent_encoding::percent_decode_str(parsed.password().unwrap_or_default())
                .decode_utf8()
                .map_err(|_| PostgresError::invalid_argument("invalid PostgreSQL password"))?
                .into_owned()
                .into(),
            ..Self::default()
        };
        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "sslmode" => {
                    config.ssl_mode = match value.as_ref() {
                        "disable" => SslMode::Disable,
                        "prefer" => SslMode::Prefer,
                        "require" => SslMode::Require,
                        _ => return Err(PostgresError::invalid_argument("invalid sslmode")),
                    }
                }
                "connect_timeout" => {
                    let seconds = value
                        .parse::<u64>()
                        .map_err(|_| PostgresError::invalid_argument("invalid connect_timeout"))?;
                    config.connect_timeout = Duration::from_secs(seconds);
                }
                "statement_timeout" => {
                    let millis = value
                        .parse::<u64>()
                        .map_err(|_| PostgresError::invalid_argument("invalid statement_timeout"))?;
                    config.statement_timeout = Some(Duration::from_millis(millis));
                }
                _ => {}
            }
        }
        config.validate()?;
        Ok(config)
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

    fn connect(&self) -> PostgresResult<ClientKind> {
        self.validate()?;
        let build_config = || {
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
            config
        };
        match self.ssl_mode {
            SslMode::Disable => build_config()
                .connect(NoTls)
                .map(ClientKind::Plain)
                .map_err(PostgresError::from),
            SslMode::Require => {
                let tls = TlsConnector::new()
                    .map_err(|e| PostgresError::new("DB2505_TLS", e.to_string()))?;
                build_config()
                    .connect(MakeTlsConnector::new(tls))
                    .map(ClientKind::Tls)
                    .map_err(PostgresError::from)
            }
            SslMode::Prefer => {
                let tls = TlsConnector::new()
                    .map_err(|e| PostgresError::new("DB2505_TLS", e.to_string()))?;
                match build_config().connect(MakeTlsConnector::new(tls)) {
                    Ok(client) => Ok(ClientKind::Tls(client)),
                    Err(_) => build_config()
                        .connect(NoTls)
                        .map(ClientKind::Plain)
                        .map_err(PostgresError::from),
                }
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

    fn cancellation(&mut self) -> PostgresCancellation {
        let tls = matches!(self, Self::Tls(_));
        let token = self.client().cancel_token();
        PostgresCancellation { token, tls }
    }
}

#[derive(Clone)]
pub struct PostgresConnection {
    state: Arc<Mutex<Option<ClientKind>>>,
    config: PostgresConfig,
}

#[derive(Clone)]
pub struct PostgresCancellation {
    token: CancelToken,
    tls: bool,
}

impl PostgresCancellation {
    pub fn cancel(&self) -> PostgresResult<()> {
        match self.tls {
            false => self
                .token
                .cancel_query(NoTls)
                .map_err(PostgresError::from),
            true => {
                let tls = TlsConnector::new()
                    .map_err(|error| PostgresError::new("DB2505_TLS", error.to_string()))?;
                self.token
                    .cancel_query(MakeTlsConnector::new(tls))
                    .map_err(PostgresError::from)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationPhase {
    Queued,
    Running,
    CancelRequested,
    Cancelled,
    Done,
}

struct OperationCancellationState {
    phase: OperationPhase,
    token: Option<PostgresCancellation>,
    cancel_done: bool,
    cancel_error: Option<PostgresError>,
}

struct OperationCancellationInner {
    state: Mutex<OperationCancellationState>,
    ready: Condvar,
}

/// Per-operation cancellation state. A PostgreSQL cancel token is armed only
/// after the target operation owns its backend session, so cancelling a queued
/// task cannot affect another query on the same connection.
#[derive(Clone)]
pub struct PostgresOperationCancellation {
    inner: Arc<OperationCancellationInner>,
}

impl Default for PostgresOperationCancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl PostgresOperationCancellation {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(OperationCancellationInner {
                state: Mutex::new(OperationCancellationState {
                    phase: OperationPhase::Queued,
                    token: None,
                    cancel_done: false,
                    cancel_error: None,
                }),
                ready: Condvar::new(),
            }),
        }
    }

    pub fn request_cancel(&self) -> PostgresResult<bool> {
        let token = {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match state.phase {
                OperationPhase::Queued => {
                    state.phase = OperationPhase::Cancelled;
                    self.inner.ready.notify_all();
                    return Ok(true);
                }
                OperationPhase::Running => {
                    state.phase = OperationPhase::CancelRequested;
                    state.cancel_done = false;
                    state.token.clone()
                }
                OperationPhase::CancelRequested
                | OperationPhase::Cancelled
                | OperationPhase::Done => return Ok(false),
            }
        };
        let Some(token) = token else {
            return Err(PostgresError::new(
                "DB2505_CANCEL_STATE",
                "running PostgreSQL operation has no armed cancellation token",
            ));
        };
        let inner = Arc::clone(&self.inner);
        if let Err(error) = super::async_ops::dispatch_cancellation(move || {
            let cancel_error = token.cancel().err();
            let mut state = inner
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.cancel_error = cancel_error;
            state.cancel_done = true;
            inner.ready.notify_all();
        }) {
            let mut state = self
                .inner
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.cancel_error = Some(error);
            state.cancel_done = true;
            self.inner.ready.notify_all();
        }
        Ok(true)
    }

    fn arm(&self, token: PostgresCancellation) -> bool {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match state.phase {
            OperationPhase::Queued => {
                state.phase = OperationPhase::Running;
                state.token = Some(token);
                true
            }
            OperationPhase::Cancelled => false,
            _ => false,
        }
    }

    fn finish(&self) -> PostgresResult<()> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while state.phase == OperationPhase::CancelRequested && !state.cancel_done {
            state = self
                .inner
                .ready
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        state.phase = OperationPhase::Done;
        state.token = None;
        match state.cancel_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn was_cancelled_before_start(&self) -> bool {
        matches!(
            self.inner
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .phase,
            OperationPhase::CancelRequested | OperationPhase::Cancelled
        )
    }
}

impl std::fmt::Debug for PostgresConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresConnection")
            .field("config", &self.config)
            .finish()
    }
}

impl PostgresConnection {
    /// Sanitized connection metadata used for observability. Credentials and
    /// the original DSN are intentionally not exposed.
    pub fn server_address(&self) -> String {
        self.config.host.clone()
    }

    pub fn server_port(&self) -> u16 {
        self.config.port
    }

    pub fn database_name(&self) -> String {
        self.config.database.clone()
    }

    pub fn open(config: PostgresConfig) -> PostgresResult<Self> {
        with_postgres_span(&config, "db.postgres.connect", "CONNECT", || {
            let mut client = config.connect()?;
            if let Some(timeout) = config.statement_timeout {
                client
                    .client()
                    .batch_execute(&format!("SET statement_timeout = {}", timeout.as_millis()))
                    .map_err(PostgresError::from)?;
            }
            let connection = Self {
                state: Arc::new(Mutex::new(Some(client))),
                config: config.clone(),
            };
            connection.health_check()?;
            Ok(connection)
        })
    }

    pub fn health_check(&self) -> PostgresResult<()> {
        self.execute("SELECT 1", &[]).map(|_| ())
    }

    fn with_cancellable_client<T>(
        &self,
        cancellation: &PostgresOperationCancellation,
        work: impl FnOnce(&mut Client) -> PostgresResult<T>,
    ) -> PostgresResult<T> {
        let mut state = self.lock()?;
        let client_kind = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?;
        if !cancellation.arm(client_kind.cancellation()) {
            return Err(PostgresError::cancelled());
        }
        let result = work(client_kind.client());
        let finish = cancellation.finish();
        match (result, finish) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }

    pub fn prepare(&self, sql: impl Into<String>) -> PostgresResult<PostgresStatement> {
        let sql = sql.into();
        if sql.trim().is_empty() {
            return Err(PostgresError::invalid_argument("SQL cannot be empty"));
        }
        with_postgres_span(&self.config, "db.postgres.prepare", "PREPARE", || {
            let mut state = self.lock()?;
            let prepared = state
                .as_mut()
                .ok_or_else(PostgresError::invalid_handle)?
                .client()
                .prepare(&sql)
                .map_err(PostgresError::from)?;
            Ok(PostgresStatement {
                connection: self.clone(),
                sql,
                prepared,
                params: Vec::new(),
                executed: false,
                rows: Vec::new(),
                columns: Vec::new(),
                cursor: 0,
            })
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

    pub fn execute_query_cancellable(
        &self,
        query: CompiledQuery<PostgresValue>,
        cancellation: &PostgresOperationCancellation,
    ) -> PostgresResult<PostgresExecutionResult> {
        let params = query
            .params
            .iter()
            .map(PostgresValue::as_param)
            .collect::<Vec<_>>();
        let refs = params
            .iter()
            .map(|param| param.as_ref() as &(dyn postgres::types::ToSql + Sync))
            .collect::<Vec<_>>();
        let normalized = query.sql.trim_start().to_ascii_uppercase();
        let returns_rows = normalized.starts_with("SELECT ")
            || normalized.starts_with("SELECT\n")
            || normalized.starts_with("SHOW ")
            || normalized.starts_with("VALUES ")
            || normalized.contains(" RETURNING ");
        with_postgres_span(
            &self.config,
            "db.postgres.query",
            if returns_rows { "QUERY" } else { "EXECUTE" },
            || {
                self.with_cancellable_client(cancellation, |client| {
                    if returns_rows {
                        client
                            .query(&query.sql, &refs)
                            .map(postgres_rows_to_result)
                            .map_err(PostgresError::from)
                    } else {
                        let affected = client
                            .execute(&query.sql, &refs)
                            .map_err(PostgresError::from)?;
                        Ok(PostgresExecutionResult {
                            rows: Vec::new(),
                            affected_rows: affected as usize,
                            columns: Vec::new(),
                        })
                    }
                })
            },
        )
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
        let cancellation = PostgresOperationCancellation::new();
        let operation_cancellation = cancellation.clone();
        let cancel = Arc::new(move || cancellation.request_cancel().map(|_| ()))
            as Arc<dyn Fn() -> PostgresResult<()> + Send + Sync + 'static>;
        PostgresFuture::new_cancellable(
            move || connection.execute_query_cancellable(query, &operation_cancellation),
            Some(cancel),
        )
    }

    pub fn prepare_async(&self, sql: impl Into<String>) -> PostgresPrepareFuture {
        let connection = self.clone();
        let sql = sql.into();
        PostgresFuture::new_cancellable(move || connection.prepare(sql), None)
    }

    pub fn execute_async(&self, query: CompiledQuery<PostgresValue>) -> PostgresExecuteFuture {
        let connection = self.clone();
        let cancellation = PostgresOperationCancellation::new();
        let operation_cancellation = cancellation.clone();
        let cancel = Arc::new(move || cancellation.request_cancel().map(|_| ()))
            as Arc<dyn Fn() -> PostgresResult<()> + Send + Sync + 'static>;
        PostgresFuture::new_cancellable(
            move || connection.execute_query_cancellable(query, &operation_cancellation),
            Some(cancel),
        )
    }

    pub fn cancellation_handle(&self) -> PostgresResult<PostgresCancellation> {
        let mut state = self.lock()?;
        let client = state
            .as_mut()
            .ok_or_else(PostgresError::invalid_handle)?;
        let tls = matches!(client, ClientKind::Tls(_));
        let token = client.client().cancel_token();
        Ok(PostgresCancellation {
            token,
            tls,
        })
    }

    pub fn begin(&self) -> PostgresResult<PostgresTransaction> {
        self.execute("BEGIN", &[])?;
        Ok(PostgresTransaction {
            connection: self.clone(),
            active: true,
        })
    }

    pub fn execute_batch(&self, sql: &str) -> PostgresResult<()> {
        with_postgres_span(&self.config, "db.postgres.query", "BATCH", || {
            let mut state = self.lock()?;
            state
                .as_mut()
                .ok_or_else(PostgresError::invalid_handle)?
                .client()
                .batch_execute(sql)
                .map_err(PostgresError::from)
        })
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
        with_postgres_span(&self.config, "db.postgres.copy", "COPY IN", || {
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
        })
    }

    pub fn copy_in_text(&self, sql: &str, text: &str) -> PostgresResult<u64> {
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY IN requires a COPY statement",
            ));
        }
        with_postgres_span(&self.config, "db.postgres.copy", "COPY IN", || {
            let mut state = self.lock()?;
            let mut writer = state
                .as_mut()
                .ok_or_else(PostgresError::invalid_handle)?
                .client()
                .copy_in(sql)
                .map_err(PostgresError::from)?;
            writer
                .write_all(text.as_bytes())
                .map_err(|error| PostgresError::new("DB2505_COPY_IN", error.to_string()))?;
            writer.finish().map_err(PostgresError::from)?;
            Ok(text.lines().count() as u64)
        })
    }

    pub fn copy_in_text_cancellable(
        &self,
        sql: &str,
        text: &str,
        cancellation: &PostgresOperationCancellation,
    ) -> PostgresResult<u64> {
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY IN requires a COPY statement",
            ));
        }
        with_postgres_span(&self.config, "db.postgres.copy", "COPY IN", || {
            self.with_cancellable_client(cancellation, |client| {
                let mut writer = client.copy_in(sql).map_err(PostgresError::from)?;
                writer
                    .write_all(text.as_bytes())
                    .map_err(|error| PostgresError::new("DB2505_COPY_IN", error.to_string()))?;
                writer.finish().map_err(PostgresError::from)?;
                Ok(text.lines().count() as u64)
            })
        })
    }

    pub fn copy_out_bytes(&self, sql: &str) -> PostgresResult<Vec<u8>> {
        let mut output = Vec::new();
        self.copy_out_to(sql, &mut output)?;
        Ok(output)
    }

    pub fn copy_out_bytes_cancellable(
        &self,
        sql: &str,
        cancellation: &PostgresOperationCancellation,
    ) -> PostgresResult<Vec<u8>> {
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY OUT requires a COPY statement",
            ));
        }
        with_postgres_span(&self.config, "db.postgres.copy", "COPY OUT", || {
            self.with_cancellable_client(cancellation, |client| {
                let mut reader = client.copy_out(sql).map_err(PostgresError::from)?;
                let mut output = Vec::new();
                std::io::copy(&mut reader, &mut output)
                    .map_err(|error| PostgresError::new("DB2505_COPY_OUT", error.to_string()))?;
                Ok(output)
            })
        })
    }

    pub fn copy_out_bytes_cancellable_limited(
        &self,
        sql: &str,
        cancellation: &PostgresOperationCancellation,
        max_bytes: usize,
    ) -> PostgresResult<Vec<u8>> {
        if max_bytes == 0 {
            return Err(PostgresError::invalid_argument(
                "COPY OUT byte limit must be positive",
            ));
        }
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY OUT requires a COPY statement",
            ));
        }
        with_postgres_span(&self.config, "db.postgres.copy", "COPY OUT", || {
            self.with_cancellable_client(cancellation, |client| {
                let mut reader = client.copy_out(sql).map_err(PostgresError::from)?;
                let mut output = Vec::new();
                let mut chunk = [0_u8; 8192];
                loop {
                    let read = reader
                        .read(&mut chunk)
                        .map_err(|error| PostgresError::new("DB2505_COPY_OUT", error.to_string()))?;
                    if read == 0 {
                        break;
                    }
                    if output.len().saturating_add(read) > max_bytes {
                        return Err(PostgresError::new(
                            "DB2505_COPY_LIMIT",
                            format!("COPY OUT exceeds the {max_bytes}-byte text API limit"),
                        ));
                    }
                    output.extend_from_slice(&chunk[..read]);
                }
                Ok(output)
            })
        })
    }

    pub fn copy_out_to<W: Write>(&self, sql: &str, mut output: W) -> PostgresResult<u64> {
        if !sql.trim_start().to_ascii_uppercase().starts_with("COPY ") {
            return Err(PostgresError::invalid_argument(
                "COPY OUT requires a COPY statement",
            ));
        }
        with_postgres_span(&self.config, "db.postgres.copy", "COPY OUT", || {
            let mut state = self.lock()?;
            let mut reader = state
                .as_mut()
                .ok_or_else(PostgresError::invalid_handle)?
                .client()
                .copy_out(sql)
                .map_err(PostgresError::from)?;
            std::io::copy(&mut reader, &mut output)
                .map_err(|error| PostgresError::new("DB2505_COPY_OUT", error.to_string()))
        })
    }

    pub fn listen(&self, channel: &str) -> PostgresResult<NotificationListener> {
        validate_identifier(channel, "channel")?;
        with_postgres_span(&self.config, "db.postgres.listen", "LISTEN", || {
            self.execute(&format!("LISTEN \"{channel}\""), &[])?;
            Ok(NotificationListener {
                connection: self.clone(),
                channel: channel.to_owned(),
            })
        })
    }

    pub fn notify(&self, channel: &str, payload: &str) -> PostgresResult<()> {
        validate_identifier(channel, "channel")?;
        self.query("SELECT pg_notify($1, $2)", &[&channel, &payload])
            .map(|_| ())
    }

    pub fn notify_cancellable(
        &self,
        channel: &str,
        payload: &str,
        cancellation: &PostgresOperationCancellation,
    ) -> PostgresResult<()> {
        validate_identifier(channel, "channel")?;
        with_postgres_span(&self.config, "db.postgres.query", "NOTIFY", || {
            self.with_cancellable_client(cancellation, |client| {
                client
                    .query("SELECT pg_notify($1, $2)", &[&channel, &payload])
                    .map(|_| ())
                    .map_err(PostgresError::from)
            })
        })
    }

    pub fn savepoint(&self, name: &str) -> PostgresResult<()> {
        validate_identifier(name, "savepoint")?;
        self.execute_batch(&format!("SAVEPOINT \"{name}\""))
    }

    pub fn rollback_to(&self, name: &str) -> PostgresResult<()> {
        validate_identifier(name, "savepoint")?;
        self.execute_batch(&format!("ROLLBACK TO SAVEPOINT \"{name}\""))
    }

    pub fn release_savepoint(&self, name: &str) -> PostgresResult<()> {
        validate_identifier(name, "savepoint")?;
        self.execute_batch(&format!("RELEASE SAVEPOINT \"{name}\""))
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
        with_postgres_span(&self.config, "db.postgres.query", "EXECUTE", || {
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
        })
    }

    pub(crate) fn query(
        &self,
        sql: &str,
        params: &[&(dyn postgres::types::ToSql + Sync)],
    ) -> PostgresResult<PostgresExecutionResult> {
        with_postgres_span(&self.config, "db.postgres.query", "QUERY", || {
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
    prepared: postgres::Statement,
    params: Vec<PostgresValue>,
    executed: bool,
    rows: Vec<Vec<PostgresValue>>,
    columns: Vec<PostgresColumn>,
    cursor: usize,
}

impl PostgresStatement {
    pub fn cancellation_handle(&self) -> PostgresResult<PostgresCancellation> {
        self.connection.cancellation_handle()
    }

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
        self.execute_internal(None)
    }

    fn execute_internal(
        &mut self,
        cancellation: Option<&PostgresOperationCancellation>,
    ) -> PostgresResult<PostgresExecutionResult> {
        self.executed = true;
        self.cursor = 0;
        let params = self
            .params
            .iter()
            .map(PostgresValue::as_param)
            .collect::<Vec<_>>();
        let refs = params
            .iter()
            .map(|param| param.as_ref() as &(dyn postgres::types::ToSql + Sync))
            .collect::<Vec<_>>();
        let normalized = self.sql.trim_start().to_ascii_uppercase();
        let returns_rows = normalized.starts_with("SELECT ")
            || normalized.starts_with("SELECT\n")
            || normalized.starts_with("SHOW ")
            || normalized.starts_with("VALUES ")
            || normalized.contains(" RETURNING ");
        let result = with_postgres_span(
            &self.connection.config,
            "db.postgres.query",
            if returns_rows { "QUERY" } else { "EXECUTE" },
            || {
                if let Some(cancellation) = cancellation {
                    self.connection
                        .with_cancellable_client(cancellation, |client| {
                            execute_prepared(client, &self.prepared, &refs, returns_rows)
                        })
                } else {
                    let mut state = self.connection.lock()?;
                    let client = state
                        .as_mut()
                        .ok_or_else(PostgresError::invalid_handle)?
                        .client();
                    execute_prepared(client, &self.prepared, &refs, returns_rows)
                }
            },
        )?;
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

    pub fn step_cancellable(
        &mut self,
        cancellation: &PostgresOperationCancellation,
    ) -> PostgresResult<i32> {
        if !self.executed {
            self.execute_internal(Some(cancellation))?;
        } else if cancellation.was_cancelled_before_start() {
            return Err(PostgresError::cancelled());
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

fn postgres_rows_to_result(rows: Vec<postgres::Row>) -> PostgresExecutionResult {
    let columns = rows
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .map(|column| PostgresColumn {
                    name: column.name().to_owned(),
                    ty: column.type_().name().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    let values = rows
        .iter()
        .map(|row| {
            (0..row.len())
                .map(|index| PostgresValue::from_cell(row, index))
                .collect()
        })
        .collect();
    PostgresExecutionResult {
        rows: values,
        affected_rows: rows.len(),
        columns,
    }
}

fn execute_prepared(
    client: &mut Client,
    prepared: &postgres::Statement,
    params: &[&(dyn postgres::types::ToSql + Sync)],
    returns_rows: bool,
) -> PostgresResult<PostgresExecutionResult> {
    if returns_rows {
        client
            .query(prepared, params)
            .map(postgres_rows_to_result)
            .map_err(PostgresError::from)
    } else {
        let affected = client
            .execute(prepared, params)
            .map_err(PostgresError::from)?;
        Ok(PostgresExecutionResult {
            rows: Vec::new(),
            affected_rows: affected as usize,
            columns: Vec::new(),
        })
    }
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

    pub fn next_timeout_cancellable(
        &self,
        timeout: Duration,
        cancellation: &PostgresOperationCancellation,
    ) -> PostgresResult<Option<Notification>> {
        let started = std::time::Instant::now();
        loop {
            if cancellation.was_cancelled_before_start() {
                return Err(PostgresError::cancelled());
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Ok(None);
            }
            let slice = remaining.min(Duration::from_millis(50));
            let mut state = self.connection.lock()?;
            let client = state
                .as_mut()
                .ok_or_else(PostgresError::invalid_handle)?
                .client();
            let mut notifications = client.notifications();
            let mut iterator = notifications.timeout_iter(slice);
            use fallible_iterator::FallibleIterator;
            let item = iterator.next().map_err(PostgresError::from)?.map(|notification| {
                Notification {
                    channel: notification.channel().to_owned(),
                    payload: notification.payload().to_owned(),
                    process_id: notification.process_id(),
                }
            });
            drop(iterator);
            drop(notifications);
            drop(state);
            if item.is_some() {
                return Ok(item);
            }
        }
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
        validate_identifier(name, "savepoint")?;
        self.execute(&format!("SAVEPOINT \"{name}\"")).map(|_| ())
    }
    pub fn rollback_to(&self, name: &str) -> PostgresResult<()> {
        validate_identifier(name, "savepoint")?;
        self.execute(&format!("ROLLBACK TO SAVEPOINT \"{name}\""))
            .map(|_| ())
    }
    pub fn release_savepoint(&self, name: &str) -> PostgresResult<()> {
        validate_identifier(name, "savepoint")?;
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

fn validate_identifier(name: &str, kind: &str) -> PostgresResult<()> {
    if name.is_empty()
        || name.len() > 63
        || name
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    {
        return Err(PostgresError::invalid_argument(format!(
            "invalid PostgreSQL {kind} name"
        )));
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

fn with_postgres_span<T>(
    config: &PostgresConfig,
    name: &str,
    operation: &str,
    work: impl FnOnce() -> PostgresResult<T>,
) -> PostgresResult<T> {
    let span = tracing::begin_external_span(SpanKind::Client, name).ok();
    if let Some(id) = span {
        let _ = tracing::span_set_attribute(id, "db.system", "postgresql");
        let _ = tracing::span_set_attribute(id, "db.operation", operation);
        let _ = tracing::span_set_attribute(id, "server.address", &config.host);
        let _ = tracing::span_set_attribute_int(id, "server.port", config.port as i64);
        let _ = tracing::span_set_attribute(id, "db.namespace", &config.database);
    }
    let result = work();
    if let Some(id) = span {
        let _ = tracing::span_set_attribute_bool(id, "db.error", result.is_err());
        let _ = tracing::span_set_status(
            id,
            if result.is_ok() {
                SpanStatus::Ok
            } else {
                SpanStatus::Error
            },
        );
        let _ = tracing::span_end(id);
    }
    result
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
