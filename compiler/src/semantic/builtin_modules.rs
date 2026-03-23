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
    // Convenience aliases used in existing examples
    registry.register_module("spectra.std.io".to_string(), make_std_io());
    registry.register_module("spectra.std.math".to_string(), make_std_math());
    registry.register_module("spectra.std.collections".to_string(), make_std_collections());
    registry.register_module("spectra.std.string".to_string(), make_std_string());
    registry.register_module("spectra.std.convert".to_string(), make_std_convert());
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
    // println(value: any) -> unit  (print + newline via f-string / wrapper)
    exports.functions.insert("println".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // eprintln(value: any) -> unit
    exports.functions.insert("eprintln".to_string(), pub_fn(vec![Type::Unknown], Type::Unit));
    // flush() -> unit
    exports.functions.insert("flush".to_string(), pub_fn(vec![], Type::Unit));
    // read_line() -> string  (future)
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
    // list_clear(handle: int) -> unit
    exports.functions.insert("list_clear".to_string(), pub_fn(vec![Type::Int], Type::Unit));
    // list_free(handle: int) -> unit
    exports.functions.insert("list_free".to_string(), pub_fn(vec![Type::Int], Type::Unit));

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

    exports
}
