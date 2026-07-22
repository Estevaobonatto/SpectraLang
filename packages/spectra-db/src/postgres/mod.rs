//! Real PostgreSQL driver backed by the upstream `postgres` protocol client.
//!
//! The public async methods run protocol work on dedicated worker threads. They
//! never block the caller's executor and reuse the generic pool contract.

mod async_ops;
mod connection;
mod error;
mod value;

pub use async_ops::{PostgresExecuteFuture, PostgresPrepareFuture, PostgresQueryFuture};
pub use connection::{
    open_pool, Notification, PostgresColumn, PostgresConfig, PostgresConnection, PostgresFactory,
    PostgresPool, PostgresStatement, PostgresTransaction, SecretString, SslMode,
};
pub use error::{PostgresError, PostgresResult};
pub use value::{PostgresType, PostgresValue};

pub type PostgresExecutionResult = connection::PostgresExecutionResult;

#[cfg(test)]
mod tests {
    use super::PostgresConfig;

    #[test]
    fn config_debug_does_not_expose_password() {
        let mut config = PostgresConfig::default();
        config.password = "secret".into();
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret"));
    }
}
