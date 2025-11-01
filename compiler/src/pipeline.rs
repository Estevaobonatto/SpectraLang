// Compilation pipeline
// Orchestrates all compilation phases: Lexer → Parser → Semantic → Midend → Backend

use crate::ast::Module as ASTModule;
use crate::error::CompilerError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::semantic::SemanticAnalyzer;

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

/// Compilation pipeline result
pub struct CompilationResult {
    pub ast: ASTModule,
    pub errors: Vec<CompilerError>,
    pub warnings: Vec<String>,
}

/// Full compilation pipeline
pub struct CompilationPipeline {
    options: CompilationOptions,
}

impl CompilationPipeline {
    pub fn new(options: CompilationOptions) -> Self {
        Self { options }
    }

    /// Run the full compilation pipeline
    pub fn compile(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<CompilationResult, Vec<CompilerError>> {
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
        let ast = parser.parse().map_err(|errors| {
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
        let semantic_errors = semantic.analyze_module(&ast);

        if !semantic_errors.is_empty() {
            return Err(semantic_errors
                .into_iter()
                .map(|e| CompilerError::Semantic(e))
                .collect());
        }

        // Phase 4: Midend (IR Generation + Optimization)
        // TODO: Connect to midend when ready
        // let ir_module = self.lower_to_ir(&ast)?;

        // if self.options.optimize {
        //     let ir_module = self.optimize_ir(ir_module)?;
        // }

        // if self.options.dump_ir {
        //     println!("=== IR ===");
        //     println!("{:#?}", ir_module);
        //     println!();
        // }

        // Phase 5: Backend (Code Generation)
        // TODO: Connect to backend when ready
        // let native_code = self.generate_code(&ir_module)?;

        Ok(CompilationResult {
            ast,
            errors: vec![],
            warnings: vec![],
        })
    }

    /// Compile and execute (for REPL)
    pub fn compile_and_execute(&self, source: &str) -> Result<(), Vec<CompilerError>> {
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

        let pipeline = CompilationPipeline::new(CompilationOptions::default());
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

        let pipeline = CompilationPipeline::new(CompilationOptions::default());
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

        let pipeline = CompilationPipeline::new(CompilationOptions::default());
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

        let pipeline = CompilationPipeline::new(CompilationOptions::default());
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

        let pipeline = CompilationPipeline::new(CompilationOptions::default());
        let result = pipeline.compile(source, "test.spectra");

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
    }
}
