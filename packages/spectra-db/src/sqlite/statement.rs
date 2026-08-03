use super::connection::SqliteConnection;
use super::error::{SqliteError, SqliteResult};
use rusqlite::types::{Value, ValueRef};
use rusqlite::ToSql;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum SqliteValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqliteValue {
    fn into_rusqlite(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Integer(v) => Value::Integer(v),
            Self::Real(v) => Value::Real(v),
            Self::Text(v) => Value::Text(v),
            Self::Blob(v) => Value::Blob(v),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    Row = 1,
    Done = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Null = 0,
    Integer = 1,
    Real = 2,
    Text = 3,
    Blob = 4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqliteExecutionResult {
    pub rows: Vec<Vec<SqliteValue>>,
    pub affected_rows: usize,
}

pub struct SqliteStatement {
    connection: SqliteConnection,
    sql: String,
    bindings: BTreeMap<usize, SqliteValue>,
    rows: Vec<Vec<SqliteValue>>,
    current_row: Option<usize>,
    affected_rows: usize,
    started: bool,
    finalized: bool,
}

impl SqliteStatement {
    pub fn prepare(connection: SqliteConnection, sql: impl Into<String>) -> SqliteResult<Self> {
        let sql = sql.into();
        if sql.trim().is_empty() {
            return Err(SqliteError::new(
                "DB2504_INVALID_SQL",
                "SQL statement is empty",
            ));
        }
        {
            let state = connection.lock()?;
            state
                .connection
                .prepare_cached(&sql)
                .map_err(SqliteError::from)?;
        }
        Ok(Self {
            connection,
            sql,
            bindings: BTreeMap::new(),
            rows: Vec::new(),
            current_row: None,
            affected_rows: 0,
            started: false,
            finalized: false,
        })
    }

    pub fn bind(&mut self, index: usize, value: SqliteValue) -> SqliteResult<()> {
        self.ensure_open()?;
        if self.started {
            return Err(SqliteError::invalid_state(
                "reset is required before binding after step",
            ));
        }
        self.bindings.insert(index, value);
        Ok(())
    }

    pub fn step(&mut self) -> SqliteResult<StepResult> {
        self.ensure_open()?;
        if !self.started {
            let state = self.connection.lock()?;
            let values: Vec<Value> = self
                .bindings
                .values()
                .cloned()
                .map(SqliteValue::into_rusqlite)
                .collect();
            let params: Vec<&dyn ToSql> = values.iter().map(|value| value as &dyn ToSql).collect();
            let mut statement = state
                .connection
                .prepare_cached(&self.sql)
                .map_err(SqliteError::from)?;
            if statement.column_count() == 0 {
                self.affected_rows = statement
                    .execute(rusqlite::params_from_iter(params))
                    .map_err(SqliteError::from)?;
            } else {
                let column_count = statement.column_count();
                let mut query = statement
                    .query(rusqlite::params_from_iter(params))
                    .map_err(SqliteError::from)?;
                while let Some(row) = query.next().map_err(SqliteError::from)? {
                    let mut values = Vec::with_capacity(column_count);
                    for index in 0..column_count {
                        values.push(value_from_ref(
                            row.get_ref(index).map_err(SqliteError::from)?,
                        ));
                    }
                    self.rows.push(values);
                }
            }
            self.started = true;
        }
        let next = self.current_row.map(|index| index + 1).unwrap_or(0);
        if next < self.rows.len() {
            self.current_row = Some(next);
            Ok(StepResult::Row)
        } else {
            Ok(StepResult::Done)
        }
    }

    pub fn reset(&mut self) -> SqliteResult<()> {
        self.ensure_open()?;
        self.rows.clear();
        self.current_row = None;
        self.affected_rows = 0;
        self.started = false;
        Ok(())
    }
    pub fn finalize(&mut self) -> SqliteResult<()> {
        self.finalized = true;
        self.rows.clear();
        self.bindings.clear();
        Ok(())
    }
    pub fn affected_rows(&self) -> SqliteResult<usize> {
        self.ensure_open()?;
        Ok(self.affected_rows)
    }
    pub fn column_count(&self) -> SqliteResult<usize> {
        self.ensure_open()?;
        Ok(self.rows.first().map_or(0, Vec::len))
    }
    pub fn column_type(&self, index: usize) -> SqliteResult<ColumnType> {
        self.value(index).map(|value| match value {
            SqliteValue::Null => ColumnType::Null,
            SqliteValue::Integer(_) => ColumnType::Integer,
            SqliteValue::Real(_) => ColumnType::Real,
            SqliteValue::Text(_) => ColumnType::Text,
            SqliteValue::Blob(_) => ColumnType::Blob,
        })
    }
    pub fn column_value(&self, index: usize) -> SqliteResult<SqliteValue> {
        self.value(index).cloned()
    }
    fn value(&self, index: usize) -> SqliteResult<&SqliteValue> {
        self.ensure_open()?;
        let row = self.current_row.ok_or_else(|| {
            SqliteError::invalid_state("step must return row before reading columns")
        })?;
        self.rows
            .get(row)
            .and_then(|values| values.get(index))
            .ok_or_else(|| SqliteError::new("DB2504_COLUMN", "column index out of range"))
    }
    fn ensure_open(&self) -> SqliteResult<()> {
        if self.finalized {
            Err(SqliteError::invalid_state("statement has been finalized"))
        } else {
            Ok(())
        }
    }
}

fn value_from_ref(value: ValueRef<'_>) -> SqliteValue {
    match value {
        ValueRef::Null => SqliteValue::Null,
        ValueRef::Integer(v) => SqliteValue::Integer(v),
        ValueRef::Real(v) => SqliteValue::Real(v),
        ValueRef::Text(v) => SqliteValue::Text(String::from_utf8_lossy(v).into_owned()),
        ValueRef::Blob(v) => SqliteValue::Blob(v.to_vec()),
    }
}
