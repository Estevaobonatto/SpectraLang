// AOT (Ahead-of-Time) code generation using Cranelift ObjectModule.
// Translates Spectra IR to native object files (.o / .obj) that can be linked
// with the Spectra runtime static library to produce standalone executables.

use cranelift::prelude::*;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule, ObjectProduct};
use spectra_midend::ir::{
    Function as IRFunction, InstructionKind, Module as IRModule, Type as IRType,
};
use std::collections::HashMap;

use crate::codegen::{CodeGenerator, HostNameRecord, PhiDescriptor};
use crate::error::{BackendCodegenError, BackendResult};

/// Options that control AOT code generation.
#[derive(Debug, Clone, Default)]
pub struct AotOptions {
    /// When `true`, the user's `main` function is exported as `spectra_user_main`
    /// and a native C-compatible `main(argc, argv)` shim is synthesised that
    /// calls `spectra_rt_startup_with_args` followed by `spectra_user_main`.
    /// Use this when producing a self-contained executable.
    ///
    /// When `false` (the default), `main` is exported as-is and no shim is
    /// generated. Use this when producing an object file for manual linking.
    pub emit_executable: bool,
}

pub struct AotCodeGenerator {
    module: ObjectModule,
    ctx: codegen::Context,
    builder_context: FunctionBuilderContext,
    function_map: HashMap<String, FuncId>,
    manual_alloc_func: FuncId,
    manual_free_func: FuncId,
    manual_frame_enter_func: FuncId,
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
    map_set_fast_func: FuncId,
    map_get_fast_func: FuncId,
    map_contains_fast_func: FuncId,
    host_name_data: HashMap<String, HostNameRecord>,
    host_name_storage: Vec<Box<[u8]>>,
}

