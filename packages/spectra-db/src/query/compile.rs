use super::ast::{Delete, Insert, Query, Select, Update};
use super::dialect::Dialect;
use super::error::QueryError;
use crate::sqlite::SqliteValue;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledQuery<V = SqliteValue> {
    pub sql: String,
    pub params: Vec<V>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryOutput {
    pub rows: Vec<Vec<SqliteValue>>,
    pub affected_rows: usize,
}

impl Query for Select {
    type Output = Vec<Vec<SqliteValue>>;
    fn compile<D: Dialect>(&self, dialect: &D) -> Result<CompiledQuery, QueryError> {
        self.compile_select(dialect)
    }
}

impl Query for Insert {
    type Output = usize;
    fn compile<D: Dialect>(&self, dialect: &D) -> Result<CompiledQuery, QueryError> {
        self.compile_insert(dialect)
    }
}

impl Query for Update {
    type Output = usize;
    fn compile<D: Dialect>(&self, dialect: &D) -> Result<CompiledQuery, QueryError> {
        self.compile_update(dialect)
    }
}

impl Query for Delete {
    type Output = usize;
    fn compile<D: Dialect>(&self, dialect: &D) -> Result<CompiledQuery, QueryError> {
        self.compile_delete(dialect)
    }
}
