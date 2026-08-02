// AOT (Ahead-of-Time) code generation using Cranelift ObjectModule.
// Translates Spectra IR to native object files (.o / .obj) that can be linked
// with the Spectra runtime static library to produce standalone executables.

use cranelift::prelude::*;
use cranelift_codegen::ir::ValueLabel;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule, ObjectProduct};
use spectra_midend::ir::{
    Function as IRFunction, InstructionKind, Module as IRModule, Type as IRType,
};
use std::collections::HashMap;

use crate::codegen::{
    validate_tensor_ir, CodeGenerator, DenseValueMap, HostCallBatchStats, HostNameRecord,
    PhiDescriptor, StringLiteralRecord,
};
use crate::error::{BackendCodegenError, BackendResult};
use crate::hostcall_abi::{declare_runtime_bindings, HostCallLoweringContext, RuntimeBindings};
use spectra_runtime::abi::RuntimeImport;

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
    /// Request native debug records in the emitted object. The backend keeps
    /// this explicit so callers cannot mistake the JSON sidecar for native
    /// debug information.
    pub native_debug: bool,
}

pub struct AotCodeGenerator {
    module: ObjectModule,
    ctx: codegen::Context,
    builder_context: FunctionBuilderContext,
    function_map: HashMap<String, FuncId>,
    runtime_bindings: RuntimeBindings,
    /// Dedup table for string literals (R-3126). Each unique
    /// `ConstString` value resolves to one entry pre-populated in
    /// [`pre_intern_string_literals`].
    string_literal_data: HashMap<String, StringLiteralRecord>,
    /// Heap storage for string literal buffers (R-3126). In AOT mode
    /// this stays empty because every entry is pre-populated with a
    /// `data_id`; the field exists to satisfy the `generate_block`
    /// signature shared with the JIT path. Layout matches the JIT
    /// side: one byte per `i64` slot.
    string_literal_storage: Vec<Box<[i64]>>,
    host_name_data: HashMap<String, HostNameRecord>,
    hostcall_batch_stats: HostCallBatchStats,
    /// Locations produced by Cranelift's register allocator for labelled IR
    /// values: (function, IR value id, CFA-relative offset).  These are
    /// intentionally collected from compiled machine code, never guessed
    /// from source or sidecar text.
    debug_locations: Vec<(String, usize, i64)>,
}

impl AotCodeGenerator {
    /// Create a new AOT code generator targeting the host machine.
    pub fn new() -> Self {
        // R-3129: opt into Cranelift's speed optimizer. The default
        // builder leaves `opt_level = "none"`, which skips almost all of
        // Cranelift's mid-end passes (GSN, DCE, LICM, value-tracking,
        // branch coalescing, etc.) and produces measurably slower code.
        // See `cranelift_codegen::settings` for the full list of options.
        let mut settings_builder = settings::builder();
        settings_builder
            .set("opt_level", "speed")
            .expect("failed to set cranelift opt_level to speed");
        let isa = cranelift_native::builder()
            .expect("Failed to create native ISA builder")
            .finish(settings::Flags::new(settings_builder))
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

        let runtime_bindings =
            declare_runtime_bindings(&mut module).expect("Failed to declare runtime ABI imports");

        Self {
            module,
            ctx,
            builder_context: FunctionBuilderContext::new(),
            function_map: HashMap::new(),
            runtime_bindings,
            host_name_data: HashMap::new(),
            hostcall_batch_stats: HostCallBatchStats::default(),
            debug_locations: Vec::new(),
            string_literal_data: HashMap::new(),
            string_literal_storage: Vec::new(),
        }
    }

    /// Compile an IR module to a native object file.
    /// Returns the raw bytes of the `.o` / `.obj` file.
    pub fn compile_to_object(
        self,
        ir_module: &IRModule,
        opts: &AotOptions,
    ) -> BackendResult<Vec<u8>> {
        let (bytes, _, _) = self.compile_to_object_with_locations_and_stats(ir_module, opts)?;
        Ok(bytes)
    }

    pub fn compile_to_object_with_locations(
        self,
        ir_module: &IRModule,
        opts: &AotOptions,
    ) -> BackendResult<(Vec<u8>, Vec<(String, usize, i64)>)> {
        let (bytes, locations, _) =
            self.compile_to_object_with_locations_and_stats(ir_module, opts)?;
        Ok((bytes, locations))
    }

