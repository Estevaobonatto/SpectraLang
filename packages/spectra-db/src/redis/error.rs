use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedisError {
    pub code: &'static str,
    pub message: String,
}

pub type RedisResult<T> = Result<T, RedisError>;

impl RedisError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new("DB2507_INVALID_ARGUMENT", message)
    }
    pub fn invalid_handle() -> Self {
        Self::new("DB2507_INVALID_HANDLE", "invalid Redis handle")
    }
    pub fn closed() -> Self {
        Self::new("DB2507_CLOSED", "Redis connection is closed")
    }
}

impl fmt::Display for RedisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for RedisError {}

impl From<redis::RedisError> for RedisError {
    fn from(error: redis::RedisError) -> Self {
        let message = error.to_string();
        let code = if message.contains("timeout") || message.contains("timed out") {
            "DB2507_TIMEOUT"
        } else if message.contains("WRONGTYPE") {
            "DB2507_TYPE"
        } else if message.contains("NOAUTH") || message.contains("AUTH") {
            "DB2507_AUTH"
        } else if message.contains("connection") || message.contains("Connection") {
            "DB2507_CONNECTION"
        } else {
            "DB2507_SERVER"
        };
        Self::new(code, message)
    }
}
