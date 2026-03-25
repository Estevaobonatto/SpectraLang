use crate::ffi::{
    register_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INTERNAL_ERROR,
    HOST_STATUS_INVALID_ARGUMENT, HOST_STATUS_NOT_FOUND, HOST_STATUS_SUCCESS,
};
use crate::initialize;
use crate::memory::ManualBox;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::slice;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::ffi::{clear_host_functions, lookup_host_function};
#[cfg(test)]
use std::ptr;

const MATH_ABS: &str = "spectra.std.math.abs";
const MATH_MIN: &str = "spectra.std.math.min";
const MATH_MAX: &str = "spectra.std.math.max";
const MATH_CLAMP: &str = "spectra.std.math.clamp";
const MATH_SQRT_F: &str = "spectra.std.math.sqrt_f";
const MATH_POW_F: &str = "spectra.std.math.pow_f";
const MATH_FLOOR_F: &str = "spectra.std.math.floor_f";
const MATH_CEIL_F: &str = "spectra.std.math.ceil_f";
const MATH_ROUND_F: &str = "spectra.std.math.round_f";

const IO_PRINT: &str = "spectra.std.io.print";
const IO_PRINTLN: &str = "spectra.std.io.println";
const IO_FLUSH: &str = "spectra.std.io.flush";
const IO_EPRINT: &str = "spectra.std.io.eprint";
const IO_EPRINTLN: &str = "spectra.std.io.eprintln";
const IO_READ_LINE: &str = "spectra.std.io.read_line";

// ── std.math (novos) ─────────────────────────────────────────────────────────
const MATH_SIN_F: &str = "spectra.std.math.sin_f";
const MATH_COS_F: &str = "spectra.std.math.cos_f";
const MATH_TAN_F: &str = "spectra.std.math.tan_f";
const MATH_LOG_F: &str = "spectra.std.math.log_f";
const MATH_LOG2_F: &str = "spectra.std.math.log2_f";
const MATH_LOG10_F: &str = "spectra.std.math.log10_f";
const MATH_ATAN2_F: &str = "spectra.std.math.atan2_f";
const MATH_PI: &str = "spectra.std.math.pi";
const MATH_E_CONST: &str = "spectra.std.math.e_const";

// ── std.string ──────────────────────────────────────────────────────────────
const STR_LEN: &str = "spectra.std.string.len";
const STR_CONTAINS: &str = "spectra.std.string.contains";
const STR_TO_UPPER: &str = "spectra.std.string.to_upper";
const STR_TO_LOWER: &str = "spectra.std.string.to_lower";
const STR_TRIM: &str = "spectra.std.string.trim";
const STR_STARTS_WITH: &str = "spectra.std.string.starts_with";
const STR_ENDS_WITH: &str = "spectra.std.string.ends_with";
const STR_CONCAT: &str = "spectra.std.string.concat";
const STR_REPEAT: &str = "spectra.std.string.repeat_str";
const STR_CHAR_AT: &str = "spectra.std.string.char_at";
const STR_SUBSTRING: &str = "spectra.std.string.substring";
const STR_REPLACE: &str = "spectra.std.string.replace";
const STR_INDEX_OF: &str = "spectra.std.string.index_of";
const STR_SPLIT_FIRST: &str = "spectra.std.string.split_first";
const STR_SPLIT_LAST: &str = "spectra.std.string.split_last";
const STR_IS_EMPTY: &str = "spectra.std.string.is_empty";
const STR_COUNT: &str = "spectra.std.string.count_occurrences";

// ── std.convert ─────────────────────────────────────────────────────────────
const CONV_INT_TO_STRING: &str = "spectra.std.convert.int_to_string";
const CONV_FLOAT_TO_STRING: &str = "spectra.std.convert.float_to_string";
const CONV_BOOL_TO_STRING: &str = "spectra.std.convert.bool_to_string";
const CONV_STRING_TO_INT: &str = "spectra.std.convert.string_to_int";
const CONV_STRING_TO_FLOAT: &str = "spectra.std.convert.string_to_float";
const CONV_INT_TO_FLOAT: &str = "spectra.std.convert.int_to_float";
const CONV_FLOAT_TO_INT: &str = "spectra.std.convert.float_to_int";
const CONV_STRING_TO_INT_OR: &str = "spectra.std.convert.string_to_int_or";
const CONV_STRING_TO_FLOAT_OR: &str = "spectra.std.convert.string_to_float_or";
const CONV_STRING_TO_BOOL: &str = "spectra.std.convert.string_to_bool";
const CONV_BOOL_TO_INT: &str = "spectra.std.convert.bool_to_int";

// ── std.random ───────────────────────────────────────────────────────────────
const RAND_SEED: &str = "spectra.std.random.random_seed";
const RAND_INT: &str = "spectra.std.random.random_int";
const RAND_FLOAT: &str = "spectra.std.random.random_float";
const RAND_BOOL: &str = "spectra.std.random.random_bool";

/// Type tags for the polymorphic io.print host call.
/// Args are pairs: (type_tag: i64, value: i64).
const _PRINT_TAG_INT: SpectraHostValue = 0;
const PRINT_TAG_STR: SpectraHostValue = 1;
const PRINT_TAG_BOOL: SpectraHostValue = 2;
const PRINT_TAG_FLOAT: SpectraHostValue = 3;

const LIST_NEW: &str = "spectra.std.collections.list_new";
const LIST_PUSH: &str = "spectra.std.collections.list_push";
const LIST_LEN: &str = "spectra.std.collections.list_len";
const LIST_GET: &str = "spectra.std.collections.list_get";
const LIST_SET: &str = "spectra.std.collections.list_set";
const LIST_CONTAINS: &str = "spectra.std.collections.list_contains";
const LIST_CLEAR: &str = "spectra.std.collections.list_clear";
const LIST_FREE: &str = "spectra.std.collections.list_free";
const LIST_FREE_ALL: &str = "spectra.std.collections.list_free_all";
const LIST_POP: &str = "spectra.std.collections.list_pop";
const LIST_POP_FRONT: &str = "spectra.std.collections.list_pop_front";
const LIST_INSERT_AT: &str = "spectra.std.collections.list_insert_at";
const LIST_REMOVE_AT: &str = "spectra.std.collections.list_remove_at";
const LIST_INDEX_OF: &str = "spectra.std.collections.list_index_of";
const LIST_SORT: &str = "spectra.std.collections.list_sort";

// ── std.collections higher-order functions ──────────────────────────────────
const LIST_MAP: &str = "spectra.std.collections.list_map";
const LIST_FILTER: &str = "spectra.std.collections.list_filter";
const LIST_REDUCE: &str = "spectra.std.collections.list_reduce";
const LIST_SORT_BY: &str = "spectra.std.collections.list_sort_by";

// ── std.fs ───────────────────────────────────────────────────────────────────
const FS_READ: &str = "spectra.std.fs.fs_read";
const FS_WRITE: &str = "spectra.std.fs.fs_write";
const FS_APPEND: &str = "spectra.std.fs.fs_append";
const FS_EXISTS: &str = "spectra.std.fs.fs_exists";
const FS_REMOVE: &str = "spectra.std.fs.fs_remove";

// ── std.env ──────────────────────────────────────────────────────────────────
const ENV_GET: &str = "spectra.std.env.env_get";
const ENV_SET: &str = "spectra.std.env.env_set";
const ENV_ARGS_COUNT: &str = "spectra.std.env.env_args_count";
const ENV_ARG: &str = "spectra.std.env.env_arg";

// ── std.option ───────────────────────────────────────────────────────────────
const OPTION_IS_SOME: &str = "spectra.std.option.is_some";
const OPTION_IS_NONE: &str = "spectra.std.option.is_none";
const OPTION_UNWRAP: &str = "spectra.std.option.option_unwrap";
const OPTION_UNWRAP_OR: &str = "spectra.std.option.option_unwrap_or";

// ── std.result ───────────────────────────────────────────────────────────────
const RESULT_IS_OK: &str = "spectra.std.result.is_ok";
const RESULT_IS_ERR: &str = "spectra.std.result.is_err";
const RESULT_UNWRAP: &str = "spectra.std.result.result_unwrap";
const RESULT_UNWRAP_OR: &str = "spectra.std.result.result_unwrap_or";
const RESULT_UNWRAP_ERR: &str = "spectra.std.result.result_unwrap_err";

