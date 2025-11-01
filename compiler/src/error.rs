use crate::span::Span;
use std::fmt;

/// Unified compiler error type
#[derive(Debug, Clone)]
pub enum CompilerError {
    Lexical(LexError),
    Parse(ParseError),
    Semantic(SemanticError),
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompilerError::Lexical(e) => write!(f, "Lexical error at {:?}: {}", e.span, e.message),
            CompilerError::Parse(e) => write!(f, "Parse error at {:?}: {}", e.span, e.message),
            CompilerError::Semantic(e) => {
                write!(f, "Semantic error at {:?}: {}", e.span, e.message)
            }
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug, Clone)]
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
