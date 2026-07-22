use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteError {
    pub code: &'static str,
    pub message: String,
}

pub type SqliteResult<T> = Result<T, SqliteError>;

impl SqliteError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_handle() -> Self {
        Self::new("DB2504_INVALID_HANDLE", "invalid SQLite handle")
    }
    pub fn closed() -> Self {
        Self::new("DB2504_CLOSED", "SQLite connection is closed")
    }
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::new("DB2504_INVALID_STATE", message)
    }
    pub fn invalid_binding(message: impl Into<String>) -> Self {
        Self::new("DB2504_INVALID_BINDING", message)
    }

    pub fn cancelled() -> Self {
        Self::new("DB2504_CANCELLED", "SQLite operation was cancelled")
    }
}

impl fmt::Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SqliteError {}

impl From<rusqlite::Error> for SqliteError {
    fn from(error: rusqlite::Error) -> Self {
        let code = match error {
            rusqlite::Error::QueryReturnedNoRows => "DB2504_NO_ROWS",
            rusqlite::Error::InvalidParameterName(_)
            | rusqlite::Error::InvalidParameterCount(_, _) => "DB2504_INVALID_BINDING",
            rusqlite::Error::SqliteFailure(ref failure, _) => match failure.extended_code {
                rusqlite::ffi::SQLITE_BUSY | rusqlite::ffi::SQLITE_LOCKED => "DB2504_BUSY",
                rusqlite::ffi::SQLITE_CONSTRAINT | rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                    "DB2504_CONSTRAINT"
                }
                _ => "DB2504_SQL_ERROR",
            },
            _ => "DB2504_SQL_ERROR",
        };
        Self::new(code, error.to_string())
    }
}