// ── std.string (novos) ───────────────────────────────────────────────────────
const STR_SPLIT_BY: &str = "spectra.std.string.split_by";
const STR_PAD_LEFT: &str = "spectra.std.string.pad_left";
const STR_PAD_RIGHT: &str = "spectra.std.string.pad_right";
const STR_REVERSE: &str = "spectra.std.string.reverse_str";

// ── std.math (novos) ─────────────────────────────────────────────────────────
const MATH_SIGN: &str = "spectra.std.math.sign";
const MATH_GCD: &str = "spectra.std.math.gcd";
const MATH_LCM: &str = "spectra.std.math.lcm";
const MATH_IS_NAN_F: &str = "spectra.std.math.is_nan_f";
const MATH_IS_INFINITE_F: &str = "spectra.std.math.is_infinite_f";
const MATH_ABS_F: &str = "spectra.std.math.abs_f";

// ── std.char ─────────────────────────────────────────────────────────────────
const CHAR_IS_ALPHA: &str = "spectra.std.char.is_alpha";
const CHAR_IS_DIGIT: &str = "spectra.std.char.is_digit_char";
const CHAR_IS_WHITESPACE: &str = "spectra.std.char.is_whitespace_char";
const CHAR_IS_UPPER: &str = "spectra.std.char.is_upper_char";
const CHAR_IS_LOWER: &str = "spectra.std.char.is_lower_char";
const CHAR_TO_UPPER: &str = "spectra.std.char.to_upper_char";
const CHAR_TO_LOWER: &str = "spectra.std.char.to_lower_char";
const CHAR_IS_ALPHANUMERIC: &str = "spectra.std.char.is_alphanumeric";

// ── std.time ─────────────────────────────────────────────────────────────────
const TIME_NOW_MILLIS: &str = "spectra.std.time.time_now_millis";
const TIME_NOW_SECS: &str = "spectra.std.time.time_now_secs";
const TIME_SLEEP_MS: &str = "spectra.std.time.sleep_ms";

// ── std.io (novos) ───────────────────────────────────────────────────────────
const IO_INPUT: &str = "spectra.std.io.input";

/// Registers the standard library host functions.
pub fn register() {
    register_math();
    register_io();
    register_collections();
    register_map();
    register_string();
    register_convert();
    register_random();
    register_fs();
    register_env();
    register_option();
    register_result();
    register_char();
    register_time();
}

fn register_math() {
    register_host_function(MATH_ABS, std_math_abs);
    register_host_function(MATH_MIN, std_math_min);
    register_host_function(MATH_MAX, std_math_max);
    register_host_function(MATH_CLAMP, std_math_clamp);
    register_host_function(MATH_SQRT_F, std_math_sqrt_f);
    register_host_function(MATH_POW_F, std_math_pow_f);
    register_host_function(MATH_FLOOR_F, std_math_floor_f);
    register_host_function(MATH_CEIL_F, std_math_ceil_f);
    register_host_function(MATH_ROUND_F, std_math_round_f);
    register_host_function(MATH_SIN_F, std_math_sin_f);
    register_host_function(MATH_COS_F, std_math_cos_f);
    register_host_function(MATH_TAN_F, std_math_tan_f);
    register_host_function(MATH_LOG_F, std_math_log_f);
    register_host_function(MATH_LOG2_F, std_math_log2_f);
    register_host_function(MATH_LOG10_F, std_math_log10_f);
    register_host_function(MATH_ATAN2_F, std_math_atan2_f);
    register_host_function(MATH_PI, std_math_pi);
    register_host_function(MATH_E_CONST, std_math_e_const);
    register_host_function(MATH_SIGN, std_math_sign);
    register_host_function(MATH_GCD, std_math_gcd);
    register_host_function(MATH_LCM, std_math_lcm);
    register_host_function(MATH_IS_NAN_F, std_math_is_nan_f);
    register_host_function(MATH_IS_INFINITE_F, std_math_is_infinite_f);
    register_host_function(MATH_ABS_F, std_math_abs_f);
}

fn register_io() {
    register_host_function(IO_PRINT, std_io_print);
    register_host_function(IO_PRINTLN, std_io_println);
    register_host_function(IO_FLUSH, std_io_flush);
    register_host_function(IO_EPRINT, std_io_eprint);
    register_host_function(IO_EPRINTLN, std_io_eprintln);
    register_host_function(IO_READ_LINE, std_io_read_line);
    register_host_function(IO_INPUT, std_io_input);
}

fn register_collections() {
    register_host_function(LIST_NEW, std_list_new);
    register_host_function(LIST_PUSH, std_list_push);
    register_host_function(LIST_LEN, std_list_len);
    register_host_function(LIST_GET, std_list_get);
    register_host_function(LIST_SET, std_list_set);
    register_host_function(LIST_CONTAINS, std_list_contains);
    register_host_function(LIST_CLEAR, std_list_clear);
    register_host_function(LIST_FREE, std_list_free);
    register_host_function(LIST_FREE_ALL, std_list_free_all);
    register_host_function(LIST_POP, std_list_pop);
    register_host_function(LIST_POP_FRONT, std_list_pop_front);
    register_host_function(LIST_INSERT_AT, std_list_insert_at);
    register_host_function(LIST_REMOVE_AT, std_list_remove_at);
    register_host_function(LIST_INDEX_OF, std_list_index_of);
    register_host_function(LIST_SORT, std_list_sort);
    register_host_function(LIST_MAP, std_list_map);
    register_host_function(LIST_FILTER, std_list_filter);
    register_host_function(LIST_REDUCE, std_list_reduce);
    register_host_function(LIST_SORT_BY, std_list_sort_by);
}

fn register_fs() {
    register_host_function(FS_READ, std_fs_read);
    register_host_function(FS_WRITE, std_fs_write);
    register_host_function(FS_APPEND, std_fs_append);
    register_host_function(FS_EXISTS, std_fs_exists);
    register_host_function(FS_REMOVE, std_fs_remove);
}

fn register_env() {
    register_host_function(ENV_GET, std_env_get);
    register_host_function(ENV_SET, std_env_set);
    register_host_function(ENV_ARGS_COUNT, std_env_args_count);
    register_host_function(ENV_ARG, std_env_arg);
}

fn register_option() {
    register_host_function(OPTION_IS_SOME, std_option_is_some);
    register_host_function(OPTION_IS_NONE, std_option_is_none);
    register_host_function(OPTION_UNWRAP, std_option_unwrap);
    register_host_function(OPTION_UNWRAP_OR, std_option_unwrap_or);
}

fn register_result() {
    register_host_function(RESULT_IS_OK, std_result_is_ok);
    register_host_function(RESULT_IS_ERR, std_result_is_err);
    register_host_function(RESULT_UNWRAP, std_result_unwrap);
    register_host_function(RESULT_UNWRAP_OR, std_result_unwrap_or);
    register_host_function(RESULT_UNWRAP_ERR, std_result_unwrap_err);
}

extern "C" fn std_math_abs(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_ptr = ctx_ref.args;
        let args_len = ctx_ref.arg_len;
        let results_ptr = ctx_ref.results;
        let results_len = ctx_ref.result_len;

        let args = slice::from_raw_parts(args_ptr, args_len);
        let results = slice::from_raw_parts_mut(results_ptr, results_len);
        results[0] = args[0].abs();
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_min(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_ptr = ctx_ref.args;
        let args_len = ctx_ref.arg_len;
        let results_ptr = ctx_ref.results;
        let results_len = ctx_ref.result_len;

        let args = slice::from_raw_parts(args_ptr, args_len);
        let results = slice::from_raw_parts_mut(results_ptr, results_len);
        results[0] = args[0].min(args[1]);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_max(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_ptr = ctx_ref.args;
        let args_len = ctx_ref.arg_len;
        let results_ptr = ctx_ref.results;
        let results_len = ctx_ref.result_len;

        let args = slice::from_raw_parts(args_ptr, args_len);
        let results = slice::from_raw_parts_mut(results_ptr, results_len);
        results[0] = args[0].max(args[1]);
    }

    HOST_STATUS_SUCCESS
}

/// Clamp an integer value between min and max (inclusive).
extern "C" fn std_math_clamp(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = args[0].clamp(args[1], args[2]);
    }
    HOST_STATUS_SUCCESS
}

