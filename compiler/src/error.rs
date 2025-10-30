use crate::span::{Location, Span};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message} at {span:?}")]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl LexError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message} at {location}")]
pub struct ParseError {
    pub message: String,
    pub location: Location,
}

impl ParseError {
    pub fn new(message: impl Into<String>, location: Location) -> Self {
        Self {
            message: message.into(),
            location,
        }
    }
}

pub type LexResult<T> = Result<T, Vec<LexError>>;
pub type ParseResult<T> = Result<T, Vec<ParseError>>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message} at {span:?}")]
pub struct SemanticError {
    pub message: String,
    pub span: Span,
}

impl SemanticError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

pub type SemanticResult<T> = Result<T, Vec<SemanticError>>;
