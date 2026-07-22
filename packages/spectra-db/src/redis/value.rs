use super::error::{RedisError, RedisResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedisValue {
    Null,
    Bytes(Vec<u8>),
    Text(String),
    Integer(i64),
    Boolean(bool),
}

impl RedisValue {
    pub fn into_bytes(self) -> RedisResult<Vec<u8>> {
        match self {
            Self::Null => Err(RedisError::invalid_argument(
                "Redis cannot store a null value",
            )),
            Self::Bytes(value) => Ok(value),
            Self::Text(value) => Ok(value.into_bytes()),
            Self::Integer(value) => Ok(value.to_string().into_bytes()),
            Self::Boolean(value) => Ok(if value { b"1".to_vec() } else { b"0".to_vec() }),
        }
    }
    pub fn from_bytes(value: Vec<u8>) -> Self {
        match String::from_utf8(value.clone()) {
            Ok(text) => Self::Text(text),
            Err(_) => Self::Bytes(value),
        }
    }
}