    pub fn compile_to_object_with_locations_and_stats(
        mut self,
        ir_module: &IRModule,
        opts: &AotOptions,
    ) -> BackendResult<(Vec<u8>, Vec<(String, usize, i64)>, HostCallBatchStats)> {
        self.hostcall_batch_stats = HostCallBatchStats::default();
        let rename_main = opts.emit_executable;
        let _tensor_ir = validate_tensor_ir(ir_module)?;

        // Pre-intern all host-function names as .rodata data sections so that
        // the generated code can reference them via GlobalValues (relocatable
        // addresses) instead of compile-time heap pointers (which would be
        // invalid in the final executable's address space).
        self.pre_intern_host_names(ir_module);

        // Pre-intern all string literals as `.rodata` data sections (R-3126).
        // Each unique `ConstString` value becomes one data section; the
        // `generate_block` path then emits `global_value` instructions
        // pointing at these sections instead of going through `manual_alloc`.
        self.pre_intern_string_literals(ir_module);

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
        let debug_locations = self.take_debug_locations();
        let product: ObjectProduct = self.module.finish();

        let bytes = product
            .emit()
            .map_err(|e| BackendCodegenError::cranelift(format!("Object emit error: {}", e)))?;
        Ok((bytes, debug_locations, self.hostcall_batch_stats))
    }

