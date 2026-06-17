use crate::{read_args, read_spectra_string, write_result};
use serde_json::{Map, Number, Value};
use spectra_runtime::ffi::{
    SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INVALID_ARGUMENT,
};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

pub const JSON_KIND_INVALID: SpectraHostValue = 0;
pub const JSON_KIND_NULL: SpectraHostValue = 1;
pub const JSON_KIND_BOOL: SpectraHostValue = 2;
pub const JSON_KIND_NUMBER: SpectraHostValue = 3;
pub const JSON_KIND_STRING: SpectraHostValue = 4;
pub const JSON_KIND_ARRAY: SpectraHostValue = 5;
pub const JSON_KIND_OBJECT: SpectraHostValue = 6;

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    pub fn kind(&self) -> SpectraHostValue {
        match self {
            JsonValue::Null => JSON_KIND_NULL,
            JsonValue::Bool(_) => JSON_KIND_BOOL,
            JsonValue::Number(_) => JSON_KIND_NUMBER,
            JsonValue::String(_) => JSON_KIND_STRING,
            JsonValue::Array(_) => JSON_KIND_ARRAY,
            JsonValue::Object(_) => JSON_KIND_OBJECT,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JsonNumber {
    repr: String,
}

impl JsonNumber {
    pub fn from_i64(value: i64) -> Self {
        Self {
            repr: value.to_string(),
        }
    }

    pub fn from_u64(value: u64) -> Self {
        Self {
            repr: value.to_string(),
        }
    }

    pub fn from_f64(value: f64) -> Result<Self, JsonEncodeError> {
        let number = Number::from_f64(value).ok_or_else(|| {
            JsonEncodeError::new(
                JsonEncodeErrorKind::NonFiniteNumber,
                "JSON numbers cannot encode NaN or infinity",
            )
        })?;
        Ok(Self {
            repr: number.to_string(),
        })
    }

    pub fn parse(text: impl Into<String>) -> Result<Self, JsonEncodeError> {
        let repr = text.into();
        Number::from_str(&repr).map_err(|error| {
            JsonEncodeError::with_cause(
                JsonEncodeErrorKind::InvalidNumber,
                "invalid JSON number representation",
                error.to_string(),
            )
        })?;
        Ok(Self { repr })
    }

    pub fn as_str(&self) -> &str {
        &self.repr
    }

    pub fn as_i64(&self) -> Option<i64> {
        Number::from_str(&self.repr).ok()?.as_i64()
    }

    pub fn as_u64(&self) -> Option<u64> {
        Number::from_str(&self.repr).ok()?.as_u64()
    }

    pub fn as_f64(&self) -> Option<f64> {
        Number::from_str(&self.repr).ok()?.as_f64()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonParseErrorKind {
    InvalidSyntax,
    UnexpectedEof,
    InvalidData,
    Io,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonParseError {
    pub kind: JsonParseErrorKind,
    pub offset: usize,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl JsonParseError {
    fn from_serde(text: &str, error: serde_json::Error) -> Self {
        let line = error.line().max(1);
        let column = error.column().max(1);
        let offset = byte_offset_for_line_column(text, line, column);
        let kind = match error.classify() {
            serde_json::error::Category::Io => JsonParseErrorKind::Io,
            serde_json::error::Category::Syntax => JsonParseErrorKind::InvalidSyntax,
            serde_json::error::Category::Data => JsonParseErrorKind::InvalidData,
            serde_json::error::Category::Eof => JsonParseErrorKind::UnexpectedEof,
        };
        Self {
            kind,
            offset,
            line,
            column,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte {} (line {}, column {})",
            self.message, self.offset, self.line, self.column
        )
    }
}

impl std::error::Error for JsonParseError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonEncodeErrorKind {
    InvalidNumber,
    NonFiniteNumber,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonEncodeError {
    pub kind: JsonEncodeErrorKind,
    pub message: String,
    pub cause: Option<String>,
}

impl JsonEncodeError {
    fn new(kind: JsonEncodeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            cause: None,
        }
    }

    fn with_cause(
        kind: JsonEncodeErrorKind,
        message: impl Into<String>,
        cause: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            cause: Some(cause.into()),
        }
    }
}

impl fmt::Display for JsonEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(cause) => write!(f, "{}: {}", self.message, cause),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for JsonEncodeError {}

pub fn parse_json(text: &str) -> Result<JsonValue, JsonParseError> {
    let value = serde_json::from_str::<Value>(text)
        .map_err(|error| JsonParseError::from_serde(text, error))?;
    Ok(from_serde_value(value))
}

pub fn encode_json(value: &JsonValue) -> Result<String, JsonEncodeError> {
    serde_json::to_string(&to_serde_value(value)?).map_err(|error| {
        JsonEncodeError::with_cause(
            JsonEncodeErrorKind::InvalidNumber,
            "failed to encode JSON value",
            error.to_string(),
        )
    })
}

pub fn encode_json_pretty(value: &JsonValue) -> Result<String, JsonEncodeError> {
    serde_json::to_string_pretty(&to_serde_value(value)?).map_err(|error| {
        JsonEncodeError::with_cause(
            JsonEncodeErrorKind::InvalidNumber,
            "failed to encode pretty JSON value",
            error.to_string(),
        )
    })
}

pub fn json_kind_of(text: &str) -> SpectraHostValue {
    parse_json(text)
        .map(|value| value.kind())
        .unwrap_or(JSON_KIND_INVALID)
}

fn from_serde_value(value: Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(value) => JsonValue::Bool(value),
        Value::Number(value) => JsonValue::Number(JsonNumber {
            repr: value.to_string(),
        }),
        Value::String(value) => JsonValue::String(value),
        Value::Array(values) => {
            JsonValue::Array(values.into_iter().map(from_serde_value).collect())
        }
        Value::Object(values) => JsonValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, from_serde_value(value)))
                .collect(),
        ),
    }
}

fn to_serde_value(value: &JsonValue) -> Result<Value, JsonEncodeError> {
    match value {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::Number(value) => {
            Number::from_str(value.as_str())
                .map(Value::Number)
                .map_err(|error| {
                    JsonEncodeError::with_cause(
                        JsonEncodeErrorKind::InvalidNumber,
                        "invalid JSON number representation",
                        error.to_string(),
                    )
                })
        }
        JsonValue::String(value) => Ok(Value::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(to_serde_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        JsonValue::Object(values) => {
            let mut out = Map::new();
            for (key, value) in values {
                out.insert(key.clone(), to_serde_value(value)?);
            }
            Ok(Value::Object(out))
        }
    }
}

fn byte_offset_for_line_column(text: &str, line: usize, column: usize) -> usize {
    let target_line = line.saturating_sub(1);
    let target_column = column.saturating_sub(1);
    let mut current_line = 0usize;
    let mut current_column = 0usize;
    for (offset, ch) in text.char_indices() {
        if current_line == target_line && current_column == target_column {
            return offset;
        }
        if ch == '\n' {
            current_line += 1;
            current_column = 0;
        } else {
            current_column += 1;
        }
    }
    text.len()
}

pub extern "C" fn json_validate(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let valid = read_spectra_string(args[0])
        .map(|text| parse_json(&text).is_ok())
        .unwrap_or(false);
    write_result(ctx, valid as SpectraHostValue)
}

pub extern "C" fn json_kind(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = read_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let kind = read_spectra_string(args[0])
        .map(|text| json_kind_of(&text))
        .unwrap_or(JSON_KIND_INVALID);
    write_result(ctx, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    #[test]
    fn round_trip_primitives_arrays_maps_nested_and_null() {
        let value = object(vec![
            ("null", JsonValue::Null),
            ("bool", JsonValue::Bool(true)),
            ("int", JsonValue::Number(JsonNumber::from_i64(-42))),
            (
                "float",
                JsonValue::Number(JsonNumber::from_f64(12.5).expect("finite number")),
            ),
            (
                "string",
                JsonValue::String("quote: \" slash: \\ newline:\n tab:\t".to_string()),
            ),
            (
                "array",
                JsonValue::Array(vec![
                    JsonValue::Null,
                    JsonValue::Bool(false),
                    JsonValue::String("nested".to_string()),
                ]),
            ),
            (
                "map",
                object(vec![(
                    "child",
                    JsonValue::Array(vec![JsonValue::Number(JsonNumber::from_u64(7))]),
                )]),
            ),
        ]);

        let encoded = encode_json(&value).expect("encode JSON");
        let decoded = parse_json(&encoded).expect("decode JSON");
        assert_eq!(decoded, value);
        assert_eq!(json_kind_of(&encoded), JSON_KIND_OBJECT);
    }

    #[test]
    fn parser_handles_common_escape_sequences_and_unicode() {
        let decoded = parse_json(r#"{"text":"line\nquote\"slash\\tab\tunicode \u263A"}"#)
            .expect("escaped JSON parses");
        let JsonValue::Object(map) = decoded else {
            panic!("expected object");
        };
        assert_eq!(
            map.get("text"),
            Some(&JsonValue::String(
                "line\nquote\"slash\\tab\tunicode ☺".to_string()
            ))
        );
    }

    #[test]
    fn invalid_json_reports_typed_error_with_byte_offset() {
        let err = parse_json("{\n  \"ok\": true,\n  bad\n}")
            .expect_err("invalid JSON must fail with typed offset");
        assert_eq!(err.kind, JsonParseErrorKind::InvalidSyntax);
        assert_eq!(err.line, 3);
        assert!(err.offset >= "{\n  \"ok\": true,\n  ".len());
        assert!(err.message.contains("key") || err.message.contains("expected"));
    }

    #[test]
    fn encoder_rejects_invalid_numbers_and_non_finite_float_values() {
        let nan = JsonNumber::from_f64(f64::NAN).expect_err("NaN is not JSON");
        assert_eq!(nan.kind, JsonEncodeErrorKind::NonFiniteNumber);

        let invalid = JsonValue::Number(JsonNumber {
            repr: "01".to_string(),
        });
        let err = encode_json(&invalid).expect_err("invalid number repr");
        assert_eq!(err.kind, JsonEncodeErrorKind::InvalidNumber);
    }

    #[test]
    fn encoder_output_is_rfc8259_json_for_supported_values() {
        let value = JsonValue::Array(vec![
            JsonValue::String("\u{0008}\u{000c}\r\n".to_string()),
            object(vec![(
                "x",
                JsonValue::Number(JsonNumber::parse("1e-9").unwrap()),
            )]),
        ]);
        let encoded = encode_json(&value).expect("encode supported JSON");
        serde_json::from_str::<Value>(&encoded).expect("serde accepts encoded RFC 8259 JSON");
        let reparsed = parse_json(&encoded).expect("self parser accepts encoded JSON");
        assert_eq!(reparsed, value);
    }

    #[test]
    fn host_kind_uses_full_parser_not_balanced_braces() {
        assert_eq!(json_kind_of(r#"{"unterminated":["#), JSON_KIND_INVALID);
        assert_eq!(json_kind_of(r#"[{"ok":true}, null]"#), JSON_KIND_ARRAY);
        assert_eq!(json_kind_of(r#""hello""#), JSON_KIND_STRING);
        assert_eq!(json_kind_of("-3.5e+7"), JSON_KIND_NUMBER);
    }
}
