//! Shared runtime import bindings for JIT and AOT code generation.
//!
//! The runtime owns the stable symbol/name catalog. This module adapts that
//! catalog to Cranelift without exposing Cranelift types to the runtime crate.

use cranelift::prelude::{types, AbiParam, Signature};
use cranelift_jit::JITBuilder;
use cranelift_module::{FuncId, Linkage, Module, ModuleError};
use spectra_runtime::abi::{AbiScalar, FastHostCall, RuntimeImport};
use std::collections::HashMap;

use crate::codegen::{HostCallBatchStats, HostNameRecord, StringLiteralRecord};

/// All runtime imports declared in one JIT or AOT module.
pub(crate) struct RuntimeBindings {
    imports: Box<[FuncId]>,
}

impl RuntimeBindings {
    pub(crate) fn get(&self, import: RuntimeImport) -> FuncId {
        self.imports[import.index()]
    }
}

/// Mutable state shared by the host-call lowering helpers.
pub(crate) struct HostCallLoweringContext<'a> {
    pub(crate) bindings: &'a RuntimeBindings,
    pub(crate) host_name_data: &'a HashMap<String, HostNameRecord>,
    pub(crate) string_literal_data: &'a mut HashMap<String, StringLiteralRecord>,
    pub(crate) string_literal_storage: &'a mut Vec<Box<[i64]>>,
    pub(crate) batch_stats: &'a mut HostCallBatchStats,
}

impl HostCallLoweringContext<'_> {
    pub(crate) fn runtime_func(&self, import: RuntimeImport) -> FuncId {
        self.bindings.get(import)
    }

    pub(crate) fn fast_func(&self, host_call: FastHostCall) -> FuncId {
        self.runtime_func(host_call.runtime_import())
    }
}

/// Registers every runtime import address with the JIT builder.
pub(crate) fn register_jit_runtime_symbols(builder: &mut JITBuilder) {
    for import in RuntimeImport::ALL {
        builder.symbol(import.symbol(), import.address());
    }
}

/// Declares every runtime import using the runtime-owned ABI catalog.
pub(crate) fn declare_runtime_bindings<M: Module>(
    module: &mut M,
) -> Result<RuntimeBindings, ModuleError> {
    let mut imports = Vec::with_capacity(RuntimeImport::COUNT);
    for import in RuntimeImport::ALL {
        let signature = make_signature(module, import.signature());
        imports.push(module.declare_function(import.symbol(), Linkage::Import, &signature)?);
    }
    Ok(RuntimeBindings {
        imports: imports.into_boxed_slice(),
    })
}

fn make_signature<M: Module>(
    module: &mut M,
    signature: spectra_runtime::abi::AbiSignature,
) -> Signature {
    let mut result = module.make_signature();
    for param in signature.params {
        result.params.push(AbiParam::new(to_cranelift_type(*param)));
    }
    for ret in signature.returns {
        result.returns.push(AbiParam::new(to_cranelift_type(*ret)));
    }
    result
}

fn to_cranelift_type(scalar: AbiScalar) -> cranelift_codegen::ir::Type {
    match scalar {
        AbiScalar::I32 => types::I32,
        AbiScalar::I64 => types::I64,
        AbiScalar::F64 => types::F64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::Module;
    use cranelift_object::{ObjectBuilder, ObjectModule};

    #[test]
    fn catalog_declaration_count_matches_runtime() {
        assert_eq!(RuntimeImport::ALL.len(), RuntimeImport::COUNT);
    }

    #[test]
    fn jit_and_aot_bindings_use_identical_signatures() {
        let jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .expect("failed to create test JIT builder");
        let mut jit = JITModule::new(jit_builder);
        let jit_bindings = declare_runtime_bindings(&mut jit).expect("failed to declare JIT ABI");

        let isa = cranelift_native::builder()
            .expect("failed to create native ISA builder")
            .finish(cranelift_codegen::settings::Flags::new(
                cranelift_codegen::settings::builder(),
            ))
            .expect("failed to build native ISA");
        let object_builder = ObjectBuilder::new(
            isa,
            "hostcall_abi_test",
            cranelift_module::default_libcall_names(),
        )
        .expect("failed to create test AOT builder");
        let mut aot = ObjectModule::new(object_builder);
        let aot_bindings = declare_runtime_bindings(&mut aot).expect("failed to declare AOT ABI");

        for import in RuntimeImport::ALL {
            let jit_signature = &jit
                .declarations()
                .get_function_decl(jit_bindings.get(*import))
                .signature;
            let aot_signature = &aot
                .declarations()
                .get_function_decl(aot_bindings.get(*import))
                .signature;
            let jit_params: Vec<_> = jit_signature
                .params
                .iter()
                .map(|param| param.value_type)
                .collect();
            let aot_params: Vec<_> = aot_signature
                .params
                .iter()
                .map(|param| param.value_type)
                .collect();
            let jit_returns: Vec<_> = jit_signature
                .returns
                .iter()
                .map(|param| param.value_type)
                .collect();
            let aot_returns: Vec<_> = aot_signature
                .returns
                .iter()
                .map(|param| param.value_type)
                .collect();
            assert_eq!(
                jit_params,
                aot_params,
                "parameter ABI mismatch for {}",
                import.symbol()
            );
            assert_eq!(
                jit_returns,
                aot_returns,
                "result ABI mismatch for {}",
                import.symbol()
            );
        }
    }
}