impl AotCodeGenerator {
    /// Create a new AOT code generator targeting the host machine.
    pub fn new() -> Self {
        let isa = cranelift_native::builder()
            .expect("Failed to create native ISA builder")
            .finish(settings::Flags::new(settings::builder()))
            .expect("Failed to build ISA");

        let builder = ObjectBuilder::new(
            isa,
            "spectra_aot_module",
            cranelift_module::default_libcall_names(),
        )
        .expect("Failed to create ObjectBuilder");

        let mut module = ObjectModule::new(builder);
        let ctx = module.make_context();

        // Declare imports for the runtime functions that will be provided by the static library.

        let mut alloc_sig = module.make_signature();
        alloc_sig.params.push(AbiParam::new(types::I64));
        alloc_sig.returns.push(AbiParam::new(types::I64));
        let manual_alloc_func = module
            .declare_function("spectra_rt_manual_alloc", Linkage::Import, &alloc_sig)
            .expect("Failed to declare alloc import");

        let mut free_sig = module.make_signature();
        free_sig.params.push(AbiParam::new(types::I64));
        let manual_free_func = module
            .declare_function("spectra_rt_manual_free", Linkage::Import, &free_sig)
            .expect("Failed to declare free import");

        let mut frame_enter_sig = module.make_signature();
        frame_enter_sig.returns.push(AbiParam::new(types::I64));
        let manual_frame_enter_func = module
            .declare_function(
                "spectra_rt_manual_frame_enter",
                Linkage::Import,
                &frame_enter_sig,
            )
            .expect("Failed to declare frame-enter import");

        let mut frame_exit_sig = module.make_signature();
        frame_exit_sig.params.push(AbiParam::new(types::I64));
        let manual_frame_exit_func = module
            .declare_function(
                "spectra_rt_manual_frame_exit",
                Linkage::Import,
                &frame_exit_sig,
            )
            .expect("Failed to declare frame-exit import");

        let mut escape_sig = module.make_signature();
        escape_sig.params.push(AbiParam::new(types::I64));
        escape_sig.params.push(AbiParam::new(types::I64));
        let manual_escape_func = module
            .declare_function("spectra_rt_manual_escape", Linkage::Import, &escape_sig)
            .expect("Failed to declare escape import");

        let mut host_invoke_sig = module.make_signature();
        for _ in 0..6 {
            host_invoke_sig.params.push(AbiParam::new(types::I64));
        }
        host_invoke_sig.returns.push(AbiParam::new(types::I32));
        let host_invoke_func = module
            .declare_function("spectra_rt_host_invoke", Linkage::Import, &host_invoke_sig)
            .expect("Failed to declare host-invoke import");

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

        let mut map_set_sig = module.make_signature();
        map_set_sig.params.push(AbiParam::new(types::I64));
        map_set_sig.params.push(AbiParam::new(types::I64));
        map_set_sig.params.push(AbiParam::new(types::I64));
        map_set_sig.returns.push(AbiParam::new(types::I32));
        let map_set_fast_func = module
            .declare_function("spectra_rt_map_set_fast", Linkage::Import, &map_set_sig)
            .expect("Failed to declare map_set fast import");

        let mut map_get_sig = module.make_signature();
        map_get_sig.params.push(AbiParam::new(types::I64));
        map_get_sig.params.push(AbiParam::new(types::I64));
        map_get_sig.returns.push(AbiParam::new(types::I64));
        let map_get_fast_func = module
            .declare_function("spectra_rt_map_get_fast", Linkage::Import, &map_get_sig)
            .expect("Failed to declare map_get fast import");

        let mut map_contains_sig = module.make_signature();
        map_contains_sig.params.push(AbiParam::new(types::I64));
        map_contains_sig.params.push(AbiParam::new(types::I64));
        map_contains_sig.returns.push(AbiParam::new(types::I64));
        let map_contains_fast_func = module
            .declare_function(
                "spectra_rt_map_contains_fast",
                Linkage::Import,
                &map_contains_sig,
            )
            .expect("Failed to declare map_contains fast import");

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
            map_set_fast_func,
            map_get_fast_func,
            map_contains_fast_func,
            host_name_data: HashMap::new(),
            host_name_storage: Vec::new(),
        }
    }

    /// Compile an IR module to a native object file.
    /// Returns the raw bytes of the `.o` / `.obj` file.
    pub fn compile_to_object(
        mut self,
        ir_module: &IRModule,
        opts: &AotOptions,
    ) -> BackendResult<Vec<u8>> {
        let rename_main = opts.emit_executable;

        // Pre-intern all host-function names as .rodata data sections so that
        // the generated code can reference them via GlobalValues (relocatable
        // addresses) instead of compile-time heap pointers (which would be
        // invalid in the final executable's address space).
        self.pre_intern_host_names(ir_module);

        // First pass: declare all functions.
        for func in &ir_module.functions {
            self.declare_function(func, rename_main)?;
        }

        // Second pass: define all functions.
        for func in &ir_module.functions {
            self.define_function(func)?;
        }

        // If building an executable, validate that a `main` entry point exists
        // and emit the native C-compatible `main(argc, argv)` shim.
        if opts.emit_executable {
            let has_main = ir_module.functions.iter().any(|f| f.name == "main");
            if !has_main {
                return Err(BackendCodegenError::missing_function("main"));
            }
            self.generate_exe_entry_point()?;
        }

        // Emit the finished object.
        let product: ObjectProduct = self.module.finish();

        Ok(product
            .emit()
            .map_err(|e| BackendCodegenError::cranelift(format!("Object emit error: {}", e)))?)
    }

    fn declare_function(
        &mut self,
        ir_func: &IRFunction,
        rename_main: bool,
    ) -> BackendResult<FuncId> {
        let mut sig = self.module.make_signature();
        for param in &ir_func.params {
            let cl_type = CodeGenerator::ir_type_to_cranelift(&param.ty)?;
            sig.params.push(AbiParam::new(cl_type));
        }
        let return_type = CodeGenerator::ir_type_to_cranelift(&ir_func.return_type)?;
        if return_type != types::I8 || ir_func.return_type != IRType::Void {
            sig.returns.push(AbiParam::new(return_type));
        }

        // When building an executable, rename `main` to `spectra_user_main` so
        // that the synthesised C-compatible `main` shim can call it without a
        // symbol clash.
        let exported_name: &str = if rename_main && ir_func.name == "main" {
            "spectra_user_main"
        } else {
            ir_func.name.as_str()
        };

        let func_id = self
            .module
            .declare_function(exported_name, Linkage::Export, &sig)
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to declare '{}': {}",
                    exported_name, e
                ))
            })?;
        // Key by IR name so internal call-site lookups (via `function_map`) work
        // regardless of the exported symbol name.
        self.function_map.insert(ir_func.name.clone(), func_id);
        Ok(func_id)
    }

    fn define_function(&mut self, ir_func: &IRFunction) -> BackendResult<()> {
        let func_id = *self
            .function_map
            .get(&ir_func.name)
            .ok_or_else(|| BackendCodegenError::missing_function(&ir_func.name))?;

        self.ctx.func.clear();
        self.ctx.func.signature = self
            .module
            .declarations()
            .get_function_decl(func_id)
            .signature
            .clone();

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut value_map: HashMap<usize, Value> = HashMap::new();
        let mut block_map: HashMap<usize, Block> = HashMap::new();
        let mut allocation_vars: Vec<Variable> = Vec::new();
        let mut stack_array_lengths: HashMap<usize, i64> = HashMap::new();
        let stack_allocas = CodeGenerator::collect_stack_allocas(ir_func);
        let manual_frame_active =
            CodeGenerator::function_needs_manual_frame(ir_func, &stack_allocas);
        let frame_token = if manual_frame_active {
            let frame_enter_ref = self
                .module
                .declare_func_in_func(self.manual_frame_enter_func, builder.func);
            let frame_call = builder.ins().call(frame_enter_ref, &[]);
            builder.inst_results(frame_call)[0]
        } else {
            builder.ins().iconst(types::I64, 0)
        };
        let frame_var = builder.declare_var(types::I64);
        builder.def_var(frame_var, frame_token);

        let params = builder.block_params(entry_block).to_vec();
        for (idx, &cl_value) in params.iter().enumerate() {
            value_map.insert(idx, cl_value);
        }

        for ir_block in &ir_func.blocks {
            if ir_block.id == 0 {
                block_map.insert(0, entry_block);
            } else {
                let block = builder.create_block();
                block_map.insert(ir_block.id, block);
            }
        }

        // Collect PHI descriptors and add block parameters to Cranelift blocks.
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

        let blocks = ir_func.blocks.clone();
        for ir_block in &blocks {
            CodeGenerator::generate_block(
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
                self.map_set_fast_func,
                self.map_get_fast_func,
                self.map_contains_fast_func,
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

        for ir_block in &ir_func.blocks {
            if ir_block.id != 0 {
                if let Some(&block) = block_map.get(&ir_block.id) {
                    builder.seal_block(block);
                }
            }
        }

        builder.finalize();

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to define '{}': {}",
                    ir_func.name, e
                ))
            })?;
        self.module.clear_context(&mut self.ctx);

        Ok(())
    }
}