/// Square root. Value and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_sqrt_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let f = f64::from_bits(args[0] as u64).sqrt();
        results[0] = f.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Power. Both arguments and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_pow_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let base = f64::from_bits(args[0] as u64);
        let exp = f64::from_bits(args[1] as u64);
        results[0] = base.powf(exp).to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Floor. Value and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_floor_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let f = f64::from_bits(args[0] as u64).floor();
        results[0] = f.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Ceil. Value and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_ceil_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let f = f64::from_bits(args[0] as u64).ceil();
        results[0] = f.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Round. Value and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_round_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let f = f64::from_bits(args[0] as u64).round();
        results[0] = f.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Sine. Argument and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_sin_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).sin().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Cosine. Argument and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_cos_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).cos().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Tangent. Argument and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_tan_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).tan().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Natural logarithm. Argument and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_log_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).ln().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Base-2 logarithm. Argument and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_log2_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).log2().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Base-10 logarithm. Argument and result are f64 bits reinterpreted as i64.
extern "C" fn std_math_log10_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).log10().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Two-argument arctangent (atan2). Arguments y, x and result are f64 bits.
extern "C" fn std_math_atan2_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let y = f64::from_bits(args[0] as u64);
        let x = f64::from_bits(args[1] as u64);
        results[0] = y.atan2(x).to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns the mathematical constant PI as f64 bits.
extern "C" fn std_math_pi(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = std::f64::consts::PI.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns the mathematical constant E as f64 bits.
extern "C" fn std_math_e_const(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = std::f64::consts::E.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Polymorphic print function (no trailing newline — use `println` for newline).
///
/// Arguments are (type_tag: i64, value: i64) pairs:
///   - tag 0 → print as integer
///   - tag 1 → print as null-terminated string (value is a pointer)
///   - tag 2 → print as bool ("true"/"false")
///   - tag 3 → print as float (value reinterpreted as f64 bits)
extern "C" fn std_io_print(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len > 0 && ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_len = ctx_ref.arg_len;
        let args = if args_len == 0 {
            &[] as &[SpectraHostValue]
        } else {
            slice::from_raw_parts(ctx_ref.args, args_len)
        };

        let mut stdout = io::stdout();
        let values_count = args_len / 2;

        for i in 0..values_count {
            if i > 0 {
                if write!(stdout, " ").is_err() {
                    return HOST_STATUS_INTERNAL_ERROR;
                }
            }
            let tag = args[i * 2];
            let value = args[i * 2 + 1];
            let ok = match tag {
                PRINT_TAG_STR => {
                    // String buffer stores each byte as a separate i64 slot
                    let ptr = value as *const i64;
                    if ptr.is_null() {
                        write!(stdout, "(null)").is_ok()
                    } else {
                        let mut bytes: Vec<u8> = Vec::new();
                        let mut offset = 0usize;
                        loop {
                            let b = *ptr.add(offset) as u8;
                            if b == 0 { break; }
                            bytes.push(b);
                            offset += 1;
                        }
                        match String::from_utf8(bytes) {
                            Ok(s) => write!(stdout, "{}", s).is_ok(),
                            Err(_) => write!(stdout, "(invalid utf8)").is_ok(),
                        }
                    }
                }
                PRINT_TAG_BOOL => write!(stdout, "{}", if value != 0 { "true" } else { "false" }).is_ok(),
                PRINT_TAG_FLOAT => {
                    let f = f64::from_bits(value as u64);
                    write!(stdout, "{}", f).is_ok()
                }
                _ => write!(stdout, "{}", value).is_ok(), // PRINT_TAG_INT or unknown
            };
            if !ok {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }

        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = values_count as SpectraHostValue;
        }
    }

    HOST_STATUS_SUCCESS
}

/// Polymorphic println: same as print but appends a trailing newline.
extern "C" fn std_io_println(ctx: *mut SpectraHostCallContext) -> i32 {
    let status = std_io_print(ctx);
    if status != HOST_STATUS_SUCCESS {
        return status;
    }
    if writeln!(io::stdout()).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    HOST_STATUS_SUCCESS
}

/// Same as io.print but writes to stderr (no trailing newline).
extern "C" fn std_io_eprint(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len > 0 && ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_len = ctx_ref.arg_len;
        let args = if args_len == 0 {
            &[] as &[SpectraHostValue]
        } else {
            slice::from_raw_parts(ctx_ref.args, args_len)
        };

        let mut stderr = io::stderr();
        let values_count = args_len / 2;

        for i in 0..values_count {
            if i > 0 {
                if write!(stderr, " ").is_err() {
                    return HOST_STATUS_INTERNAL_ERROR;
                }
            }
            let tag = args[i * 2];
            let value = args[i * 2 + 1];
            let ok = match tag {
                PRINT_TAG_STR => {
                    // String buffer stores each byte as a separate i64 slot
                    let ptr = value as *const i64;
                    if ptr.is_null() {
                        write!(stderr, "(null)").is_ok()
                    } else {
                        let mut bytes: Vec<u8> = Vec::new();
                        let mut offset = 0usize;
                        loop {
                            let b = *ptr.add(offset) as u8;
                            if b == 0 { break; }
                            bytes.push(b);
                            offset += 1;
                        }
                        match String::from_utf8(bytes) {
                            Ok(s) => write!(stderr, "{}", s).is_ok(),
                            Err(_) => write!(stderr, "(invalid utf8)").is_ok(),
                        }
                    }
                }
                PRINT_TAG_BOOL => write!(stderr, "{}", if value != 0 { "true" } else { "false" }).is_ok(),
                PRINT_TAG_FLOAT => {
                    let f = f64::from_bits(value as u64);
                    write!(stderr, "{}", f).is_ok()
                }
                _ => write!(stderr, "{}", value).is_ok(),
            };
            if !ok {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }

        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = values_count as SpectraHostValue;
        }
    }

    HOST_STATUS_SUCCESS
}

