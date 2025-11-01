// Code generation using Cranelift JIT
// Translates Spectra IR to native machine code

use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use spectra_midend::ir::{Function as IRFunction, Module as IRModule, Parameter, Type as IRType};
use std::collections::HashMap;

pub struct CodeGenerator {
    /// Cranelift JIT module
    module: JITModule,
    /// Function builder context
    ctx: codegen::Context,
    /// Builder for creating IR
    builder_context: FunctionBuilderContext,
    /// Mapping from IR function names to Cranelift function IDs
    function_map: HashMap<String, FuncId>,
}

impl CodeGenerator {
    /// Create a new code generator
    pub fn new() -> Self {
        let builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .expect("Failed to create JIT builder");

        let module = JITModule::new(builder);
        let ctx = module.make_context();

        Self {
            module,
            ctx,
            builder_context: FunctionBuilderContext::new(),
            function_map: HashMap::new(),
        }
    }

    /// Generate code for an entire module
    pub fn generate_module(&mut self, ir_module: &IRModule) -> Result<(), String> {
        // First pass: declare all functions
        for func in &ir_module.functions {
            self.declare_function(func)?;
        }

        // Second pass: define all functions
        for func in &ir_module.functions {
            self.define_function(func)?;
        }

        // Finalize all functions
        self.module
            .finalize_definitions()
            .map_err(|e| format!("Failed to finalize definitions: {}", e))?;

        Ok(())
    }

    /// Declare a function signature
    fn declare_function(&mut self, ir_func: &IRFunction) -> Result<FuncId, String> {
        let mut sig = self.module.make_signature();

        // Add parameters
        for param in &ir_func.params {
            let cl_type = Self::ir_type_to_cranelift(&param.ty)?;
            sig.params.push(AbiParam::new(cl_type));
        }

        // Add return type
        let return_type = Self::ir_type_to_cranelift(&ir_func.return_type)?;
        if return_type != types::I8 || ir_func.return_type != IRType::Void {
            sig.returns.push(AbiParam::new(return_type));
        }

        // Declare function in module
        let func_id = self
            .module
            .declare_function(&ir_func.name, Linkage::Export, &sig)
            .map_err(|e| format!("Failed to declare function '{}': {}", ir_func.name, e))?;

        self.function_map.insert(ir_func.name.clone(), func_id);

        Ok(func_id)
    }

    /// Define a function body
    fn define_function(&mut self, ir_func: &IRFunction) -> Result<(), String> {
        let func_id = *self
            .function_map
            .get(&ir_func.name)
            .ok_or_else(|| format!("Function '{}' not declared", ir_func.name))?;

        // Clear context
        self.ctx.func.clear();

        // Set function signature
        self.ctx.func.signature = self
            .module
            .declarations()
            .get_function_decl(func_id)
            .signature
            .clone();

        // Create function builder
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        // Create entry block
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Simple return for now - full implementation will follow
        builder.ins().return_(&[]);

        // Finalize function
        builder.finalize();

        // Define function in module
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| format!("Failed to define function '{}': {}", ir_func.name, e))?;

        // Clear context
        self.module.clear_context(&mut self.ctx);

        Ok(())
    }

    /// Convert IR type to Cranelift type
    fn ir_type_to_cranelift(ty: &IRType) -> Result<types::Type, String> {
        match ty {
            IRType::Void => Ok(types::I8),
            IRType::Bool => Ok(types::I8),
            IRType::Int => Ok(types::I64),
            IRType::Float => Ok(types::F64),
            IRType::String => Ok(types::I64),
            IRType::Char => Ok(types::I32),
            IRType::Pointer(_) => Ok(types::I64),
            IRType::Function { .. } => Ok(types::I64),
        }
    }

    /// Get pointer to a compiled function
    pub fn get_function_ptr(&mut self, name: &str) -> Result<*const u8, String> {
        let func_id = self
            .function_map
            .get(name)
            .ok_or_else(|| format!("Function '{}' not found", name))?;

        Ok(self.module.get_finalized_function(*func_id))
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_creation() {
        let codegen = CodeGenerator::new();
        assert!(codegen.function_map.is_empty());
    }

    #[test]
    fn test_type_conversion() {
        assert_eq!(
            CodeGenerator::ir_type_to_cranelift(&IRType::Bool).unwrap(),
            types::I8
        );
        assert_eq!(
            CodeGenerator::ir_type_to_cranelift(&IRType::Int).unwrap(),
            types::I64
        );
        assert_eq!(
            CodeGenerator::ir_type_to_cranelift(&IRType::Float).unwrap(),
            types::F64
        );
    }

    #[test]
    fn test_simple_function_generation() {
        let mut codegen = CodeGenerator::new();

        let func = IRFunction::new(
            "test_func",
            vec![Parameter {
                id: 0,
                name: "a".to_string(),
                ty: IRType::Int,
            }],
            IRType::Int,
        );

        let result = codegen.declare_function(&func);
        assert!(result.is_ok());
    }
}