impl Default for AotCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl AotCodeGenerator {
    /// Synthesises a native `main(int argc, char** argv)` entry point that:
    ///   1. calls `spectra_rt_startup_with_args(argc, argv)` to initialise the runtime;
    ///   2. calls `spectra_user_main()` (the renamed Spectra `main` function);
    ///   3. returns `0` to the OS.
    fn generate_exe_entry_point(&mut self) -> BackendResult<()> {
        // ── declare spectra_rt_startup_with_args import ──────────────────────
        let mut startup_sig = self.module.make_signature();
        startup_sig.params.push(AbiParam::new(types::I32)); // argc: i32
        startup_sig.params.push(AbiParam::new(types::I64)); // argv: *const *const u8 (ptr)
        let startup_func_id = self
            .module
            .declare_function(
                "spectra_rt_startup_with_args",
                Linkage::Import,
                &startup_sig,
            )
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to declare 'spectra_rt_startup_with_args': {}",
                    e
                ))
            })?;

        // ── look up spectra_user_main (stored under IR name "main") ──────────
        let user_main_func_id = *self
            .function_map
            .get("main")
            .ok_or_else(|| BackendCodegenError::missing_function("main"))?;

        // ── declare native C main ─────────────────────────────────────────────
        let mut native_main_sig = self.module.make_signature();
        native_main_sig.params.push(AbiParam::new(types::I32)); // argc
        native_main_sig.params.push(AbiParam::new(types::I64)); // argv
        native_main_sig.returns.push(AbiParam::new(types::I32)); // return int
        let native_main_func_id = self
            .module
            .declare_function("main", Linkage::Export, &native_main_sig)
            .map_err(|e| {
                BackendCodegenError::cranelift(format!("Failed to declare native 'main': {}", e))
            })?;

        // ── define the shim body ──────────────────────────────────────────────
        self.ctx.func.clear();
        self.ctx.func.signature = self
            .module
            .declarations()
            .get_function_decl(native_main_func_id)
            .signature
            .clone();

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        builder.seal_block(block);

        let params = builder.block_params(block).to_vec();
        let argc = params[0];
        let argv = params[1];

        // Call spectra_rt_startup_with_args(argc, argv)
        let startup_ref = self
            .module
            .declare_func_in_func(startup_func_id, builder.func);
        builder.ins().call(startup_ref, &[argc, argv]);

        // Call spectra_user_main() — ignore any return value
        let user_main_ref = self
            .module
            .declare_func_in_func(user_main_func_id, builder.func);
        builder.ins().call(user_main_ref, &[]);

        // Call spectra_rt_maybe_pause() — pauses when running via double-click.
        let pause_sig = self.module.make_signature();
        let pause_func_id = self
            .module
            .declare_function("spectra_rt_maybe_pause", Linkage::Import, &pause_sig)
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to declare 'spectra_rt_maybe_pause': {}",
                    e
                ))
            })?;
        let pause_ref = self
            .module
            .declare_func_in_func(pause_func_id, builder.func);
        builder.ins().call(pause_ref, &[]);

        // return 0
        let zero = builder.ins().iconst(types::I32, 0);
        builder.ins().return_(&[zero]);
        builder.finalize();

        self.module
            .define_function(native_main_func_id, &mut self.ctx)
            .map_err(|e| {
                BackendCodegenError::cranelift(format!(
                    "Failed to define native 'main' shim: {}",
                    e
                ))
            })?;
        self.module.clear_context(&mut self.ctx);

        Ok(())
    }

    /// Scans all IR functions for `HostCall` instructions and pre-interns each
    /// unique host name as a `.rodata` data section in the object module.
    /// This must be done before any function bodies are compiled so that every
    /// `HostCall` in `generate_block` finds a ready `DataId` in `host_name_data`.
    fn pre_intern_host_names(&mut self, ir_module: &IRModule) {
        for func in &ir_module.functions {
            for block in &func.blocks {
                for instr in &block.instructions {
                    if let InstructionKind::HostCall { host, .. } = &instr.kind {
                        if !self.host_name_data.contains_key(host.as_str()) {
                            self.create_host_name_data(host);
                        }
                    }
                }
            }
        }
    }

    /// Creates a `.rodata` data section for a host function name string and
    /// stores the resulting `HostNameRecord` (with `data_id = Some(...)`) in
    /// `self.host_name_data`.
    fn create_host_name_data(&mut self, name: &str) {
        // Build a safe symbol name from the (possibly dotted) host function key.
        let safe = name.replace('.', "__").replace('-', "_");
        let symbol = format!(".__spectra_host_{safe}");

        let data_id: DataId = match self
            .module
            .declare_data(&symbol, Linkage::Local, false, false)
        {
            Ok(id) => id,
            Err(_) => return, // Already declared — shouldn't happen but be defensive.
        };

        let mut data_ctx = DataDescription::new();
        data_ctx.define(name.as_bytes().to_vec().into_boxed_slice());
        let _ = self.module.define_data(data_id, &data_ctx);

        let record = HostNameRecord {
            ptr: 0,
            len: name.len(),
            data_id: Some(data_id),
        };
        self.host_name_data.insert(name.to_string(), record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BackendErrorKind;
    use spectra_midend::ir::{Function as IRFunction, Terminator, Type as IRType};

    #[test]
    fn r2007_aot_missing_branch_target_returns_typed_error() {
        let mut module = IRModule::new("r2007_aot_missing_branch_target");
        let mut func = IRFunction::new("main", vec![], IRType::Void);
        let entry_block_id = func.add_block("entry");
        func.get_block_mut(entry_block_id)
            .unwrap()
            .set_terminator(Terminator::Branch { target: 42 });
        module.add_function(func);

        let err = AotCodeGenerator::new()
            .compile_to_object(&module, &AotOptions::default())
            .expect_err("AOT missing target block must be reported, not panic");
        assert_eq!(err.kind(), &BackendErrorKind::MissingBlock);
        assert!(err.message().contains("42"));
    }
}
