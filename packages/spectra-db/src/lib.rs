//! Shared database infrastructure for Spectra drivers.
//!
//! This crate intentionally contains no database protocol implementation. Drivers
//! such as SQLite and PostgreSQL provide a `ConnectionFactory` and consume the
//! pool without exposing a fake database surface.

mod error;
mod metrics;
mod pool;
pub mod sqlite;

pub use error::{PoolError, PoolResult};
pub use metrics::PoolMetrics;
pub use pool::{ConnectionFactory, ConnectionPool, PoolConfig, PooledConnection};
