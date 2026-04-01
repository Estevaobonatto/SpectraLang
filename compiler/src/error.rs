use crate::span::Span;
use std::fmt;

/// Common interface for all compiler error types.
pub trait DiagnosticError {
    fn message(&self) -> &str;
    fn span(&self) -> Option<&Span>;
    fn hint(&self) -> Option<&str>;
    fn context(&self) -> Option<&str>;
}

/// Unified compiler error type
#[derive(Debug, Clone)]
pub enum CompilerError {
    Lexical(LexError),
    Parse(ParseError),
    Semantic(SemanticError),
    Midend(MidendError),
    Backend(BackendError),
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompilerError::Lexical(e) => fmt_span_error(
                f,
                "Lexical",
                &e.message,
                &e.span,
                e.context.as_deref(),
                e.hint.as_deref(),
            ),
            CompilerError::Parse(e) => fmt_span_error(
                f,
                "Parse",
                &e.message,
                &e.span,
                e.context.as_deref(),
                e.hint.as_deref(),
            ),
            CompilerError::Semantic(e) => fmt_span_error(
                f,
                "Semantic",
                &e.message,
                &e.span,
                e.context.as_deref(),
                e.hint.as_deref(),
            ),
            CompilerError::Midend(e) => write!(f, "Midend error: {}", e.message),
            CompilerError::Backend(e) => write!(f, "Backend error: {}", e.message),
        }
    }
}

fn fmt_span_error(
    f: &mut fmt::Formatter<'_>,
    phase: &str,
    message: &str,
    span: &Span,
    context: Option<&str>,
    hint: Option<&str>,
) -> fmt::Result {
    write!(
        f,
        "{} error at line {}, column {}: {}",
        phase, span.start_location.line, span.start_location.column, message
    )?;

    if let Some(context) = context {
        write!(f, " ({})", context)?;
    }

    if let Some(hint) = hint {
        write!(f, " [hint: {}]", hint)?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct MidendError {
    pub message: String,
}

impl MidendError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendError {
    pub message: String,
}

impl BackendError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub span: Span,
    pub context: Option<String>,
    pub hint: Option<String>,
    pub code: Option<String>,
}

impl LexError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            context: None,
            hint: None,
            code: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub context: Option<String>,
    pub hint: Option<String>,
    pub code: Option<String>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            context: None,
            hint: None,
            code: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct SemanticError {
    pub message: String,
    pub span: Span,
    pub context: Option<String>,
    pub hint: Option<String>,
    pub code: Option<String>,
}

impl SemanticError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            context: None,
            hint: None,
            code: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

// --- DiagnosticError implementations ---

impl DiagnosticError for LexError {
    fn message(&self) -> &str { &self.message }
    fn span(&self) -> Option<&Span> { Some(&self.span) }
    fn hint(&self) -> Option<&str> { self.hint.as_deref() }
    fn context(&self) -> Option<&str> { self.context.as_deref() }
}

impl DiagnosticError for ParseError {
    fn message(&self) -> &str { &self.message }
    fn span(&self) -> Option<&Span> { Some(&self.span) }
    fn hint(&self) -> Option<&str> { self.hint.as_deref() }
    fn context(&self) -> Option<&str> { self.context.as_deref() }
}

impl DiagnosticError for SemanticError {
    fn message(&self) -> &str { &self.message }
    fn span(&self) -> Option<&Span> { Some(&self.span) }
    fn hint(&self) -> Option<&str> { self.hint.as_deref() }
    fn context(&self) -> Option<&str> { self.context.as_deref() }
}

impl DiagnosticError for MidendError {
    fn message(&self) -> &str { &self.message }
    fn span(&self) -> Option<&Span> { None }
    fn hint(&self) -> Option<&str> { None }
    fn context(&self) -> Option<&str> { None }
}

impl DiagnosticError for BackendError {
    fn message(&self) -> &str { &self.message }
    fn span(&self) -> Option<&Span> { None }
    fn hint(&self) -> Option<&str> { None }
    fn context(&self) -> Option<&str> { None }
}
