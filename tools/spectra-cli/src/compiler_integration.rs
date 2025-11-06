// Full compiler integration
// Provides a backend driver that plugs midend + backend into the shared pipeline.

use spectra_backend::CodeGenerator;
use spectra_compiler::{
    error::MidendError, BackendDriver, BackendError, CompilationOptions, CompilationPipeline,
    CompilerError,
};
use spectra_midend::{
    ir::Module as IRModule,
    lowering::ASTLowering,
    passes::{
        constant_folding::ConstantFolding, dead_code_elimination::DeadCodeElimination,
        validation::LoopStructureValidation, Pass,
    },
};

#[derive(Debug)]
struct FullPipelineArtifacts {
    ir_module: IRModule,
    applied_passes: Vec<&'static str>,
}

struct FullPipelineBackend {
    codegen: Option<CodeGenerator>,
}

impl FullPipelineBackend {
    fn new() -> Self {
        Self { codegen: None }
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

        let mut loop_check = LoopStructureValidation::new();
        loop_check.run(&mut ir_module);

        if loop_check.has_errors() {
            let errors: Vec<CompilerError> = loop_check
                .take_errors()
                .into_iter()
                .map(|err| {
                    CompilerError::Midend(MidendError::new(format!(
                        "Loop validation failed in function '{}' at block {} ('{}'): {}",
                        err.function, err.header_block, err.header_label, err.message
                    )))
                })
                .collect();
            return Err(errors);
        }

        applied_passes.push("Loop Structure Validation");

        if options.dump_ir {
            println!("=== IR (after optimization) ===");
            println!("{:#?}", ir_module);
            println!();
        }

        let mut codegen = CodeGenerator::new();
        if let Err(error) = codegen.generate_module(&ir_module) {
            return Err(vec![CompilerError::Backend(BackendError::new(error))]);
        }

        self.codegen = Some(codegen);

        Ok(FullPipelineArtifacts {
            ir_module,
            applied_passes,
        })
    }

    fn execute(
        &mut self,
        artifacts: &Self::Artifacts,
        _options: &CompilationOptions,
    ) -> Result<(), Vec<CompilerError>> {
        let codegen = match self.codegen.as_mut() {
            Some(codegen) => codegen,
            None => {
                println!(
                    "\n⚠️ Backend artifacts missing code generator; JIT execution unavailable"
                );
                return Ok(());
            }
        };

        if !artifacts
            .ir_module
            .functions
            .iter()
            .any(|func| func.name == "main")
        {
            println!("\n⚠️ No entry point 'main' found; skipping execution");
            return Ok(());
        }

        spectra_runtime::initialize();

        let return_value = unsafe {
            codegen
                .execute_entry_point("main", &artifacts.ir_module)
                .map_err(|err| vec![CompilerError::Backend(BackendError::new(err))])?
        };

        if let Some(value) = return_value {
            println!(
                "\n✅ Execution completed (JIT)\n   - main() returned {}",
                value
            );
        } else {
            println!("\n✅ Execution completed (JIT)\n   - main() returned void");
        }

        Ok(())
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

        let artifacts = compilation.backend_artifacts;

        if self.options.optimize && !artifacts.applied_passes.is_empty() {
            println!(
                "Optimization passes applied: {}",
                artifacts.applied_passes.join(", ")
            );
        }

        println!(
            "IR functions emitted: {}",
            artifacts.ir_module.functions.len()
        );

        if self.options.run_jit {
            pipeline.execute_artifacts(&artifacts).map_err(|errors| {
                let mut error_msg = String::from("Execution errors:\n");
                for error in errors {
                    error_msg.push_str(&format!("  • {}\n", error));
                }
                error_msg
            })?;
        }

        println!("✨ Compilation successful!");
        println!("━━━━━━━━━━━━━━━━━━━━");

        Ok(())
    }

    /// Compile and execute (JIT)
    #[allow(dead_code)]
    pub fn compile_and_execute(&mut self, source: &str) -> Result<(), String> {
        println!("🚀 SpectraLang Compiler");
        println!("━━━━━━━━━━━━━━━━━━━━");
        println!();

        let pipeline = CompilationPipeline::new(self.options.clone());
        let mut pipeline = pipeline.with_backend(FullPipelineBackend::new());
        let compilation = pipeline.compile(source, "<jit>").map_err(|errors| {
            let mut error_msg = String::from("Compilation errors:\n");
            for error in errors {
                error_msg.push_str(&format!("  • {}\n", error));
            }
            error_msg
        })?;

        let artifacts = &compilation.backend_artifacts;

        if self.options.optimize && !artifacts.applied_passes.is_empty() {
            println!(
                "Optimization passes applied: {}",
                artifacts.applied_passes.join(", ")
            );
        }

        println!(
            "IR functions emitted: {}",
            artifacts.ir_module.functions.len()
        );

        pipeline.execute_artifacts(artifacts).map_err(|errors| {
            let mut error_msg = String::from("Execution errors:\n");
            for error in errors {
                error_msg.push_str(&format!("  • {}\n", error));
            }
            error_msg
        })?;

        println!("✨ Compilation successful!");
        println!("━━━━━━━━━━━━━━━━━━━━");

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
            run_jit: false,
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
