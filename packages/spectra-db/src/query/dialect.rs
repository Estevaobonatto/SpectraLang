use super::error::QueryError;

pub trait Dialect {
    fn quote_identifier(&self, identifier: &str) -> Result<String, QueryError>;
    fn placeholder(&self, index: usize) -> String;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SqliteDialect;

#[derive(Debug, Clone, Copy, Default)]
pub struct PostgresDialect;

impl Dialect for PostgresDialect {
    fn quote_identifier(&self, identifier: &str) -> Result<String, QueryError> {
        if identifier.is_empty()
            || identifier.len() > 128
            || identifier.chars().any(|ch| ch == '\0' || ch.is_control())
        {
            return Err(QueryError::InvalidIdentifier(identifier.to_owned()));
        }
        let escaped = identifier.replace('"', "\"\"");
        Ok(format!("\"{escaped}\""))
    }

    fn placeholder(&self, index: usize) -> String {
        format!("${index}")
    }
}

impl Dialect for SqliteDialect {
    fn quote_identifier(&self, identifier: &str) -> Result<String, QueryError> {
        if identifier.is_empty()
            || identifier.len() > 128
            || identifier.chars().any(|ch| ch == '\0' || ch.is_control())
        {
            return Err(QueryError::InvalidIdentifier(identifier.to_owned()));
        }
        let escaped = identifier.replace('"', "\"\"");
        Ok(format!("\"{escaped}\""))
    }

    fn placeholder(&self, index: usize) -> String {
        format!("?{index}")
    }
}
