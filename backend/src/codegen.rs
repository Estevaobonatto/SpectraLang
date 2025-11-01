// Code generation using Cranelift JIT
// Translates Spectra IR to native machine code

use cranelift::prelude::*;
use cranelift_codegen::ir::StackSlot;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use spectra_midend::ir::{
    BasicBlock as IRBasicBlock, Function as IRFunction, Instruction, InstructionKind,
    Module as IRModule, Terminator, Type as IRType, Value as IRValue,
};
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

        // Create value and block mappings
        let mut value_map: HashMap<usize, Value> = HashMap::new();
        let mut block_map: HashMap<usize, Block> = HashMap::new();
        let mut stack_slot_map: HashMap<usize, StackSlot> = HashMap::new();

        // Map function parameters to Cranelift values
        let params = builder.block_params(entry_block).to_vec();
        for (idx, &cl_value) in params.iter().enumerate() {
            value_map.insert(idx, cl_value);
        }

        // Create all basic blocks
        for ir_block in &ir_func.blocks {
            if ir_block.id == 0 {
                block_map.insert(0, entry_block);
            } else {
                let block = builder.create_block();
                block_map.insert(ir_block.id, block);
            }
        }

        // Generate code for each block
        let blocks = ir_func.blocks.clone();
        for ir_block in &blocks {
            Self::generate_block_static(
                &mut builder,
                ir_block,
                &mut value_map,
                &mut stack_slot_map,
                &block_map,
                &self.function_map,
                &mut self.module,
            )?;
        }

        // Seal all blocks after generating code
        for ir_block in &ir_func.blocks {
            if ir_block.id != 0 {
                // Entry block already sealed
                if let Some(&block) = block_map.get(&ir_block.id) {
                    builder.seal_block(block);
                }
            }
        }

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

    /// Generate code for a basic block
    fn generate_block_static(
        builder: &mut FunctionBuilder,
        ir_block: &IRBasicBlock,
        value_map: &mut HashMap<usize, Value>,
        stack_slot_map: &mut HashMap<usize, StackSlot>,
        block_map: &HashMap<usize, Block>,
        function_map: &HashMap<String, FuncId>,
        module: &mut JITModule,
    ) -> Result<(), String> {
        // Get Cranelift block
        let block = *block_map
            .get(&ir_block.id)
            .ok_or_else(|| format!("Block {} not found", ir_block.id))?;

        // Switch to block
        if builder.current_block() != Some(block) {
            builder.switch_to_block(block);
        }

        // Generate instructions
        for instr in &ir_block.instructions {
            Self::generate_instruction_static(builder, instr, value_map, stack_slot_map, function_map, module)?;
        }

        // Generate terminator
        if let Some(ref terminator) = ir_block.terminator {
            Self::generate_terminator_static(builder, terminator, value_map, block_map)?;
        }

        Ok(())
    }

    /// Generate a single instruction
    fn generate_instruction_static(
        builder: &mut FunctionBuilder,
        instr: &Instruction,
        value_map: &mut HashMap<usize, Value>,
        stack_slot_map: &mut HashMap<usize, StackSlot>,
        function_map: &HashMap<String, FuncId>,
        module: &mut JITModule,
    ) -> Result<(), String> {
        // Helper to get value from map
        let get_value = |v: &IRValue| -> Result<Value, String> {
            value_map
                .get(&v.id)
                .copied()
                .ok_or_else(|| format!("Value {} not found", v.id))
        };

        match &instr.kind {
            // Arithmetic operations
            InstructionKind::Add { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().iadd(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Sub { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().isub(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Mul { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().imul(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Div { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().sdiv(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Rem { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().srem(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            // Comparison operations
            InstructionKind::Eq { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().icmp(IntCC::Equal, lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Ne { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().icmp(IntCC::NotEqual, lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Lt { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().icmp(IntCC::SignedLessThan, lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Le { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder
                    .ins()
                    .icmp(IntCC::SignedLessThanOrEqual, lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Gt { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThan, lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Ge { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val =
                    builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            // Logical operations
            InstructionKind::And { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().band(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Or { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = builder.ins().bor(lhs_val, rhs_val);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Not { result, operand } => {
                let operand_val = get_value(operand)?;
                let result_val = builder.ins().bnot(operand_val);
                value_map.insert(result.id, result_val);
            }

            // Memory operations
            InstructionKind::Alloca { result, ty } => {
                // Calculate actual size in bytes (important for arrays)
                let size_bytes = Self::type_size_bytes(ty) as u32;
                // Use 8-byte alignment for better compatibility
                let alignment = 8;
                let stack_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    size_bytes,
                    alignment,
                ));
                
                // ONLY store arrays in stack_slot_map (they need cross-block access)
                // Regular mutable variables work fine with just value_map
                if matches!(ty, IRType::Array { .. }) {
                    stack_slot_map.insert(result.id, stack_slot);
                }
                
                // For immediate use in the same block, always generate stack_addr
                let addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
                value_map.insert(result.id, addr);
            }

            InstructionKind::Load { result, ptr } => {
                let ptr_val = get_value(ptr)?;
                let result_val = builder.ins().load(types::I64, MemFlags::new(), ptr_val, 0);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Store { ptr, value } => {
                let ptr_val = get_value(ptr)?;
                let value_val = get_value(value)?;
                builder.ins().store(MemFlags::new(), value_val, ptr_val, 0);
            }

            InstructionKind::GetElementPtr {
                result,
                ptr,
                index,
                element_type,
            } => {
                // Check if ptr refers to a stack slot that needs regeneration
                let ptr_val = if let Some(&stack_slot) = stack_slot_map.get(&ptr.id) {
                    // Regenerate stack_addr in this block (solves SSA dominance issue)
                    builder.ins().stack_addr(types::I64, stack_slot, 0)
                } else {
                    get_value(ptr)?
                };
                
                let index_val = get_value(index)?;
                
                // Calcular o tamanho do elemento em bytes
                let elem_size = match element_type {
                    IRType::Int | IRType::Float => 8,
                    IRType::Bool | IRType::Char => 1,
                    _ => 8, // default
                };
                
                // offset = index * elem_size
                let elem_size_val = builder.ins().iconst(types::I64, elem_size);
                let offset = builder.ins().imul(index_val, elem_size_val);
                
                // ptr + offset
                let result_val = builder.ins().iadd(ptr_val, offset);
                value_map.insert(result.id, result_val);
            }

            // Function call
            InstructionKind::Call {
                result,
                function,
                args,
            } => {
                let func_id = *function_map
                    .get(function)
                    .ok_or_else(|| format!("Function '{}' not found", function))?;

                let func_ref = module.declare_func_in_func(func_id, builder.func);

                let arg_values: Result<Vec<_>, _> = args.iter().map(|arg| get_value(arg)).collect();
                let arg_values = arg_values?;

                let call = builder.ins().call(func_ref, &arg_values);

                if let Some(result) = result {
                    let results = builder.inst_results(call);
                    if !results.is_empty() {
                        value_map.insert(result.id, results[0]);
                    }
                }
            }

            // Copy operation
            InstructionKind::Copy { result, source } => {
                let source_val = get_value(source)?;
                value_map.insert(result.id, source_val);
            }

            // PHI nodes are handled during SSA construction
            InstructionKind::Phi { .. } => {
                // PHI nodes should be resolved during SSA construction
                // For now, we skip them in code generation
            }

            // Constant instructions
            InstructionKind::ConstInt { result, value } => {
                let result_val = builder.ins().iconst(types::I64, *value);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::ConstFloat { result, value } => {
                let result_val = builder.ins().f64const(*value);
                value_map.insert(result.id, result_val);
            }

            InstructionKind::ConstBool { result, value } => {
                let result_val = builder.ins().iconst(types::I8, if *value { 1 } else { 0 });
                value_map.insert(result.id, result_val);
            }
        }

        Ok(())
    }

    /// Generate terminator instruction
    fn generate_terminator_static(
        builder: &mut FunctionBuilder,
        terminator: &Terminator,
        value_map: &HashMap<usize, Value>,
        block_map: &HashMap<usize, Block>,
    ) -> Result<(), String> {
        // Helper to get value from map
        let get_value = |v: &IRValue| -> Result<Value, String> {
            value_map
                .get(&v.id)
                .copied()
                .ok_or_else(|| format!("Value {} not found", v.id))
        };

        match terminator {
            Terminator::Unreachable => {
                builder
                    .ins()
                    .trap(cranelift::codegen::ir::TrapCode::UnreachableCodeReached);
            }

            Terminator::Return { value } => {
                if let Some(val) = value {
                    let return_val = get_value(val)?;
                    builder.ins().return_(&[return_val]);
                } else {
                    builder.ins().return_(&[]);
                }
            }

            Terminator::Branch { target } => {
                let target_block = *block_map
                    .get(target)
                    .ok_or_else(|| format!("Block {} not found", target))?;
                builder.ins().jump(target_block, &[]);
            }

            Terminator::CondBranch {
                condition,
                true_block,
                false_block,
            } => {
                let cond_val = get_value(condition)?;
                let true_bb = *block_map
                    .get(true_block)
                    .ok_or_else(|| format!("Block {} not found", true_block))?;
                let false_bb = *block_map
                    .get(false_block)
                    .ok_or_else(|| format!("Block {} not found", false_block))?;
                builder.ins().brif(cond_val, true_bb, &[], false_bb, &[]);
            }

            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                let switch_val = get_value(value)?;
                let default_bb = *block_map
                    .get(default)
                    .ok_or_else(|| format!("Block {} not found", default))?;

                // Create switch using series of conditional branches
                for (idx, (case_val, target)) in cases.iter().enumerate() {
                    let target_bb = *block_map
                        .get(target)
                        .ok_or_else(|| format!("Block {} not found", target))?;

                    let case_const = builder.ins().iconst(types::I64, *case_val);
                    let cmp = builder.ins().icmp(IntCC::Equal, switch_val, case_const);

                    if idx < cases.len() - 1 {
                        let next_check = builder.create_block();
                        builder.ins().brif(cmp, target_bb, &[], next_check, &[]);
                        builder.seal_block(next_check);
                        builder.switch_to_block(next_check);
                    } else {
                        builder.ins().brif(cmp, target_bb, &[], default_bb, &[]);
                    }
                }

                if cases.is_empty() {
                    builder.ins().jump(default_bb, &[]);
                }
            }
        }

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
            IRType::Array { .. } => Ok(types::I64), // Arrays são representados como ponteiros
            IRType::Tuple { .. } => Ok(types::I64), // Tuples são representadas como ponteiros
            IRType::Struct { .. } => Ok(types::I64), // Structs são representados como ponteiros
            IRType::Function { .. } => Ok(types::I64),
        }
    }
    
    /// Get size in bytes of an IR type
    fn type_size_bytes(ty: &IRType) -> usize {
        match ty {
            IRType::Void => 0,
            IRType::Bool => 1,
            IRType::Char => 4,
            IRType::Int => 8,
            IRType::Float => 8,
            IRType::String => 8,
            IRType::Pointer(_) => 8,
            IRType::Array { element_type, size } => {
                Self::type_size_bytes(element_type) * size
            }
            IRType::Tuple { elements } => {
                // Soma dos tamanhos de cada elemento (sem padding por enquanto)
                elements.iter().map(|elem_ty| Self::type_size_bytes(elem_ty)).sum()
            }
            IRType::Struct { fields, .. } => {
                // Soma dos tamanhos de cada campo (sem padding por enquanto)
                fields.iter().map(|(_, field_ty)| Self::type_size_bytes(field_ty)).sum()
            }
            IRType::Function { .. } => 8,
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

    #[test]
    fn test_arithmetic_instructions() {
        use spectra_midend::ir::{BasicBlock, InstructionKind, Terminator, Value};

        let mut codegen = CodeGenerator::new();

        // Create function: fn add(a: int, b: int) -> int { return a + b; }
        let mut func = IRFunction::new(
            "add",
            vec![
                Parameter {
                    id: 0,
                    name: "a".to_string(),
                    ty: IRType::Int,
                },
                Parameter {
                    id: 1,
                    name: "b".to_string(),
                    ty: IRType::Int,
                },
            ],
            IRType::Int,
        );

        // Create entry block
        let entry_block_id = func.add_block("entry");
        let entry_block = func.get_block_mut(entry_block_id).unwrap();

        // Add instruction: result = a + b
        let result_value = Value { id: 2 };
        entry_block.add_instruction(InstructionKind::Add {
            result: result_value,
            lhs: Value { id: 0 }, // a
            rhs: Value { id: 1 }, // b
        });

        // Return instruction
        entry_block.set_terminator(Terminator::Return {
            value: Some(result_value),
        });

        // Generate code
        let result = codegen.declare_function(&func);
        assert!(result.is_ok());

        let result = codegen.define_function(&func);
        assert!(result.is_ok());
    }

    #[test]
    fn test_comparison_instructions() {
        use spectra_midend::ir::{BasicBlock, InstructionKind, Terminator, Value};

        let mut codegen = CodeGenerator::new();

        // Create function: fn is_greater(a: int, b: int) -> bool { return a > b; }
        let mut func = IRFunction::new(
            "is_greater",
            vec![
                Parameter {
                    id: 0,
                    name: "a".to_string(),
                    ty: IRType::Int,
                },
                Parameter {
                    id: 1,
                    name: "b".to_string(),
                    ty: IRType::Int,
                },
            ],
            IRType::Bool,
        );

        // Create entry block
        let entry_block_id = func.add_block("entry");
        let entry_block = func.get_block_mut(entry_block_id).unwrap();

        // Comparison: result = a > b
        let result_value = Value { id: 2 };
        entry_block.add_instruction(InstructionKind::Gt {
            result: result_value,
            lhs: Value { id: 0 },
            rhs: Value { id: 1 },
        });

        // Return
        entry_block.set_terminator(Terminator::Return {
            value: Some(result_value),
        });

        // Generate code
        assert!(codegen.declare_function(&func).is_ok());
        assert!(codegen.define_function(&func).is_ok());
    }

    #[test]
    fn test_logical_instructions() {
        use spectra_midend::ir::{BasicBlock, InstructionKind, Terminator, Value};

        let mut codegen = CodeGenerator::new();

        // Create function: fn and_op(a: bool, b: bool) -> bool { return a && b; }
        let mut func = IRFunction::new(
            "and_op",
            vec![
                Parameter {
                    id: 0,
                    name: "a".to_string(),
                    ty: IRType::Bool,
                },
                Parameter {
                    id: 1,
                    name: "b".to_string(),
                    ty: IRType::Bool,
                },
            ],
            IRType::Bool,
        );

        // Create entry block
        let entry_block_id = func.add_block("entry");
        let entry_block = func.get_block_mut(entry_block_id).unwrap();

        // Logical AND: result = a && b
        let result_value = Value { id: 2 };
        entry_block.add_instruction(InstructionKind::And {
            result: result_value,
            lhs: Value { id: 0 },
            rhs: Value { id: 1 },
        });

        // Return
        entry_block.set_terminator(Terminator::Return {
            value: Some(result_value),
        });

        // Generate code
        assert!(codegen.declare_function(&func).is_ok());
        assert!(codegen.define_function(&func).is_ok());
    }
}
