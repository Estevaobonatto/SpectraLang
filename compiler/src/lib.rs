pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod pipeline;
pub mod semantic;
pub mod span;
pub mod token;

pub use ast::{BinaryOperator, Module, UnaryOperator};
pub use error::{CompilerError, LexError, ParseError, SemanticError};
pub use lexer::Lexer;
pub use parser::Parser;
pub use pipeline::{CompilationOptions, CompilationPipeline, CompilationResult};
pub use semantic::analyze_modules;
pub use span::{span_union, Location, Span};
pub use token::{Keyword, Operator, Token, TokenKind};

pub type LexResult = Result<Vec<Token>, Vec<LexError>>;
pub type ParseResult = Result<Module, Vec<ParseError>>;
pub type SemanticResult = Result<(), Vec<SemanticError>>;
