use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresError {
    pub code: &'static str,
    pub message: String,
}

pub type PostgresResult<T> = Result<T, PostgresError>;

impl PostgresError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new("DB2505_INVALID_ARGUMENT", message)
    }

    pub fn invalid_handle() -> Self {
        Self::new("DB2505_INVALID_HANDLE", "invalid PostgreSQL handle")
    }

    pub fn cancelled() -> Self {
        Self::new(
            "DB2505_CANCELLED_OR_TIMEOUT",
            "PostgreSQL operation was cancelled",
        )
    }
}

impl fmt::Display for PostgresError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for PostgresError {}

impl From<postgres::Error> for PostgresError {
    fn from(error: postgres::Error) -> Self {
        if let Some(db) = error.as_db_error() {
            let code = match db.code().code() {
                "23505" => "DB2505_CONSTRAINT_UNIQUE",
                "23503" => "DB2505_CONSTRAINT_FOREIGN_KEY",
                "23514" => "DB2505_CONSTRAINT_CHECK",
                "57014" => "DB2505_CANCELLED_OR_TIMEOUT",
                "40001" => "DB2505_SERIALIZATION_FAILURE",
                "40P01" => "DB2505_DEADLOCK",
                _ => "DB2505_SERVER",
            };
            return Self::new(code, db.message().to_owned());
        }
        Self::new("DB2505_CONNECTION", error.to_string())
    }
}
