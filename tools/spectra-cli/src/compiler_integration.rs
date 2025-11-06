// Full compiler integration
// Provides a backend driver that plugs midend + backend into the shared pipeline.

use spectra_backend::CodeGenerator;
use spectra_compiler::{
    BackendDriver, BackendError, CompilationOptions, CompilationPipeline, CompilerError,
};
use spectra_midend::{
    ir::Module as IRModule,
    lowering::ASTLowering,
    passes::{constant_folding::ConstantFolding, dead_code_elimination::DeadCodeElimination, Pass},
};

#[derive(Debug)]
struct FullPipelineArtifacts {
    ir_module: IRModule,
    applied_passes: Vec<&'static str>,
}

struct FullPipelineBackend;

impl FullPipelineBackend {
    fn new() -> Self {
        Self
    }
}

impl BackendDriver for FullPipelineBackend {
    type Artifacts = FullPipelineArtifacts;

    fn run(
        &mut self,
        ast: &spectra_compiler::ast::Module,
        options: &CompilationOptions,
    ) -> Result<Self::Artifacts, Vec<CompilerError>> {
        let mut lowering = ASTLowering::new();
        let mut ir_module = lowering.lower_module(ast);

        if options.dump_ir {
            println!("=== IR (before optimization) ===");
            println!("{:#?}", ir_module);
            println!();
        }

        let mut applied_passes = Vec::new();

        if options.optimize {
            if options.opt_level >= 1 {
                let mut cf = ConstantFolding::new();
                if cf.run(&mut ir_module) {
                    applied_passes.push("Constant Folding");
                }
            }

            if options.opt_level >= 2 {
                let mut dce = DeadCodeElimination::new();
                if dce.run(&mut ir_module) {
                    applied_passes.push("Dead Code Elimination");
                }
            }
        }

        if options.dump_ir {
            println!("=== IR (after optimization) ===");
            println!("{:#?}", ir_module);
            println!();
        }

        let mut codegen = CodeGenerator::new();
        if let Err(error) = codegen.generate_module(&ir_module) {
            return Err(vec![CompilerError::Backend(BackendError::new(error))]);
        }

        Ok(FullPipelineArtifacts {
            ir_module,
            applied_passes,
        })
    }
}

/// Complete compiler that integrates all phases
pub struct SpectraCompiler {
    options: CompilationOptions,
}

impl SpectraCompiler {
    pub fn new(options: CompilationOptions) -> Self {
        Self { options }
    }

    /// Compile source code to native code
    pub fn compile(&mut self, source: &str, filename: &str) -> Result<(), String> {
        println!("🚀 SpectraLang Compiler");
        println!("━━━━━━━━━━━━━━━━━━━━");
        println!();

        let pipeline = CompilationPipeline::new(self.options.clone());
        let mut pipeline = pipeline.with_backend(FullPipelineBackend::new());
        let compilation = pipeline.compile(source, filename).map_err(|errors| {
            let mut error_msg = String::from("Compilation errors:\n");
            for error in errors {
                error_msg.push_str(&format!("  • {}\n", error));
            }
            error_msg
        })?;

        let FullPipelineArtifacts {
            ir_module,
            applied_passes,
        } = compilation.backend_artifacts;

        if self.options.optimize && !applied_passes.is_empty() {
            println!("Optimization passes applied: {}", applied_passes.join(", "));
        }

        println!("IR functions emitted: {}", ir_module.functions.len());

        println!("✨ Compilation successful!");
        println!("━━━━━━━━━━━━━━━━━━━━");

        Ok(())
    }

    /// Compile and execute (JIT)
    #[allow(dead_code)]
    pub fn compile_and_execute(&mut self, source: &str) -> Result<(), String> {
        self.compile(source, "<jit>")?;

        // TODO: Execute the compiled code via JIT
        println!("\n🎯 Execution (TODO: JIT execution not yet implemented)");

        Ok(())
    }
}

impl Default for SpectraCompiler {
    fn default() -> Self {
        Self::new(CompilationOptions::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end_simple() {
        let source = r#"
            module test;
            
            fn add(a: int, b: int) -> int {
                return a + b;
            }
            
            pub fn main() {
                let x = add(5, 3);
                return;
            }
        "#;

        let mut compiler = SpectraCompiler::default();
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_end_to_end_with_optimization() {
        let source = r#"
            module test;
            
            fn compute() -> int {
                let x = 10 + 20;
                let y = x * 2;
                return y;
            }
            
            pub fn main() {
                let result = compute();
                return;
            }
        "#;

        let options = CompilationOptions {
            optimize: true,
            opt_level: 2,
            dump_ir: false,
            dump_ast: false,
        };

        let mut compiler = SpectraCompiler::new(options);
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_end_to_end_control_flow() {
        let source = r#"
            module test;
            
            fn max(a: int, b: int) -> int {
                if a > b {
                    return a;
                } else {
                    return b;
                }
            }
            
            pub fn main() {
                let result = max(10, 20);
                return;
            }
        "#;

        let mut compiler = SpectraCompiler::default();
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_end_to_end_loop() {
        let source = r#"
            module test;
            
            fn factorial(n: int) -> int {
                let result = 1;
                let i = 1;
                
                while i <= n {
                    result = result * i;
                    i = i + 1;
                }
                
                return result;
            }
            
            pub fn main() {
                let result = factorial(5);
                return;
            }
        "#;

        let mut compiler = SpectraCompiler::default();
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }
}
