// Compilation pipeline
// Orchestrates all compilation phases: Lexer → Parser → Semantic → Midend → Backend

use crate::ast::Module as ASTModule;
use crate::error::CompilerError;
use crate::parser::workspace::{ModuleLoader, ModuleParseError};
use crate::semantic::SemanticAnalyzer;
use std::collections::HashSet;
use std::fmt::Debug;
use std::time::{Duration, Instant};

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
    /// Execute compiled code via JIT after successful compilation
    pub run_jit: bool,
    /// Collect timing metrics for each compilation stage
    pub collect_metrics: bool,
    /// Enabled experimental language features
    pub experimental_features: HashSet<String>,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        Self {
            optimize: true,
            opt_level: 2,
            dump_ir: false,
            dump_ast: false,
            run_jit: false,
            collect_metrics: false,
            experimental_features: HashSet::new(),
        }
    }
}

/// Wall-clock timings for each compilation stage
#[derive(Debug, Clone, Default)]
pub struct CompilationMetrics {
    pub total: Duration,
    pub lexing: Duration,
    pub parsing: Duration,
    pub semantic: Duration,
    pub backend: Duration,
}

/// Trait implemented by backends that continue compilation beyond semantic analysis.
pub trait BackendDriver {
    type Artifacts: Debug;

    fn run(
        &mut self,
        ast: &ASTModule,
        options: &CompilationOptions,
    ) -> Result<Self::Artifacts, Vec<CompilerError>>;

    fn execute(
        &mut self,
        _artifacts: &Self::Artifacts,
        _options: &CompilationOptions,
    ) -> Result<(), Vec<CompilerError>> {
        Ok(())
    }
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
    pub metrics: Option<CompilationMetrics>,
}

/// Full compilation pipeline
pub struct CompilationPipeline<B = NoopBackend>
where
    B: BackendDriver,
{
    options: CompilationOptions,
    backend: B,
    module_loader: ModuleLoader,
}

impl CompilationPipeline<NoopBackend> {
    pub fn new(options: CompilationOptions) -> Self {
        Self {
            options,
            backend: NoopBackend::default(),
            module_loader: ModuleLoader::new(),
        }
    }

    pub fn with_backend<B>(self, backend: B) -> CompilationPipeline<B>
    where
        B: BackendDriver,
    {
        CompilationPipeline {
            options: self.options,
            backend,
            module_loader: self.module_loader,
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
        filename: &str,
    ) -> Result<CompilationResult<B::Artifacts>, Vec<CompilerError>> {
        let collect_metrics = self.options.collect_metrics;
        let total_start = collect_metrics.then(Instant::now);
        let mut metrics = collect_metrics.then_some(CompilationMetrics::default());

        // Phases 1 & 2: Lexical Analysis + Parsing (with incremental cache)
        let parse_outcome = self.module_loader.parse_module(
            filename,
            source,
            &self.options.experimental_features,
        );

        let parse_outcome = parse_outcome.map_err(|error| match error {
            ModuleParseError::Lexical(errors) => errors
                .into_iter()
                .map(CompilerError::Lexical)
                .collect::<Vec<_>>(),
            ModuleParseError::Parse(errors) => errors
                .into_iter()
                .map(CompilerError::Parse)
                .collect::<Vec<_>>(),
        })?;

        if let Some(metrics) = metrics.as_mut() {
            metrics.lexing = parse_outcome.lexing_duration;
            metrics.parsing = parse_outcome.parsing_duration;
        }

        let mut ast = parse_outcome.module;

        if self.options.dump_ast {
            println!("=== AST ===");
            println!("{:#?}", ast);
            println!();
        }

        // Phase 3: Semantic Analysis
        let mut semantic = SemanticAnalyzer::new();
        let semantic_start = collect_metrics.then(Instant::now);
        let semantic_errors = semantic.analyze_module(&mut ast);
        if let (Some(metrics), Some(start)) = (metrics.as_mut(), semantic_start) {
            metrics.semantic = start.elapsed();
        }

        if !semantic_errors.is_empty() {
            return Err(semantic_errors
                .into_iter()
                .map(|e| CompilerError::Semantic(e))
                .collect());
        }

        let backend_start = collect_metrics.then(Instant::now);
        let backend_artifacts = self.backend.run(&ast, &self.options)?;
        if let (Some(metrics), Some(start)) = (metrics.as_mut(), backend_start) {
            metrics.backend = start.elapsed();
        }

        if let (Some(metrics), Some(start)) = (metrics.as_mut(), total_start) {
            metrics.total = start.elapsed();
        }

        Ok(CompilationResult {
            ast,
            errors: vec![],
            warnings: vec![],
            backend_artifacts,
            metrics,
        })
    }

    pub fn execute_artifacts(
        &mut self,
        artifacts: &B::Artifacts,
    ) -> Result<(), Vec<CompilerError>> {
        self.backend.execute(artifacts, &self.options)
    }

    /// Compile and execute (for REPL)
    pub fn compile_and_execute(&mut self, source: &str) -> Result<(), Vec<CompilerError>> {
        let compilation = self.compile(source, "<repl>")?;
        self.execute_artifacts(&compilation.backend_artifacts)
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
