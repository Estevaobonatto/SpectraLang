// Builtin (virtual) module registrations
// Maps well-known `std.*` module paths to their exported function signatures
// without requiring physical `.spectra` files.  The actual implementation of
// each function lives in the runtime FFI layer (runtime/src/stdlib/mod.rs).

use super::module_registry::{
    ExportVisibility, ExportedFunction, ExportedType, ModuleExports, ModuleRegistry,
};
use crate::ast::Type;

/// Register all built-in standard library modules in the given registry.
pub fn register_builtin_modules(registry: &mut ModuleRegistry) {
    registry.register_module("std.io".to_string(), make_std_io());
    registry.register_module("std.math".to_string(), make_std_math());
    registry.register_module("std.collections".to_string(), make_std_collections());
    registry.register_module("std.string".to_string(), make_std_string());
    registry.register_module("std.convert".to_string(), make_std_convert());
    registry.register_module("std.random".to_string(), make_std_random());
    registry.register_module("std.fs".to_string(), make_std_fs());
    registry.register_module("std.env".to_string(), make_std_env());
    registry.register_module("std.option".to_string(), make_std_option());
    registry.register_module("std.result".to_string(), make_std_result());
    registry.register_module("std.char".to_string(), make_std_char());
    registry.register_module("std.time".to_string(), make_std_time());
    registry.register_module("std.tensor".to_string(), make_std_tensor());
    registry.register_module("std.ml".to_string(), make_std_ml());
    registry.register_module("std.concurrent".to_string(), make_std_concurrent());
    registry.register_module("std.serve".to_string(), make_std_serve());
    // Convenience aliases used in existing examples
    registry.register_module("spectra.std.io".to_string(), make_std_io());
    registry.register_module("spectra.std.math".to_string(), make_std_math());
    registry.register_module(
        "spectra.std.collections".to_string(),
        make_std_collections(),
    );
    registry.register_module("spectra.std.string".to_string(), make_std_string());
    registry.register_module("spectra.std.convert".to_string(), make_std_convert());
    registry.register_module("spectra.std.random".to_string(), make_std_random());
    registry.register_module("spectra.std.fs".to_string(), make_std_fs());
    registry.register_module("spectra.std.env".to_string(), make_std_env());
    registry.register_module("spectra.std.option".to_string(), make_std_option());
    registry.register_module("spectra.std.result".to_string(), make_std_result());
    registry.register_module("spectra.std.char".to_string(), make_std_char());
    registry.register_module("spectra.std.time".to_string(), make_std_time());
    registry.register_module("spectra.std.tensor".to_string(), make_std_tensor());
    registry.register_module("spectra.std.ml".to_string(), make_std_ml());
    registry.register_module("spectra.std.concurrent".to_string(), make_std_concurrent());
    registry.register_module("spectra.std.serve".to_string(), make_std_serve());
}

fn pub_fn(params: Vec<Type>, return_type: Type) -> ExportedFunction {
    ExportedFunction {
        params,
        return_type,
        visibility: ExportVisibility::Public,
    }
}

fn make_std_io() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "io".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // print(value: any) -> unit
    // The runtime FFI accepts a single value and prints it.
    exports
        .functions
        .insert("print".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // println(value: any) -> unit  (print + newline)
    exports.functions.insert(
        "println".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unit),
    );
    // eprint(value: any) -> unit  (stderr, no newline)
    exports.functions.insert(
        "eprint".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unit),
    );
    // eprintln(value: any) -> unit
    exports.functions.insert(
        "eprintln".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unit),
    );
    // flush() -> unit
    exports
        .functions
        .insert("flush".to_string(), pub_fn(vec![], Type::Unit));
    // read_line() -> string
    exports
        .functions
        .insert("read_line".to_string(), pub_fn(vec![], Type::String));
    // input(prompt: string) -> string  (prints prompt, flushes, reads line)
    exports.functions.insert(
        "input".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );

    exports
}

