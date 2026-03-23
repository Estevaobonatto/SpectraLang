// Builtin (virtual) module registrations
// Maps well-known `std.*` module paths to their exported function signatures
// without requiring physical `.spectra` files.  The actual implementation of
// each function lives in the runtime FFI layer (runtime/src/stdlib/mod.rs).

use crate::ast::Type;
use super::module_registry::{ExportedFunction, ExportedType, ExportVisibility, ModuleExports, ModuleRegistry};

/// Register all built-in standard library modules in the given registry.
pub fn register_builtin_modules(registry: &mut ModuleRegistry) {
    registry.register_module("std.io".to_string(), make_std_io());
    registry.register_module("std.math".to_string(), make_std_math());
    registry.register_module("std.collections".to_string(), make_std_collections());
    registry.register_module("std.string".to_string(), make_std_string());
    registry.register_module("std.convert".to_string(), make_std_convert());
    registry.register_module("std.random".to_string(), make_std_random());
    // Convenience aliases used in existing examples
    registry.register_module("spectra.std.io".to_string(), make_std_io());
    registry.register_module("spectra.std.math".to_string(), make_std_math());
    registry.register_module("spectra.std.collections".to_string(), make_std_collections());
    registry.register_module("spectra.std.string".to_string(), make_std_string());
    registry.register_module("spectra.std.convert".to_string(), make_std_convert());
    registry.register_module("spectra.std.random".to_string(), make_std_random());
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
    exports.functions.insert("print".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // println(value: any) -> unit  (print + newline)
    exports.functions.insert("println".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // eprint(value: any) -> unit  (stderr, no newline)
    exports.functions.insert("eprint".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // eprintln(value: any) -> unit
    exports.functions.insert("eprintln".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // flush() -> unit
    exports.functions.insert("flush".to_string(), pub_fn(vec![], Type::Unit));
    // read_line() -> string
    exports.functions.insert("read_line".to_string(), pub_fn(vec![], Type::String));

    exports
}

fn make_std_math() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "math".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    exports.functions.insert("abs".to_string(), pub_fn(vec![Type::Int], Type::Int));
    exports.functions.insert("min".to_string(), pub_fn(vec![Type::Int, Type::Int], Type::Int));
    exports.functions.insert("max".to_string(), pub_fn(vec![Type::Int, Type::Int], Type::Int));
    exports.functions.insert("clamp".to_string(), pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Int));
    exports.functions.insert("sqrt_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("pow_f".to_string(), pub_fn(vec![Type::Float, Type::Float], Type::Float));
    exports.functions.insert("floor_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("ceil_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("round_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("sin_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("cos_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("tan_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("log_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("log2_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("log10_f".to_string(), pub_fn(vec![Type::Float], Type::Float));
    exports.functions.insert("atan2_f".to_string(), pub_fn(vec![Type::Float, Type::Float], Type::Float));
    exports.functions.insert("pi".to_string(), pub_fn(vec![], Type::Float));
    exports.functions.insert("e_const".to_string(), pub_fn(vec![], Type::Float));

    exports
}

fn make_std_collections() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "collections".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // list_new() -> int (handle)
    exports.functions.insert("list_new".to_string(), pub_fn(vec![], Type::Int));
    // list_push(handle: int, value: int) -> unit
    exports.functions.insert("list_push".to_string(), pub_fn(vec![Type::Int, Type::Int], Type::Unit));
    // list_len(handle: int) -> int
    exports.functions.insert("list_len".to_string(), pub_fn(vec![Type::Int], Type::Int));
    // list_get(handle: int, index: int) -> int  (-1 if out of bounds)
    exports.functions.insert("list_get".to_string(), pub_fn(vec![Type::Int, Type::Int], Type::Int));
    // list_set(handle: int, index: int, value: int) -> unit
    exports.functions.insert("list_set".to_string(), pub_fn(vec![Type::Int, Type::Int, Type::Int], Type::Unit));
    // list_contains(handle: int, value: int) -> bool
    exports.functions.insert("list_contains".to_string(), pub_fn(vec![Type::Int, Type::Int], Type::Bool));
    // list_clear(handle: int) -> unit
    exports.functions.insert("list_clear".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // list_free(handle: int) -> unit
    exports.functions.insert("list_free".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // list_free_all() -> int
    exports.functions.insert("list_free_all".to_string(), pub_fn(vec![], Type::Int));

    // type aliases
    exports.types.insert("List".to_string(), ExportedType {
        members: vec!["new".to_string(), "push".to_string(), "len".to_string()],
        visibility: ExportVisibility::Public,
        is_enum: false,
    });

    exports
}

fn make_std_string() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "string".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // len(s: string) -> int — number of characters (bytes for ASCII content)
    exports.functions.insert("len".to_string(), pub_fn(vec![Type::String], Type::Int));
    // contains(s: string, sub: string) -> bool
    exports.functions.insert("contains".to_string(), pub_fn(vec![Type::String, Type::String], Type::Bool));
    // to_upper(s: string) -> string
    exports.functions.insert("to_upper".to_string(), pub_fn(vec![Type::String], Type::String));
    // to_lower(s: string) -> string
    exports.functions.insert("to_lower".to_string(), pub_fn(vec![Type::String], Type::String));
    // trim(s: string) -> string
    exports.functions.insert("trim".to_string(), pub_fn(vec![Type::String], Type::String));
    // starts_with(s: string, prefix: string) -> bool
    exports.functions.insert("starts_with".to_string(), pub_fn(vec![Type::String, Type::String], Type::Bool));
    // ends_with(s: string, suffix: string) -> bool
    exports.functions.insert("ends_with".to_string(), pub_fn(vec![Type::String, Type::String], Type::Bool));
    // concat(a: string, b: string) -> string
    exports.functions.insert("concat".to_string(), pub_fn(vec![Type::String, Type::String], Type::String));
    // repeat_str(s: string, n: int) -> string
    exports.functions.insert("repeat_str".to_string(), pub_fn(vec![Type::String, Type::Int], Type::String));
    // char_at(s: string, index: int) -> int  (returns char code; -1 if out of bounds)
    exports.functions.insert("char_at".to_string(), pub_fn(vec![Type::String, Type::Int], Type::Int));
    // substring(s: string, start: int, end: int) -> string
    exports.functions.insert("substring".to_string(), pub_fn(vec![Type::String, Type::Int, Type::Int], Type::String));
    // replace(s: string, from: string, to: string) -> string
    exports.functions.insert("replace".to_string(), pub_fn(vec![Type::String, Type::String, Type::String], Type::String));
    // index_of(s: string, sub: string) -> int  (-1 if not found)
    exports.functions.insert("index_of".to_string(), pub_fn(vec![Type::String, Type::String], Type::Int));
    // split_first(s: string, sep: string) -> string
    exports.functions.insert("split_first".to_string(), pub_fn(vec![Type::String, Type::String], Type::String));
    // split_last(s: string, sep: string) -> string
    exports.functions.insert("split_last".to_string(), pub_fn(vec![Type::String, Type::String], Type::String));
    // is_empty(s: string) -> bool
    exports.functions.insert("is_empty".to_string(), pub_fn(vec![Type::String], Type::Bool));
    // count_occurrences(s: string, sub: string) -> int
    exports.functions.insert("count_occurrences".to_string(), pub_fn(vec![Type::String, Type::String], Type::Int));

    exports
}

fn make_std_convert() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "convert".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // to_string(val: int) -> string  (also accepts float via Unknown type)
    exports.functions.insert("int_to_string".to_string(), pub_fn(vec![Type::Int], Type::String));
    // float_to_string(val: float) -> string
    exports.functions.insert("float_to_string".to_string(), pub_fn(vec![Type::Float], Type::String));
    // bool_to_string(val: bool) -> string
    exports.functions.insert("bool_to_string".to_string(), pub_fn(vec![Type::Bool], Type::String));
    // string_to_int(s: string) -> int  (returns 0 on parse error)
    exports.functions.insert("string_to_int".to_string(), pub_fn(vec![Type::String], Type::Int));
    // string_to_float(s: string) -> float  (returns 0.0 on parse error)
    exports.functions.insert("string_to_float".to_string(), pub_fn(vec![Type::String], Type::Float));
    // int_to_float(val: int) -> float
    exports.functions.insert("int_to_float".to_string(), pub_fn(vec![Type::Int], Type::Float));
    // float_to_int(val: float) -> int  (truncates)
    exports.functions.insert("float_to_int".to_string(), pub_fn(vec![Type::Float], Type::Int));
    // string_to_int_or(s: string, default: int) -> int
    exports.functions.insert("string_to_int_or".to_string(), pub_fn(vec![Type::String, Type::Int], Type::Int));
    // string_to_float_or(s: string, default: float) -> float
    exports.functions.insert("string_to_float_or".to_string(), pub_fn(vec![Type::String, Type::Float], Type::Float));
    // string_to_bool(s: string) -> bool
    exports.functions.insert("string_to_bool".to_string(), pub_fn(vec![Type::String], Type::Bool));
    // bool_to_int(b: bool) -> int
    exports.functions.insert("bool_to_int".to_string(), pub_fn(vec![Type::Bool], Type::Int));

    exports
}

fn make_std_random() -> ModuleExports {
    let mut exports = ModuleExports {
        stdlib_path: Some(vec!["std".to_string(), "random".to_string()]),
        package_name: Some("std".to_string()),
        ..Default::default()
    };

    // random_seed(seed: int) -> unit
    exports.functions.insert("random_seed".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // random_int(min: int, max: int) -> int
    exports.functions.insert("random_int".to_string(), pub_fn(vec![Type::Int, Type::Int], Type::Int));
    // random_float() -> float  ([0.0, 1.0))
    exports.functions.insert("random_float".to_string(), pub_fn(vec![], Type::Float));
    // random_bool() -> bool
    exports.functions.insert("random_bool".to_string(), pub_fn(vec![], Type::Bool));

    exports
}
