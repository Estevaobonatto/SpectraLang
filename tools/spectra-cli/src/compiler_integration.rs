// Full compiler integration
// Provides a backend driver that plugs midend + backend into the shared pipeline.

use std::time::{Duration, Instant};

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
struct PassReport {
    name: &'static str,
    duration: Duration,
    modified: bool,
}

#[derive(Debug)]
struct FullPipelineArtifacts {
    ir_module: IRModule,
    passes: Vec<PassReport>,
    lowering_duration: Duration,
    codegen_duration: Duration,
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
        let lowering_start = Instant::now();
        let mut ir_module = lowering.lower_module(ast);
        let lowering_duration = lowering_start.elapsed();

        if options.dump_ir {
            println!("=== IR (before optimization) ===");
            println!("{:#?}", ir_module);
            println!();
        }

        let mut pass_reports = Vec::new();

        if options.optimize {
            if options.opt_level >= 1 {
                let mut cf = ConstantFolding::new();
                let pass_start = Instant::now();
                let modified = cf.run(&mut ir_module);
                pass_reports.push(PassReport {
                    name: "Constant Folding",
                    duration: pass_start.elapsed(),
                    modified,
                });
            }

            if options.opt_level >= 2 {
                let mut dce = DeadCodeElimination::new();
                let pass_start = Instant::now();
                let modified = dce.run(&mut ir_module);
                pass_reports.push(PassReport {
                    name: "Dead Code Elimination",
                    duration: pass_start.elapsed(),
                    modified,
                });
            }
        }

        let mut loop_check = LoopStructureValidation::new();
        let validation_start = Instant::now();
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

        pass_reports.push(PassReport {
            name: "Loop Structure Validation",
            duration: validation_start.elapsed(),
            modified: false,
        });

        if options.dump_ir {
            println!("=== IR (after optimization) ===");
            println!("{:#?}", ir_module);
            println!();
        }

        let mut codegen = CodeGenerator::new();
        let codegen_start = Instant::now();
        let codegen_result = codegen.generate_module(&ir_module);
        let codegen_duration = codegen_start.elapsed();

        if let Err(error) = codegen_result {
            return Err(vec![CompilerError::Backend(BackendError::new(error))]);
        }

        self.codegen = Some(codegen);

        Ok(FullPipelineArtifacts {
            ir_module,
            passes: pass_reports,
            lowering_duration,
            codegen_duration,
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

        let runtime_state = spectra_runtime::initialize();
        let execution_start = Instant::now();

        let return_value = unsafe {
            codegen.execute_entry_point("main", &artifacts.ir_module)
        };
        let execution_duration = execution_start.elapsed();

        let return_value = return_value
            .map_err(|err| vec![CompilerError::Backend(BackendError::new(err))])?;

        let runtime_uptime = runtime_state.uptime();
        let init_thread = runtime_state.init_thread_id();

        if let Some(value) = return_value {
            println!(
                "\n✅ Execution completed (JIT)\n   - main() returned {}\n   - execution time {:?}\n   - runtime uptime {:?} (init thread {:?})",
                value, execution_duration, runtime_uptime, init_thread
            );
        } else {
            println!(
                "\n✅ Execution completed (JIT)\n   - main() returned void\n   - execution time {:?}\n   - runtime uptime {:?} (init thread {:?})",
                execution_duration, runtime_uptime, init_thread
            );
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

        if self.options.optimize {
            let modified_passes: Vec<_> = artifacts
                .passes
                .iter()
                .filter(|report| report.modified)
                .map(|report| report.name)
                .collect();

            if modified_passes.is_empty() {
                println!("Optimization passes applied: none (IR unchanged)");
            } else {
                println!(
                    "Optimization passes applied: {}",
                    modified_passes.join(", ")
                );
            }
        }

        if !artifacts.passes.is_empty() {
            println!("Pass timings:");
            for report in &artifacts.passes {
                let status = if report.modified { "modified" } else { "no change" };
                println!(
                    "  • {:<28} {:>10?} ({})",
                    report.name, report.duration, status
                );
            }
        }

        println!("Lowering time: {:?}", artifacts.lowering_duration);
        println!("Code generation time: {:?}", artifacts.codegen_duration);

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

        if self.options.optimize {
            let modified_passes: Vec<_> = artifacts
                .passes
                .iter()
                .filter(|report| report.modified)
                .map(|report| report.name)
                .collect();

            if modified_passes.is_empty() {
                println!("Optimization passes applied: none (IR unchanged)");
            } else {
                println!(
                    "Optimization passes applied: {}",
                    modified_passes.join(", ")
                );
            }
        }

        if !artifacts.passes.is_empty() {
            println!("Pass timings:");
            for report in &artifacts.passes {
                let status = if report.modified { "modified" } else { "no change" };
                println!(
                    "  • {:<28} {:>10?} ({})",
                    report.name, report.duration, status
                );
            }
        }

        println!("Lowering time: {:?}", artifacts.lowering_duration);
        println!("Code generation time: {:?}", artifacts.codegen_duration);

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
