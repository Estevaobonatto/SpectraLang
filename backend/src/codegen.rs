// Code generation using Cranelift JIT
// Translates Spectra IR to native machine code

use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataId, FuncId, Linkage, Module};
use spectra_midend::ir::{
    BasicBlock as IRBasicBlock, Function as IRFunction, Instruction, InstructionKind,
    Module as IRModule, Terminator, Type as IRType, Value as IRValue,
};
use std::collections::{HashMap, HashSet};

use crate::error::{BackendCodegenError, BackendResult};

#[derive(Clone, Copy)]
pub(crate) struct HostNameRecord {
    pub(crate) ptr: u64,
    pub(crate) len: usize,
    /// When `Some`, the name resides in a Cranelift data section (AOT mode).
    /// When `None`, `ptr` is a compile-time heap pointer valid in JIT mode.
    pub(crate) data_id: Option<DataId>,
}

pub(crate) fn intern_host_name(
    host_name_data: &mut HashMap<String, HostNameRecord>,
    host_name_storage: &mut Vec<Box<[u8]>>,
    name: &str,
) -> HostNameRecord {
    if let Some(record) = host_name_data.get(name) {
        return *record;
    }

    let boxed = name.as_bytes().to_vec().into_boxed_slice();
    let ptr = boxed.as_ptr() as u64;
    let len = boxed.len();
    host_name_storage.push(boxed);

    let record = HostNameRecord {
        ptr,
        len,
        data_id: None,
    };
    host_name_data.insert(name.to_string(), record);
    record
}

pub struct CodeGenerator {
    /// Cranelift JIT module
    module: JITModule,
    /// Function builder context
    ctx: codegen::Context,
    /// Builder for creating IR
    builder_context: FunctionBuilderContext,
    /// Mapping from IR function names to Cranelift function IDs
    function_map: HashMap<String, FuncId>,
    /// Import for runtime-backed manual allocation
    manual_alloc_func: FuncId,
    /// Import for freeing manual allocations
    manual_free_func: FuncId,
    /// Import for starting a manual allocation frame
    manual_frame_enter_func: FuncId,
    /// Import for ending a manual allocation frame
    manual_frame_exit_func: FuncId,
    /// Import for escaping a struct allocation to the parent frame on return
    manual_escape_func: FuncId,
    /// Import for invoking host functions by name
    host_invoke_func: FuncId,
    /// Import for fast-ABI `concurrent.task_spawn`
    concurrent_spawn_fast_func: FuncId,
    /// Import for fast-ABI `concurrent.task_join`
    concurrent_join_fast_func: FuncId,
    /// Import for fast-ABI `str.builder_new`
    builder_new_fast_func: FuncId,
    /// Import for fast-ABI `str.builder_push`
    builder_push_fast_func: FuncId,
    /// Import for fast-ABI `str.builder_len`
    builder_len_fast_func: FuncId,
    /// Import for fast-ABI `str.builder_finish`
    builder_finish_fast_func: FuncId,
    /// Import for fast-ABI `str.builder_free`
    builder_free_fast_func: FuncId,
    /// Cached host name pointers and backing storage
    host_name_data: HashMap<String, HostNameRecord>,
    host_name_storage: Vec<Box<[u8]>>,
}

/// Describes a PHI node so that the backend can emit Cranelift block parameters.
#[derive(Debug, Clone)]
pub(crate) struct PhiDescriptor {
    pub result_id: usize,
    pub incoming: HashMap<usize, usize>, // predecessor_block_id -> incoming_value_id
}

/// Collect the Cranelift block arguments that should be passed for the PHIs
/// in `target_block` when jumping from `current_block`.
fn get_phi_args(
    target_block: usize,
    current_block: usize,
    phi_map: &HashMap<usize, Vec<PhiDescriptor>>,
    value_map: &HashMap<usize, Value>,
) -> BackendResult<Vec<cranelift_codegen::ir::BlockArg>> {
    let mut args = Vec::new();
    if let Some(phis) = phi_map.get(&target_block) {
        for phi in phis {
            let incoming_id = phi.incoming.get(&current_block).ok_or_else(|| {
                BackendCodegenError::missing_phi_incoming(current_block, target_block)
            })?;
            let val = value_map
                .get(incoming_id)
                .copied()
                .ok_or_else(|| BackendCodegenError::missing_value(*incoming_id))?;
            args.push(val.into());
        }
    }
    Ok(args)
}

impl CodeGenerator {
    /// Create a new code generator
    pub fn new() -> Self {
        let mut builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .expect("Failed to create JIT builder");

        builder.symbol(
            "spectra_rt_manual_alloc",
            spectra_runtime::ffi::spectra_rt_manual_alloc as *const u8,
        );
        builder.symbol(
            "spectra_rt_manual_free",
            spectra_runtime::ffi::spectra_rt_manual_free as *const u8,
        );
        builder.symbol(
            "spectra_rt_manual_frame_enter",
            spectra_runtime::ffi::spectra_rt_manual_frame_enter as *const u8,
        );
        builder.symbol(
            "spectra_rt_manual_frame_exit",
            spectra_runtime::ffi::spectra_rt_manual_frame_exit as *const u8,
        );
        builder.symbol(
            "spectra_rt_manual_escape",
            spectra_runtime::ffi::spectra_rt_manual_escape as *const u8,
        );
        builder.symbol(
            "spectra_rt_host_invoke",
            spectra_runtime::ffi::spectra_rt_host_invoke as *const u8,
        );
        builder.symbol(
            "spectra_rt_concurrent_spawn_fast",
            spectra_runtime::ffi::spectra_rt_concurrent_spawn_fast as *const u8,
        );
        builder.symbol(
            "spectra_rt_concurrent_join_fast",
            spectra_runtime::ffi::spectra_rt_concurrent_join_fast as *const u8,
        );
        builder.symbol(
            "spectra_rt_builder_new",
            spectra_runtime::ffi::spectra_rt_builder_new as *const u8,
        );
        builder.symbol(
            "spectra_rt_builder_push",
            spectra_runtime::ffi::spectra_rt_builder_push as *const u8,
        );
        builder.symbol(
            "spectra_rt_builder_len",
            spectra_runtime::ffi::spectra_rt_builder_len as *const u8,
        );
        builder.symbol(
            "spectra_rt_builder_finish",
            spectra_runtime::ffi::spectra_rt_builder_finish as *const u8,
        );
        builder.symbol(
            "spectra_rt_builder_free",
            spectra_runtime::ffi::spectra_rt_builder_free as *const u8,
        );

        let mut module = JITModule::new(builder);
        let ctx = module.make_context();

        let mut alloc_sig = module.make_signature();
        alloc_sig.params.push(AbiParam::new(types::I64));
        alloc_sig.returns.push(AbiParam::new(types::I64));

        let manual_alloc_func = module
            .declare_function("spectra_rt_manual_alloc", Linkage::Import, &alloc_sig)
            .expect("Failed to declare runtime allocation import");

        let mut free_sig = module.make_signature();
        free_sig.params.push(AbiParam::new(types::I64));

        let manual_free_func = module
            .declare_function("spectra_rt_manual_free", Linkage::Import, &free_sig)
            .expect("Failed to declare runtime free import");

        let mut frame_enter_sig = module.make_signature();
        frame_enter_sig.returns.push(AbiParam::new(types::I64));
        let manual_frame_enter_func = module
            .declare_function(
                "spectra_rt_manual_frame_enter",
                Linkage::Import,
                &frame_enter_sig,
            )
            .expect("Failed to declare runtime frame enter import");

        let mut frame_exit_sig = module.make_signature();
        frame_exit_sig.params.push(AbiParam::new(types::I64));
        let manual_frame_exit_func = module
            .declare_function(
                "spectra_rt_manual_frame_exit",
                Linkage::Import,
                &frame_exit_sig,
            )
            .expect("Failed to declare runtime frame exit import");

        // escape(ptr: i64, current_frame_id: i64) — moves struct to parent frame on return
        let mut escape_sig = module.make_signature();
        escape_sig.params.push(AbiParam::new(types::I64));
        escape_sig.params.push(AbiParam::new(types::I64));
        let manual_escape_func = module
            .declare_function("spectra_rt_manual_escape", Linkage::Import, &escape_sig)
            .expect("Failed to declare runtime escape import");

        let mut host_invoke_sig = module.make_signature();
        host_invoke_sig.params.push(AbiParam::new(types::I64));
        host_invoke_sig.params.push(AbiParam::new(types::I64));
        host_invoke_sig.params.push(AbiParam::new(types::I64));
        host_invoke_sig.params.push(AbiParam::new(types::I64));
        host_invoke_sig.params.push(AbiParam::new(types::I64));
        host_invoke_sig.params.push(AbiParam::new(types::I64));
        host_invoke_sig.returns.push(AbiParam::new(types::I32));

        let host_invoke_func = module
            .declare_function("spectra_rt_host_invoke", Linkage::Import, &host_invoke_sig)
            .expect("Failed to declare runtime host invoke import");

        let mut concurrent_spawn_sig = module.make_signature();
        concurrent_spawn_sig.params.push(AbiParam::new(types::I64));
        concurrent_spawn_sig.returns.push(AbiParam::new(types::I64));
        let concurrent_spawn_fast_func = module
            .declare_function(
                "spectra_rt_concurrent_spawn_fast",
                Linkage::Import,
                &concurrent_spawn_sig,
            )
            .expect("Failed to declare concurrent spawn fast import");

        let mut concurrent_join_sig = module.make_signature();
        concurrent_join_sig.params.push(AbiParam::new(types::I64));
        concurrent_join_sig.returns.push(AbiParam::new(types::I64));
        let concurrent_join_fast_func = module
            .declare_function(
                "spectra_rt_concurrent_join_fast",
                Linkage::Import,
                &concurrent_join_sig,
            )
            .expect("Failed to declare concurrent join fast import");

        let mut builder_new_sig = module.make_signature();
        builder_new_sig.params.push(AbiParam::new(types::I64));
        builder_new_sig.returns.push(AbiParam::new(types::I64));
        let builder_new_fast_func = module
            .declare_function("spectra_rt_builder_new", Linkage::Import, &builder_new_sig)
            .expect("Failed to declare builder_new fast import");

        let mut builder_push_sig = module.make_signature();
        builder_push_sig.params.push(AbiParam::new(types::I64));
        builder_push_sig.params.push(AbiParam::new(types::I64));
        let builder_push_fast_func = module
            .declare_function("spectra_rt_builder_push", Linkage::Import, &builder_push_sig)
            .expect("Failed to declare builder_push fast import");

        let mut builder_len_sig = module.make_signature();
        builder_len_sig.params.push(AbiParam::new(types::I64));
        builder_len_sig.returns.push(AbiParam::new(types::I64));
        let builder_len_fast_func = module
            .declare_function("spectra_rt_builder_len", Linkage::Import, &builder_len_sig)
            .expect("Failed to declare builder_len fast import");

        let mut builder_finish_sig = module.make_signature();
        builder_finish_sig.params.push(AbiParam::new(types::I64));
        builder_finish_sig.returns.push(AbiParam::new(types::I64));
        let builder_finish_fast_func = module
            .declare_function("spectra_rt_builder_finish", Linkage::Import, &builder_finish_sig)
            .expect("Failed to declare builder_finish fast import");

        let mut builder_free_sig = module.make_signature();
        builder_free_sig.params.push(AbiParam::new(types::I64));
        let builder_free_fast_func = module
            .declare_function("spectra_rt_builder_free", Linkage::Import, &builder_free_sig)
            .expect("Failed to declare builder_free fast import");

        Self {
            module,
            ctx,
            builder_context: FunctionBuilderContext::new(),
            function_map: HashMap::new(),
            manual_alloc_func,
            manual_free_func,
            manual_frame_enter_func,
            manual_frame_exit_func,
            manual_escape_func,
            host_invoke_func,
            concurrent_spawn_fast_func,
            concurrent_join_fast_func,
            builder_new_fast_func,
            builder_push_fast_func,
            builder_len_fast_func,
            builder_finish_fast_func,
            builder_free_fast_func,
            host_name_data: HashMap::new(),
            host_name_storage: Vec::new(),
        }
    }

