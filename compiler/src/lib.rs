pub mod ast;
pub mod error;
pub mod lexer;
pub mod lint;
pub mod parser;
pub mod pipeline;
pub mod resolver;
pub mod semantic;
pub mod span;
pub mod token;

pub use ast::{BinaryOperator, Module, UnaryOperator};
pub use error::{BackendError, CompilerError, LexError, MidendError, ParseError, SemanticError};
pub use lexer::Lexer;
pub use lint::{LintDiagnostic, LintOptions, LintRule};
pub use parser::Parser;
pub use pipeline::{
    BackendDriver, CompilationOptions, CompilationPipeline, CompilationResult, NoopBackend,
};
pub use resolver::{ModuleGraph, ModuleResolutionError, ModuleResolver, ModuleResolverOptions};
pub use semantic::analyze_modules;
pub use span::{span_union, Location, Span};
pub use token::{Keyword, Operator, Token, TokenKind};

pub type LexResult = Result<Vec<Token>, Vec<LexError>>;
pub type ParseResult = Result<Module, Vec<ParseError>>;
pub type SemanticResult = Result<(), Vec<SemanticError>>;
