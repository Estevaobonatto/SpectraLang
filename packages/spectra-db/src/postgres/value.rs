use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresType {
    Null,
    Bool,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Text,
    Bytes,
    Uuid,
    Timestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PostgresValue {
    Null,
    Bool(bool),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    Text(String),
    Bytes(Vec<u8>),
    Uuid(Uuid),
    Timestamp(DateTime<Utc>),
}

impl PostgresValue {
    pub fn ty(&self) -> PostgresType {
        match self {
            Self::Null => PostgresType::Null,
            Self::Bool(_) => PostgresType::Bool,
            Self::Int16(_) => PostgresType::Int16,
            Self::Int32(_) => PostgresType::Int32,
            Self::Int64(_) => PostgresType::Int64,
            Self::Float32(_) => PostgresType::Float32,
            Self::Float64(_) => PostgresType::Float64,
            Self::Text(_) => PostgresType::Text,
            Self::Bytes(_) => PostgresType::Bytes,
            Self::Uuid(_) => PostgresType::Uuid,
            Self::Timestamp(_) => PostgresType::Timestamp,
        }
    }

    pub(crate) fn as_param(&self) -> Box<dyn postgres::types::ToSql + Sync> {
        match self {
            Self::Null => Box::new(None::<String>),
            Self::Bool(value) => Box::new(*value),
            Self::Int16(value) => Box::new(*value),
            Self::Int32(value) => Box::new(*value),
            Self::Int64(value) => Box::new(*value),
            Self::Float32(value) => Box::new(*value),
            Self::Float64(value) => Box::new(*value),
            Self::Text(value) => Box::new(value.clone()),
            Self::Bytes(value) => Box::new(value.clone()),
            Self::Uuid(value) => Box::new(*value),
            Self::Timestamp(value) => Box::new(*value),
        }
    }

    pub(crate) fn from_cell(row: &postgres::Row, index: usize) -> Self {
        let ty = row.columns()[index].type_();
        if ty == &postgres::types::Type::BOOL {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Bool);
        }
        if ty == &postgres::types::Type::INT2 {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Int16);
        }
        if ty == &postgres::types::Type::INT4 {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Int32);
        }
        if ty == &postgres::types::Type::INT8 {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Int64);
        }
        if ty == &postgres::types::Type::FLOAT4 {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Float32);
        }
        if ty == &postgres::types::Type::FLOAT8 {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Float64);
        }
        if ty == &postgres::types::Type::BYTEA {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Bytes);
        }
        if ty == &postgres::types::Type::UUID {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Uuid);
        }
        if ty == &postgres::types::Type::TIMESTAMPTZ {
            return row
                .try_get(index)
                .ok()
                .flatten()
                .map_or(Self::Null, Self::Timestamp);
        }
        row.try_get::<usize, Option<String>>(index)
            .ok()
            .flatten()
            .map_or(Self::Null, Self::Text)
    }
}
