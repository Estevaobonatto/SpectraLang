use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationError {
    pub code: &'static str,
    pub message: String,
}

pub type MigrationResult<T> = Result<T, MigrationError>;

impl MigrationError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for MigrationError {}

impl From<crate::sqlite::SqliteError> for MigrationError {
    fn from(error: crate::sqlite::SqliteError) -> Self {
        let code = match error.code {
            "DB2504_BUSY" => "DB2503_LOCKED",
            _ => "DB2503_SQLITE",
        };
        Self::new(code, error.to_string())
    }
}
