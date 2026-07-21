use std::fmt;
use std::time::Duration;

pub type PoolResult<T> = Result<T, PoolError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    InvalidConfig(&'static str),
    Closed,
    AcquireTimeout(Duration),
    Factory(String),
    ConnectionInvalid,
    Released,
    ShutdownTimeout(Duration),
}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid pool configuration: {message}"),
            Self::Closed => f.write_str("connection pool is closed"),
            Self::AcquireTimeout(duration) => {
                write!(f, "connection acquisition timed out after {duration:?}")
            }
            Self::Factory(message) => write!(f, "connection factory failed: {message}"),
            Self::ConnectionInvalid => f.write_str("connection is invalid"),
            Self::Released => f.write_str("pooled connection has already been released"),
            Self::ShutdownTimeout(duration) => {
                write!(f, "pool shutdown timed out after {duration:?}")
            }
        }
    }
}

impl std::error::Error for PoolError {}