/// Polymorphic eprintln: same as eprint but appends a trailing newline.
extern "C" fn std_io_eprintln(ctx: *mut SpectraHostCallContext) -> i32 {
    let status = std_io_eprint(ctx);
    if status != HOST_STATUS_SUCCESS {
        return status;
    }
    if writeln!(io::stderr()).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_io_read_line(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    // Strip trailing CRLF or LF
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ptr = alloc_spectra_string(&line);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ptr;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_io_flush(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    if let Err(_) = io::stdout().flush() {
        return HOST_STATUS_INTERNAL_ERROR;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                results[0] = 0;
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_new(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let memory = initialize().memory();
        let list = match memory.allocate_manual(StdList::default()) {
            Ok(list) => list,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };

        let handle = with_list_registry(|registry| registry.insert(list));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_push(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let value = args[1];

        match with_list_registry(|registry| registry.push(handle, value)) {
            Ok(len) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = len as SpectraHostValue;
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_list_len(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;

        match with_list_registry(|registry| registry.len(handle)) {
            Ok(len) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = len as SpectraHostValue;
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_list_clear(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;

        match with_list_registry(|registry| registry.clear_list(handle)) {
            Ok(()) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = 0;
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_list_free(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;

        match with_list_registry(|registry| registry.remove(handle)) {
            Ok(_) => {
                if ctx_ref.result_len > 0 {
                    if ctx_ref.results.is_null() {
                        return HOST_STATUS_INVALID_ARGUMENT;
                    }
                    let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                    if !results.is_empty() {
                        results[0] = 0;
                    }
                }
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_list_free_all(ctx: *mut SpectraHostCallContext) -> i32 {
    let freed = with_list_registry(|registry| registry.clear_all());

    if ctx.is_null() {
        return HOST_STATUS_SUCCESS;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len > 0 {
            if ctx_ref.results.is_null() {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                results[0] = freed as SpectraHostValue;
            }
        }
    }

    HOST_STATUS_SUCCESS
}

fn with_list_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut ListRegistry) -> R,
{
    let registry = list_registry();
    let mut guard = registry
        .lock()
        .expect("collections registry mutex poisoned");
    action(&mut guard)
}

fn list_registry() -> &'static Mutex<ListRegistry> {
    static REGISTRY: OnceLock<Mutex<ListRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ListRegistry::new()))
}

#[derive(Default)]
struct StdList {
    data: Vec<SpectraHostValue>,
}

struct ListRegistry {
    next_id: usize,
    lists: HashMap<usize, ManualBox<StdList>>,
}

impl ListRegistry {
    fn new() -> Self {
        Self {
            next_id: 1,
            lists: HashMap::new(),
        }
    }

    fn insert(&mut self, list: ManualBox<StdList>) -> usize {
        let mut handle = self.next_id.max(1);
        while self.lists.contains_key(&handle) {
            handle = handle.wrapping_add(1).max(1);
        }
        self.next_id = handle.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        self.lists.insert(handle, list);
        handle
    }

    fn push(&mut self, handle: usize, value: SpectraHostValue) -> Result<usize, i32> {
        match self.lists.get_mut(&handle) {
            Some(list) => {
                list.data.push(value);
                Ok(list.data.len())
            }
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn len(&self, handle: usize) -> Result<usize, i32> {
        match self.lists.get(&handle) {
            Some(list) => Ok(list.data.len()),
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn get(&self, handle: usize, index: i64) -> SpectraHostValue {
        match self.lists.get(&handle) {
            Some(list) if index >= 0 && (index as usize) < list.data.len() => {
                list.data[index as usize]
            }
            _ => -1,
        }
    }

    fn set(&mut self, handle: usize, index: i64, value: SpectraHostValue) {
        if let Some(list) = self.lists.get_mut(&handle) {
            if index >= 0 && (index as usize) < list.data.len() {
                list.data[index as usize] = value;
            }
        }
    }

    fn contains(&self, handle: usize, value: SpectraHostValue) -> bool {
        match self.lists.get(&handle) {
            Some(list) => list.data.contains(&value),
            None => false,
        }
    }

    fn clear_list(&mut self, handle: usize) -> Result<(), i32> {
        match self.lists.get_mut(&handle) {
            Some(list) => {
                list.data.clear();
                Ok(())
            }
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn remove(&mut self, handle: usize) -> Result<(), i32> {
        if self.lists.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(HOST_STATUS_NOT_FOUND)
        }
    }

    fn clear_all(&mut self) -> usize {
        let count = self.lists.len();
        self.lists.clear();
        self.next_id = 1;
        count
    }

    fn pop(&mut self, handle: usize) -> SpectraHostValue {
        match self.lists.get_mut(&handle) {
            Some(list) => list.data.pop().unwrap_or(-1),
            None => -1,
        }
    }

    fn pop_front(&mut self, handle: usize) -> SpectraHostValue {
        match self.lists.get_mut(&handle) {
            Some(list) if !list.data.is_empty() => list.data.remove(0),
            _ => -1,
        }
    }

    fn insert_at(&mut self, handle: usize, index: i64, value: SpectraHostValue) {
        if let Some(list) = self.lists.get_mut(&handle) {
            let idx = index.clamp(0, list.data.len() as i64) as usize;
            list.data.insert(idx, value);
        }
    }

    fn remove_at(&mut self, handle: usize, index: i64) -> SpectraHostValue {
        if let Some(list) = self.lists.get_mut(&handle) {
            if index >= 0 && (index as usize) < list.data.len() {
                return list.data.remove(index as usize);
            }
        }
        -1
    }

    fn index_of(&self, handle: usize, value: SpectraHostValue) -> SpectraHostValue {
        match self.lists.get(&handle) {
            Some(list) => list
                .data
                .iter()
                .position(|&v| v == value)
                .map(|i| i as i64)
                .unwrap_or(-1),
            None => -1,
        }
    }

    fn sort_asc(&mut self, handle: usize) {
        if let Some(list) = self.lists.get_mut(&handle) {
            list.data.sort();
        }
    }

    /// Returns a clone of the list's data without holding any other lock.
    fn snapshot(&self, handle: usize) -> Option<Vec<SpectraHostValue>> {
        self.lists.get(&handle).map(|l| l.data.clone())
    }

    /// Replaces a list's data with `data` (used after an out-of-lock sort/transform).
    fn restore(&mut self, handle: usize, data: Vec<SpectraHostValue>) {
        if let Some(list) = self.lists.get_mut(&handle) {
            list.data = data;
        }
    }
}

// ── std.collections extras ──────────────────────────────────────────────────

extern "C" fn std_list_get(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let index = args[1];
        let result = with_list_registry(|registry| registry.get(handle, index));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = result;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_set(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let index = args[1];
        let value = args[2];
        with_list_registry(|registry| registry.set(handle, index, value));
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_contains(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let value = args[1];
        let found = with_list_registry(|registry| registry.contains(handle, value));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = found as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_pop(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let val = with_list_registry(|registry| registry.pop(handle));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = val;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_pop_front(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let val = with_list_registry(|registry| registry.pop_front(handle));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = val;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_insert_at(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let index = args[1];
        let value = args[2];
        with_list_registry(|registry| registry.insert_at(handle, index, value));
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_remove_at(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let index = args[1];
        let val = with_list_registry(|registry| registry.remove_at(handle, index));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = val;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_index_of(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let value = args[1];
        let idx = with_list_registry(|registry| registry.index_of(handle, value));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = idx;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_list_sort(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        with_list_registry(|registry| registry.sort_asc(handle));
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.collections higher-order functions ──────────────────────────────────

/// `list_map(handle, fn_ptr) -> new_handle`
///
/// Creates a new list by applying the Spectra closure `fn_ptr(elem: int) -> int`
/// to every element of the source list.
extern "C" fn std_list_map(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let invoke = match ctx_ref.invoke_fn {
            Some(f) => f,
            None => return HOST_STATUS_INTERNAL_ERROR,
        };
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let src_handle = args[0] as usize;
        let fn_ptr = args[1];

        // Snapshot source data in a single lock acquisition so the lock is not
        // held while calling back into JIT code.
        let src_data = match with_list_registry(|reg| reg.snapshot(src_handle)) {
            Some(d) => d,
            None => return HOST_STATUS_NOT_FOUND,
        };

        // Allocate the destination list.
        let memory = initialize().memory();
        let dest_list = match memory.allocate_manual(StdList::default()) {
            Ok(l) => l,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };
        let dest_handle = with_list_registry(|reg| reg.insert(dest_list));

        for &elem in &src_data {
            let arg_buf = [elem];
            let mut out = 0i64;
            let status = invoke(fn_ptr, arg_buf.as_ptr(), 1, &mut out);
            if status != HOST_STATUS_SUCCESS {
                let _ = with_list_registry(|reg| reg.remove(dest_handle));
                return status;
            }
            let _ = with_list_registry(|reg| reg.push(dest_handle, out));
        }

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = dest_handle as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

/// `list_filter(handle, fn_ptr) -> new_handle`
///
/// Creates a new list containing only the elements for which the Spectra closure
/// `fn_ptr(elem: int) -> int` returns a non-zero (truthy) value.
extern "C" fn std_list_filter(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let invoke = match ctx_ref.invoke_fn {
            Some(f) => f,
            None => return HOST_STATUS_INTERNAL_ERROR,
        };
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let src_handle = args[0] as usize;
        let fn_ptr = args[1];

        let src_data = match with_list_registry(|reg| reg.snapshot(src_handle)) {
            Some(d) => d,
            None => return HOST_STATUS_NOT_FOUND,
        };

        let memory = initialize().memory();
        let dest_list = match memory.allocate_manual(StdList::default()) {
            Ok(l) => l,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };
        let dest_handle = with_list_registry(|reg| reg.insert(dest_list));

        for &elem in &src_data {
            let arg_buf = [elem];
            let mut out = 0i64;
            let status = invoke(fn_ptr, arg_buf.as_ptr(), 1, &mut out);
            if status != HOST_STATUS_SUCCESS {
                let _ = with_list_registry(|reg| reg.remove(dest_handle));
                return status;
            }
            if out != 0 {
                let _ = with_list_registry(|reg| reg.push(dest_handle, elem));
            }
        }

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = dest_handle as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

/// `list_reduce(handle, initial, fn_ptr) -> int`
///
/// Folds the list left-to-right using `fn_ptr(accumulator: int, elem: int) -> int`,
/// starting with `initial` as the accumulator. Returns the final accumulator value.
extern "C" fn std_list_reduce(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let invoke = match ctx_ref.invoke_fn {
            Some(f) => f,
            None => return HOST_STATUS_INTERNAL_ERROR,
        };
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let src_handle = args[0] as usize;
        let mut accumulator = args[1];
        let fn_ptr = args[2];

        let src_data = match with_list_registry(|reg| reg.snapshot(src_handle)) {
            Some(d) => d,
            None => return HOST_STATUS_NOT_FOUND,
        };

        for &elem in &src_data {
            let arg_buf = [accumulator, elem];
            let mut out = 0i64;
            let status = invoke(fn_ptr, arg_buf.as_ptr(), 2, &mut out);
            if status != HOST_STATUS_SUCCESS {
                return status;
            }
            accumulator = out;
        }

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = accumulator;
    }
    HOST_STATUS_SUCCESS
}

/// `list_sort_by(handle, fn_ptr) -> unit`
///
/// Sorts the list in-place using the Spectra comparator closure
/// `fn_ptr(a: int, b: int) -> int` (negative ⇒ a < b, 0 ⇒ equal, positive ⇒ a > b).
extern "C" fn std_list_sort_by(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let invoke = match ctx_ref.invoke_fn {
            Some(f) => f,
            None => return HOST_STATUS_INTERNAL_ERROR,
        };
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let fn_ptr = args[1];

        // Snapshot, sort outside the lock, then restore.
        let mut data = match with_list_registry(|reg| reg.snapshot(handle)) {
            Some(d) => d,
            None => return HOST_STATUS_NOT_FOUND,
        };

        // Use a cell to propagate callback errors out of the sort closure.
        let mut callback_err: i32 = HOST_STATUS_SUCCESS;
        data.sort_by(|&a, &b| {
            if callback_err != HOST_STATUS_SUCCESS {
                return std::cmp::Ordering::Equal;
            }
            let arg_buf = [a, b];
            let mut out = 0i64;
            let status = invoke(fn_ptr, arg_buf.as_ptr(), 2, &mut out);
            if status != HOST_STATUS_SUCCESS {
                callback_err = status;
                return std::cmp::Ordering::Equal;
            }
            out.cmp(&0)
        });
        if callback_err != HOST_STATUS_SUCCESS {
            return callback_err;
        }

        with_list_registry(|reg| reg.restore(handle, data));
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.string & std.convert registrations ─────────────────────────────────

fn register_string() {
    register_host_function(STR_LEN, std_string_len);
    register_host_function(STR_CONTAINS, std_string_contains);
    register_host_function(STR_TO_UPPER, std_string_to_upper);
    register_host_function(STR_TO_LOWER, std_string_to_lower);
    register_host_function(STR_TRIM, std_string_trim);
    register_host_function(STR_STARTS_WITH, std_string_starts_with);
    register_host_function(STR_ENDS_WITH, std_string_ends_with);
    register_host_function(STR_CONCAT, std_string_concat);
    register_host_function(STR_REPEAT, std_string_repeat);
    register_host_function(STR_CHAR_AT, std_string_char_at);
    register_host_function(STR_SUBSTRING, std_string_substring);
    register_host_function(STR_REPLACE, std_string_replace);
    register_host_function(STR_INDEX_OF, std_string_index_of);
    register_host_function(STR_SPLIT_FIRST, std_string_split_first);
    register_host_function(STR_SPLIT_LAST, std_string_split_last);
    register_host_function(STR_IS_EMPTY, std_string_is_empty);
    register_host_function(STR_COUNT, std_string_count_occurrences);
    register_host_function(STR_SPLIT_BY, std_string_split_by);
    register_host_function(STR_PAD_LEFT, std_string_pad_left);
    register_host_function(STR_PAD_RIGHT, std_string_pad_right);
    register_host_function(STR_REVERSE, std_string_reverse);
}

fn register_convert() {
    register_host_function(CONV_INT_TO_STRING, std_convert_int_to_string);
    register_host_function(CONV_FLOAT_TO_STRING, std_convert_float_to_string);
    register_host_function(CONV_BOOL_TO_STRING, std_convert_bool_to_string);
    register_host_function(CONV_STRING_TO_INT, std_convert_string_to_int);
    register_host_function(CONV_STRING_TO_FLOAT, std_convert_string_to_float);
    register_host_function(CONV_INT_TO_FLOAT, std_convert_int_to_float);
    register_host_function(CONV_FLOAT_TO_INT, std_convert_float_to_int);
    register_host_function(CONV_STRING_TO_INT_OR, std_convert_string_to_int_or);
    register_host_function(CONV_STRING_TO_FLOAT_OR, std_convert_string_to_float_or);
    register_host_function(CONV_STRING_TO_BOOL, std_convert_string_to_bool);
    register_host_function(CONV_BOOL_TO_INT, std_convert_bool_to_int);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Read a Spectra string (null-terminated i64 array) from a raw pointer value.
/// Returns `None` if the pointer is null or the bytes are not valid UTF-8.
unsafe fn read_spectra_string(ptr_val: SpectraHostValue) -> Option<String> {
    if ptr_val == 0 {
        return None;
    }
    let raw = ptr_val as *const i64;
    let mut bytes: Vec<u8> = Vec::new();
    let mut offset = 0usize;
    loop {
        let b = *raw.add(offset) as u8;
        if b == 0 {
            break;
        }
        bytes.push(b);
        offset += 1;
    }
    String::from_utf8(bytes).ok()
}

/// Allocate a new Spectra string using the runtime manual allocator.
/// Each character is stored as one `i64` slot; the array is null-terminated.
/// Returns the pointer cast to `i64`, or `0` on allocation failure.
unsafe fn alloc_spectra_string(s: &str) -> SpectraHostValue {
    use crate::ffi::spectra_rt_manual_alloc;
    let bytes = s.as_bytes();
    let total_bytes = (bytes.len() + 1) * std::mem::size_of::<i64>();
    let raw = spectra_rt_manual_alloc(total_bytes) as *mut i64;
    if raw.is_null() {
        return 0;
    }
    for (i, &b) in bytes.iter().enumerate() {
        *raw.add(i) = b as i64;
    }
    *raw.add(bytes.len()) = 0; // null terminator
    raw as i64
}

// ── std.string host functions ────────────────────────────────────────────────

extern "C" fn std_string_len(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let len = match read_spectra_string(args[0]) {
            Some(s) => s.len() as SpectraHostValue,
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = len;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_contains(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(sub)) => s.contains(sub.as_str()) as SpectraHostValue,
            _ => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_to_upper(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => alloc_spectra_string(&s.to_uppercase()),
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_to_lower(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => alloc_spectra_string(&s.to_lowercase()),
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_trim(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => alloc_spectra_string(s.trim()),
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_starts_with(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(prefix)) => s.starts_with(prefix.as_str()) as SpectraHostValue,
            _ => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_ends_with(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(suffix)) => s.ends_with(suffix.as_str()) as SpectraHostValue,
            _ => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_concat(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(a), Some(b)) => alloc_spectra_string(&(a + &b)),
            (Some(a), None) => alloc_spectra_string(&a),
            (None, Some(b)) => alloc_spectra_string(&b),
            (None, None) => alloc_spectra_string(""),
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_repeat(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let n = args[1].max(0) as usize;
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => alloc_spectra_string(&s.repeat(n)),
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_char_at(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let idx = args[1];
        let result = match read_spectra_string(args[0]) {
            Some(s) if idx >= 0 && (idx as usize) < s.len() => {
                s.as_bytes()[idx as usize] as SpectraHostValue
            }
            _ => -1,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.string extras ────────────────────────────────────────────────────────

/// Returns a substring from `start` (inclusive) to `end` (exclusive).
/// Clamps indices to valid range; returns empty string on invalid input.
extern "C" fn std_string_substring(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let start = args[1];
        let end = args[2];
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => {
                let len = s.len() as i64;
                let s_start = start.clamp(0, len) as usize;
                let s_end = end.clamp(0, len) as usize;
                let slice = if s_start <= s_end { &s[s_start..s_end] } else { "" };
                alloc_spectra_string(slice)
            }
            None => alloc_spectra_string(""),
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Replaces all occurrences of `from` with `to` in `s`.
extern "C" fn std_string_replace(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match (
            read_spectra_string(args[0]),
            read_spectra_string(args[1]),
            read_spectra_string(args[2]),
        ) {
            (Some(s), Some(from), Some(to)) => alloc_spectra_string(&s.replace(from.as_str(), &to)),
            (Some(s), _, _) => alloc_spectra_string(&s),
            _ => alloc_spectra_string(""),
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns the byte index of the first occurrence of `sub` in `s`, or -1 if not found.
extern "C" fn std_string_index_of(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(sub)) => match s.find(sub.as_str()) {
                Some(idx) => idx as SpectraHostValue,
                None => -1,
            },
            _ => -1,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns the part of `s` before the first occurrence of `sep`.
/// Returns `s` unchanged if `sep` is not found.
extern "C" fn std_string_split_first(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(sep)) => {
                let part = s.splitn(2, sep.as_str()).next().unwrap_or("");
                alloc_spectra_string(part)
            }
            (Some(s), _) => alloc_spectra_string(&s),
            _ => alloc_spectra_string(""),
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns the part of `s` after the last occurrence of `sep`.
/// Returns empty string if `sep` is not found.
extern "C" fn std_string_split_last(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(sep)) => {
                let part = s.rsplitn(2, sep.as_str()).next().unwrap_or("");
                alloc_spectra_string(part)
            }
            _ => alloc_spectra_string(""),
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns 1 if the string is empty (or null), 0 otherwise.
extern "C" fn std_string_is_empty(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match read_spectra_string(args[0]) {
            Some(s) => s.is_empty() as SpectraHostValue,
            None => 1,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns the number of non-overlapping occurrences of `sub` in `s`.
extern "C" fn std_string_count_occurrences(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(sub)) if !sub.is_empty() => s.matches(sub.as_str()).count() as SpectraHostValue,
            _ => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.convert host functions ───────────────────────────────────────────────

extern "C" fn std_convert_int_to_string(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = alloc_spectra_string(&args[0].to_string());
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_convert_float_to_string(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let f = f64::from_bits(args[0] as u64);
        let ptr = alloc_spectra_string(&f.to_string());
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_convert_bool_to_string(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = alloc_spectra_string(if args[0] != 0 { "true" } else { "false" });
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = ptr;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_convert_string_to_int(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match read_spectra_string(args[0]) {
            Some(s) => s.trim().parse::<i64>().unwrap_or(0),
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_convert_string_to_float(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result: i64 = match read_spectra_string(args[0]) {
            Some(s) => {
                let f: f64 = s.trim().parse().unwrap_or(0.0);
                f.to_bits() as i64
            }
            None => 0.0_f64.to_bits() as i64,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_convert_int_to_float(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let f = args[0] as f64;
        let result = f.to_bits() as i64;
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_convert_float_to_int(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let f = f64::from_bits(args[0] as u64);
        let result = f as i64;
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.convert extras ───────────────────────────────────────────────────────

/// Parses a string as int; returns `default` if parsing fails.
extern "C" fn std_convert_string_to_int_or(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let default_val = args[1];
        let result = match read_spectra_string(args[0]) {
            Some(s) => s.trim().parse::<i64>().unwrap_or(default_val),
            None => default_val,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Parses a string as float; returns `default` (f64 bits) if parsing fails.
extern "C" fn std_convert_string_to_float_or(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let default_val = args[1];
        let result = match read_spectra_string(args[0]) {
            Some(s) => match s.trim().parse::<f64>() {
                Ok(f) => f.to_bits() as i64,
                Err(_) => default_val,
            },
            None => default_val,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns 1 (true) if the string equals "true" (case-insensitive), 0 otherwise.
extern "C" fn std_convert_string_to_bool(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result = match read_spectra_string(args[0]) {
            Some(s) => s.trim().eq_ignore_ascii_case("true") as SpectraHostValue,
            None => 0,
        };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Converts a bool to int: true → 1, false → 0.
extern "C" fn std_convert_bool_to_int(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let result: SpectraHostValue = if args[0] != 0 { 1 } else { 0 };
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.random ───────────────────────────────────────────────────────────────

fn random_state() -> &'static Mutex<u64> {
    static STATE: OnceLock<Mutex<u64>> = OnceLock::new();
    STATE.get_or_init(|| {
        // Default seed derived from the system time for variety across runs.
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| {
                d.subsec_nanos() as u64
                    ^ d.as_secs().wrapping_mul(6364136223846793005)
            })
            .unwrap_or(12345);
        Mutex::new(seed)
    })
}

/// Linear Congruential Generator step (Knuth constants). Returns total state.
#[inline]
fn lcg_next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}

fn register_random() {
    register_host_function(RAND_SEED, std_random_seed);
    register_host_function(RAND_INT, std_random_int);
    register_host_function(RAND_FLOAT, std_random_float);
    register_host_function(RAND_BOOL, std_random_bool);
}

/// Sets the random seed.
extern "C" fn std_random_seed(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        *random_state().lock().expect("random mutex poisoned") = args[0] as u64;
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns a random integer in [min, max). Returns `min` when min >= max.
extern "C" fn std_random_int(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let min = args[0];
        let max = args[1];
        let result = if min >= max {
            min
        } else {
            let range = (max - min) as u64;
            let rand = lcg_next(&mut *random_state().lock().expect("random mutex poisoned"));
            min + (rand % range) as i64
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = result;
    }
    HOST_STATUS_SUCCESS
}

/// Returns a random float in [0.0, 1.0) as f64 bits.
extern "C" fn std_random_float(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let rand = lcg_next(&mut *random_state().lock().expect("random mutex poisoned"));
        // Map to [0.0, 1.0) via the 53 significant bits of f64 mantissa.
        let f: f64 = (rand >> 11) as f64 / (1u64 << 53) as f64;
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f.to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns a random bool (0 or 1).
extern "C" fn std_random_bool(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let rand = lcg_next(&mut *random_state().lock().expect("random mutex poisoned"));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = (rand & 1) as i64;
    }
    HOST_STATUS_SUCCESS
}

// ── std.fs host functions ────────────────────────────────────────────────────

extern "C" fn std_fs_read(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path = match read_spectra_string(args[0]) {
            Some(p) => p,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = alloc_spectra_string(&content);
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_fs_write(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path = match read_spectra_string(args[0]) {
            Some(p) => p,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let content = read_spectra_string(args[1]).unwrap_or_default();
        let ok = std::fs::write(&path, content.as_bytes()).is_ok();
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ok as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_fs_append(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path = match read_spectra_string(args[0]) {
            Some(p) => p,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let content = read_spectra_string(args[1]).unwrap_or_default();
        use std::io::Write as _;
        let ok = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .and_then(|mut f| f.write_all(content.as_bytes()))
            .is_ok();
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ok as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_fs_exists(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path = match read_spectra_string(args[0]) {
            Some(p) => p,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let exists = std::path::Path::new(&path).exists();
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = exists as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_fs_remove(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path = match read_spectra_string(args[0]) {
            Some(p) => p,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let ok = std::fs::remove_file(&path).is_ok();
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ok as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

// ── std.env host functions ───────────────────────────────────────────────────

extern "C" fn std_env_get(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let key = match read_spectra_string(args[0]) {
            Some(k) => k,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let value = std::env::var(&key).unwrap_or_default();
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = alloc_spectra_string(&value);
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_env_set(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let key = match read_spectra_string(args[0]) {
            Some(k) => k,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let value = read_spectra_string(args[1]).unwrap_or_default();
        std::env::set_var(&key, &value);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = 1;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_env_args_count(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        // Use explicitly forwarded program args when available (JIT runner sets
        // these via spectra_runtime::set_program_args; AOT executables use
        // spectra_rt_startup_with_args). Fall back to std::env::args otherwise.
        let count = if let Some(args) = crate::ffi::get_program_args() {
            args.len() as i64
        } else {
            std::env::args().count() as i64
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = count;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_env_arg(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let index = args[0] as usize;
        // Use explicitly forwarded program args when available; fall back to
        // std::env::args so the function is still usable without prior setup.
        let arg = if let Some(prog_args) = crate::ffi::get_program_args() {
            prog_args.get(index).cloned().unwrap_or_default()
        } else {
            std::env::args().nth(index).unwrap_or_default()
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = alloc_spectra_string(&arg);
    }
    HOST_STATUS_SUCCESS
}

// ── std.option host functions ────────────────────────────────────────────────
// Option layout in heap: ptr[0] = tag (0=Some, 1=None), ptr[1] = payload

extern "C" fn std_option_is_some(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        let tag = if ptr.is_null() { 1i64 } else { *ptr };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = (tag == 0) as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_option_is_none(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        let tag = if ptr.is_null() { 1i64 } else { *ptr };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = (tag != 0) as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_option_unwrap(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        if ptr.is_null() || *ptr != 0 {
            panic!("option_unwrap called on None");
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = *ptr.add(1);
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_option_unwrap_or(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        let default_val = args[1];
        let tag = if ptr.is_null() { 1i64 } else { *ptr };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = if tag == 0 { *ptr.add(1) } else { default_val };
    }
    HOST_STATUS_SUCCESS
}

// ── std.result host functions ────────────────────────────────────────────────
// Result layout in heap: ptr[0] = tag (0=Ok, 1=Err), ptr[1] = payload

extern "C" fn std_result_is_ok(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        let tag = if ptr.is_null() { 1i64 } else { *ptr };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = (tag == 0) as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_result_is_err(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        let tag = if ptr.is_null() { 1i64 } else { *ptr };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = (tag != 0) as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_result_unwrap(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        if ptr.is_null() || *ptr != 0 {
            panic!("result_unwrap called on Err");
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = *ptr.add(1);
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_result_unwrap_or(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        let default_val = args[1];
        let tag = if ptr.is_null() { 1i64 } else { *ptr };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = if tag == 0 { *ptr.add(1) } else { default_val };
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_result_unwrap_err(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = args[0] as *const i64;
        if ptr.is_null() || *ptr == 0 {
            panic!("result_unwrap_err called on Ok");
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = *ptr.add(1);
    }
    HOST_STATUS_SUCCESS
}

// ── std.char register & host functions ──────────────────────────────────────

fn register_char() {
    register_host_function(CHAR_IS_ALPHA, std_char_is_alpha);
    register_host_function(CHAR_IS_DIGIT, std_char_is_digit);
    register_host_function(CHAR_IS_WHITESPACE, std_char_is_whitespace);
    register_host_function(CHAR_IS_UPPER, std_char_is_upper);
    register_host_function(CHAR_IS_LOWER, std_char_is_lower);
    register_host_function(CHAR_TO_UPPER, std_char_to_upper);
    register_host_function(CHAR_TO_LOWER, std_char_to_lower);
    register_host_function(CHAR_IS_ALPHANUMERIC, std_char_is_alphanumeric);
}

extern "C" fn std_char_is_alpha(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let v = char::from_u32(args[0] as u32).map(|c| c.is_alphabetic()).unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_char_is_digit(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let v = char::from_u32(args[0] as u32).map(|c| c.is_ascii_digit()).unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_char_is_whitespace(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let v = char::from_u32(args[0] as u32).map(|c| c.is_whitespace()).unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_char_is_upper(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let v = char::from_u32(args[0] as u32).map(|c| c.is_uppercase()).unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_char_is_lower(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let v = char::from_u32(args[0] as u32).map(|c| c.is_lowercase()).unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns the uppercase version of the Unicode code point `c`.
extern "C" fn std_char_to_upper(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let upper = char::from_u32(args[0] as u32)
            .and_then(|c| c.to_uppercase().next())
            .unwrap_or(char::from_u32(args[0] as u32).unwrap_or('\0'));
        results[0] = upper as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns the lowercase version of the Unicode code point `c`.
extern "C" fn std_char_to_lower(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let lower = char::from_u32(args[0] as u32)
            .and_then(|c| c.to_lowercase().next())
            .unwrap_or(char::from_u32(args[0] as u32).unwrap_or('\0'));
        results[0] = lower as i64;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_char_is_alphanumeric(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let v = char::from_u32(args[0] as u32).map(|c| c.is_alphanumeric()).unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

// ── std.time register & host functions ──────────────────────────────────────

fn register_time() {
    register_host_function(TIME_NOW_MILLIS, std_time_now_millis);
    register_host_function(TIME_NOW_SECS, std_time_now_secs);
    register_host_function(TIME_SLEEP_MS, std_time_sleep_ms);
}

/// Returns milliseconds elapsed since the Unix epoch (January 1, 1970 UTC).
/// Returns -1 if the system clock is before the epoch.
extern "C" fn std_time_now_millis(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(-1);
    }
    HOST_STATUS_SUCCESS
}

/// Returns seconds elapsed since the Unix epoch. Returns -1 on error.
extern "C" fn std_time_now_secs(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(-1);
    }
    HOST_STATUS_SUCCESS
}

/// Sleeps for `ms` milliseconds. Negative values are treated as zero.
extern "C" fn std_time_sleep_ms(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ms = args[0].max(0) as u64;
        std::thread::sleep(Duration::from_millis(ms));
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

// ── std.string new functions ─────────────────────────────────────────────────

/// Splits `s` by `sep` and returns a list handle (int) whose elements are
/// string pointers (i64) for each part. Returns -1 on allocation failure.
extern "C" fn std_string_split_by(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let (s, sep) = match (read_spectra_string(args[0]), read_spectra_string(args[1])) {
            (Some(s), Some(sep)) => (s, sep),
            _ => {
                results[0] = -1;
                return HOST_STATUS_SUCCESS;
            }
        };
        let memory = crate::initialize().memory();
        let list = match memory.allocate_manual(StdList::default()) {
            Ok(l) => l,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };
        let handle = with_list_registry(|reg| reg.insert(list));
        for part in s.split(sep.as_str()) {
            let ptr = alloc_spectra_string(part);
            let _ = with_list_registry(|reg| reg.push(handle, ptr));
        }
        results[0] = handle as SpectraHostValue;
    }
    HOST_STATUS_SUCCESS
}

/// Pads `s` on the left with `pad_char` (Unicode code point) until the result
/// has `width` bytes. If `s` is already at or longer than `width`, returns `s`
/// unchanged.
extern "C" fn std_string_pad_left(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let width = args[1].max(0) as usize;
        let pad_ch = char::from_u32(args[2] as u32).unwrap_or(' ');
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => {
                if s.len() >= width {
                    alloc_spectra_string(&s)
                } else {
                    let padding: String = std::iter::repeat(pad_ch).take(width - s.len()).collect();
                    alloc_spectra_string(&(padding + &s))
                }
            }
            None => alloc_spectra_string(""),
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ptr;
    }
    HOST_STATUS_SUCCESS
}

/// Pads `s` on the right with `pad_char` (Unicode code point) until the result
/// has `width` bytes. If `s` is already at or longer than `width`, returns `s`
/// unchanged.
extern "C" fn std_string_pad_right(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let width = args[1].max(0) as usize;
        let pad_ch = char::from_u32(args[2] as u32).unwrap_or(' ');
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => {
                if s.len() >= width {
                    alloc_spectra_string(&s)
                } else {
                    let padding: String = std::iter::repeat(pad_ch).take(width - s.len()).collect();
                    alloc_spectra_string(&(s + &padding))
                }
            }
            None => alloc_spectra_string(""),
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ptr;
    }
    HOST_STATUS_SUCCESS
}

/// Returns a new string with the characters of `s` in reverse order.
extern "C" fn std_string_reverse(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let ptr = match read_spectra_string(args[0]) {
            Some(s) => alloc_spectra_string(&s.chars().rev().collect::<String>()),
            None => alloc_spectra_string(""),
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ptr;
    }
    HOST_STATUS_SUCCESS
}

// ── std.math new functions ───────────────────────────────────────────────────

/// Returns the sign of `n`: -1 for negative, 0 for zero, 1 for positive.
extern "C" fn std_math_sign(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = args[0].signum();
    }
    HOST_STATUS_SUCCESS
}

/// Greatest common divisor of `a` and `b` (always non-negative; gcd(0,0) = 0).
extern "C" fn std_math_gcd(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let mut a = args[0].unsigned_abs();
        let mut b = args[1].unsigned_abs();
        while b != 0 {
            let t = b;
            b = a % b;
            a = t;
        }
        results[0] = a as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Least common multiple of `a` and `b` (always non-negative; lcm(n,0) = 0).
extern "C" fn std_math_lcm(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let a = args[0].unsigned_abs();
        let b = args[1].unsigned_abs();
        if a == 0 || b == 0 {
            results[0] = 0;
        } else {
            let mut ga = a;
            let mut gb = b;
            while gb != 0 {
                let t = gb;
                gb = ga % gb;
                ga = t;
            }
            // ga is now gcd(a, b)
            results[0] = ((a / ga) * b) as i64;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns 1 if the float value is NaN, 0 otherwise. Argument is f64 bits as i64.
extern "C" fn std_math_is_nan_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).is_nan() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns 1 if the float value is +∞ or −∞, 0 otherwise. Argument is f64 bits.
extern "C" fn std_math_is_infinite_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).is_infinite() as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Returns |x| for a float. Argument and result are f64 bits as i64.
extern "C" fn std_math_abs_f(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = f64::from_bits(args[0] as u64).abs().to_bits() as i64;
    }
    HOST_STATUS_SUCCESS
}

// ── std.io new functions ─────────────────────────────────────────────────────

/// Prints `prompt` (without newline), flushes stdout, then reads a line from
/// stdin. Strips the trailing newline before returning.
extern "C" fn std_io_input(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        if let Some(prompt) = read_spectra_string(args[0]) {
            let mut stdout = io::stdout();
            let _ = write!(stdout, "{}", prompt);
            let _ = stdout.flush();
        }
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        let ptr = alloc_spectra_string(&line);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = ptr;
    }
    HOST_STATUS_SUCCESS
}

// ── std.collections map (HashMap<i64, i64>) ──────────────────────────────────

const MAP_NEW: &str = "spectra.std.collections.map_new";
const MAP_SET: &str = "spectra.std.collections.map_set";
const MAP_GET: &str = "spectra.std.collections.map_get";
const MAP_CONTAINS: &str = "spectra.std.collections.map_contains";
const MAP_REMOVE: &str = "spectra.std.collections.map_remove";
const MAP_LEN: &str = "spectra.std.collections.map_len";
const MAP_CLEAR: &str = "spectra.std.collections.map_clear";
const MAP_FREE: &str = "spectra.std.collections.map_free";

fn register_map() {
    register_host_function(MAP_NEW, std_map_new);
    register_host_function(MAP_SET, std_map_set);
    register_host_function(MAP_GET, std_map_get);
    register_host_function(MAP_CONTAINS, std_map_contains);
    register_host_function(MAP_REMOVE, std_map_remove);
    register_host_function(MAP_LEN, std_map_len);
    register_host_function(MAP_CLEAR, std_map_clear);
    register_host_function(MAP_FREE, std_map_free);
}

struct MapRegistry {
    next_id: usize,
    maps: HashMap<usize, ManualBox<StdMap>>,
}

#[derive(Default)]
struct StdMap {
    data: HashMap<i64, i64>,
}

impl MapRegistry {
    fn new() -> Self {
        Self { next_id: 1, maps: HashMap::new() }
    }

    fn insert(&mut self, map: ManualBox<StdMap>) -> usize {
        let mut handle = self.next_id.max(1);
        while self.maps.contains_key(&handle) {
            handle = handle.wrapping_add(1).max(1);
        }
        self.next_id = handle.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        self.maps.insert(handle, map);
        handle
    }
}

fn map_registry() -> &'static Mutex<MapRegistry> {
    static REGISTRY: OnceLock<Mutex<MapRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(MapRegistry::new()))
}

fn with_map_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut MapRegistry) -> R,
{
    let registry = map_registry();
    let mut guard = registry.lock().expect("map registry mutex poisoned");
    action(&mut guard)
}

/// Creates a new empty map and returns its handle.
extern "C" fn std_map_new(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let memory = initialize().memory();
        let map = match memory.allocate_manual(StdMap::default()) {
            Ok(m) => m,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };
        let handle = with_map_registry(|reg| reg.insert(map));
        results[0] = handle as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Inserts or updates `key → value` in the map identified by `handle`.
/// Args: [handle, key, value]. Returns 0.
extern "C" fn std_map_set(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 3 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let key = args[1];
        let value = args[2];
        let ok = with_map_registry(|reg| match reg.maps.get_mut(&handle) {
            Some(m) => { m.data.insert(key, value); true }
            None => false,
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns the value for `key` in the map, or 0 if not found.
/// Args: [handle, key]. Returns: value.
extern "C" fn std_map_get(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let handle = args[0] as usize;
        let key = args[1];
        let value = with_map_registry(|reg| {
            reg.maps.get(&handle).and_then(|m| m.data.get(&key).copied()).unwrap_or(0)
        });
        results[0] = value;
    }
    HOST_STATUS_SUCCESS
}

/// Returns 1 if the map contains `key`, 0 otherwise.
/// Args: [handle, key]. Returns: bool as i64.
extern "C" fn std_map_contains(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let handle = args[0] as usize;
        let key = args[1];
        let found = with_map_registry(|reg| {
            reg.maps.get(&handle).map(|m| m.data.contains_key(&key)).unwrap_or(false)
        });
        results[0] = if found { 1 } else { 0 };
    }
    HOST_STATUS_SUCCESS
}

/// Removes `key` from the map. Returns the removed value, or 0 if not present.
/// Args: [handle, key]. Returns: removed_value.
extern "C" fn std_map_remove(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let key = args[1];
        let removed = with_map_registry(|reg| {
            reg.maps.get_mut(&handle).and_then(|m| m.data.remove(&key)).unwrap_or(0)
        });
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = removed;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Returns the number of entries in the map.
/// Args: [handle]. Returns: len.
extern "C" fn std_map_len(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        let handle = args[0] as usize;
        let len = with_map_registry(|reg| {
            reg.maps.get(&handle).map(|m| m.data.len()).unwrap_or(0)
        });
        results[0] = len as i64;
    }
    HOST_STATUS_SUCCESS
}

/// Removes all entries from the map without freeing the handle.
/// Args: [handle]. Returns 0.
extern "C" fn std_map_clear(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        with_map_registry(|reg| {
            if let Some(m) = reg.maps.get_mut(&handle) {
                m.data.clear();
            }
        });
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = 0;
        }
    }
    HOST_STATUS_SUCCESS
}

/// Frees the map and its handle.
/// Args: [handle].
extern "C" fn std_map_free(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        with_map_registry(|reg| { reg.maps.remove(&handle); });
    }
    HOST_STATUS_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("stdlib test guard poisoned")
    }

    #[test]
    fn math_abs_host_function_produces_positive_value() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let func = lookup_host_function(MATH_ABS).expect("math abs not registered");
        let args = [-42];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };

        let status = func(&mut ctx);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 42);
    }

    #[test]
    fn io_print_returns_argument_count() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let func = lookup_host_function(IO_PRINT).expect("io print not registered");
        let args = [1, 2, 3];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };

        let status = func(&mut ctx);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 3);
    }

    #[test]
    fn collections_list_lifecycle() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        let new_fn = lookup_host_function(LIST_NEW).expect("list_new not registered");
        let mut handle_result = [0];
        let mut new_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: handle_result.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };
        assert_eq!(new_fn(&mut new_ctx), HOST_STATUS_SUCCESS);
        let handle = handle_result[0] as usize;

        let push_fn = lookup_host_function(LIST_PUSH).expect("list_push not registered");
        for value in [10, 20, 30] {
            let push_args = [handle as SpectraHostValue, value];
            let mut push_result = [0];
            let mut push_ctx = SpectraHostCallContext {
                args: push_args.as_ptr(),
                arg_len: 2,
                results: push_result.as_mut_ptr(),
                result_len: 1,
                invoke_fn: None,
            };
            assert_eq!(push_fn(&mut push_ctx), HOST_STATUS_SUCCESS);
        }

        let len_fn = lookup_host_function(LIST_LEN).expect("list_len not registered");
        let len_args = [handle as SpectraHostValue];
        let mut len_result = [0];
        let mut len_ctx = SpectraHostCallContext {
            args: len_args.as_ptr(),
            arg_len: 1,
            results: len_result.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };
        assert_eq!(len_fn(&mut len_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(len_result[0], 3);

        let clear_fn = lookup_host_function(LIST_CLEAR).expect("list_clear not registered");
        let clear_args = [handle as SpectraHostValue];
        let mut clear_result = [0];
        let mut clear_ctx = SpectraHostCallContext {
            args: clear_args.as_ptr(),
            arg_len: 1,
            results: clear_result.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };
        assert_eq!(clear_fn(&mut clear_ctx), HOST_STATUS_SUCCESS);

        let free_fn = lookup_host_function(LIST_FREE).expect("list_free not registered");
        let free_args = [handle as SpectraHostValue];
        let mut free_ctx = SpectraHostCallContext {
            args: free_args.as_ptr(),
            arg_len: 1,
            results: ptr::null_mut(),
            result_len: 0,
            invoke_fn: None,
        };
        assert_eq!(free_fn(&mut free_ctx), HOST_STATUS_SUCCESS);

        let free_all_fn =
            lookup_host_function(LIST_FREE_ALL).expect("list_free_all not registered");
        let mut free_all_results = [0];
        let mut free_all_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: free_all_results.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };
        assert_eq!(free_all_fn(&mut free_all_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(free_all_results[0], 0);
    }
}