fn make_std_math() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "math".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    exports
        .functions
        .insert("abs".to_string(), pub_fn(vec![Type::Int], Type::Int));
    exports.functions.insert(
        "min".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    exports.functions.insert(
        "max".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    exports.functions.insert(
        "clamp".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Int),
    );
    exports
        .functions
        .insert("sqrt_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert(
        "pow_f".to_string(),
        pub_fn(vec![Type::Float, Type::Float], Type::Float),
    );
    exports.functions.insert(
        "floor_f".to_string(),
        pub_fn(vec![Type::Float], Type::Float),
    );
    exports
        .functions
        .insert("ceil_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert(
        "round_f".to_string(),
        pub_fn(vec![Type::Float], Type::Float),
    );
    exports
        .functions
        .insert("sin_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("cos_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("tan_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("log_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports
        .functions
        .insert("log2_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert(
        "log10_f".to_string(),
        pub_fn(vec![Type::Float], Type::Float),
    );
    exports.functions.insert(
        "atan2_f".to_string(),
        pub_fn(vec![Type::Float, Type::Float], Type::Float),
    );
    exports
        .functions
        .insert("pi".to_string(), pub_fn(vec![], Type::Float));
    exports
        .functions
        .insert("e_const".to_string(), pub_fn(vec![], Type::Float));
    // sign(n: int) -> int — returns -1, 0, or 1
    exports
        .functions
        .insert("sign".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // gcd(a: int, b: int) -> int
    exports.functions.insert(
        "gcd".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // lcm(a: int, b: int) -> int
    exports.functions.insert(
        "lcm".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // is_nan_f(x: float) -> bool
    exports.functions.insert(
        "is_nan_f".to_string(),
        pub_fn(vec![Type::Float], Type::Bool),
    );
    // is_infinite_f(x: float) -> bool
    exports.functions.insert(
        "is_infinite_f".to_string(),
        pub_fn(vec![Type::Float], Type::Bool),
    );
    // abs_f(x: float) -> float
    exports
        .functions
        .insert("abs_f".to_string(), pub_fn(vec![Type::Float], Type::Float));

    exports
}

fn make_std_collections() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "collections".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // list_new() -> int (handle)
    exports
        .functions
        .insert("list_new".to_string(), pub_fn(vec![], Type::Int));
    // list_push(handle: int, value: int) -> unit
    exports.functions.insert(
        "list_push".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Unit),
    );
    // list_len(handle: int) -> int
    exports
        .functions
        .insert("list_len".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // list_get(handle: int, index: int) -> int  (-1 if out of bounds)
    exports.functions.insert(
        "list_get".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // list_set(handle: int, index: int, value: int) -> unit
    exports.functions.insert(
        "list_set".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Unit),
    );
    // list_contains(handle: int, value: int) -> bool
    exports.functions.insert(
        "list_contains".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Bool),
    );
    // list_clear(handle: int) -> unit
    exports.functions.insert(
        "list_clear".to_string(),
        pub_fn(vec![Type::Int], Type::Unit),
    );
    // list_free(handle: int) -> unit
    exports
        .functions
        .insert("list_free".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // list_free_all() -> int
    exports
        .functions
        .insert("list_free_all".to_string(), pub_fn(vec![], Type::Int));
    // list_pop(handle: int) -> int  (returns popped value; -1 if empty)
    exports
        .functions
        .insert("list_pop".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // list_pop_front(handle: int) -> int  (returns removed front value; -1 if empty)
    exports.functions.insert(
        "list_pop_front".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    // list_insert_at(handle: int, index: int, value: int) -> unit
    exports.functions.insert(
        "list_insert_at".to_string(),
        pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Unit),
    );
    // list_remove_at(handle: int, index: int) -> int  (returns removed value; -1 if out of bounds)
    exports.functions.insert(
        "list_remove_at".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // list_index_of(handle: int, value: int) -> int  (-1 if not found)
    exports.functions.insert(
        "list_index_of".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // list_sort(handle: int) -> unit  (sorts ascending in place)
    exports
        .functions
        .insert("list_sort".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    let fn_int_to_int = Type::Fn {
        params: vec![Type::Int],
        return_type: Box::new(Type::Int),
    };
    let fn_int_int_to_int = Type::Fn {
        params: vec![Type::Int, Type::Int],
        return_type: Box::new(Type::Int),
    };
    // list_map(handle: int, f: fn(int)->int) -> int  (returns new list handle)
    exports.functions.insert(
        "list_map".to_string(),
        pub_fn(vec![Type::Int, fn_int_to_int.clone()], Type::Int),
    );
    // list_filter(handle: int, pred: fn(int)->int) -> int  (returns new list handle)
    exports.functions.insert(
        "list_filter".to_string(),
        pub_fn(vec![Type::Int, fn_int_to_int], Type::Int),
    );
    // list_reduce(handle: int, initial: int, f: fn(int,int)->int) -> int
    exports.functions.insert(
        "list_reduce".to_string(),
        pub_fn(
            vec![Type::Int, Type::Int, fn_int_int_to_int.clone()],
            Type::Int,
        ),
    );
    // list_sort_by(handle: int, cmp: fn(int,int)->int) -> unit
    exports.functions.insert(
        "list_sort_by".to_string(),
        pub_fn(vec![Type::Int, fn_int_int_to_int], Type::Unit),
    );

    // type aliases
    exports.types.insert(
        "List".to_string(),
        ExportedType {
            members: vec!["new".to_string(), "push".to_string(), "len".to_string()],
            visibility: ExportVisibility::Public,
            is_enum: false,
            struct_fields: None,
            enum_variants: None,
            enum_struct_variants: None,
        },
    );

    exports
}

fn make_std_tensor() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "tensor".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let float = Type::Float;
    let tensor_float_rank1 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(1),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_rank2 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(2),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_rank0 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(0),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_dynamic = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: None,
        dims: None,
        layout: None,
        device: None,
    };
    let unit = Type::Unit;
    let bool_ty = Type::Bool;

    let functions = [
        (
            "vector_f",
            vec![int.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "matrix_f",
            vec![int.clone(), int.clone(), float.clone()],
            tensor_float_rank2.clone(),
        ),
        ("zeros", vec![int.clone()], int.clone()),
        ("ones", vec![int.clone()], int.clone()),
        ("full", vec![int.clone(), int.clone()], int.clone()),
        (
            "full_f",
            vec![int.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "arange",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("zeros2", vec![int.clone(), int.clone()], int.clone()),
        ("ones2", vec![int.clone(), int.clone()], int.clone()),
        (
            "full2",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "full2_f",
            vec![int.clone(), int.clone(), float.clone()],
            tensor_float_rank2.clone(),
        ),
        ("len", vec![int.clone()], int.clone()),
        ("rank", vec![int.clone()], int.clone()),
        ("dim", vec![int.clone(), int.clone()], int.clone()),
        ("rows", vec![int.clone()], int.clone()),
        ("cols", vec![int.clone()], int.clone()),
        ("is_valid", vec![int.clone()], bool_ty.clone()),
        ("get", vec![int.clone(), int.clone()], int.clone()),
        ("get_f", vec![int.clone(), int.clone()], float.clone()),
        (
            "set",
            vec![int.clone(), int.clone(), int.clone()],
            unit.clone(),
        ),
        (
            "set_f",
            vec![int.clone(), int.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "get2",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "get2_f",
            vec![int.clone(), int.clone(), int.clone()],
            float.clone(),
        ),
        (
            "set2",
            vec![int.clone(), int.clone(), int.clone(), int.clone()],
            unit.clone(),
        ),
        (
            "set2_f",
            vec![int.clone(), int.clone(), int.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "reshape",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("flatten", vec![int.clone()], int.clone()),
        (
            "permute",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "slice",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("concat", vec![int.clone(), int.clone()], int.clone()),
        ("stack", vec![int.clone(), int.clone()], int.clone()),
        (
            "add",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        (
            "sub",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        (
            "mul",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        (
            "div",
            vec![int.clone(), int.clone()],
            tensor_float_dynamic.clone(),
        ),
        ("sum", vec![int.clone()], int.clone()),
        ("sum_f", vec![int.clone()], float.clone()),
        ("sum_t", vec![int.clone()], tensor_float_rank0.clone()),
        ("mean_f", vec![int.clone()], float.clone()),
        ("mean_t", vec![int.clone()], tensor_float_rank0.clone()),
        ("max", vec![int.clone()], int.clone()),
        ("min", vec![int.clone()], int.clone()),
        ("argmax", vec![int.clone()], int.clone()),
        (
            "matmul",
            vec![int.clone(), int.clone()],
            tensor_float_rank2.clone(),
        ),
        (
            "matmul_batched",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        ("transpose", vec![int.clone()], tensor_float_rank2.clone()),
        ("dot", vec![int.clone(), int.clone()], int.clone()),
        (
            "dot_t",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        ("neg", vec![int.clone()], tensor_float_dynamic.clone()),
        ("exp_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("log_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("sqrt_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("relu", vec![int.clone()], tensor_float_dynamic.clone()),
        ("sigmoid_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("tanh_f", vec![int.clone()], tensor_float_dynamic.clone()),
        ("seed", vec![int.clone()], unit.clone()),
        (
            "uniform",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "uniform_f",
            vec![int.clone(), float.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "normal_f",
            vec![int.clone(), float.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        (
            "bernoulli",
            vec![int.clone(), float.clone()],
            tensor_float_rank1.clone(),
        ),
        ("categorical", vec![int.clone(), int.clone()], int.clone()),
        ("set_deterministic_mode", vec![int.clone()], int.clone()),
        ("deterministic_mode", vec![], int.clone()),
        ("tolerance_abs", vec![], float.clone()),
        ("tolerance_rel", vec![], float.clone()),
        ("device", vec![int.clone()], int.clone()),
        ("device_available", vec![int.clone()], bool_ty.clone()),
        ("device_status", vec![int.clone()], int.clone()),
        ("to_device", vec![int.clone(), int.clone()], int.clone()),
        ("cpu", vec![int.clone()], int.clone()),
        ("sync", vec![int.clone()], unit.clone()),
        ("precision", vec![int.clone()], int.clone()),
        ("to_precision", vec![int.clone(), int.clone()], int.clone()),
        ("stats_allocations", vec![], int.clone()),
        ("stats_active", vec![], int.clone()),
        ("stats_peak_bytes", vec![], int.clone()),
        ("stats_reused_buffers", vec![], int.clone()),
        ("stats_pool_hits", vec![], int.clone()),
        ("stats_pool_misses", vec![], int.clone()),
        ("stats_active_bytes", vec![], int.clone()),
        ("stats_scratch_reuses", vec![], int.clone()),
        ("kernel_strategy", vec![], int.clone()),
        ("stats_kernel_ops", vec![], int.clone()),
        ("stats_kernel_elements", vec![], int.clone()),
        ("stats_device_transfers", vec![], int.clone()),
        ("stats_gpu_kernel_ops", vec![], int.clone()),
        ("stats_cpu_fallbacks", vec![], int.clone()),
        ("stats_graph_nodes", vec![], int.clone()),
        ("stats_lifetime_records", vec![], int.clone()),
        ("stats_released_lifetimes", vec![], int.clone()),
        ("stats_allocation_sites", vec![], int.clone()),
        ("stats_reuse_rate_per_mille", vec![], int.clone()),
        ("memory_report", vec![], Type::String),
        ("reset_stats", vec![], unit.clone()),
        (
            "requires_grad",
            vec![int.clone(), bool_ty.clone()],
            tensor_float_dynamic.clone(),
        ),
        ("diff", vec![tensor_float_rank0.clone()], unit.clone()),
        ("backward", vec![tensor_float_rank0.clone()], unit.clone()),
        ("grad", vec![int.clone()], tensor_float_dynamic.clone()),
        ("zero_grad", vec![int.clone()], unit.clone()),
        ("set_grad_enabled", vec![bool_ty.clone()], unit.clone()),
        ("grad_enabled", vec![], bool_ty.clone()),
        ("free", vec![int.clone()], unit.clone()),
        ("free_all", vec![], int.clone()),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports.types.insert(
        "Tensor".to_string(),
        ExportedType {
            members: vec![
                "shape".to_string(),
                "dtype".to_string(),
                "device".to_string(),
                "precision".to_string(),
                "layout".to_string(),
            ],
            visibility: ExportVisibility::Public,
            is_enum: false,
            struct_fields: None,
            enum_variants: None,
            enum_struct_variants: None,
        },
    );

    exports
}

fn make_std_ml() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "ml".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let float = Type::Float;
    let unit = Type::Unit;
    let bool_ty = Type::Bool;
    let tensor_float_rank0 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(0),
        dims: None,
        layout: None,
        device: None,
    };
    let tensor_float_rank2 = Type::Tensor {
        dtype: Box::new(Type::Float),
        rank: Some(2),
        dims: None,
        layout: None,
        device: None,
    };

    let functions = [
        ("module_new", vec![], int.clone()),
        (
            "module_add_parameter",
            vec![int.clone(), int.clone()],
            unit.clone(),
        ),
        ("module_parameter_count", vec![int.clone()], int.clone()),
        (
            "module_parameter",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "module_set_training",
            vec![int.clone(), bool_ty.clone()],
            unit.clone(),
        ),
        ("module_is_training", vec![int.clone()], bool_ty.clone()),
        (
            "linear",
            vec![int.clone(), int.clone(), int.clone()],
            tensor_float_rank2.clone(),
        ),
        (
            "conv2d",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
            ],
            int.clone(),
        ),
        (
            "dropout",
            vec![int.clone(), float.clone(), bool_ty.clone()],
            int.clone(),
        ),
        (
            "max_pool2d",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
                int.clone(),
            ],
            int.clone(),
        ),
        (
            "mse_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        (
            "bce_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        (
            "cross_entropy_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        (
            "nll_loss",
            vec![int.clone(), int.clone()],
            tensor_float_rank0.clone(),
        ),
        ("sgd_step", vec![int.clone(), float.clone()], unit.clone()),
        (
            "sgd_momentum_step",
            vec![int.clone(), int.clone(), float.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "adam_step",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                int.clone(),
            ],
            unit.clone(),
        ),
        (
            "adamw_step",
            vec![
                int.clone(),
                int.clone(),
                int.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                float.clone(),
                int.clone(),
                float.clone(),
            ],
            unit.clone(),
        ),
        (
            "exp_lr",
            vec![float.clone(), float.clone(), int.clone()],
            float.clone(),
        ),
        (
            "unscale_grad",
            vec![int.clone(), float.clone()],
            unit.clone(),
        ),
        (
            "dataset_from_tensors",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataset_from_csv",
            vec![Type::String, int.clone(), int.clone()],
            int.clone(),
        ),
        ("dataset_from_jsonl", vec![Type::String], int.clone()),
        (
            "dataset_from_npy",
            vec![Type::String, Type::String, int.clone()],
            int.clone(),
        ),
        ("dataset_from_directory", vec![Type::String], int.clone()),
        ("dataset_len", vec![int.clone()], int.clone()),
        (
            "dataset_map_features",
            vec![int.clone(), float.clone(), float.clone()],
            int.clone(),
        ),
        (
            "dataset_filter_label_min",
            vec![int.clone(), float.clone()],
            int.clone(),
        ),
        (
            "dataset_train_split",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataset_test_split",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataloader_new",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("dataloader_batch_count", vec![int.clone()], int.clone()),
        (
            "dataloader_batch_features",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataloader_batch_labels",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "dataframe_from_csv",
            vec![Type::String, int.clone()],
            int.clone(),
        ),
        ("dataframe_rows", vec![int.clone()], int.clone()),
        ("dataframe_cols", vec![int.clone()], int.clone()),
        (
            "dataframe_column",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports.types.insert(
        "Module".to_string(),
        ExportedType {
            members: vec!["parameters".to_string(), "training".to_string()],
            visibility: ExportVisibility::Public,
            is_enum: false,
            struct_fields: None,
            enum_variants: None,
            enum_struct_variants: None,
        },
    );

    exports
}

fn make_std_concurrent() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "concurrent".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let bool_ty = Type::Bool;
    let unit = Type::Unit;

    let functions = [
        ("task_spawn", vec![int.clone()], int.clone()),
        ("task_join", vec![int.clone()], int.clone()),
        ("task_is_done", vec![int.clone()], bool_ty.clone()),
        ("channel_new", vec![], int.clone()),
        (
            "channel_send",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        ("channel_recv", vec![int.clone()], int.clone()),
        ("channel_len", vec![int.clone()], int.clone()),
        ("channel_close", vec![int.clone()], unit.clone()),
        ("counter_new", vec![int.clone()], int.clone()),
        ("counter_add", vec![int.clone(), int.clone()], int.clone()),
        ("counter_get", vec![int.clone()], int.clone()),
        (
            "pipeline_sum",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("stats_tasks_spawned", vec![], int.clone()),
        ("stats_channels", vec![], int.clone()),
        ("reset", vec![], unit.clone()),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports
}

fn make_std_serve() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "serve".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    let int = Type::Int;
    let bool_ty = Type::Bool;

    let functions = [
        ("server_new", vec![int.clone()], int.clone()),
        ("server_warmup", vec![int.clone()], bool_ty.clone()),
        ("server_is_warm", vec![int.clone()], bool_ty.clone()),
        (
            "server_enqueue",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        (
            "server_cancel",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        (
            "server_process_batch",
            vec![int.clone(), int.clone()],
            int.clone(),
        ),
        ("server_result", vec![int.clone(), int.clone()], int.clone()),
        ("server_pending", vec![int.clone()], int.clone()),
        (
            "server_set_timeout",
            vec![int.clone(), int.clone()],
            bool_ty.clone(),
        ),
        ("server_resident_model", vec![int.clone()], int.clone()),
        (
            "server_benchmark",
            vec![int.clone(), int.clone(), int.clone()],
            int.clone(),
        ),
        ("reset", vec![], Type::Unit),
    ];

    for (name, params, return_type) in functions {
        exports
            .functions
            .insert(name.to_string(), pub_fn(params, return_type));
    }

    exports
}

fn make_std_string() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "string".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // len(s: string) -> int — number of characters (bytes for ASCII content)
    exports
        .functions
        .insert("len".to_string(), pub_fn(vec![Type::String], Type::Int));
    // contains(s: string, sub: string) -> bool
    exports.functions.insert(
        "contains".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // to_upper(s: string) -> string
    exports.functions.insert(
        "to_upper".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // to_lower(s: string) -> string
    exports.functions.insert(
        "to_lower".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // trim(s: string) -> string
    exports
        .functions
        .insert("trim".to_string(), pub_fn(vec![Type::String], Type::String));
    // starts_with(s: string, prefix: string) -> bool
    exports.functions.insert(
        "starts_with".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // ends_with(s: string, suffix: string) -> bool
    exports.functions.insert(
        "ends_with".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // concat(a: string, b: string) -> string
    exports.functions.insert(
        "concat".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::String),
    );
    // repeat_str(s: string, n: int) -> string
    exports.functions.insert(
        "repeat_str".to_string(),
        pub_fn(vec![Type::String, Type::Int], Type::String),
    );
    // char_at(s: string, index: int) -> int  (returns char code; -1 if out of bounds)
    exports.functions.insert(
        "char_at".to_string(),
        pub_fn(vec![Type::String, Type::Int], Type::Int),
    );
    // substring(s: string, start: int, end: int) -> string
    exports.functions.insert(
        "substring".to_string(),
        pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String),
    );
    // replace(s: string, from: string, to: string) -> string
    exports.functions.insert(
        "replace".to_string(),
        pub_fn(vec![Type::String, Type::String, Type::String], Type::String),
    );
    // index_of(s: string, sub: string) -> int  (-1 if not found)
    exports.functions.insert(
        "index_of".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Int),
    );
    // split_first(s: string, sep: string) -> string
    exports.functions.insert(
        "split_first".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::String),
    );
    // split_last(s: string, sep: string) -> string
    exports.functions.insert(
        "split_last".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::String),
    );
    // is_empty(s: string) -> bool
    exports.functions.insert(
        "is_empty".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );
    // count_occurrences(s: string, sub: string) -> int
    exports.functions.insert(
        "count_occurrences".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Int),
    );
    // split_by(s: string, sep: string) -> int  (returns list handle; each element is a string pointer)
    exports.functions.insert(
        "split_by".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Int),
    );
    // pad_left(s: string, width: int, pad_char: int) -> string
    exports.functions.insert(
        "pad_left".to_string(),
        pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String),
    );
    // pad_right(s: string, width: int, pad_char: int) -> string
    exports.functions.insert(
        "pad_right".to_string(),
        pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String),
    );
    // reverse_str(s: string) -> string
    exports.functions.insert(
        "reverse_str".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );

    exports
}

fn make_std_convert() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "convert".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // to_string(val: int) -> string  (also accepts float via Unknown type)
    exports.functions.insert(
        "int_to_string".to_string(),
        pub_fn(vec![Type::Int], Type::String),
    );
    // float_to_string(val: float) -> string
    exports.functions.insert(
        "float_to_string".to_string(),
        pub_fn(vec![Type::Float], Type::String),
    );
    // bool_to_string(val: bool) -> string
    exports.functions.insert(
        "bool_to_string".to_string(),
        pub_fn(vec![Type::Bool], Type::String),
    );
    // string_to_int(s: string) -> int  (returns 0 on parse error)
    exports.functions.insert(
        "string_to_int".to_string(),
        pub_fn(vec![Type::String], Type::Int),
    );
    // string_to_float(s: string) -> float  (returns 0.0 on parse error)
    exports.functions.insert(
        "string_to_float".to_string(),
        pub_fn(vec![Type::String], Type::Float),
    );
    // int_to_float(val: int) -> float
    exports.functions.insert(
        "int_to_float".to_string(),
        pub_fn(vec![Type::Int], Type::Float),
    );
    // float_to_int(val: float) -> int  (truncates)
    exports.functions.insert(
        "float_to_int".to_string(),
        pub_fn(vec![Type::Float], Type::Int),
    );
    // string_to_int_or(s: string, default: int) -> int
    exports.functions.insert(
        "string_to_int_or".to_string(),
        pub_fn(vec![Type::String, Type::Int], Type::Int),
    );
    // string_to_float_or(s: string, default: float) -> float
    exports.functions.insert(
        "string_to_float_or".to_string(),
        pub_fn(vec![Type::String, Type::Float], Type::Float),
    );
    // string_to_bool(s: string) -> bool
    exports.functions.insert(
        "string_to_bool".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );
    // bool_to_int(b: bool) -> int
    exports.functions.insert(
        "bool_to_int".to_string(),
        pub_fn(vec![Type::Bool], Type::Int),
    );

    exports
}

fn make_std_random() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "random".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // random_seed(seed: int) -> unit
    exports.functions.insert(
        "random_seed".to_string(),
        pub_fn(vec![Type::Int], Type::Unit),
    );
    // random_int(min: int, max: int) -> int
    exports.functions.insert(
        "random_int".to_string(),
        pub_fn(vec![Type::Int, Type::Int], Type::Int),
    );
    // random_float() -> float  ([0.0, 1.0))
    exports
        .functions
        .insert("random_float".to_string(), pub_fn(vec![], Type::Float));
    // random_bool() -> bool
    exports
        .functions
        .insert("random_bool".to_string(), pub_fn(vec![], Type::Bool));

    exports
}

fn make_std_fs() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "fs".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // fs_read(path: string) -> string  (reads entire file; returns "" on error)
    exports.functions.insert(
        "fs_read".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // fs_write(path: string, content: string) -> bool
    exports.functions.insert(
        "fs_write".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // fs_append(path: string, content: string) -> bool
    exports.functions.insert(
        "fs_append".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // fs_exists(path: string) -> bool
    exports.functions.insert(
        "fs_exists".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );
    // fs_remove(path: string) -> bool
    exports.functions.insert(
        "fs_remove".to_string(),
        pub_fn(vec![Type::String], Type::Bool),
    );

    exports
}

fn make_std_env() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "env".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // env_get(key: string) -> string  (returns "" if not set)
    exports.functions.insert(
        "env_get".to_string(),
        pub_fn(vec![Type::String], Type::String),
    );
    // env_set(key: string, value: string) -> bool
    exports.functions.insert(
        "env_set".to_string(),
        pub_fn(vec![Type::String, Type::String], Type::Bool),
    );
    // env_args_count() -> int
    exports
        .functions
        .insert("env_args_count".to_string(), pub_fn(vec![], Type::Int));
    // env_arg(index: int) -> string  (returns "" if out of bounds)
    exports
        .functions
        .insert("env_arg".to_string(), pub_fn(vec![Type::Int], Type::String));

    exports
}

fn make_std_option() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "option".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // is_some(opt: unknown) -> bool
    exports.functions.insert(
        "is_some".to_string(),
        pub_fn(vec![Type::Unknown], Type::Bool),
    );
    // is_none(opt: unknown) -> bool
    exports.functions.insert(
        "is_none".to_string(),
        pub_fn(vec![Type::Unknown], Type::Bool),
    );
    // option_unwrap(opt: unknown) -> unknown  (panics on None)
    exports.functions.insert(
        "option_unwrap".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unknown),
    );
    // option_unwrap_or(opt: unknown, default: unknown) -> unknown
    exports.functions.insert(
        "option_unwrap_or".to_string(),
        pub_fn(vec![Type::Unknown, Type::Unknown], Type::Unknown),
    );

    exports
}

fn make_std_result() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "result".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // is_ok(res: unknown) -> bool
    exports
        .functions
        .insert("is_ok".to_string(), pub_fn(vec![Type::Unknown], Type::Bool));
    // is_err(res: unknown) -> bool
    exports.functions.insert(
        "is_err".to_string(),
        pub_fn(vec![Type::Unknown], Type::Bool),
    );
    // result_unwrap(res: unknown) -> unknown  (panics on Err)
    exports.functions.insert(
        "result_unwrap".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unknown),
    );
    // result_unwrap_or(res: unknown, default: unknown) -> unknown
    exports.functions.insert(
        "result_unwrap_or".to_string(),
        pub_fn(vec![Type::Unknown, Type::Unknown], Type::Unknown),
    );
    // result_unwrap_err(res: unknown) -> unknown  (panics on Ok)
    exports.functions.insert(
        "result_unwrap_err".to_string(),
        pub_fn(vec![Type::Unknown], Type::Unknown),
    );

    exports
}

fn make_std_char() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "char".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // All functions take an int (Unicode code point) and return bool or int.
    // is_alpha(c: int) -> bool
    exports
        .functions
        .insert("is_alpha".to_string(), pub_fn(vec![Type::Int], Type::Bool));
    // is_digit_char(c: int) -> bool
    exports.functions.insert(
        "is_digit_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // is_whitespace_char(c: int) -> bool
    exports.functions.insert(
        "is_whitespace_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // is_upper_char(c: int) -> bool
    exports.functions.insert(
        "is_upper_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // is_lower_char(c: int) -> bool
    exports.functions.insert(
        "is_lower_char".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );
    // to_upper_char(c: int) -> int  (returns uppercased code point)
    exports.functions.insert(
        "to_upper_char".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    // to_lower_char(c: int) -> int  (returns lowercased code point)
    exports.functions.insert(
        "to_lower_char".to_string(),
        pub_fn(vec![Type::Int], Type::Int),
    );
    // is_alphanumeric(c: int) -> bool
    exports.functions.insert(
        "is_alphanumeric".to_string(),
        pub_fn(vec![Type::Int], Type::Bool),
    );

    exports
}

fn make_std_time() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "time".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // time_now_millis() -> int  (milliseconds since Unix epoch; -1 on error)
    exports
        .functions
        .insert("time_now_millis".to_string(), pub_fn(vec![], Type::Int));
    // time_now_secs() -> int  (seconds since Unix epoch; -1 on error)
    exports
        .functions
        .insert("time_now_secs".to_string(), pub_fn(vec![], Type::Int));
    // sleep_ms(ms: int) -> unit  (sleeps for ms milliseconds)
    exports
        .functions
        .insert("sleep_ms".to_string(), pub_fn(vec![Type::Int], Type::Unit));

    exports
}
