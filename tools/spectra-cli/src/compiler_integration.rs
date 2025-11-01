// Full compiler integration
// Complete pipeline: Source → AST → IR → Native Code

use spectra_backend::CodeGenerator;
use spectra_compiler::{CompilationOptions, CompilationPipeline};
use spectra_midend::{
    ir::Module as IRModule,
    lowering::ASTLowering,
    passes::{constant_folding::ConstantFolding, dead_code_elimination::DeadCodeElimination, Pass},
};

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

        // Phase 1-3: Frontend (Lexer → Parser → Semantic)
        println!("📝 Phase 1-3: Frontend Analysis");
        let pipeline = CompilationPipeline::new(self.options.clone());
        let compilation_result = pipeline.compile(source, filename).map_err(|errors| {
            let mut error_msg = String::from("Compilation errors:\n");
            for error in errors {
                error_msg.push_str(&format!("  • {}\n", error));
            }
            error_msg
        })?;

        println!("  ✅ Lexical analysis complete");
        println!("  ✅ Parsing complete");
        println!("  ✅ Semantic analysis complete");
        println!();

        // Phase 4: Midend (AST → IR + Optimization)
        println!("🔄 Phase 4: Midend (IR Generation)");
        let mut lowering = ASTLowering::new();
        let mut ir_module = lowering.lower_module(&compilation_result.ast);
        println!("  ✅ AST lowered to IR");

        if self.options.dump_ir {
            println!("\n=== IR (Before Optimization) ===");
            self.dump_ir(&ir_module);
        }

        // Optimization passes
        if self.options.optimize {
            println!(
                "  🔧 Running optimization passes (level {})...",
                self.options.opt_level
            );

            if self.options.opt_level >= 1 {
                let mut cf = ConstantFolding::new();
                if cf.run(&mut ir_module) {
                    println!("    • Constant folding applied");
                }
            }

            if self.options.opt_level >= 2 {
                let mut dce = DeadCodeElimination::new();
                if dce.run(&mut ir_module) {
                    println!("    • Dead code eliminated");
                }
            }

            println!("  ✅ Optimization complete");
        }

        if self.options.dump_ir {
            println!("\n=== IR (After Optimization) ===");
            self.dump_ir(&ir_module);
        }
        println!();

        // Phase 5: Backend (IR → Native Code)
        println!("⚙️  Phase 5: Backend (Code Generation)");
        let mut codegen = CodeGenerator::new();
        codegen
            .generate_module(&ir_module)
            .map_err(|e| format!("Code generation failed: {}", e))?;
        println!("  ✅ Native code generated");
        println!();

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

    /// Dump IR for debugging
    fn dump_ir(&self, ir_module: &IRModule) {
        println!("Module: {}", ir_module.name);
        println!();

        for func in &ir_module.functions {
            println!("function {}(", func.name);
            for (i, param) in func.params.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                print!("%{}: {:?}", param.id, param.ty);
            }
            println!(") -> {:?} {{", func.return_type);

            for block in &func.blocks {
                println!("  {}:", block.label);
                for instr in &block.instructions {
                    println!("    {:?}", instr.kind);
                }
                if let Some(term) = &block.terminator {
                    println!("    {:?}", term);
                }
                println!();
            }

            println!("}}");
            println!();
        }
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