    /// Generate code for an entire module
    pub fn generate_module(&mut self, ir_module: &IRModule) -> BackendResult<()> {
        // First pass: declare all functions
        for func in &ir_module.functions {
            self.declare_function(func)?;
        }

        // Second pass: define all functions
        for func in &ir_module.functions {
            self.define_function(func)?;
        }

        // Finalize all functions
        self.module.finalize_definitions().map_err(|e| {
            BackendCodegenError::cranelift(format!("Failed to finalize definitions: {}", e))
        })?;

        Ok(())
    }

    /// Declare a function signature
    fn declare_function(&mut self, ir_func: &IRFunction) -> BackendResult<FuncId> {
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
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to declare function '{}': {}",
                    ir_func.name, e
                ))
            })?;

        self.function_map.insert(ir_func.name.clone(), func_id);

        Ok(func_id)
    }

    /// Define a function body
    fn define_function(&mut self, ir_func: &IRFunction) -> BackendResult<()> {
        let func_id = *self
            .function_map
            .get(&ir_func.name)
            .ok_or_else(|| BackendCodegenError::missing_function(&ir_func.name))?;

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
        let mut allocation_vars: Vec<Variable> = Vec::new();
        let mut stack_array_lengths: HashMap<usize, i64> = HashMap::new();
        let stack_allocas = Self::collect_stack_allocas(ir_func);
        let manual_frame_active = Self::function_needs_manual_frame(ir_func, &stack_allocas);
        let frame_token = if manual_frame_active {
            let frame_enter_ref = self
                .module
                .declare_func_in_func(self.manual_frame_enter_func, builder.func);
            let frame_call = builder.ins().call(frame_enter_ref, &[]);
            builder.inst_results(frame_call)[0]
        } else {
            builder.ins().iconst(types::I64, 0)
        };
        // In cranelift 0.130+, declare_var(Type) -> Variable (no manual index tracking needed)
        let frame_var = builder.declare_var(types::I64);
        builder.def_var(frame_var, frame_token);
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

        // Collect PHI descriptors and add block parameters to Cranelift blocks.
        // Cranelift uses block parameters (not PHI nodes) for SSA values that
        // are defined by predecessor terminators.
        let mut phi_map: HashMap<usize, Vec<PhiDescriptor>> = HashMap::new();
        for ir_block in &ir_func.blocks {
            let mut phis = Vec::new();
            for instr in &ir_block.instructions {
                if let InstructionKind::Phi { result, incoming } = &instr.kind {
                    let mut incoming_map = HashMap::new();
                    for (val, pred_bb) in incoming {
                        incoming_map.insert(*pred_bb, val.id);
                    }
                    phis.push(PhiDescriptor {
                        result_id: result.id,
                        incoming: incoming_map,
                    });
                }
            }
            if !phis.is_empty() {
                phi_map.insert(ir_block.id, phis);
            }
        }

        // Add block parameters for PHI nodes to Cranelift blocks.
        for ir_block in &ir_func.blocks {
            if let Some(phis) = phi_map.get(&ir_block.id) {
                let block = *block_map
                    .get(&ir_block.id)
                    .ok_or_else(|| BackendCodegenError::missing_block(ir_block.id))?;
                for _ in phis {
                    builder.append_block_param(block, types::I64);
                }
            }
        }

        // Generate code for each block
        let blocks = ir_func.blocks.clone();
        for ir_block in &blocks {
            Self::generate_block(
                &mut self.module,
                &self.function_map,
                &mut self.host_name_data,
                &mut self.host_name_storage,
                self.manual_alloc_func,
                self.manual_free_func,
                self.manual_frame_exit_func,
                self.manual_escape_func,
                self.host_invoke_func,
                self.concurrent_spawn_fast_func,
                self.concurrent_join_fast_func,
                self.builder_new_fast_func,
                self.builder_push_fast_func,
                self.builder_len_fast_func,
                self.builder_finish_fast_func,
                self.builder_free_fast_func,
                &mut builder,
                ir_block,
                &mut value_map,
                &block_map,
                &mut allocation_vars,
                &mut stack_array_lengths,
                &stack_allocas,
                frame_var,
                manual_frame_active,
                ir_block.id,
                &phi_map,
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
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to define function '{}': {}",
                    ir_func.name, e
                ))
            })?;

        // Clear context
        self.module.clear_context(&mut self.ctx);

        Ok(())
    }

    pub(crate) fn collect_stack_allocas(ir_func: &IRFunction) -> HashSet<usize> {
        let mut stack_allocas = HashSet::new();
        let mut alloca_types = HashMap::new();
        let mut derived_from_alloca = HashMap::new();
        let mut contained_roots: HashMap<usize, Vec<usize>> = HashMap::new();

        for block in &ir_func.blocks {
            for instruction in &block.instructions {
                if let InstructionKind::Alloca { result, ty } = &instruction.kind {
                    if Self::is_stack_alloca_type(ty) {
                        stack_allocas.insert(result.id);
                        alloca_types.insert(result.id, ty.clone());
                        derived_from_alloca.insert(result.id, result.id);
                    }
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for block in &ir_func.blocks {
                for instruction in &block.instructions {
                    if let InstructionKind::GetElementPtr { result, ptr, .. } = &instruction.kind {
                        if let Some(root) = derived_from_alloca.get(&ptr.id).copied() {
                            if derived_from_alloca.insert(result.id, root).is_none() {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        for block in &ir_func.blocks {
            for instruction in &block.instructions {
                if let InstructionKind::Store { ptr, value } = &instruction.kind {
                    if let Some(value_root) = derived_from_alloca.get(&value.id).copied() {
                        if let Some(ptr_root) = derived_from_alloca.get(&ptr.id).copied() {
                            if ptr_root != value_root {
                                contained_roots
                                    .entry(ptr_root)
                                    .or_default()
                                    .push(value_root);
                            }
                        }
                    }
                }
            }
        }

        for block in &ir_func.blocks {
            if let Some(Terminator::Return { value: Some(value) }) = &block.terminator {
                if let Some(root) = derived_from_alloca.get(&value.id) {
                    stack_allocas.remove(root);
                }
            }

            for instruction in &block.instructions {
                match &instruction.kind {
                    InstructionKind::Store { ptr, value } => {
                        if let Some(root) = derived_from_alloca.get(&value.id) {
                            if derived_from_alloca.get(&ptr.id).is_none() {
                                stack_allocas.remove(root);
                            }
                        }
                    }
                    InstructionKind::HostCall { args, .. }
                    | InstructionKind::CallIndirect { args, .. } => {
                        for arg in args {
                            if let Some(root) = derived_from_alloca.get(&arg.id) {
                                stack_allocas.remove(root);
                            }
                        }
                    }
                    InstructionKind::AsyncReady {
                        value: Some(value), ..
                    } => {
                        if let Some(root) = derived_from_alloca.get(&value.id) {
                            stack_allocas.remove(root);
                        }
                    }
                    InstructionKind::MakeDynFatPtr { data_ptr, .. } => {
                        if let Some(root) = derived_from_alloca.get(&data_ptr.id) {
                            stack_allocas.remove(root);
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for (container, values) in &contained_roots {
                if !stack_allocas.contains(container) {
                    for value_root in values {
                        if stack_allocas.remove(value_root) {
                            changed = true;
                        }
                    }
                }
            }
        }

        stack_allocas
            .into_iter()
            .filter(|id| alloca_types.contains_key(id))
            .collect()
    }

    pub(crate) fn function_needs_manual_frame(
        ir_func: &IRFunction,
        stack_allocas: &HashSet<usize>,
    ) -> bool {
        for block in &ir_func.blocks {
            for instruction in &block.instructions {
                match &instruction.kind {
                    InstructionKind::Alloca { result, .. }
                        if !stack_allocas.contains(&result.id) =>
                    {
                        return true;
                    }
                    InstructionKind::HostCall { .. } => return true,
                    _ => {}
                }
            }
        }
        false
    }

    fn is_stack_alloca_type(ty: &IRType) -> bool {
        match ty {
            IRType::Int | IRType::Float | IRType::Bool | IRType::Char => true,
            IRType::Array { element_type, size } => {
                *size <= 4096 && Self::is_stack_alloca_type(element_type)
            }
            IRType::Struct { fields, .. } => {
                fields.len() <= 64
                    && fields
                        .iter()
                        .all(|(_, field_ty)| Self::is_stack_alloca_type(field_ty))
            }
            _ => false,
        }
    }

    /// Generate code for a basic block
    pub(crate) fn generate_block<M: Module>(
        module: &mut M,
        function_map: &HashMap<String, FuncId>,
        host_name_data: &mut HashMap<String, HostNameRecord>,
        host_name_storage: &mut Vec<Box<[u8]>>,
        manual_alloc_func: FuncId,
        manual_free_func: FuncId,
        manual_frame_exit_func: FuncId,
        manual_escape_func: FuncId,
        host_invoke_func: FuncId,
        concurrent_spawn_fast_func: FuncId,
        concurrent_join_fast_func: FuncId,
        builder_new_fast_func: FuncId,
        builder_push_fast_func: FuncId,
        builder_len_fast_func: FuncId,
        builder_finish_fast_func: FuncId,
        builder_free_fast_func: FuncId,
        builder: &mut FunctionBuilder,
        ir_block: &IRBasicBlock,
        value_map: &mut HashMap<usize, Value>,
        block_map: &HashMap<usize, Block>,
        allocation_vars: &mut Vec<Variable>,
        stack_array_lengths: &mut HashMap<usize, i64>,
        stack_allocas: &HashSet<usize>,
        frame_var: Variable,
        manual_frame_active: bool,
        current_block_id: usize,
        phi_map: &HashMap<usize, Vec<PhiDescriptor>>,
    ) -> BackendResult<()> {
        // Get Cranelift block
        let block = *block_map
            .get(&ir_block.id)
            .ok_or_else(|| BackendCodegenError::missing_block(ir_block.id))?;

        // Switch to block
        if builder.current_block() != Some(block) {
            builder.switch_to_block(block);
        }

        // Generate instructions
        let track_allocations = ir_block.id == 0;
        for instr in &ir_block.instructions {
            Self::generate_instruction(
                module,
                function_map,
                host_name_data,
                host_name_storage,
                manual_alloc_func,
                manual_free_func,
                host_invoke_func,
                concurrent_spawn_fast_func,
                concurrent_join_fast_func,
                builder_new_fast_func,
                builder_push_fast_func,
                builder_len_fast_func,
                builder_finish_fast_func,
                builder_free_fast_func,
                builder,
                instr,
                value_map,
                allocation_vars,
                stack_array_lengths,
                stack_allocas,
                track_allocations,
                ir_block.id,
                block_map,
                phi_map,
            )?;
        }

        // Generate terminator
        if let Some(ref terminator) = ir_block.terminator {
            Self::generate_terminator_static(
                builder,
                terminator,
                value_map,
                block_map,
                module,
                manual_free_func,
                manual_frame_exit_func,
                manual_escape_func,
                frame_var,
                manual_frame_active,
                current_block_id,
                phi_map,
            )?;
        }

        Ok(())
    }

    /// Generate a single instruction
    pub(crate) fn generate_instruction<M: Module>(
        module: &mut M,
        function_map: &HashMap<String, FuncId>,
        host_name_data: &mut HashMap<String, HostNameRecord>,
        host_name_storage: &mut Vec<Box<[u8]>>,
        manual_alloc_func: FuncId,
        manual_free_func: FuncId,
        host_invoke_func: FuncId,
        concurrent_spawn_fast_func: FuncId,
        concurrent_join_fast_func: FuncId,
        builder_new_fast_func: FuncId,
        builder_push_fast_func: FuncId,
        builder_len_fast_func: FuncId,
        builder_finish_fast_func: FuncId,
        builder_free_fast_func: FuncId,
        builder: &mut FunctionBuilder,
        instr: &Instruction,
        value_map: &mut HashMap<usize, Value>,
        allocation_vars: &mut Vec<Variable>,
        stack_array_lengths: &mut HashMap<usize, i64>,
        stack_allocas: &HashSet<usize>,
        track_allocations: bool,
        current_block_id: usize,
        block_map: &HashMap<usize, Block>,
        phi_map: &HashMap<usize, Vec<PhiDescriptor>>,
    ) -> BackendResult<()> {
        // Helper to get value from map
        let get_value = |v: &IRValue| -> BackendResult<Value> {
            value_map
                .get(&v.id)
                .copied()
                .ok_or_else(|| BackendCodegenError::missing_value(v.id))
        };

        match &instr.kind {
            // Arithmetic operations
            InstructionKind::Add { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fadd(lhs_val, rhs_val)
                } else {
                    builder.ins().iadd(lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Sub { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fsub(lhs_val, rhs_val)
                } else {
                    builder.ins().isub(lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Mul { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fmul(lhs_val, rhs_val)
                } else {
                    builder.ins().imul(lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Div { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fdiv(lhs_val, rhs_val)
                } else {
                    builder.ins().sdiv(lhs_val, rhs_val)
                };
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
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fcmp(FloatCC::Equal, lhs_val, rhs_val)
                } else {
                    builder.ins().icmp(IntCC::Equal, lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Ne { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fcmp(FloatCC::NotEqual, lhs_val, rhs_val)
                } else {
                    builder.ins().icmp(IntCC::NotEqual, lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Lt { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fcmp(FloatCC::LessThan, lhs_val, rhs_val)
                } else {
                    builder.ins().icmp(IntCC::SignedLessThan, lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Le { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder
                        .ins()
                        .fcmp(FloatCC::LessThanOrEqual, lhs_val, rhs_val)
                } else {
                    builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Gt { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder.ins().fcmp(FloatCC::GreaterThan, lhs_val, rhs_val)
                } else {
                    builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThan, lhs_val, rhs_val)
                };
                value_map.insert(result.id, result_val);
            }

            InstructionKind::Ge { result, lhs, rhs } => {
                let lhs_val = get_value(lhs)?;
                let rhs_val = get_value(rhs)?;
                let result_val = if builder.func.dfg.value_type(lhs_val) == types::F64 {
                    builder
                        .ins()
                        .fcmp(FloatCC::GreaterThanOrEqual, lhs_val, rhs_val)
                } else {
                    builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, lhs_val, rhs_val)
                };
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
                let result_val = builder.ins().icmp_imm(IntCC::Equal, operand_val, 0);
                value_map.insert(result.id, result_val);
            }

            // Memory operations
            InstructionKind::Alloca { result, ty } => {
                let size_bytes = Self::type_size_bytes(ty) as i64;
                if stack_allocas.contains(&result.id) {
                    let slot =
                        builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                            size_bytes as u32,
                            0,
                        ));
                    let ptr = builder.ins().stack_addr(types::I64, slot, 0);
                    value_map.insert(result.id, ptr);
                    if let IRType::Array { element_type, size } = ty {
                        if matches!(**element_type, IRType::Int | IRType::Char) {
                            stack_array_lengths.insert(result.id, *size as i64);
                        }
                    }
                    return Ok(());
                }

                let size_value = builder.ins().iconst(types::I64, size_bytes);
                let func_ref = module.declare_func_in_func(manual_alloc_func, builder.func);
                let call = builder.ins().call(func_ref, &[size_value]);
                let results = builder.inst_results(call);
                if let Some(&ptr) = results.first() {
                    value_map.insert(result.id, ptr);

                    if track_allocations {
                        // cranelift 0.130+: declare_var returns a Variable
                        let var = builder.declare_var(types::I64);
                        builder.def_var(var, ptr);
                        allocation_vars.push(var);
                    }
                } else {
                    return Err(BackendCodegenError::invalid_ir(
                        "runtime allocation did not return a pointer",
                    ));
                }
            }

            InstructionKind::Load { result, ptr, ty } => {
                let ptr_val = get_value(ptr)?;
                let cranelift_ty = Self::ir_type_to_cranelift(ty)?;
                let result_val = builder
                    .ins()
                    .load(cranelift_ty, MemFlags::new(), ptr_val, 0);
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
                let ptr_val = get_value(ptr)?;

                let index_val = get_value(index)?;

                // Calcular o tamanho do elemento em bytes
                let elem_size = match element_type {
                    IRType::Int | IRType::Float => 8,
                    IRType::Bool => 1,
                    IRType::Char => 4,
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
                    .ok_or_else(|| BackendCodegenError::missing_function(function))?;

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
            InstructionKind::HostCall {
                result,
                host,
                args,
                result_type,
            } => {
                if host == "spectra.std.string.len" && args.len() == 1 {
                    let ptr = get_value(&args[0])?;
                    if let Some(result_value) = result {
                        let value = Self::emit_string_len_inline(builder, ptr);
                        value_map.insert(result_value.id, value);
                    }
                    return Ok(());
                }

                if host == "spectra.std.string.char_at" && args.len() == 2 {
                    let ptr = get_value(&args[0])?;
                    let index = get_value(&args[1])?;
                    if let Some(result_value) = result {
                        let value = if let Some(length) = stack_array_lengths.get(&args[0].id) {
                            Self::emit_stack_string_char_at_inline(builder, ptr, index, *length)
                        } else {
                            Self::emit_string_char_at_inline(builder, ptr, index)
                        };
                        value_map.insert(result_value.id, value);
                    }
                    return Ok(());
                }

                if host == "spectra.std.concurrent.task_spawn" && args.len() == 1 {
                    let value = get_value(&args[0])?;
                    let func_ref = module
                        .declare_func_in_func(concurrent_spawn_fast_func, builder.func);
                    let call = builder.ins().call(func_ref, &[value]);
                    let results = builder.inst_results(call);
                    if let Some(result_value) = result {
                        if let Some(ret) = results.first() {
                            value_map.insert(result_value.id, *ret);
                        }
                    }
                    return Ok(());
                }

                if host == "spectra.std.concurrent.task_join" && args.len() == 1 {
                    let task_id = get_value(&args[0])?;
                    let func_ref = module
                        .declare_func_in_func(concurrent_join_fast_func, builder.func);
                    let call = builder.ins().call(func_ref, &[task_id]);
                    let results = builder.inst_results(call);
                    if let Some(result_value) = result {
                        if let Some(ret) = results.first() {
                            value_map.insert(result_value.id, *ret);
                        }
                    }
                    return Ok(());
                }

                if host == "spectra.std.string.builder_new" && args.len() == 1 {
                    let capacity = get_value(&args[0])?;
                    let func_ref =
                        module.declare_func_in_func(builder_new_fast_func, builder.func);
                    let call = builder.ins().call(func_ref, &[capacity]);
                    let results = builder.inst_results(call);
                    if let Some(result_value) = result {
                        if let Some(ret) = results.first() {
                            value_map.insert(result_value.id, *ret);
                        }
                    }
                    return Ok(());
                }

                if host == "spectra.std.string.builder_push" && args.len() == 2 {
                    let handle = get_value(&args[0])?;
                    let str_ptr = get_value(&args[1])?;
                    let func_ref =
                        module.declare_func_in_func(builder_push_fast_func, builder.func);
                    builder.ins().call(func_ref, &[handle, str_ptr]);
                    return Ok(());
                }

                if host == "spectra.std.string.builder_len" && args.len() == 1 {
                    let handle = get_value(&args[0])?;
                    let func_ref =
                        module.declare_func_in_func(builder_len_fast_func, builder.func);
                    let call = builder.ins().call(func_ref, &[handle]);
                    let results = builder.inst_results(call);
                    if let Some(result_value) = result {
                        if let Some(ret) = results.first() {
                            value_map.insert(result_value.id, *ret);
                        }
                    }
                    return Ok(());
                }

                if host == "spectra.std.string.builder_finish" && args.len() == 1 {
                    let handle = get_value(&args[0])?;
                    let func_ref =
                        module.declare_func_in_func(builder_finish_fast_func, builder.func);
                    let call = builder.ins().call(func_ref, &[handle]);
                    let results = builder.inst_results(call);
                    if let Some(result_value) = result {
                        if let Some(ret) = results.first() {
                            value_map.insert(result_value.id, *ret);
                        }
                    }
                    return Ok(());
                }

                if host == "spectra.std.string.builder_free" && args.len() == 1 {
                    let handle = get_value(&args[0])?;
                    let func_ref =
                        module.declare_func_in_func(builder_free_fast_func, builder.func);
                    builder.ins().call(func_ref, &[handle]);
                    return Ok(());
                }

                let record = intern_host_name(host_name_data, host_name_storage, host);
                let name_ptr = if let Some(data_id) = record.data_id {
                    // AOT mode: the name lives in a .rodata section; get its address
                    // via a GlobalValue so the linker patches it correctly.
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    builder.ins().global_value(types::I64, gv)
                } else {
                    // JIT mode: the name is a heap-allocated byte slice that lives
                    // for the duration of the code generator — safe to embed as an
                    // immediate pointer.
                    builder.ins().iconst(types::I64, record.ptr as i64)
                };
                let name_len = builder.ins().iconst(types::I64, record.len as i64);

                let (args_ptr, args_count, args_allocation) = if args.is_empty() {
                    (
                        builder.ins().iconst(types::I64, 0),
                        builder.ins().iconst(types::I64, 0),
                        None,
                    )
                } else {
                    let size = builder.ins().iconst(types::I64, (args.len() as i64) * 8);
                    let alloc_ref = module.declare_func_in_func(manual_alloc_func, builder.func);
                    let call = builder.ins().call(alloc_ref, &[size]);
                    let ptr = builder.inst_results(call)[0];

                    for (idx, arg) in args.iter().enumerate() {
                        let mut value = get_value(arg)?;
                        let ty = builder.func.dfg.value_type(value);
                        value = match ty {
                            types::I64 => value,
                            types::I8 | types::I16 | types::I32 => {
                                builder.ins().sextend(types::I64, value)
                            }
                            types::F64 => {
                                // Reinterpret float bits as i64 so the runtime
                                // can receive and convert them.
                                builder.ins().bitcast(types::I64, MemFlags::new(), value)
                            }
                            other => {
                                return Err(BackendCodegenError::unsupported_host_argument_type(
                                    other,
                                ))
                            }
                        };
                        let offset = (idx as i32) * 8;
                        builder.ins().store(MemFlags::new(), value, ptr, offset);
                    }

                    let count = builder.ins().iconst(types::I64, args.len() as i64);
                    (ptr, count, Some(ptr))
                };

                let (results_ptr, result_len_val, result_allocation) = if result.is_some() {
                    let size = builder.ins().iconst(types::I64, 8);
                    let alloc_ref = module.declare_func_in_func(manual_alloc_func, builder.func);
                    let call = builder.ins().call(alloc_ref, &[size]);
                    let ptr = builder.inst_results(call)[0];
                    let len = builder.ins().iconst(types::I64, 1);
                    (ptr, len, Some(ptr))
                } else {
                    (
                        builder.ins().iconst(types::I64, 0),
                        builder.ins().iconst(types::I64, 0),
                        None,
                    )
                };

                let func_ref = module.declare_func_in_func(host_invoke_func, builder.func);
                let call = builder.ins().call(
                    func_ref,
                    &[
                        name_ptr,
                        name_len,
                        args_ptr,
                        args_count,
                        results_ptr,
                        result_len_val,
                    ],
                );
                let status = builder.inst_results(call)[0];

                let zero = builder.ins().iconst(types::I32, 0);
                let is_ok = builder.ins().icmp(IntCC::Equal, status, zero);
                let success_block = builder.create_block();
                let failure_block = builder.create_block();
                builder
                    .ins()
                    .brif(is_ok, success_block, &[], failure_block, &[]);

                builder.switch_to_block(failure_block);
                if let Some(ptr) = result_allocation {
                    let free_ref = module.declare_func_in_func(manual_free_func, builder.func);
                    builder.ins().call(free_ref, &[ptr]);
                }
                if let Some(ptr) = args_allocation {
                    let free_ref = module.declare_func_in_func(manual_free_func, builder.func);
                    builder.ins().call(free_ref, &[ptr]);
                }
                builder
                    .ins()
                    .trap(cranelift::codegen::ir::TrapCode::user(1).unwrap());
                builder.seal_block(failure_block);

                builder.switch_to_block(success_block);
                builder.seal_block(success_block);

                if let (Some(result_value), Some(ptr)) = (result, result_allocation) {
                    let raw_value = builder.ins().load(types::I64, MemFlags::new(), ptr, 0);
                    let value = match result_type {
                        Some(IRType::Float) => {
                            builder
                                .ins()
                                .bitcast(types::F64, MemFlags::new(), raw_value)
                        }
                        Some(IRType::Bool) => builder.ins().ireduce(types::I8, raw_value),
                        Some(IRType::Char) => builder.ins().ireduce(types::I32, raw_value),
                        _ => raw_value,
                    };
                    value_map.insert(result_value.id, value);

                    let free_ref = module.declare_func_in_func(manual_free_func, builder.func);
                    builder.ins().call(free_ref, &[ptr]);
                }

                if let Some(ptr) = args_allocation {
                    let free_ref = module.declare_func_in_func(manual_free_func, builder.func);
                    builder.ins().call(free_ref, &[ptr]);
                }
            }
            InstructionKind::AsyncSuspend { .. } | InstructionKind::AsyncResume { .. } => {}
            InstructionKind::AsyncReady {
                result,
                value,
                output_type,
            } => {
                let raw = if let Some(value) = value {
                    get_value(value)?
                } else {
                    builder.ins().iconst(types::I64, 0)
                };
                let value = match output_type {
                    IRType::Float => {
                        if builder.func.dfg.value_type(raw) == types::F64 {
                            builder.ins().bitcast(types::I64, MemFlags::new(), raw)
                        } else {
                            raw
                        }
                    }
                    IRType::Bool | IRType::Char => {
                        let ty = builder.func.dfg.value_type(raw);
                        if ty == types::I64 {
                            raw
                        } else {
                            builder.ins().uextend(types::I64, raw)
                        }
                    }
                    _ => raw,
                };
                value_map.insert(result.id, value);
            }

            InstructionKind::FuncAddr { result, function } => {
                let func_id = *function_map
                    .get(function)
                    .ok_or_else(|| BackendCodegenError::missing_function(function))?;

                let func_ref = module.declare_func_in_func(func_id, builder.func);
                let func_addr = builder
                    .ins()
                    .func_addr(module.target_config().pointer_type(), func_ref);
                value_map.insert(result.id, func_addr);
            }

            InstructionKind::CallIndirect {
                result,
                fn_ptr,
                args,
                signature_params,
                signature_return,
            } => {
                let ptr_val = get_value(fn_ptr)?;

                let mut sig = module.make_signature();
                for param_ty in signature_params {
                    let cl_type = Self::ir_type_to_cranelift(param_ty)?;
                    sig.params.push(AbiParam::new(cl_type));
                }

                let ret_ty = Self::ir_type_to_cranelift(signature_return)?;
                if ret_ty != types::I8 || **signature_return != IRType::Void {
                    sig.returns.push(AbiParam::new(ret_ty));
                }

                let sig_ref = builder.import_signature(sig);

                let arg_values: Result<Vec<_>, _> = args.iter().map(|arg| get_value(arg)).collect();
                let arg_values = arg_values?;

                let call = builder.ins().call_indirect(sig_ref, ptr_val, &arg_values);

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

            // Cast instruction
            InstructionKind::Cast {
                result,
                operand,
                from_ty,
                to_ty,
            } => {
                let operand_val = get_value(operand)?;
                let operand_cl_ty = builder.func.dfg.value_type(operand_val);
                let cl_val = match (from_ty, to_ty) {
                    (IRType::Int, IRType::Float) => {
                        builder.ins().fcvt_from_sint(types::F64, operand_val)
                    }
                    (IRType::Float, IRType::Int) => {
                        builder.ins().fcvt_to_sint_sat(types::I64, operand_val)
                    }
                    (IRType::Char, IRType::Int) => match operand_cl_ty {
                        types::I8 | types::I16 | types::I32 => {
                            builder.ins().uextend(types::I64, operand_val)
                        }
                        types::I64 => operand_val,
                        _ => operand_val,
                    },
                    (IRType::Int, IRType::Char) => match operand_cl_ty {
                        types::I64 => builder.ins().ireduce(types::I32, operand_val),
                        types::I32 => operand_val,
                        types::I8 | types::I16 => builder.ins().uextend(types::I32, operand_val),
                        _ => operand_val,
                    },
                    _ => operand_val, // same-type or struct->dyn: pass through
                };
                value_map.insert(result.id, cl_val);
            }

            // dyn Trait fat pointer operations
            InstructionKind::MakeDynFatPtr {
                result,
                data_ptr,
                vtable_ptr,
            } => {
                // Allocate 16 bytes on stack: [data_ptr i64, vtable_ptr i64]
                let slot =
                    builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                        cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                        16,
                        0,
                    ));
                let data_val = get_value(data_ptr)?;
                let vtable_val = get_value(vtable_ptr)?;
                builder.ins().stack_store(data_val, slot, 0);
                builder.ins().stack_store(vtable_val, slot, 8);
                let fat_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                value_map.insert(result.id, fat_ptr);
            }

            InstructionKind::LoadDynDataPtr { result, fat_ptr } => {
                let ptr_val = get_value(fat_ptr)?;
                let data = builder.ins().load(
                    types::I64,
                    cranelift_codegen::ir::MemFlags::new(),
                    ptr_val,
                    0,
                );
                value_map.insert(result.id, data);
            }

            InstructionKind::LoadDynVtablePtr { result, fat_ptr } => {
                let ptr_val = get_value(fat_ptr)?;
                let vtable = builder.ins().load(
                    types::I64,
                    cranelift_codegen::ir::MemFlags::new(),
                    ptr_val,
                    8,
                );
                value_map.insert(result.id, vtable);
            }

            InstructionKind::LoadVtableSlot {
                result,
                vtable_ptr,
                slot_index,
            } => {
                let vptr_val = get_value(vtable_ptr)?;
                let offset = (*slot_index as i32) * 8;
                let fn_ptr = builder.ins().load(
                    types::I64,
                    cranelift_codegen::ir::MemFlags::new(),
                    vptr_val,
                    offset,
                );
                value_map.insert(result.id, fn_ptr);
            }

            // PHI nodes are lowered to Cranelift block parameters.
            // The block was already created with the appropriate parameters in
            // define_function(). Here we simply read the parameter that
            // corresponds to this PHI and expose it in the value_map.
            InstructionKind::Phi { result, .. } => {
                let block = *block_map
                    .get(&current_block_id)
                    .ok_or_else(|| BackendCodegenError::missing_block(current_block_id))?;
                if let Some(phis) = phi_map.get(&current_block_id) {
                    for (idx, phi) in phis.iter().enumerate() {
                        if phi.result_id == result.id {
                            let phi_val = builder.block_params(block)[idx];
                            value_map.insert(result.id, phi_val);
                            break;
                        }
                    }
                }
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

    fn emit_string_char_at_inline(
        builder: &mut FunctionBuilder,
        ptr: Value,
        index: Value,
    ) -> Value {
        let cursor_var = builder.declare_var(types::I64);
        let result_var = builder.declare_var(types::I64);
        let zero = builder.ins().iconst(types::I64, 0);
        let missing = builder.ins().iconst(types::I64, -1);
        builder.def_var(cursor_var, zero);
        builder.def_var(result_var, missing);

        let null_check_block = builder.create_block();
        let loop_block = builder.create_block();
        let target_check_block = builder.create_block();
        let found_block = builder.create_block();
        let advance_block = builder.create_block();
        let done_block = builder.create_block();

        let negative = builder.ins().icmp(IntCC::SignedLessThan, index, zero);
        builder
            .ins()
            .brif(negative, done_block, &[], null_check_block, &[]);

        builder.switch_to_block(null_check_block);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder
            .ins()
            .brif(is_null, done_block, &[], loop_block, &[]);
        builder.seal_block(null_check_block);

        builder.switch_to_block(loop_block);
        let cursor = builder.use_var(cursor_var);
        let offset = builder.ins().imul_imm(cursor, 8);
        let addr = builder.ins().iadd(ptr, offset);
        let slot = builder.ins().load(types::I64, MemFlags::new(), addr, 0);
        let is_terminator = builder.ins().icmp(IntCC::Equal, slot, zero);
        builder
            .ins()
            .brif(is_terminator, done_block, &[], target_check_block, &[]);

        builder.switch_to_block(target_check_block);
        let is_target = builder.ins().icmp(IntCC::Equal, cursor, index);
        builder
            .ins()
            .brif(is_target, found_block, &[], advance_block, &[]);
        builder.seal_block(target_check_block);

        builder.switch_to_block(found_block);
        let byte = builder.ins().band_imm(slot, 0xff);
        builder.def_var(result_var, byte);
        builder.ins().jump(done_block, &[]);
        builder.seal_block(found_block);

        builder.switch_to_block(advance_block);
        let next = builder.ins().iadd_imm(cursor, 1);
        builder.def_var(cursor_var, next);
        builder.ins().jump(loop_block, &[]);
        builder.seal_block(advance_block);
        builder.seal_block(loop_block);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        builder.use_var(result_var)
    }

    fn emit_stack_string_char_at_inline(
        builder: &mut FunctionBuilder,
        ptr: Value,
        index: Value,
        allocation_len: i64,
    ) -> Value {
        let result_var = builder.declare_var(types::I64);
        let zero = builder.ins().iconst(types::I64, 0);
        let missing = builder.ins().iconst(types::I64, -1);
        builder.def_var(result_var, missing);

        let bounds_block = builder.create_block();
        let load_block = builder.create_block();
        let value_block = builder.create_block();
        let done_block = builder.create_block();

        let negative = builder.ins().icmp(IntCC::SignedLessThan, index, zero);
        builder
            .ins()
            .brif(negative, done_block, &[], bounds_block, &[]);

        builder.switch_to_block(bounds_block);
        let max_valid_index = builder
            .ins()
            .iconst(types::I64, allocation_len.saturating_sub(1));
        let out_of_bounds =
            builder
                .ins()
                .icmp(IntCC::SignedGreaterThanOrEqual, index, max_valid_index);
        builder
            .ins()
            .brif(out_of_bounds, done_block, &[], load_block, &[]);
        builder.seal_block(bounds_block);

        builder.switch_to_block(load_block);
        let offset = builder.ins().imul_imm(index, 8);
        let addr = builder.ins().iadd(ptr, offset);
        let slot = builder.ins().load(types::I64, MemFlags::new(), addr, 0);
        let is_terminator = builder.ins().icmp(IntCC::Equal, slot, zero);
        builder
            .ins()
            .brif(is_terminator, done_block, &[], value_block, &[]);
        builder.seal_block(load_block);

        builder.switch_to_block(value_block);
        let byte = builder.ins().band_imm(slot, 0xff);
        builder.def_var(result_var, byte);
        builder.ins().jump(done_block, &[]);
        builder.seal_block(value_block);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        builder.use_var(result_var)
    }

    fn emit_string_len_inline(builder: &mut FunctionBuilder, ptr: Value) -> Value {
        let count_var = builder.declare_var(types::I64);
        let zero = builder.ins().iconst(types::I64, 0);
        builder.def_var(count_var, zero);

        let loop_block = builder.create_block();
        let advance_block = builder.create_block();
        let done_block = builder.create_block();

        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder
            .ins()
            .brif(is_null, done_block, &[], loop_block, &[]);

        builder.switch_to_block(loop_block);
        let count = builder.use_var(count_var);
        let offset = builder.ins().imul_imm(count, 8);
        let addr = builder.ins().iadd(ptr, offset);
        let slot = builder.ins().load(types::I64, MemFlags::new(), addr, 0);
        let is_terminator = builder.ins().icmp(IntCC::Equal, slot, zero);
        builder
            .ins()
            .brif(is_terminator, done_block, &[], advance_block, &[]);

        builder.switch_to_block(advance_block);
        let next = builder.ins().iadd_imm(count, 1);
        builder.def_var(count_var, next);
        builder.ins().jump(loop_block, &[]);
        builder.seal_block(advance_block);
        builder.seal_block(loop_block);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        builder.use_var(count_var)
    }

    /// Generate terminator instruction
    pub(crate) fn generate_terminator_static<M: Module>(
        builder: &mut FunctionBuilder,
        terminator: &Terminator,
        value_map: &HashMap<usize, Value>,
        block_map: &HashMap<usize, Block>,
        module: &mut M,
        manual_free_func: FuncId,
        manual_frame_exit_func: FuncId,
        manual_escape_func: FuncId,
        frame_var: Variable,
        manual_frame_active: bool,
        current_block_id: usize,
        phi_map: &HashMap<usize, Vec<PhiDescriptor>>,
    ) -> BackendResult<()> {
        // Helper to get value from map
        let get_value = |v: &IRValue| -> BackendResult<Value> {
            value_map
                .get(&v.id)
                .copied()
                .ok_or_else(|| BackendCodegenError::missing_value(v.id))
        };

        match terminator {
            Terminator::Unreachable => {
                builder
                    .ins()
                    // TrapCode::UnreachableCodeReached was removed in cranelift 0.114+;
                    // use user(1) as sentinel for explicit unreachable IR
                    .trap(cranelift::codegen::ir::TrapCode::user(1).unwrap());
            }

            Terminator::Return { value } => {
                let mut return_values: Vec<Value> = Vec::new();
                if let Some(val) = value {
                    let return_val = get_value(val)?;
                    return_values.push(return_val);

                    // Escape the return value from the current frame to the parent frame so it
                    // survives frame_exit below. Only applies to pointer-sized values (i64);
                    // booleans/i8 are scalars and the escape call would fail the Cranelift
                    // verifier with a type mismatch.
                    let return_type = builder.func.dfg.value_type(return_val);
                    if manual_frame_active && return_type == cranelift::prelude::types::I64 {
                        let escape_ref =
                            module.declare_func_in_func(manual_escape_func, builder.func);
                        let frame_val_for_escape = builder.use_var(frame_var);
                        builder
                            .ins()
                            .call(escape_ref, &[return_val, frame_val_for_escape]);
                    }
                }

                // Free all locally alloca'd allocations that are still in this frame.
                // Any allocation that was escaped above has already been moved to the parent
                // frame and will therefore not be found by frame_exit, so it is safe to call
                // frame_exit unconditionally for the remaining ones.
                if manual_frame_active {
                    let _ = manual_free_func; // kept for API compat; frame_exit handles cleanup
                    let frame_exit_ref =
                        module.declare_func_in_func(manual_frame_exit_func, builder.func);
                    let frame_val = builder.use_var(frame_var);
                    builder.ins().call(frame_exit_ref, &[frame_val]);
                }

                if return_values.is_empty() {
                    builder.ins().return_(&[]);
                } else {
                    builder.ins().return_(&return_values);
                }
            }

            Terminator::Branch { target } => {
                let target_block = *block_map
                    .get(target)
                    .ok_or_else(|| BackendCodegenError::missing_block(*target))?;
                let phi_args = get_phi_args(*target, current_block_id, phi_map, value_map)?;
                builder.ins().jump(target_block, &phi_args);
            }

            Terminator::CondBranch {
                condition,
                true_block,
                false_block,
            } => {
                let cond_val = get_value(condition)?;
                let true_bb = *block_map
                    .get(true_block)
                    .ok_or_else(|| BackendCodegenError::missing_block(*true_block))?;
                let false_bb = *block_map
                    .get(false_block)
                    .ok_or_else(|| BackendCodegenError::missing_block(*false_block))?;
                let true_args = get_phi_args(*true_block, current_block_id, phi_map, value_map)?;
                let false_args = get_phi_args(*false_block, current_block_id, phi_map, value_map)?;
                builder
                    .ins()
                    .brif(cond_val, true_bb, &true_args, false_bb, &false_args);
            }

            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                let switch_val = get_value(value)?;
                let default_bb = *block_map
                    .get(default)
                    .ok_or_else(|| BackendCodegenError::missing_block(*default))?;
                let default_args = get_phi_args(*default, current_block_id, phi_map, value_map)?;

                // Create switch using series of conditional branches.
                // For intermediate "next_check" blocks we do not need PHI args
                // because they are internal control-flow blocks, not user BBs.
                for (idx, (case_val, target)) in cases.iter().enumerate() {
                    let target_bb = *block_map
                        .get(target)
                        .ok_or_else(|| BackendCodegenError::missing_block(*target))?;

                    let case_const = builder.ins().iconst(types::I64, *case_val);
                    let cmp = builder.ins().icmp(IntCC::Equal, switch_val, case_const);

                    let target_args = get_phi_args(*target, current_block_id, phi_map, value_map)?;

                    if idx < cases.len() - 1 {
                        let next_check = builder.create_block();
                        builder
                            .ins()
                            .brif(cmp, target_bb, &target_args, next_check, &[]);
                        builder.seal_block(next_check);
                        builder.switch_to_block(next_check);
                    } else {
                        builder
                            .ins()
                            .brif(cmp, target_bb, &target_args, default_bb, &default_args);
                    }
                }

                if cases.is_empty() {
                    builder.ins().jump(default_bb, &default_args);
                }
            }
        }

        Ok(())
    }

    /// Convert IR type to Cranelift type
    pub(crate) fn ir_type_to_cranelift(ty: &IRType) -> BackendResult<types::Type> {
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
            IRType::Enum { .. } => Ok(types::I64), // Enums são representados como ponteiros ou ints
            IRType::Function { .. } => Ok(types::I64),
            IRType::Tensor { .. } => Ok(types::I64),
            IRType::Task { .. } => Ok(types::I64),
            IRType::Range => Ok(types::I64),
            IRType::DynTrait { .. } => Ok(types::I64), // fat pointer represented as i64 address
        }
    }

    /// Get size in bytes of an IR type
    pub(crate) fn type_size_bytes(ty: &IRType) -> usize {
        match ty {
            IRType::Void => 0,
            IRType::Bool => 1,
            IRType::Char => 4,
            IRType::Int => 8,
            IRType::Float => 8,
            IRType::String => 8,
            IRType::Pointer(_) => 8,
            IRType::Task { .. } => 8,
            IRType::Range => 8,
            IRType::Array { element_type, size } => Self::type_size_bytes(element_type) * size,
            IRType::Tuple { elements } => {
                // Soma dos tamanhos de cada elemento (sem padding por enquanto)
                elements
                    .iter()
                    .map(|elem_ty| Self::type_size_bytes(elem_ty))
                    .sum()
            }
            IRType::Struct { fields, .. } => {
                // Soma dos tamanhos de cada campo (sem padding por enquanto)
                fields
                    .iter()
                    .map(|(_, field_ty)| Self::type_size_bytes(field_ty))
                    .sum()
            }
            IRType::Enum { variants, .. } => {
                // Tamanho máximo entre todos os variants (tag + max data)
                let max_variant_size = variants
                    .iter()
                    .map(|(_, data_types)| {
                        if let Some(types) = data_types {
                            8 + types
                                .iter()
                                .map(|ty| Self::type_size_bytes(ty))
                                .sum::<usize>()
                        } else {
                            8 // Apenas o tag
                        }
                    })
                    .max()
                    .unwrap_or(8);
                max_variant_size
            }
            IRType::Function { .. } => 8,
            IRType::Tensor { .. } => 8,
            IRType::DynTrait { .. } => 16, // fat pointer: data_ptr (8) + vtable_ptr (8)
        }
    }

    /// Get pointer to a compiled function
    pub fn get_function_ptr(&mut self, name: &str) -> BackendResult<*const u8> {
        let func_id = self
            .function_map
            .get(name)
            .ok_or_else(|| BackendCodegenError::missing_function(name))?;

        Ok(self.module.get_finalized_function(*func_id))
    }

    /// Execute an entry point with signature `fn() -> int` or `fn() -> void`.
    pub unsafe fn execute_entry_point(
        &mut self,
        name: &str,
        ir_module: &IRModule,
    ) -> BackendResult<Option<i64>> {
        let ptr = self.get_function_ptr(name)?;

        let return_type = ir_module
            .get_function(name)
            .map(|f| f.return_type.clone())
            .unwrap_or(IRType::Void);

        match return_type {
            IRType::Void => {
                let func: extern "C" fn() = std::mem::transmute(ptr);
                func();
                Ok(None)
            }
            IRType::Int => {
                let func: extern "C" fn() -> i64 = std::mem::transmute(ptr);
                Ok(Some(func()))
            }
            IRType::Bool => {
                let func: extern "C" fn() -> i8 = std::mem::transmute(ptr);
                Ok(Some(func() as i64))
            }
            other => Err(BackendCodegenError::unsupported_execution_return_type(
                other,
            )),
        }
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
    use crate::BackendErrorKind;
    use spectra_midend::ir::Parameter;

    #[test]
    fn test_codegen_creation() {
        let codegen = CodeGenerator::new();
        assert!(codegen.function_map.is_empty());
    }

    #[test]
    fn local_scalar_array_allocas_use_native_stack_when_not_escaping() {
        let function = IRFunction {
            name: "stackable".to_string(),
            params: vec![],
            return_type: IRType::Int,
            next_value_id: 5,
            next_block_id: 1,
            blocks: vec![IRBasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        id: 0,
                        kind: InstructionKind::Alloca {
                            result: IRValue { id: 0 },
                            ty: IRType::Array {
                                element_type: Box::new(IRType::Int),
                                size: 4,
                            },
                        },
                    },
                    Instruction {
                        id: 1,
                        kind: InstructionKind::ConstInt {
                            result: IRValue { id: 1 },
                            value: 0,
                        },
                    },
                    Instruction {
                        id: 2,
                        kind: InstructionKind::GetElementPtr {
                            result: IRValue { id: 2 },
                            ptr: IRValue { id: 0 },
                            index: IRValue { id: 1 },
                            element_type: IRType::Int,
                        },
                    },
                    Instruction {
                        id: 3,
                        kind: InstructionKind::ConstInt {
                            result: IRValue { id: 3 },
                            value: 42,
                        },
                    },
                    Instruction {
                        id: 4,
                        kind: InstructionKind::Store {
                            ptr: IRValue { id: 2 },
                            value: IRValue { id: 3 },
                        },
                    },
                ],
                terminator: Some(Terminator::Return {
                    value: Some(IRValue { id: 3 }),
                }),
            }],
        };

        let stack_allocas = CodeGenerator::collect_stack_allocas(&function);
        assert!(stack_allocas.contains(&0));
    }

    #[test]
    fn escaping_alloca_stays_on_manual_runtime_heap() {
        let function = IRFunction {
            name: "escaping".to_string(),
            params: vec![],
            return_type: IRType::Array {
                element_type: Box::new(IRType::Int),
                size: 4,
            },
            next_value_id: 1,
            next_block_id: 1,
            blocks: vec![IRBasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![Instruction {
                    id: 0,
                    kind: InstructionKind::Alloca {
                        result: IRValue { id: 0 },
                        ty: IRType::Array {
                            element_type: Box::new(IRType::Int),
                            size: 4,
                        },
                    },
                }],
                terminator: Some(Terminator::Return {
                    value: Some(IRValue { id: 0 }),
                }),
            }],
        };

        let stack_allocas = CodeGenerator::collect_stack_allocas(&function);
        assert!(!stack_allocas.contains(&0));
    }

    #[test]
    fn struct_contained_stack_alloca_does_not_escape() {
        let array_type = IRType::Array {
            element_type: Box::new(IRType::Int),
            size: 4,
        };
        let holder_type = IRType::Struct {
            name: "Holder".to_string(),
            fields: vec![("items".to_string(), array_type.clone())],
        };
        let function = IRFunction {
            name: "contained".to_string(),
            params: vec![],
            return_type: IRType::Int,
            next_value_id: 5,
            next_block_id: 1,
            blocks: vec![IRBasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        id: 0,
                        kind: InstructionKind::Alloca {
                            result: IRValue { id: 0 },
                            ty: holder_type,
                        },
                    },
                    Instruction {
                        id: 1,
                        kind: InstructionKind::Alloca {
                            result: IRValue { id: 1 },
                            ty: array_type,
                        },
                    },
                    Instruction {
                        id: 2,
                        kind: InstructionKind::ConstInt {
                            result: IRValue { id: 2 },
                            value: 0,
                        },
                    },
                    Instruction {
                        id: 3,
                        kind: InstructionKind::GetElementPtr {
                            result: IRValue { id: 3 },
                            ptr: IRValue { id: 0 },
                            index: IRValue { id: 2 },
                            element_type: IRType::Int,
                        },
                    },
                    Instruction {
                        id: 4,
                        kind: InstructionKind::Store {
                            ptr: IRValue { id: 3 },
                            value: IRValue { id: 1 },
                        },
                    },
                ],
                terminator: Some(Terminator::Return {
                    value: Some(IRValue { id: 2 }),
                }),
            }],
        };

        let stack_allocas = CodeGenerator::collect_stack_allocas(&function);
        assert!(stack_allocas.contains(&0));
        assert!(stack_allocas.contains(&1));
        assert!(!CodeGenerator::function_needs_manual_frame(
            &function,
            &stack_allocas
        ));
    }

    #[test]
    fn escaping_container_forces_contained_alloca_to_escape() {
        let array_type = IRType::Array {
            element_type: Box::new(IRType::Int),
            size: 4,
        };
        let holder_type = IRType::Struct {
            name: "Holder".to_string(),
            fields: vec![("items".to_string(), array_type.clone())],
        };
        let function = IRFunction {
            name: "escaping_container".to_string(),
            params: vec![],
            return_type: IRType::Struct {
                name: "Holder".to_string(),
                fields: vec![("items".to_string(), array_type.clone())],
            },
            next_value_id: 5,
            next_block_id: 1,
            blocks: vec![IRBasicBlock {
                id: 0,
                label: "entry".to_string(),
                instructions: vec![
                    Instruction {
                        id: 0,
                        kind: InstructionKind::Alloca {
                            result: IRValue { id: 0 },
                            ty: holder_type,
                        },
                    },
                    Instruction {
                        id: 1,
                        kind: InstructionKind::Alloca {
                            result: IRValue { id: 1 },
                            ty: array_type,
                        },
                    },
                    Instruction {
                        id: 2,
                        kind: InstructionKind::ConstInt {
                            result: IRValue { id: 2 },
                            value: 0,
                        },
                    },
                    Instruction {
                        id: 3,
                        kind: InstructionKind::GetElementPtr {
                            result: IRValue { id: 3 },
                            ptr: IRValue { id: 0 },
                            index: IRValue { id: 2 },
                            element_type: IRType::Int,
                        },
                    },
                    Instruction {
                        id: 4,
                        kind: InstructionKind::Store {
                            ptr: IRValue { id: 3 },
                            value: IRValue { id: 1 },
                        },
                    },
                ],
                terminator: Some(Terminator::Return {
                    value: Some(IRValue { id: 0 }),
                }),
            }],
        };

        let stack_allocas = CodeGenerator::collect_stack_allocas(&function);
        assert!(!stack_allocas.contains(&0));
        assert!(!stack_allocas.contains(&1));
        assert!(CodeGenerator::function_needs_manual_frame(
            &function,
            &stack_allocas
        ));
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
        assert_eq!(
            CodeGenerator::ir_type_to_cranelift(&IRType::Task {
                output: Box::new(IRType::Int),
            })
            .unwrap(),
            types::I64
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
        use spectra_midend::ir::{InstructionKind, Terminator, Value};

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
    fn r2007_missing_branch_target_returns_typed_error() {
        use spectra_midend::ir::Terminator;

        let mut codegen = CodeGenerator::new();
        let mut func = IRFunction::new("missing_branch_target", vec![], IRType::Void);
        let entry_block_id = func.add_block("entry");
        let entry_block = func.get_block_mut(entry_block_id).unwrap();
        entry_block.set_terminator(Terminator::Branch { target: 999 });

        assert!(codegen.declare_function(&func).is_ok());
        let err = codegen
            .define_function(&func)
            .expect_err("missing target block must be reported, not panic");
        assert_eq!(err.kind(), &BackendErrorKind::MissingBlock);
        assert!(err.message().contains("999"));
    }

    #[test]
    fn r2007_missing_phi_incoming_returns_typed_error() {
        use spectra_midend::ir::{InstructionKind, Terminator, Value};

        let mut codegen = CodeGenerator::new();
        let mut func = IRFunction::new("missing_phi_incoming", vec![], IRType::Int);
        let entry_block_id = func.add_block("entry");
        let join_block_id = func.add_block("join");

        func.get_block_mut(entry_block_id)
            .unwrap()
            .set_terminator(Terminator::Branch {
                target: join_block_id,
            });

        let phi_result = Value { id: 0 };
        let join_block = func.get_block_mut(join_block_id).unwrap();
        join_block.add_instruction(InstructionKind::Phi {
            result: phi_result,
            incoming: vec![(Value { id: 1 }, 123)],
        });
        join_block.set_terminator(Terminator::Return {
            value: Some(phi_result),
        });

        assert!(codegen.declare_function(&func).is_ok());
        let err = codegen
            .define_function(&func)
            .expect_err("missing phi incoming must be reported, not panic");
        assert_eq!(err.kind(), &BackendErrorKind::MissingPhiIncoming);
        assert!(err.message().contains(&join_block_id.to_string()));
    }

    #[test]
    fn test_comparison_instructions() {
        use spectra_midend::ir::{InstructionKind, Terminator, Value};

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
        use spectra_midend::ir::{InstructionKind, Terminator, Value};

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

    #[test]
    fn test_typed_host_float_result_cast_to_int_codegen() {
        use spectra_midend::ir::{InstructionKind, Terminator, Value};

        let mut codegen = CodeGenerator::new();
        let mut func = IRFunction::new("host_float_to_int", vec![], IRType::Int);

        let entry_block_id = func.add_block("entry");
        let entry_block = func.get_block_mut(entry_block_id).unwrap();

        let float_arg = Value { id: 0 };
        entry_block.add_instruction(InstructionKind::ConstFloat {
            result: float_arg,
            value: 9.9,
        });

        let host_result = Value { id: 1 };
        entry_block.add_instruction(InstructionKind::HostCall {
            result: Some(host_result),
            host: "spectra.std.math.floor_f".to_string(),
            args: vec![float_arg],
            result_type: Some(IRType::Float),
        });

        let cast_result = Value { id: 2 };
        entry_block.add_instruction(InstructionKind::Cast {
            result: cast_result,
            operand: host_result,
            from_ty: IRType::Float,
            to_ty: IRType::Int,
        });

        entry_block.set_terminator(Terminator::Return {
            value: Some(cast_result),
        });

        assert!(codegen.declare_function(&func).is_ok());
        assert!(codegen.define_function(&func).is_ok());
    }

    #[test]
    fn test_async_ready_suspend_resume_codegen() {
        use spectra_midend::ir::{InstructionKind, Terminator, Value};

        let mut codegen = CodeGenerator::new();
        let mut func = IRFunction::new(
            "async_minimal",
            vec![],
            IRType::Task {
                output: Box::new(IRType::Int),
            },
        );

        let entry_block_id = func.add_block("entry");
        let entry_block = func.get_block_mut(entry_block_id).unwrap();

        let payload = Value { id: 0 };
        entry_block.add_instruction(InstructionKind::ConstInt {
            result: payload,
            value: 7,
        });

        let task = Value { id: 1 };
        entry_block.add_instruction(InstructionKind::AsyncReady {
            result: task,
            value: Some(payload),
            output_type: IRType::Int,
        });
        entry_block.add_instruction(InstructionKind::AsyncSuspend { task, state: 0 });
        entry_block.add_instruction(InstructionKind::AsyncResume { task, state: 0 });
        entry_block.set_terminator(Terminator::Return { value: Some(task) });

        assert!(codegen.declare_function(&func).is_ok());
        assert!(codegen.define_function(&func).is_ok());
    }
}
