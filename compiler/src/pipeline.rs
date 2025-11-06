// Compilation pipeline
// Orchestrates all compilation phases: Lexer → Parser → Semantic → Midend → Backend

use crate::ast::Module as ASTModule;
use crate::error::CompilerError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::semantic::SemanticAnalyzer;
use std::fmt::Debug;

/// Compilation options
#[derive(Debug, Clone)]
pub struct CompilationOptions {
    /// Enable optimization passes
    pub optimize: bool,
    /// Optimization level (0-3)
    pub opt_level: u8,
    /// Dump IR for debugging
    pub dump_ir: bool,
    /// Dump AST for debugging
    pub dump_ast: bool,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        Self {
            optimize: true,
            opt_level: 2,
            dump_ir: false,
            dump_ast: false,
        }
    }
}

/// Trait implemented by backends that continue compilation beyond semantic analysis.
pub trait BackendDriver {
    type Artifacts: Debug;

    fn run(
        &mut self,
        ast: &ASTModule,
        options: &CompilationOptions,
    ) -> Result<Self::Artifacts, Vec<CompilerError>>;
}

/// Default backend that performs no additional work.
#[derive(Debug, Default)]
pub struct NoopBackend;

impl BackendDriver for NoopBackend {
    type Artifacts = ();

    fn run(
        &mut self,
        _ast: &ASTModule,
        _options: &CompilationOptions,
    ) -> Result<Self::Artifacts, Vec<CompilerError>> {
        Ok(())
    }
}

/// Compilation pipeline result
#[derive(Debug)]
pub struct CompilationResult<A>
where
    A: Debug,
{
    pub ast: ASTModule,
    pub errors: Vec<CompilerError>,
    pub warnings: Vec<String>,
    pub backend_artifacts: A,
}

/// Full compilation pipeline
pub struct CompilationPipeline<B = NoopBackend>
where
    B: BackendDriver,
{
    options: CompilationOptions,
    backend: B,
}

impl CompilationPipeline<NoopBackend> {
    pub fn new(options: CompilationOptions) -> Self {
        Self {
            options,
            backend: NoopBackend::default(),
        }
    }

    pub fn with_backend<B>(self, backend: B) -> CompilationPipeline<B>
    where
        B: BackendDriver,
    {
        CompilationPipeline {
            options: self.options,
            backend,
        }
    }
}

impl<B> CompilationPipeline<B>
where
    B: BackendDriver,
{
    /// Run the full compilation pipeline
    pub fn compile(
        &mut self,
        source: &str,
        _filename: &str,
    ) -> Result<CompilationResult<B::Artifacts>, Vec<CompilerError>> {
        // Phase 1: Lexical Analysis
        let lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|errors| {
            errors
                .into_iter()
                .map(CompilerError::Lexical)
                .collect::<Vec<_>>()
        })?;

        // Phase 2: Parsing
        let parser = Parser::new(tokens);
        let mut ast = parser.parse().map_err(|errors| {
            errors
                .into_iter()
                .map(CompilerError::Parse)
                .collect::<Vec<_>>()
        })?;

        if self.options.dump_ast {
            println!("=== AST ===");
            println!("{:#?}", ast);
            println!();
        }

        // Phase 3: Semantic Analysis
        let mut semantic = SemanticAnalyzer::new();
        let semantic_errors = semantic.analyze_module(&mut ast);

        if !semantic_errors.is_empty() {
            return Err(semantic_errors
                .into_iter()
                .map(|e| CompilerError::Semantic(e))
                .collect());
        }

        let backend_artifacts = self.backend.run(&ast, &self.options)?;

        Ok(CompilationResult {
            ast,
            errors: vec![],
            warnings: vec![],
            backend_artifacts,
        })
    }

    /// Compile and execute (for REPL)
    pub fn compile_and_execute(&mut self, source: &str) -> Result<(), Vec<CompilerError>> {
        let _result = self.compile(source, "<repl>")?;

        // TODO: Execute compiled code
        println!("Compilation successful! (Execution not yet implemented)");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_compilation() {
        let source = r#"
            fn main() {
                let x = 10;
                return x;
            }
        "#;

        let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, "test.spectra");

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.errors.len(), 0);
    }

    #[test]
    fn test_arithmetic_compilation() {
        let source = r#"
            fn add(a: int, b: int) -> int {
                return a + b;
            }
        "#;

        let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_if_statement_compilation() {
        let source = r#"
            fn max(a: int, b: int) -> int {
                if a > b {
                    return a;
                } else {
                    return b;
                }
            }
        "#;

        let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_loop_compilation() {
        let source = r#"
            fn sum_to_n(n: int) -> int {
                let sum = 0;
                let i = 0;
                
                while i <= n {
                    sum = sum + i;
                    i = i + 1;
                }
                
                return sum;
            }
        "#;

        let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_semantic_error_detection() {
        let source = r#"
            fn main() {
                return undefined_variable;
            }
        "#;

        let mut pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, "test.spectra");

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
    }
}