    pub fn take_debug_locations(&mut self) -> Vec<(String, usize, i64)> {
        std::mem::take(&mut self.debug_locations)
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
        builder.func.collect_debug_info();

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut value_map = DenseValueMap::with_capacity(ir_func.next_value_id);
        let mut block_map: HashMap<usize, Block> = HashMap::new();
        let mut allocation_vars: Vec<Variable> = Vec::new();
        let mut stack_array_lengths: HashMap<usize, i64> = HashMap::new();
        let mut string_literal_lengths: HashMap<usize, i64> = HashMap::new();
        let stack_allocas = CodeGenerator::collect_stack_allocas(ir_func);
        let scalar_alloca_types =
            CodeGenerator::collect_promotable_scalar_allocas_with_stack_allocas(
                ir_func,
                &stack_allocas,
            );
        let mut scalar_alloca_vars = HashMap::with_capacity(scalar_alloca_types.len());
        for (alloca_id, ty) in &scalar_alloca_types {
            let variable = builder.declare_var(CodeGenerator::ir_type_to_cranelift(ty)?);
            scalar_alloca_vars.insert(*alloca_id, variable);
        }
        let manual_frame_active =
            CodeGenerator::function_needs_manual_frame(ir_func, &stack_allocas);
        let frame_token = if manual_frame_active {
            let frame_enter_ref = self.module.declare_func_in_func(
                self.runtime_bindings.get(RuntimeImport::ManualFrameEnter),
                builder.func,
            );
            let frame_call = builder.ins().call(frame_enter_ref, &[]);
            builder.inst_results(frame_call)[0]
        } else {
            builder.ins().iconst(types::I64, 0)
        };
        let frame_var = builder.declare_var(types::I64);
        builder.def_var(frame_var, frame_token);

        let params = builder.block_params(entry_block).to_vec();
        for (param, &cl_value) in ir_func.params.iter().zip(params.iter()) {
            value_map.insert(param.id, cl_value);
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
        let mut hostcall = HostCallLoweringContext {
            bindings: &self.runtime_bindings,
            host_name_data: &self.host_name_data,
            string_literal_data: &mut self.string_literal_data,
            string_literal_storage: &mut self.string_literal_storage,
            batch_stats: &mut self.hostcall_batch_stats,
        };
        for ir_block in &blocks {
            CodeGenerator::generate_block(
                &mut self.module,
                &self.function_map,
                &mut hostcall,
                &mut builder,
                ir_block,
                &mut value_map,
                &block_map,
                &mut allocation_vars,
                &mut stack_array_lengths,
                &mut string_literal_lengths,
                &stack_allocas,
                &scalar_alloca_vars,
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

        // Attach Cranelift value labels only after all IR values have been
        // mapped. The allocator will resolve these labels to a real register
        // or CFA-relative location in the compiled machine code.
        for local in &ir_func.locals {
            if let Some(value_id) = local.value_id {
                if let Some(value) = value_map.get(value_id) {
                    builder.set_val_label(value, ValueLabel::from_u32(value_id as u32));
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
        if let Some(compiled) = self.ctx.compiled_code() {
            for local in &ir_func.locals {
                let Some(value_id) = local.value_id else {
                    continue;
                };
                let label = ValueLabel::from_u32(value_id as u32);
                if let Some(ranges) = compiled.value_labels_ranges.get(&label) {
                    for range in ranges {
                        let rendered = format!("{:?}", range.loc);
                        if let Some(offset) = rendered
                            .strip_prefix("CFAOffset(")
                            .and_then(|s| s.strip_suffix(')'))
                            .and_then(|s| s.parse::<i64>().ok())
                        {
                            self.debug_locations
                                .push((ir_func.name.clone(), value_id, offset));
                            break;
                        }
                    }
                }
            }
        }
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
        let mut names = ir_module
            .functions
            .iter()
            .flat_map(|func| func.blocks.iter())
            .flat_map(|block| block.instructions.iter())
            .filter_map(|instr| match &instr.kind {
                InstructionKind::HostCall { host, .. } => Some(host.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        for name in names {
            if !self.host_name_data.contains_key(name) {
                self.create_host_name_data(name);
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

    /// R-3126: scan every function for `ConstString` instructions and
    /// pre-declare each unique literal as a `.rodata` data section.
    /// Mirrors [`pre_intern_host_names`].
    fn pre_intern_string_literals(&mut self, ir_module: &IRModule) {
        for func in &ir_module.functions {
            for block in &func.blocks {
                for instr in &block.instructions {
                    if let InstructionKind::ConstString { value, .. } = &instr.kind {
                        if !self.string_literal_data.contains_key(value.as_str()) {
                            self.create_string_literal_data(value);
                        }
                    }
                }
            }
        }
    }

    /// R-3126: declare a `.rodata` data section for one string literal
    /// value. Stores the resulting `StringLiteralRecord` (with
    /// `data_id = Some(...)`) in `self.string_literal_data`.
    fn create_string_literal_data(&mut self, value: &str) {
        // Layout: one byte per `i64` slot (8 bytes each), null-terminated.
        // This matches the JIT `Box<[i64]>` buffer and the `*8` indexing
        // in `emit_stack_string_char_at_inline`.
        let mut slots: Vec<i64> = value.as_bytes().iter().map(|&b| b as i64).collect();
        slots.push(0);
        let len_with_null = slots.len() as i64;
        // Convert the i64 slots to a raw byte buffer for the data section.
        // Safety: `i64` is `repr(i64)` and we want the same byte layout.
        let bytes: Vec<u8> = unsafe {
            std::slice::from_raw_parts(
                slots.as_ptr() as *const u8,
                slots.len() * std::mem::size_of::<i64>(),
            )
            .to_vec()
        };
        // Use a simple FNV-1a 64-bit hash for compact, deterministic naming
        // without depending on an external hash crate.
        let mut hash: u64 = 0xcbf29ce484222325;
        for &b in &bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let symbol = format!(".__spectra_strlit_{:016x}", hash);

        let data_id: DataId = match self
            .module
            .declare_data(&symbol, Linkage::Local, false, false)
        {
            Ok(id) => id,
            Err(_) => return, // Already declared.
        };

        let mut data_ctx = DataDescription::new();
        data_ctx.define(bytes.into_boxed_slice());
        let _ = self.module.define_data(data_id, &data_ctx);

        let record = StringLiteralRecord {
            ptr: 0,
            len_with_null,
            data_id: Some(data_id),
        };
        self.string_literal_data.insert(value.to_string(), record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BackendErrorKind;
    use spectra_midend::ir::{Function as IRFunction, InstructionKind, Terminator, Type as IRType};

    #[test]
    fn r3104_aot_preinterns_duplicate_host_names_once() {
        let mut module = IRModule::new("r3104_aot_host_names");
        let mut function = IRFunction::new("main", vec![], IRType::Void);
        let entry = function.add_block("entry");
        let block = function.get_block_mut(entry).unwrap();
        for _ in 0..2 {
            block.add_instruction(InstructionKind::HostCall {
                result: None,
                host: "spectra.std.test.duplicate".to_string(),
                args: vec![],
                result_type: None,
            });
        }
        block.set_terminator(Terminator::Return { value: None });
        module.add_function(function);

        let mut codegen = AotCodeGenerator::new();
        codegen.pre_intern_host_names(&module);
        assert_eq!(codegen.host_name_data.len(), 1);
        assert!(codegen
            .host_name_data
            .contains_key("spectra.std.test.duplicate"));
        assert!(codegen
            .host_name_data
            .get("spectra.std.test.duplicate")
            .and_then(|record| record.data_id)
            .is_some());
    }

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
