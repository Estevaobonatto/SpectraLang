use crate::ffi::{
    register_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_ARITHMETIC_ERROR,
    HOST_STATUS_INTERNAL_ERROR, HOST_STATUS_INVALID_ARGUMENT, HOST_STATUS_NOT_FOUND,
    HOST_STATUS_SUCCESS,
};
use crate::initialize;
use crate::memory::ManualBox;
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::slice;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::ffi::{clear_host_functions, lookup_host_function};
#[cfg(test)]
use std::ptr;

const MATH_ABS: &str = "spectra.std.math.abs";
const MATH_MIN: &str = "spectra.std.math.min";
const MATH_MAX: &str = "spectra.std.math.max";
const MATH_ADD: &str = "spectra.std.math.add";
const MATH_SUB: &str = "spectra.std.math.sub";
const MATH_MUL: &str = "spectra.std.math.mul";
const MATH_DIV: &str = "spectra.std.math.div";
const MATH_MOD: &str = "spectra.std.math.mod";
const MATH_POW: &str = "spectra.std.math.pow";
const MATH_RNG_SEED: &str = "spectra.std.math.rng_seed";
const MATH_RNG_NEXT: &str = "spectra.std.math.rng_next";
const MATH_RNG_NEXT_RANGE: &str = "spectra.std.math.rng_next_range";
const MATH_RNG_FREE: &str = "spectra.std.math.rng_free";
const MATH_RNG_FREE_ALL: &str = "spectra.std.math.rng_free_all";
const MATH_MEDIAN: &str = "spectra.std.math.median";
const MATH_VARIANCE: &str = "spectra.std.math.variance";
const MATH_STD_DEV: &str = "spectra.std.math.std_dev";
const MATH_MODE: &str = "spectra.std.math.mode";
const MATH_FLOAT_TO_INT: &str = "spectra.std.math.float_to_int";
const MATH_INT_TO_FLOAT: &str = "spectra.std.math.int_to_float";
const MATH_FLOAT_ADD: &str = "spectra.std.math.float_add";
const MATH_FLOAT_SUB: &str = "spectra.std.math.float_sub";
const MATH_FLOAT_MUL: &str = "spectra.std.math.float_mul";
const MATH_FLOAT_DIV: &str = "spectra.std.math.float_div";
const MATH_FLOAT_ABS: &str = "spectra.std.math.float_abs";
const MATH_FLOAT_SQRT: &str = "spectra.std.math.float_sqrt";
const MATH_FLOAT_EXP: &str = "spectra.std.math.float_exp";
const MATH_FLOAT_LN: &str = "spectra.std.math.float_ln";
const MATH_FLOAT_POW: &str = "spectra.std.math.float_pow";
const MATH_TRIG_SIN: &str = "spectra.std.math.trig_sin";
const MATH_TRIG_COS: &str = "spectra.std.math.trig_cos";
const MATH_TRIG_TAN: &str = "spectra.std.math.trig_tan";
const MATH_TRIG_ATAN2: &str = "spectra.std.math.trig_atan2";
const MATH_CLAMP: &str = "spectra.std.math.clamp";
const MATH_MEAN: &str = "spectra.std.math.mean";

const IO_PRINT: &str = "spectra.std.io.print";
const IO_FLUSH: &str = "spectra.std.io.flush";
const IO_PRINT_ERR: &str = "spectra.std.io.print_err";
const IO_PRINT_TO_BUFFER: &str = "spectra.std.io.print_to_buffer";
const IO_WRITE_FILE: &str = "spectra.std.io.write_file";
const IO_READ_FILE: &str = "spectra.std.io.read_file";
const LOG_SET_LEVEL: &str = "spectra.std.log.set_level";
const LOG_ADD_SINK: &str = "spectra.std.log.add_sink";
const LOG_CLEAR_SINKS: &str = "spectra.std.log.clear_sinks";
const LOG_RECORD: &str = "spectra.std.log.record";
const TIME_NOW: &str = "spectra.std.time.now";
const TIME_NOW_MONOTONIC: &str = "spectra.std.time.now_monotonic";
const TIME_SLEEP: &str = "spectra.std.time.sleep";
const TEXT_NEW: &str = "spectra.std.text.new";
const TEXT_FROM_LIST: &str = "spectra.std.text.from_list";
const TEXT_TO_LIST: &str = "spectra.std.text.to_list";
const TEXT_LEN: &str = "spectra.std.text.len";
const TEXT_FROM_INT: &str = "spectra.std.text.from_int";
const TEXT_FROM_FLOAT: &str = "spectra.std.text.from_float";
const TEXT_PARSE_INT: &str = "spectra.std.text.parse_int";
const TEXT_PARSE_FLOAT: &str = "spectra.std.text.parse_float";
const TEXT_CONCAT: &str = "spectra.std.text.concat";
const TEXT_SUBSTRING: &str = "spectra.std.text.substring";
const TEXT_FREE: &str = "spectra.std.text.free";
const TEXT_FREE_ALL: &str = "spectra.std.text.free_all";

const LIST_NEW: &str = "spectra.std.collections.list_new";
const LIST_PUSH: &str = "spectra.std.collections.list_push";
const LIST_LEN: &str = "spectra.std.collections.list_len";
const LIST_CLEAR: &str = "spectra.std.collections.list_clear";
const LIST_FREE: &str = "spectra.std.collections.list_free";
const LIST_FREE_ALL: &str = "spectra.std.collections.list_free_all";

/// Registers the minimal standard library host functions.
pub fn register() {
    register_math();
    register_io();
    register_log();
    register_time();
    register_text();
    register_collections();
}

fn register_math() {
    register_host_function(MATH_ABS, std_math_abs);
    register_host_function(MATH_MIN, std_math_min);
    register_host_function(MATH_MAX, std_math_max);
    register_host_function(MATH_ADD, std_math_add);
    register_host_function(MATH_SUB, std_math_sub);
    register_host_function(MATH_MUL, std_math_mul);
    register_host_function(MATH_DIV, std_math_div);
    register_host_function(MATH_MOD, std_math_mod);
    register_host_function(MATH_POW, std_math_pow);
    register_host_function(MATH_CLAMP, std_math_clamp);
    register_host_function(MATH_MEAN, std_math_mean);
    register_host_function(MATH_FLOAT_TO_INT, std_math_float_to_int);
    register_host_function(MATH_INT_TO_FLOAT, std_math_int_to_float);
    register_host_function(MATH_FLOAT_ADD, std_math_float_add);
    register_host_function(MATH_FLOAT_SUB, std_math_float_sub);
    register_host_function(MATH_FLOAT_MUL, std_math_float_mul);
    register_host_function(MATH_FLOAT_DIV, std_math_float_div);
    register_host_function(MATH_FLOAT_ABS, std_math_float_abs);
    register_host_function(MATH_FLOAT_SQRT, std_math_float_sqrt);
    register_host_function(MATH_FLOAT_EXP, std_math_float_exp);
    register_host_function(MATH_FLOAT_LN, std_math_float_ln);
    register_host_function(MATH_FLOAT_POW, std_math_float_pow);
    register_host_function(MATH_TRIG_SIN, std_math_trig_sin);
    register_host_function(MATH_TRIG_COS, std_math_trig_cos);
    register_host_function(MATH_TRIG_TAN, std_math_trig_tan);
    register_host_function(MATH_TRIG_ATAN2, std_math_trig_atan2);
    register_host_function(MATH_RNG_SEED, std_math_rng_seed);
    register_host_function(MATH_RNG_NEXT, std_math_rng_next);
    register_host_function(MATH_RNG_NEXT_RANGE, std_math_rng_next_range);
    register_host_function(MATH_RNG_FREE, std_math_rng_free);
    register_host_function(MATH_RNG_FREE_ALL, std_math_rng_free_all);
    register_host_function(MATH_MEDIAN, std_math_median);
    register_host_function(MATH_VARIANCE, std_math_variance);
    register_host_function(MATH_STD_DEV, std_math_std_dev);
    register_host_function(MATH_MODE, std_math_mode);
}

fn register_io() {
    register_host_function(IO_PRINT, std_io_print);
    register_host_function(IO_FLUSH, std_io_flush);
    register_host_function(IO_PRINT_ERR, std_io_print_err);
    register_host_function(IO_PRINT_TO_BUFFER, std_io_print_to_buffer);
    register_host_function(IO_WRITE_FILE, std_io_write_file);
    register_host_function(IO_READ_FILE, std_io_read_file);
}

fn register_log() {
    register_host_function(LOG_SET_LEVEL, std_log_set_level);
    register_host_function(LOG_ADD_SINK, std_log_add_sink);
    register_host_function(LOG_CLEAR_SINKS, std_log_clear_sinks);
    register_host_function(LOG_RECORD, std_log_record);
}

fn register_time() {
    register_host_function(TIME_NOW, std_time_now);
    register_host_function(TIME_NOW_MONOTONIC, std_time_now_monotonic);
    register_host_function(TIME_SLEEP, std_time_sleep);
}

fn register_text() {
    register_host_function(TEXT_NEW, std_text_new);
    register_host_function(TEXT_FROM_LIST, std_text_from_list);
    register_host_function(TEXT_TO_LIST, std_text_to_list);
    register_host_function(TEXT_LEN, std_text_len);
    register_host_function(TEXT_FROM_INT, std_text_from_int);
    register_host_function(TEXT_FROM_FLOAT, std_text_from_float);
    register_host_function(TEXT_PARSE_INT, std_text_parse_int);
    register_host_function(TEXT_PARSE_FLOAT, std_text_parse_float);
    register_host_function(TEXT_CONCAT, std_text_concat);
    register_host_function(TEXT_SUBSTRING, std_text_substring);
    register_host_function(TEXT_FREE, std_text_free);
    register_host_function(TEXT_FREE_ALL, std_text_free_all);
}

fn register_collections() {
    register_host_function(LIST_NEW, std_list_new);
    register_host_function(LIST_PUSH, std_list_push);
    register_host_function(LIST_LEN, std_list_len);
    register_host_function(LIST_CLEAR, std_list_clear);
    register_host_function(LIST_FREE, std_list_free);
    register_host_function(LIST_FREE_ALL, std_list_free_all);
}

const I64_MIN_F64: f64 = i64::MIN as f64;
const I64_MAX_F64: f64 = i64::MAX as f64;

fn encode_f64(value: f64) -> SpectraHostValue {
    i64::from_ne_bytes(value.to_bits().to_ne_bytes())
}

fn decode_f64(value: SpectraHostValue) -> f64 {
    f64::from_bits(u64::from_ne_bytes(value.to_ne_bytes()))
}

fn usize_to_i64(value: usize) -> Result<SpectraHostValue, i32> {
    SpectraHostValue::try_from(value).map_err(|_| HOST_STATUS_ARITHMETIC_ERROR)
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

extern "C" fn std_math_add(ctx: *mut SpectraHostCallContext) -> i32 {
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

        match args[0].checked_add(args[1]) {
            Some(sum) => {
                results[0] = sum;
                HOST_STATUS_SUCCESS
            }
            None => HOST_STATUS_ARITHMETIC_ERROR,
        }
    }
}

extern "C" fn std_math_sub(ctx: *mut SpectraHostCallContext) -> i32 {
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

        match args[0].checked_sub(args[1]) {
            Some(diff) => {
                results[0] = diff;
                HOST_STATUS_SUCCESS
            }
            None => HOST_STATUS_ARITHMETIC_ERROR,
        }
    }
}

extern "C" fn std_math_mul(ctx: *mut SpectraHostCallContext) -> i32 {
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

        match args[0].checked_mul(args[1]) {
            Some(prod) => {
                results[0] = prod;
                HOST_STATUS_SUCCESS
            }
            None => HOST_STATUS_ARITHMETIC_ERROR,
        }
    }
}

extern "C" fn std_math_div(ctx: *mut SpectraHostCallContext) -> i32 {
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

        if args[1] == 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        match args[0].checked_div(args[1]) {
            Some(quot) => {
                results[0] = quot;
                HOST_STATUS_SUCCESS
            }
            None => HOST_STATUS_ARITHMETIC_ERROR,
        }
    }
}

extern "C" fn std_math_mod(ctx: *mut SpectraHostCallContext) -> i32 {
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

        if args[1] == 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        match args[0].checked_rem(args[1]) {
            Some(rem) => {
                results[0] = rem;
                HOST_STATUS_SUCCESS
            }
            None => HOST_STATUS_ARITHMETIC_ERROR,
        }
    }
}

extern "C" fn std_math_pow(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let base = args[0];
        let exponent = args[1];

        if exponent < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let exponent = match u32::try_from(exponent) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        match base.checked_pow(exponent) {
            Some(power) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = power;
                HOST_STATUS_SUCCESS
            }
            None => HOST_STATUS_ARITHMETIC_ERROR,
        }
    }
}

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

        let value = args[0];
        let min = args[1];
        let max = args[2];

        if min > max {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        results[0] = value.clamp(min, max);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_mean(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let mut sum: i128 = 0;
        for value in args {
            sum += *value as i128;
        }

        let count = args.len() as i128;
        if count == 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let mean = sum / count;

        let clamped_mean = match i64::try_from(mean) {
            Ok(val) => val,
            Err(_) => return HOST_STATUS_ARITHMETIC_ERROR,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = clamped_mean;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_to_int(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);

        let int_value = if value.is_nan() {
            0
        } else if value.is_infinite() {
            if value.is_sign_negative() {
                i64::MIN
            } else {
                i64::MAX
            }
        } else if value >= I64_MAX_F64 {
            i64::MAX
        } else if value <= I64_MIN_F64 {
            i64::MIN
        } else {
            value.trunc() as i64
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = int_value;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_int_to_float(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = args[0] as f64;
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_add(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let lhs = decode_f64(args[0]);
        let rhs = decode_f64(args[1]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(lhs + rhs);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_sub(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let lhs = decode_f64(args[0]);
        let rhs = decode_f64(args[1]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(lhs - rhs);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_mul(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let lhs = decode_f64(args[0]);
        let rhs = decode_f64(args[1]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(lhs * rhs);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_div(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let lhs = decode_f64(args[0]);
        let rhs = decode_f64(args[1]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(lhs / rhs);
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_abs(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.abs());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_sqrt(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.sqrt());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_exp(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.exp());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_ln(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.ln());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_float_pow(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let base = decode_f64(args[0]);
        let exponent = decode_f64(args[1]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(base.powf(exponent));
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_trig_sin(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.sin());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_trig_cos(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.cos());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_trig_tan(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = decode_f64(args[0]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(value.tan());
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_trig_atan2(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let y = decode_f64(args[0]);
        let x = decode_f64(args[1]);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encode_f64(y.atan2(x));
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_median(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let mut values: Vec<SpectraHostValue> = args.to_vec();
        values.sort();

        let len = values.len();
        let median = if len % 2 == 1 {
            values[len / 2]
        } else {
            let upper = values[len / 2] as i128;
            let lower = values[(len / 2) - 1] as i128;
            match upper.checked_add(lower) {
                Some(sum) => (sum / 2) as SpectraHostValue,
                None => return HOST_STATUS_ARITHMETIC_ERROR,
            }
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = median;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_variance(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        match compute_population_variance(args) {
            Ok(variance) => match i64::try_from(variance) {
                Ok(value) => {
                    let results =
                        slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                    results[0] = value;
                    HOST_STATUS_SUCCESS
                }
                Err(_) => HOST_STATUS_ARITHMETIC_ERROR,
            },
            Err(status) => status,
        }
    }
}

extern "C" fn std_math_std_dev(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        match compute_population_variance(args) {
            Ok(variance) => {
                let variance_u128 = match u128::try_from(variance) {
                    Ok(value) => value,
                    Err(_) => return HOST_STATUS_ARITHMETIC_ERROR,
                };
                let std_dev = integer_sqrt_u128(variance_u128);
                match i64::try_from(std_dev) {
                    Ok(value) => {
                        let results =
                            slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                        results[0] = value;
                        HOST_STATUS_SUCCESS
                    }
                    Err(_) => HOST_STATUS_ARITHMETIC_ERROR,
                }
            }
            Err(status) => status,
        }
    }
}

extern "C" fn std_math_mode(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let mut counts: HashMap<SpectraHostValue, i64> = HashMap::new();
        let mut best_value = args[0];
        let mut best_count = 0i64;

        for value in args {
            let counter = counts.entry(*value).or_insert(0);
            if *counter == i64::MAX {
                return HOST_STATUS_ARITHMETIC_ERROR;
            }
            *counter += 1;

            if *counter > best_count || (*counter == best_count && *value < best_value) {
                best_count = *counter;
                best_value = *value;
            }
        }

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = best_value;
    }

    HOST_STATUS_SUCCESS
}

fn compute_population_variance(args: &[SpectraHostValue]) -> Result<i128, i32> {
    if args.is_empty() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }

    let mut sum: i128 = 0;
    for value in args {
        sum = match sum.checked_add(*value as i128) {
            Some(val) => val,
            None => return Err(HOST_STATUS_ARITHMETIC_ERROR),
        };
    }

    let count = args.len() as i128;
    if count == 0 {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }

    let mean = sum / count;
    let mut variance_acc: i128 = 0;
    for value in args {
        let diff = (*value as i128) - mean;
        let square = match diff.checked_mul(diff) {
            Some(val) => val,
            None => return Err(HOST_STATUS_ARITHMETIC_ERROR),
        };
        variance_acc = match variance_acc.checked_add(square) {
            Some(val) => val,
            None => return Err(HOST_STATUS_ARITHMETIC_ERROR),
        };
    }

    Ok(variance_acc / count)
}

fn integer_sqrt_u128(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }

    let mut bit = 1u128 << 126;
    while bit > value {
        bit >>= 2;
    }

    let mut remainder = value;
    let mut result = 0u128;

    while bit != 0 {
        if remainder >= result + bit {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }

    result
}

extern "C" fn std_math_rng_seed(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let seed = args[0] as u64;

        let memory = initialize().memory();
        let rng = match memory.allocate_manual(StdRng::new(seed)) {
            Ok(rng) => rng,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };

        let handle = with_rng_registry(|registry| registry.insert(rng));
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_math_rng_next(ctx: *mut SpectraHostCallContext) -> i32 {
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

        match with_rng_registry(|registry| registry.next_value(handle)) {
            Ok(value) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = value;
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_math_rng_next_range(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let handle = args[0] as usize;
        let min = args[1];
        let max = args[2];

        match with_rng_registry(|registry| registry.next_in_range(handle, min, max)) {
            Ok(value) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = value;
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_math_rng_free(ctx: *mut SpectraHostCallContext) -> i32 {
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

        match with_rng_registry(|registry| registry.remove(handle)) {
            Ok(_) => {
                if ctx_ref.result_len > 0 {
                    if ctx_ref.results.is_null() {
                        return HOST_STATUS_INVALID_ARGUMENT;
                    }
                    let results =
                        slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
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

extern "C" fn std_math_rng_free_all(ctx: *mut SpectraHostCallContext) -> i32 {
    let freed = with_rng_registry(|registry| registry.clear_all());

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

extern "C" fn std_io_print(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len > 0 && ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_ptr = ctx_ref.args;
        let args_len = ctx_ref.arg_len;
        let results_ptr = ctx_ref.results;
        let results_len = ctx_ref.result_len;

        let args = if args_len == 0 {
            &[]
        } else {
            slice::from_raw_parts(args_ptr, args_len)
        };

        let mut stdout = io::stdout();
        for (index, value) in args.iter().enumerate() {
            if index > 0 && write!(stdout, " ").is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
            if write!(stdout, "{}", value).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        if writeln!(stdout).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }

        if results_len > 0 && !results_ptr.is_null() {
            let results = slice::from_raw_parts_mut(results_ptr, results_len);
            if !results.is_empty() {
                results[0] = args_len as SpectraHostValue;
            }
        }
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

extern "C" fn std_io_print_err(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len > 0 && ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args_ptr = ctx_ref.args;
        let args_len = ctx_ref.arg_len;

        let args = if args_len == 0 {
            &[]
        } else {
            slice::from_raw_parts(args_ptr, args_len)
        };

        let mut stderr = io::stderr();
        for (index, value) in args.iter().enumerate() {
            if index > 0 && write!(stderr, " ").is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
            if write!(stderr, "{}", value).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        if writeln!(stderr).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                results[0] = args_len as SpectraHostValue;
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_io_print_to_buffer(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        let values = &args[1..];

        let mut buffer = String::new();
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                buffer.push(' ');
            }
            if write!(buffer, "{}", value).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        buffer.push('\n');

        let bytes = buffer.into_bytes();
        match with_list_registry(|registry| registry.extend_with_bytes(handle, &bytes)) {
            Ok(len) => {
                if ctx_ref.result_len > 0 {
                    let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                    if !results.is_empty() {
                        match usize_to_i64(len) {
                            Ok(value) => results[0] = value,
                            Err(code) => return code,
                        }
                    }
                }
                HOST_STATUS_SUCCESS
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_io_write_file(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path_handle = args[0] as usize;
        let data_handle = args[1] as usize;
        let append = if ctx_ref.arg_len >= 3 { args[2] != 0 } else { false };

        let (path_bytes, data_bytes) = match with_list_registry(|registry| -> Result<(Vec<u8>, Vec<u8>), i32> {
            let path = registry.to_bytes(path_handle)?;
            let data = registry.to_bytes(data_handle)?;
            Ok((path, data))
        }) {
            Ok(result) => result,
            Err(code) => return code,
        };

        let path_str = match String::from_utf8(path_bytes) {
            Ok(path) => path,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let mut options = OpenOptions::new();
        options.write(true).create(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }

        let mut file = match options.open(&path_str) {
            Ok(file) => file,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };

        if let Err(_) = file.write_all(&data_bytes) {
            return HOST_STATUS_INTERNAL_ERROR;
        }

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                match usize_to_i64(data_bytes.len()) {
                    Ok(value) => results[0] = value,
                    Err(code) => return code,
                }
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_io_read_file(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let path_handle = args[0] as usize;
        let target_handle = if ctx_ref.arg_len >= 2 {
            Some(args[1] as usize)
        } else {
            None
        };

        let path_bytes = match with_list_registry(|registry| registry.to_bytes(path_handle)) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };

        let path_str = match String::from_utf8(path_bytes) {
            Ok(path) => path,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let contents = match fs::read(&path_str) {
            Ok(data) => data,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };

        let (handle, len) = match target_handle {
            Some(existing) => match with_list_registry(|registry| registry.replace_with_bytes(existing, &contents)) {
                Ok(len) => (existing, len),
                Err(code) => return code,
            },
            None => match with_list_registry(|registry| registry.create_from_bytes(&contents)) {
                Ok(handle) => (handle, contents.len()),
                Err(code) => return code,
            },
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        if results.is_empty() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        match usize_to_i64(handle) {
            Ok(value) => results[0] = value,
            Err(code) => return code,
        }
        if results.len() > 1 {
            match usize_to_i64(len) {
                Ok(value) => results[1] = value,
                Err(code) => return code,
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_log_set_level(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let level = match LogLevel::from_value(args[0]) {
            Ok(level) => level,
            Err(code) => return code,
        };

        let updated = with_logging_registry(|registry| registry.set_level(level));

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                results[0] = updated.to_value();
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_log_add_sink(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let kind_value = args[0];

        let mut next_index = 1;
        let sink_kind = match kind_value {
            0 => LogSinkKind::Stdout,
            1 => LogSinkKind::Stderr,
            2 => {
                if args.len() <= next_index {
                    return HOST_STATUS_INVALID_ARGUMENT;
                }
                let path_handle_value = args[next_index];
                next_index += 1;
                if path_handle_value < 0 {
                    return HOST_STATUS_INVALID_ARGUMENT;
                }
                let path_handle = path_handle_value as usize;
                let bytes = match with_list_registry(|registry| registry.to_bytes(path_handle)) {
                    Ok(bytes) => bytes,
                    Err(code) => return code,
                };
                let path = match String::from_utf8(bytes) {
                    Ok(path) => path,
                    Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
                };
                LogSinkKind::File(path)
            }
            3 => {
                if args.len() <= next_index {
                    return HOST_STATUS_INVALID_ARGUMENT;
                }
                let buffer_handle_value = args[next_index];
                next_index += 1;
                if buffer_handle_value < 0 {
                    return HOST_STATUS_INVALID_ARGUMENT;
                }
                LogSinkKind::Buffer(buffer_handle_value as usize)
            }
            4 => {
                if args.len() <= next_index {
                    return HOST_STATUS_INVALID_ARGUMENT;
                }
                let list_handle_value = args[next_index];
                next_index += 1;
                if list_handle_value < 0 {
                    return HOST_STATUS_INVALID_ARGUMENT;
                }
                LogSinkKind::EntryList(list_handle_value as usize)
            }
            _ => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let min_level = if args.len() > next_index {
            match LogLevel::from_value(args[next_index]) {
                Ok(level) => level,
                Err(code) => return code,
            }
        } else {
            LogLevel::Trace
        };

        let count = with_logging_registry(|registry| registry.add_sink(LogSink { kind: sink_kind, min_level }));

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                match usize_to_i64(count) {
                    Ok(value) => results[0] = value,
                    Err(code) => return code,
                }
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_log_clear_sinks(ctx: *mut SpectraHostCallContext) -> i32 {
    let cleared = with_logging_registry(|registry| registry.clear_sinks());

    if ctx.is_null() {
        return HOST_STATUS_SUCCESS;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 {
            if ctx_ref.results.is_null() {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                match usize_to_i64(cleared) {
                    Ok(value) => results[0] = value,
                    Err(code) => return code,
                }
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_log_record(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let level = match LogLevel::from_value(args[0]) {
            Ok(level) => level,
            Err(code) => return code,
        };

        let message_handle_value = args[1];
        if message_handle_value < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let message_handle = message_handle_value as usize;

        let message_bytes = match with_list_registry(|registry| registry.to_bytes(message_handle)) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };

        let message = match String::from_utf8(message_bytes) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let metadata = if args.len() >= 3 {
            let metadata_handle_value = args[2];
            if metadata_handle_value < 0 {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            let metadata_handle = metadata_handle_value as usize;
            let bytes = match with_list_registry(|registry| registry.to_bytes(metadata_handle)) {
                Ok(bytes) => bytes,
                Err(code) => return code,
            };
            Some(match String::from_utf8(bytes) {
                Ok(value) => value,
                Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
            })
        } else {
            None
        };

        let sinks = with_logging_registry(|registry| registry.snapshot_for(level));

        let applicable = match sinks {
            Some(sinks) => sinks,
            None => {
                if ctx_ref.result_len > 0 {
                    let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                    if !results.is_empty() {
                        results[0] = 0;
                    }
                }
                return HOST_STATUS_SUCCESS;
            }
        };

        let rendered = match build_log_entry(level, &message, metadata.as_deref()) {
            Ok(rendered) => rendered,
            Err(code) => return code,
        };

        let mut dispatched = 0usize;
        for sink in applicable {
            match sink.kind {
                LogSinkKind::Stdout => {
                    if io::stdout().write_all(&rendered.text).is_err() {
                        return HOST_STATUS_INTERNAL_ERROR;
                    }
                    dispatched += 1;
                }
                LogSinkKind::Stderr => {
                    if io::stderr().write_all(&rendered.text).is_err() {
                        return HOST_STATUS_INTERNAL_ERROR;
                    }
                    dispatched += 1;
                }
                LogSinkKind::File(ref path) => {
                    let mut options = OpenOptions::new();
                    options.create(true).append(true);
                    match options.open(path) {
                        Ok(mut file) => {
                            if let Err(_) = file.write_all(&rendered.text) {
                                return HOST_STATUS_INTERNAL_ERROR;
                            }
                        }
                        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
                    }
                    dispatched += 1;
                }
                LogSinkKind::Buffer(handle) => {
                    match with_list_registry(|registry| registry.extend_with_bytes(handle, &rendered.text)) {
                        Ok(_) => {
                            dispatched += 1;
                        }
                        Err(code) => return code,
                    }
                }
                LogSinkKind::EntryList(list_handle) => {
                    let entry_handle = match with_list_registry(|registry| registry.create_from_bytes(&rendered.json)) {
                        Ok(handle) => handle,
                        Err(code) => return code,
                    };
                    match with_list_registry(|registry| registry.push(list_handle, entry_handle as SpectraHostValue)) {
                        Ok(_) => {
                            dispatched += 1;
                        }
                        Err(code) => return code,
                    }
                }
            }
        }

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                match usize_to_i64(dispatched) {
                    Ok(value) => results[0] = value,
                    Err(code) => return code,
                }
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_time_now(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len < 2 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let duration = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };

        let seconds = match SpectraHostValue::try_from(duration.as_secs()) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };
        let nanos = SpectraHostValue::from(duration.subsec_nanos() as i64);

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = seconds;
        results[1] = nanos;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_time_now_monotonic(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len < 2 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let elapsed = monotonic_origin().elapsed();
        let seconds = match SpectraHostValue::try_from(elapsed.as_secs()) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        };
        let nanos = SpectraHostValue::from(elapsed.subsec_nanos() as i64);

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = seconds;
        results[1] = nanos;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_time_sleep(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.arg_len > 2 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len > 0 && ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let seconds = args[0];
        if seconds < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let nanos = if args.len() == 2 { args[1] } else { 0 };
        if nanos < 0 || nanos >= 1_000_000_000 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let secs_u64 = match u64::try_from(seconds) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let nanos_u32 = match u32::try_from(nanos) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let base = Duration::from_secs(secs_u64);
        let duration = match base.checked_add(Duration::from_nanos(nanos_u32 as u64)) {
            Some(total) => total,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };

        thread::sleep(duration);

        if ctx_ref.result_len > 0 {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                results[0] = 0;
            }
        }
    }

    HOST_STATUS_SUCCESS
}

fn monotonic_origin() -> Instant {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    *ORIGIN.get_or_init(Instant::now)
}

extern "C" fn std_text_new(ctx: *mut SpectraHostCallContext) -> i32 {
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

        let handle = match with_string_registry(|registry| registry.create(String::new())) {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_from_list(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let list_handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        let bytes = match with_list_registry(|registry| registry.to_bytes(list_handle)) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };

        let text = match String::from_utf8(bytes) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let handle = match with_string_registry(|registry| registry.create(text)) {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_to_list(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let text_handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        let bytes = match with_string_registry(|registry| registry.bytes(text_handle)) {
            Ok(bytes) => bytes,
            Err(code) => return code,
        };

        let list_handle = match with_list_registry(|registry| registry.create_from_bytes(&bytes)) {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = list_handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_len(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let text_handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        let length = match with_string_registry(|registry| registry.len(text_handle)) {
            Ok(value) => value,
            Err(code) => return code,
        };

        if length > i64::MAX as usize {
            return HOST_STATUS_ARITHMETIC_ERROR;
        }
        let length_value = length as SpectraHostValue;

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = length_value;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_concat(ctx: *mut SpectraHostCallContext) -> i32 {
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
        if args[0] < 0 || args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let first_handle = args[0] as usize;
        let second_handle = args[1] as usize;

        let handle_result = with_string_registry(|registry| {
            let first = registry.clone_value(first_handle)?;
            let second = registry.clone_value(second_handle)?;
            let mut combined = String::with_capacity(first.len() + second.len());
            combined.push_str(&first);
            combined.push_str(&second);
            registry.create(combined)
        });

        let handle = match handle_result {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_substring(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.arg_len > 3 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let text_handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let start = match usize::try_from(args[1]) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let length = if ctx_ref.arg_len == 3 {
            if args[2] < 0 {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            match usize::try_from(args[2]) {
                Ok(value) => Some(value),
                Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
            }
        } else {
            None
        };

        let original = match with_string_registry(|registry| registry.clone_value(text_handle)) {
            Ok(value) => value,
            Err(code) => return code,
        };

        let mut offsets: Vec<usize> = original.char_indices().map(|(idx, _)| idx).collect();
        offsets.push(original.len());
        let total_scalars = offsets.len().saturating_sub(1);

        if start > total_scalars {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let end_scalar = match length {
            Some(len) => match start.checked_add(len) {
                Some(sum) if sum <= total_scalars => sum,
                _ => return HOST_STATUS_INVALID_ARGUMENT,
            },
            None => total_scalars,
        };

        let start_byte = offsets[start];
        let end_byte = offsets[end_scalar];
        let substring = original[start_byte..end_byte].to_string();

        let handle = match with_string_registry(|registry| registry.create(substring)) {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_from_int(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let value = args[0];
        let text = value.to_string();

        let handle = match with_string_registry(|registry| registry.create(text)) {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_from_float(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let float_value = decode_f64(args[0]);
        let text = float_value.to_string();

        let handle = match with_string_registry(|registry| registry.create(text)) {
            Ok(handle) => handle,
            Err(code) => return code,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = handle as SpectraHostValue;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_parse_int(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        let text = match with_string_registry(|registry| registry.clone_value(handle)) {
            Ok(value) => value,
            Err(code) => return code,
        };

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let parsed = match trimmed.parse::<SpectraHostValue>() {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = parsed;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_parse_float(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        let text = match with_string_registry(|registry| registry.clone_value(handle)) {
            Ok(value) => value,
            Err(code) => return code,
        };

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let parsed = match trimmed.parse::<f64>() {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_INVALID_ARGUMENT,
        };

        let encoded = encode_f64(parsed);
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = encoded;
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_free(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let text_handle = if args[0] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        } else {
            args[0] as usize
        };

        match with_string_registry(|registry| registry.remove(text_handle)) {
            Ok(_) => {}
            Err(code) => return code,
        }

        if ctx_ref.result_len > 0 {
            if ctx_ref.results.is_null() {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            if !results.is_empty() {
                results[0] = 0;
            }
        }
    }

    HOST_STATUS_SUCCESS
}

extern "C" fn std_text_free_all(ctx: *mut SpectraHostCallContext) -> i32 {
    let freed = with_string_registry(|registry| registry.clear_all());

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
                match usize_to_i64(freed) {
                    Ok(value) => results[0] = value,
                    Err(code) => return code,
                }
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

fn with_string_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut StringRegistry) -> R,
{
    let registry = string_registry();
    let mut guard = registry
        .lock()
        .expect("text registry mutex poisoned");
    action(&mut guard)
}

fn string_registry() -> &'static Mutex<StringRegistry> {
    static REGISTRY: OnceLock<Mutex<StringRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(StringRegistry::new()))
}

struct StdRng {
    state: u64,
}

impl StdRng {
    fn new(seed: u64) -> Self {
        let mixed = seed ^ 0x5deece66d;
        let initial = if mixed == 0 {
            0x2545f4914f6cdd1d
        } else {
            mixed
        };
        Self { state: initial }
    }

    fn advance(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.state
    }

    fn next_value(&mut self) -> SpectraHostValue {
        (self.advance() >> 1) as SpectraHostValue
    }

    fn next_in_range(
        &mut self,
        min: SpectraHostValue,
        max: SpectraHostValue,
    ) -> Result<SpectraHostValue, i32> {
        if min > max {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }

        let span = (max as i128) - (min as i128) + 1;
        if span <= 0 {
            return Err(HOST_STATUS_ARITHMETIC_ERROR);
        }

        let raw = self.advance();
        let span_u128 = span as u128;
        let offset = (raw as u128 % span_u128) as i128;
        let value = (min as i128) + offset;

        if value < i64::MIN as i128 || value > i64::MAX as i128 {
            return Err(HOST_STATUS_ARITHMETIC_ERROR);
        }

        Ok(value as SpectraHostValue)
    }
}

struct RngRegistry {
    next_id: usize,
    rngs: HashMap<usize, ManualBox<StdRng>>,
}

impl RngRegistry {
    fn new() -> Self {
        Self {
            next_id: 1,
            rngs: HashMap::new(),
        }
    }

    fn insert(&mut self, rng: ManualBox<StdRng>) -> usize {
        let mut handle = self.next_id.max(1);
        while self.rngs.contains_key(&handle) {
            handle = handle.wrapping_add(1).max(1);
        }
        self.next_id = handle.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        self.rngs.insert(handle, rng);
        handle
    }

    fn next_value(&mut self, handle: usize) -> Result<SpectraHostValue, i32> {
        match self.rngs.get_mut(&handle) {
            Some(rng) => Ok(rng.next_value()),
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn next_in_range(
        &mut self,
        handle: usize,
        min: SpectraHostValue,
        max: SpectraHostValue,
    ) -> Result<SpectraHostValue, i32> {
        match self.rngs.get_mut(&handle) {
            Some(rng) => rng.next_in_range(min, max),
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn remove(&mut self, handle: usize) -> Result<(), i32> {
        if self.rngs.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(HOST_STATUS_NOT_FOUND)
        }
    }

    fn clear_all(&mut self) -> usize {
        let count = self.rngs.len();
        self.rngs.clear();
        self.next_id = 1;
        count
    }
}

fn with_rng_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut RngRegistry) -> R,
{
    let registry = rng_registry();
    let mut guard = registry
        .lock()
        .expect("rng registry mutex poisoned");
    action(&mut guard)
}

fn rng_registry() -> &'static Mutex<RngRegistry> {
    static REGISTRY: OnceLock<Mutex<RngRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(RngRegistry::new()))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i64)]
enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    fn from_value(value: SpectraHostValue) -> Result<Self, i32> {
        match value {
            0 => Ok(LogLevel::Trace),
            1 => Ok(LogLevel::Debug),
            2 => Ok(LogLevel::Info),
            3 => Ok(LogLevel::Warn),
            4 => Ok(LogLevel::Error),
            _ => Err(HOST_STATUS_INVALID_ARGUMENT),
        }
    }

    fn to_value(self) -> SpectraHostValue {
        self as SpectraHostValue
    }

    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

#[derive(Clone)]
enum LogSinkKind {
    Stdout,
    Stderr,
    File(String),
    Buffer(usize),
    EntryList(usize),
}

#[derive(Clone)]
struct LogSink {
    kind: LogSinkKind,
    min_level: LogLevel,
}

struct LoggingRegistry {
    global_level: LogLevel,
    sinks: Vec<LogSink>,
}

impl LoggingRegistry {
    fn new() -> Self {
        Self {
            global_level: LogLevel::Info,
            sinks: Vec::new(),
        }
    }

    fn set_level(&mut self, level: LogLevel) -> LogLevel {
        self.global_level = level;
        level
    }

    fn add_sink(&mut self, sink: LogSink) -> usize {
        self.sinks.push(sink);
        self.sinks.len()
    }

    fn clear_sinks(&mut self) -> usize {
        let count = self.sinks.len();
        self.sinks.clear();
        count
    }

    fn snapshot_for(&self, level: LogLevel) -> Option<Vec<LogSink>> {
        if level < self.global_level {
            return None;
        }
        Some(
            self.sinks
                .iter()
                .filter(|sink| level >= sink.min_level)
                .cloned()
                .collect(),
        )
    }
}

fn with_logging_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut LoggingRegistry) -> R,
{
    let registry = logging_registry();
    let mut guard = registry
        .lock()
        .expect("logging registry mutex poisoned");
    action(&mut guard)
}

fn logging_registry() -> &'static Mutex<LoggingRegistry> {
    static REGISTRY: OnceLock<Mutex<LoggingRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(LoggingRegistry::new()))
}

enum MetadataPayload {
    None,
    Structured(JsonMap<String, JsonValue>),
    Raw(String),
}

struct RenderedLogEntry {
    text: Vec<u8>,
    json: Vec<u8>,
}

fn parse_metadata(metadata: Option<&str>) -> Result<MetadataPayload, i32> {
    match metadata {
        None => Ok(MetadataPayload::None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(MetadataPayload::None);
            }
            if trimmed.starts_with('{') {
                let parsed: JsonValue = serde_json::from_str(trimmed)
                    .map_err(|_| HOST_STATUS_INVALID_ARGUMENT)?;
                match parsed {
                    JsonValue::Object(map) => Ok(MetadataPayload::Structured(map)),
                    _ => Err(HOST_STATUS_INVALID_ARGUMENT),
                }
            } else if trimmed.contains('=') {
                let map = parse_key_value_metadata(trimmed)?;
                Ok(MetadataPayload::Structured(map))
            } else {
                Ok(MetadataPayload::Raw(trimmed.to_string()))
            }
        }
    }
}

fn parse_key_value_metadata(input: &str) -> Result<JsonMap<String, JsonValue>, i32> {
    let mut map = JsonMap::new();
    let segments: Vec<&str> = if input.contains(',') {
        input.split(',').collect()
    } else {
        input.split_whitespace().collect()
    };

    for segment in segments {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut parts = segment.splitn(2, '=');
        let key = parts.next().ok_or(HOST_STATUS_INVALID_ARGUMENT)?.trim();
        let value_raw = parts.next().ok_or(HOST_STATUS_INVALID_ARGUMENT)?.trim();

        if key.is_empty() {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }

        let value = if (value_raw.starts_with('"') && value_raw.ends_with('"'))
            || (value_raw.starts_with('\'') && value_raw.ends_with('\''))
        {
            unescape_quoted(value_raw)?
        } else {
            value_raw.to_string()
        };

        map.insert(key.to_string(), convert_scalar_value(&value));
    }

    Ok(map)
}

fn unescape_quoted(input: &str) -> Result<String, i32> {
    let quote = input.chars().next().ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    if input.len() < 2 || !input.ends_with(quote) {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let inner = &input[1..input.len() - 1];
    let mut result = String::new();
    let mut chars = inner.chars();
    let mut escaping = false;
    while let Some(ch) = chars.next() {
        if escaping {
            let translated = match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                _ => ch,
            };
            result.push(translated);
            escaping = false;
        } else if ch == '\\' {
            escaping = true;
        } else {
            result.push(ch);
        }
    }
    if escaping {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    Ok(result)
}

fn convert_scalar_value(value: &str) -> JsonValue {
    if value.eq_ignore_ascii_case("true") {
        JsonValue::Bool(true)
    } else if value.eq_ignore_ascii_case("false") {
        JsonValue::Bool(false)
    } else if let Ok(int_val) = value.parse::<i64>() {
        JsonValue::Number(int_val.into())
    } else if let Ok(float_val) = value.parse::<f64>() {
        match serde_json::Number::from_f64(float_val) {
            Some(number) => JsonValue::Number(number),
            None => JsonValue::String(value.to_string()),
        }
    } else {
        JsonValue::String(value.to_string())
    }
}

fn render_field_value(value: &JsonValue) -> String {
    match value {
        JsonValue::String(text) => {
            if text.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/')) {
                text.clone()
            } else {
                let mut escaped = String::new();
                for ch in text.chars() {
                    match ch {
                        '"' => escaped.push_str("\\\""),
                        '\\' => escaped.push_str("\\\\"),
                        other => escaped.push(other),
                    }
                }
                format!("\"{}\"", escaped)
            }
        }
        _ => value.to_string(),
    }
}

fn build_log_entry(level: LogLevel, message: &str, metadata: Option<&str>) -> Result<RenderedLogEntry, i32> {
    let duration = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration,
        Err(_) => return Err(HOST_STATUS_INTERNAL_ERROR),
    };

    let sanitized_message = message.trim_end_matches(['\n', '\r']).to_string();
    let metadata_payload = parse_metadata(metadata)?;

    let mut line = String::new();
    let _ = write!(
        line,
        "{}.{:09} {} {}",
        duration.as_secs(),
        duration.subsec_nanos(),
        level.as_str(),
        sanitized_message
    );

    let mut json_obj = JsonMap::new();
    json_obj.insert(
        "timestamp".to_string(),
        json!({
            "seconds": duration.as_secs(),
            "nanos": duration.subsec_nanos(),
        }),
    );
    json_obj.insert(
        "level".to_string(),
        JsonValue::String(level.as_str().to_string()),
    );
    json_obj.insert(
        "message".to_string(),
        JsonValue::String(sanitized_message.clone()),
    );

    match metadata_payload {
        MetadataPayload::Structured(ref map) => {
            if !map.is_empty() {
                let mut segments = Vec::new();
                for (key, value) in map.iter() {
                    segments.push(format!("{}={}", key, render_field_value(value)));
                }
                if !segments.is_empty() {
                    line.push(' ');
                    line.push_str(&segments.join(" "));
                }
            }
            json_obj.insert("fields".to_string(), JsonValue::Object(map.clone()));
        }
        MetadataPayload::Raw(ref raw) => {
            if !raw.is_empty() {
                line.push(' ');
                line.push_str(raw);
                json_obj.insert("raw_metadata".to_string(), JsonValue::String(raw.clone()));
            }
        }
        MetadataPayload::None => {}
    }

    line.push('\n');

    let json_bytes = serde_json::to_vec(&JsonValue::Object(json_obj))
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;

    Ok(RenderedLogEntry {
        text: line.into_bytes(),
        json: json_bytes,
    })
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

    fn clear_list(&mut self, handle: usize) -> Result<(), i32> {
        match self.lists.get_mut(&handle) {
            Some(list) => {
                list.data.clear();
                Ok(())
            }
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn to_bytes(&self, handle: usize) -> Result<Vec<u8>, i32> {
        match self.lists.get(&handle) {
            Some(list) => {
                let mut bytes = Vec::with_capacity(list.data.len());
                for value in &list.data {
                    if *value < 0 || *value > 255 {
                        return Err(HOST_STATUS_INVALID_ARGUMENT);
                    }
                    bytes.push(*value as u8);
                }
                Ok(bytes)
            }
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn extend_with_bytes(&mut self, handle: usize, bytes: &[u8]) -> Result<usize, i32> {
        match self.lists.get_mut(&handle) {
            Some(list) => {
                list.data
                    .extend(bytes.iter().map(|byte| *byte as SpectraHostValue));
                Ok(list.data.len())
            }
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn replace_with_bytes(&mut self, handle: usize, bytes: &[u8]) -> Result<usize, i32> {
        match self.lists.get_mut(&handle) {
            Some(list) => {
                list.data.clear();
                list.data
                    .extend(bytes.iter().map(|byte| *byte as SpectraHostValue));
                Ok(list.data.len())
            }
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn create_from_bytes(&mut self, bytes: &[u8]) -> Result<usize, i32> {
        let data: Vec<SpectraHostValue> = bytes
            .iter()
            .map(|byte| *byte as SpectraHostValue)
            .collect();
        let memory = initialize().memory();
        let list = StdList { data };
        let manual = memory
            .allocate_manual(list)
            .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;
        Ok(self.insert(manual))
    }

    #[cfg(test)]
    fn snapshot_values(&self, handle: usize) -> Result<Vec<SpectraHostValue>, i32> {
        match self.lists.get(&handle) {
            Some(list) => Ok(list.data.clone()),
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
}

struct StdString {
    data: String,
}

struct StringRegistry {
    next_id: usize,
    strings: HashMap<usize, ManualBox<StdString>>,
}

impl StringRegistry {
    fn new() -> Self {
        Self {
            next_id: 1,
            strings: HashMap::new(),
        }
    }

    fn insert(&mut self, string: ManualBox<StdString>) -> usize {
        let mut handle = self.next_id.max(1);
        while self.strings.contains_key(&handle) {
            handle = handle.wrapping_add(1).max(1);
        }
        self.next_id = handle.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        self.strings.insert(handle, string);
        handle
    }

    fn create(&mut self, value: String) -> Result<usize, i32> {
        let memory = initialize().memory();
        let string = StdString { data: value };
        let manual = memory
            .allocate_manual(string)
            .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;
        Ok(self.insert(manual))
    }

    fn clone_value(&self, handle: usize) -> Result<String, i32> {
        match self.strings.get(&handle) {
            Some(string) => Ok(string.data.clone()),
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn bytes(&self, handle: usize) -> Result<Vec<u8>, i32> {
        match self.strings.get(&handle) {
            Some(string) => Ok(string.data.as_bytes().to_vec()),
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn len(&self, handle: usize) -> Result<usize, i32> {
        match self.strings.get(&handle) {
            Some(string) => Ok(string.data.chars().count()),
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn remove(&mut self, handle: usize) -> Result<(), i32> {
        if self.strings.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(HOST_STATUS_NOT_FOUND)
        }
    }

    fn clear_all(&mut self) -> usize {
        let count = self.strings.len();
        self.strings.clear();
        self.next_id = 1;
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use std::f64::consts::{FRAC_PI_2, PI};
    use std::time::{Duration, Instant};
    use serde_json::Value as JsonValue;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("stdlib test guard poisoned")
    }

    fn encode_float(value: f64) -> SpectraHostValue {
        encode_f64(value)
    }

    fn decode_float(value: SpectraHostValue) -> f64 {
        decode_f64(value)
    }

    fn list_from_bytes(bytes: &[u8]) -> usize {
        with_list_registry(|registry| {
            registry
                .create_from_bytes(bytes)
                .expect("failed to create list from bytes")
        })
    }

    fn list_to_string(handle: usize) -> String {
        with_list_registry(|registry| {
            let bytes = registry
                .to_bytes(handle)
                .expect("failed to convert list to bytes");
            String::from_utf8(bytes).expect("invalid UTF-8 in list contents")
        })
    }

    fn list_values(handle: usize) -> Vec<SpectraHostValue> {
        with_list_registry(|registry| {
            registry
                .snapshot_values(handle)
                .expect("failed to snapshot list values")
        })
    }

    fn invoke(function_name: &str, args: &[SpectraHostValue], result_len: usize) -> (i32, Vec<SpectraHostValue>) {
        let func = lookup_host_function(function_name)
            .unwrap_or_else(|| panic!("{function_name} not registered"));
        let mut results = vec![0; result_len];
        let mut ctx = SpectraHostCallContext {
            args: if args.is_empty() { ptr::null() } else { args.as_ptr() },
            arg_len: args.len(),
            results: if result_len == 0 {
                ptr::null_mut()
            } else {
                results.as_mut_ptr()
            },
            result_len,
        };

        let status = func(&mut ctx);
        (status, results)
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
        };

        let status = func(&mut ctx);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 42);
    }

    #[test]
    fn math_add_sub_mul_register_correct_results() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let add = lookup_host_function(MATH_ADD).expect("math add not registered");
        let sub = lookup_host_function(MATH_SUB).expect("math sub not registered");
        let mul = lookup_host_function(MATH_MUL).expect("math mul not registered");

        for (func, expected) in [(add, 12), (sub, 2), (mul, 35)] {
            let args = [7, 5];
            let mut results = [0];
            let mut ctx = SpectraHostCallContext {
                args: args.as_ptr(),
                arg_len: 2,
                results: results.as_mut_ptr(),
                result_len: 1,
            };

            assert_eq!(func(&mut ctx), HOST_STATUS_SUCCESS);
            assert_eq!(results[0], expected);
        }
    }

    #[test]
    fn math_div_mod_pow_cover_common_cases() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let div = lookup_host_function(MATH_DIV).expect("math div not registered");
        let modulo = lookup_host_function(MATH_MOD).expect("math mod not registered");
        let pow = lookup_host_function(MATH_POW).expect("math pow not registered");

        let mut results = [0];

        let args_div = [21, 7];
        let mut div_ctx = SpectraHostCallContext {
            args: args_div.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(div(&mut div_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 3);

        let args_mod = [23, 6];
        let mut mod_ctx = SpectraHostCallContext {
            args: args_mod.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(modulo(&mut mod_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 5);

        let args_pow = [3, 4];
        let mut pow_ctx = SpectraHostCallContext {
            args: args_pow.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(pow(&mut pow_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 81);
    }

    #[test]
    fn math_division_by_zero_returns_invalid_argument() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let div = lookup_host_function(MATH_DIV).expect("math div not registered");
        let args = [10, 0];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(div(&mut ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_overflow_returns_arithmetic_error() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let add = lookup_host_function(MATH_ADD).expect("math add not registered");
        let args = [i64::MAX, 1];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(add(&mut ctx), HOST_STATUS_ARITHMETIC_ERROR);
    }

    #[test]
    fn math_pow_negative_exponent_is_invalid_argument() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let pow = lookup_host_function(MATH_POW).expect("math pow not registered");
        let args = [2, -1];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(pow(&mut ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_clamp_respects_bounds() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let clamp = lookup_host_function(MATH_CLAMP).expect("math clamp not registered");

        let args = [10, 0, 5];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: 3,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(clamp(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 5);

        let args_below = [-10, -4, 3];
        let mut below_ctx = SpectraHostCallContext {
            args: args_below.as_ptr(),
            arg_len: 3,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(clamp(&mut below_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], -4);

        let args_invalid = [1, 5, 2];
        let mut invalid_ctx = SpectraHostCallContext {
            args: args_invalid.as_ptr(),
            arg_len: 3,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(clamp(&mut invalid_ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_mean_handles_multiple_values() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let mean = lookup_host_function(MATH_MEAN).expect("math mean not registered");

        let args = [10, 20, 30];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(mean(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 20);

        let args_single = [-3];
        let mut single_ctx = SpectraHostCallContext {
            args: args_single.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(mean(&mut single_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], -3);

        let mut empty_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(mean(&mut empty_ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_float_addition_produces_expected_sum() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let add = lookup_host_function(MATH_FLOAT_ADD).expect("float add not registered");

        let args = [encode_float(1.5), encode_float(2.25)];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(add(&mut ctx), HOST_STATUS_SUCCESS);
        let sum = decode_float(results[0]);
        assert!((sum - 3.75).abs() < 1e-9);
    }

    #[test]
    fn math_float_to_int_saturates_and_handles_nan() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let to_int =
            lookup_host_function(MATH_FLOAT_TO_INT).expect("float_to_int not registered");

        let finite_args = [encode_float(42.9)];
        let mut results = [0];
        let mut finite_ctx = SpectraHostCallContext {
            args: finite_args.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(to_int(&mut finite_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 42);

        let large_args = [encode_float(1e40)];
        let mut large_ctx = SpectraHostCallContext {
            args: large_args.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(to_int(&mut large_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], i64::MAX);

        let nan_args = [encode_float(f64::NAN)];
        let mut nan_ctx = SpectraHostCallContext {
            args: nan_args.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(to_int(&mut nan_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 0);
    }

    #[test]
    fn math_trig_functions_cover_common_angles() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let sin_fn = lookup_host_function(MATH_TRIG_SIN).expect("sin not registered");
        let cos_fn = lookup_host_function(MATH_TRIG_COS).expect("cos not registered");
        let atan2_fn = lookup_host_function(MATH_TRIG_ATAN2).expect("atan2 not registered");

        let sin_args = [encode_float(FRAC_PI_2)];
        let mut results = [0];
        let mut sin_ctx = SpectraHostCallContext {
            args: sin_args.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(sin_fn(&mut sin_ctx), HOST_STATUS_SUCCESS);
        assert!((decode_float(results[0]) - 1.0).abs() < 1e-9);

        let cos_args = [encode_float(PI)];
        let mut cos_ctx = SpectraHostCallContext {
            args: cos_args.as_ptr(),
            arg_len: 1,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(cos_fn(&mut cos_ctx), HOST_STATUS_SUCCESS);
        assert!((decode_float(results[0]) + 1.0).abs() < 1e-9);

        let atan2_args = [encode_float(1.0), encode_float(-1.0)];
        let mut atan2_ctx = SpectraHostCallContext {
            args: atan2_args.as_ptr(),
            arg_len: 2,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(atan2_fn(&mut atan2_ctx), HOST_STATUS_SUCCESS);
        let angle = decode_float(results[0]);
        assert!((angle - (3.0 * PI / 4.0)).abs() < 1e-9);
    }

    #[test]
    fn math_median_handles_even_and_odd_inputs() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let median_fn = lookup_host_function(MATH_MEDIAN).expect("math median not registered");

        let odd_args = [9, 1, 5];
        let mut results = [0];
        let mut odd_ctx = SpectraHostCallContext {
            args: odd_args.as_ptr(),
            arg_len: odd_args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(median_fn(&mut odd_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 5);

        let even_args = [10, 2, 8, 1];
        let mut even_ctx = SpectraHostCallContext {
            args: even_args.as_ptr(),
            arg_len: even_args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(median_fn(&mut even_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 5);

        let mut empty_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(median_fn(&mut empty_ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_variance_handles_values() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let variance_fn = lookup_host_function(MATH_VARIANCE).expect("math variance not registered");

        let args = [1, 3, 5];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(variance_fn(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 2);

        let constant_args = [42, 42, 42];
        let mut constant_ctx = SpectraHostCallContext {
            args: constant_args.as_ptr(),
            arg_len: constant_args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(variance_fn(&mut constant_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 0);

        let mut empty_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(variance_fn(&mut empty_ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_standard_deviation_handles_values() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let std_dev_fn = lookup_host_function(MATH_STD_DEV).expect("math std_dev not registered");

        let args = [1, 3, 5];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(std_dev_fn(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 1);

        let constant_args = [42, 42, 42, 42];
        let mut constant_ctx = SpectraHostCallContext {
            args: constant_args.as_ptr(),
            arg_len: constant_args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(std_dev_fn(&mut constant_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 0);

        let mut empty_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(std_dev_fn(&mut empty_ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn math_mode_prefers_smallest_value_on_tie() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let mode_fn = lookup_host_function(MATH_MODE).expect("math mode not registered");

        let args = [3, 1, 2, 3, 2, 2];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(mode_fn(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 2);

        let tie_args = [7, 9, 9, 7];
        let mut tie_ctx = SpectraHostCallContext {
            args: tie_args.as_ptr(),
            arg_len: tie_args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(mode_fn(&mut tie_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 7);

        let mut empty_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(mode_fn(&mut empty_ctx), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn io_print_err_accepts_arguments_and_counts() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let print_err = lookup_host_function(IO_PRINT_ERR).expect("print_err not registered");

        let args = [1, 2, 3];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(print_err(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 3);
    }

    #[test]
    fn io_print_to_buffer_appends_textual_output() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let print_buffer =
            lookup_host_function(IO_PRINT_TO_BUFFER).expect("print_to_buffer not registered");

        let buffer_handle = list_from_bytes(&[]);
        let args = [buffer_handle as SpectraHostValue, 42, -7];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(print_buffer(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 6); // "42 -7\n"

        let contents = list_to_string(buffer_handle);
        assert_eq!(contents, "42 -7\n");
    }

    #[test]
    fn io_write_and_read_file_roundtrip() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let write_file =
            lookup_host_function(IO_WRITE_FILE).expect("write_file not registered");
        let read_file =
            lookup_host_function(IO_READ_FILE).expect("read_file not registered");

        let temp_dir = std::env::temp_dir();
        let unique_name = format!(
            "spectra_io_test_{}_{:?}.txt",
            std::process::id(),
            std::time::SystemTime::now()
        );
        let file_path = temp_dir.join(unique_name);
        let path_string = file_path
            .to_str()
            .expect("temp file path must be valid UTF-8")
            .to_string();

        let path_handle = list_from_bytes(path_string.as_bytes());
        let data_handle = list_from_bytes(b"Hello Spectra!\n");

        let args = [
            path_handle as SpectraHostValue,
            data_handle as SpectraHostValue,
            0,
        ];
        let mut write_results = [0];
        let mut write_ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: write_results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(write_file(&mut write_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(write_results[0], 15);

        let append_data_handle = list_from_bytes(b"Second line\n");
        let append_args = [
            path_handle as SpectraHostValue,
            append_data_handle as SpectraHostValue,
            1,
        ];
        let mut append_ctx = SpectraHostCallContext {
            args: append_args.as_ptr(),
            arg_len: append_args.len(),
            results: write_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(write_file(&mut append_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(write_results[0], 12);

        let mut read_results = [0, 0];
        let mut read_ctx = SpectraHostCallContext {
            args: [path_handle as SpectraHostValue].as_ptr(),
            arg_len: 1,
            results: read_results.as_mut_ptr(),
            result_len: 2,
        };

        assert_eq!(read_file(&mut read_ctx), HOST_STATUS_SUCCESS);
        let buffer_handle = read_results[0] as usize;
        assert_eq!(read_results[1], 27);

        let contents = list_to_string(buffer_handle);
        assert_eq!(contents, "Hello Spectra!\nSecond line\n");

        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn log_record_respects_global_level() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let set_level = lookup_host_function(LOG_SET_LEVEL).expect("log set_level not registered");
        let add_sink = lookup_host_function(LOG_ADD_SINK).expect("log add_sink not registered");
        let clear_sinks =
            lookup_host_function(LOG_CLEAR_SINKS).expect("log clear_sinks not registered");
        let record_fn = lookup_host_function(LOG_RECORD).expect("log record not registered");

        let mut clear_results = [0];
        let mut clear_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: clear_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(clear_sinks(&mut clear_ctx), HOST_STATUS_SUCCESS);

        let buffer_handle = list_from_bytes(&[]);
        let sink_args = [
            3,
            buffer_handle as SpectraHostValue,
            LogLevel::Trace.to_value(),
        ];
        let mut sink_results = [0];
        let mut sink_ctx = SpectraHostCallContext {
            args: sink_args.as_ptr(),
            arg_len: sink_args.len(),
            results: sink_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(add_sink(&mut sink_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(sink_results[0], 1);

        let level_args = [LogLevel::Warn.to_value()];
        let mut level_results = [0];
        let mut level_ctx = SpectraHostCallContext {
            args: level_args.as_ptr(),
            arg_len: 1,
            results: level_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(set_level(&mut level_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(level_results[0], LogLevel::Warn.to_value());

        let skip_message = list_from_bytes(b"should not log");
        let skip_args = [
            LogLevel::Info.to_value(),
            skip_message as SpectraHostValue,
        ];
        let mut skip_results = [0];
        let mut skip_ctx = SpectraHostCallContext {
            args: skip_args.as_ptr(),
            arg_len: skip_args.len(),
            results: skip_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(record_fn(&mut skip_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(skip_results[0], 0);
        assert!(list_to_string(buffer_handle).is_empty());

        let emit_message = list_from_bytes(b"critical failure");
        let emit_args = [
            LogLevel::Error.to_value(),
            emit_message as SpectraHostValue,
        ];
        let mut emit_results = [0];
        let mut emit_ctx = SpectraHostCallContext {
            args: emit_args.as_ptr(),
            arg_len: emit_args.len(),
            results: emit_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(record_fn(&mut emit_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(emit_results[0], 1);
        let contents = list_to_string(buffer_handle);
        assert!(contents.contains("ERROR critical failure"));
    }

    #[test]
    fn log_sink_level_and_metadata_are_applied() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let set_level = lookup_host_function(LOG_SET_LEVEL).expect("log set_level not registered");
        let add_sink = lookup_host_function(LOG_ADD_SINK).expect("log add_sink not registered");
        let clear_sinks =
            lookup_host_function(LOG_CLEAR_SINKS).expect("log clear_sinks not registered");
        let record_fn = lookup_host_function(LOG_RECORD).expect("log record not registered");

        let mut clear_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(clear_sinks(&mut clear_ctx), HOST_STATUS_SUCCESS);

        let buffer_handle = list_from_bytes(&[]);
        let sink_args = [
            3,
            buffer_handle as SpectraHostValue,
            LogLevel::Info.to_value(),
        ];
        let mut sink_ctx = SpectraHostCallContext {
            args: sink_args.as_ptr(),
            arg_len: sink_args.len(),
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(add_sink(&mut sink_ctx), HOST_STATUS_SUCCESS);

        let level_args = [LogLevel::Trace.to_value()];
        let mut level_ctx = SpectraHostCallContext {
            args: level_args.as_ptr(),
            arg_len: 1,
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(set_level(&mut level_ctx), HOST_STATUS_SUCCESS);

        let skip_message = list_from_bytes(b"skip");
        let skip_args = [
            LogLevel::Debug.to_value(),
            skip_message as SpectraHostValue,
        ];
        let mut skip_ctx = SpectraHostCallContext {
            args: skip_args.as_ptr(),
            arg_len: skip_args.len(),
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(record_fn(&mut skip_ctx), HOST_STATUS_SUCCESS);
        assert!(list_to_string(buffer_handle).is_empty());

        let emit_message = list_from_bytes(b"user logged in");
        let metadata_handle = list_from_bytes(b"{\"request_id\":42}");
        let emit_args = [
            LogLevel::Info.to_value(),
            emit_message as SpectraHostValue,
            metadata_handle as SpectraHostValue,
        ];
        let mut emit_results = [0];
        let mut emit_ctx = SpectraHostCallContext {
            args: emit_args.as_ptr(),
            arg_len: emit_args.len(),
            results: emit_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(record_fn(&mut emit_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(emit_results[0], 1);
        let contents = list_to_string(buffer_handle);
        assert!(contents.contains("INFO user logged in"));
        assert!(contents.contains("request_id=42"));
    }

    #[test]
    fn log_key_value_metadata_is_structured() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let set_level = lookup_host_function(LOG_SET_LEVEL).expect("log set_level not registered");
        let add_sink = lookup_host_function(LOG_ADD_SINK).expect("log add_sink not registered");
        let clear_sinks =
            lookup_host_function(LOG_CLEAR_SINKS).expect("log clear_sinks not registered");
        let record_fn = lookup_host_function(LOG_RECORD).expect("log record not registered");

        let mut clear_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(clear_sinks(&mut clear_ctx), HOST_STATUS_SUCCESS);

        let buffer_handle = list_from_bytes(&[]);
        let sink_args = [
            3,
            buffer_handle as SpectraHostValue,
            LogLevel::Trace.to_value(),
        ];
        let mut sink_ctx = SpectraHostCallContext {
            args: sink_args.as_ptr(),
            arg_len: sink_args.len(),
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(add_sink(&mut sink_ctx), HOST_STATUS_SUCCESS);

        let level_args = [LogLevel::Trace.to_value()];
        let mut level_ctx = SpectraHostCallContext {
            args: level_args.as_ptr(),
            arg_len: 1,
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(set_level(&mut level_ctx), HOST_STATUS_SUCCESS);

        let message_handle = list_from_bytes(b"kv payload received");
        let metadata_handle = list_from_bytes(b"user=\"tom\",attempts=3,success=true");
        let args = [
            LogLevel::Debug.to_value(),
            message_handle as SpectraHostValue,
            metadata_handle as SpectraHostValue,
        ];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(record_fn(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 1);

        let contents = list_to_string(buffer_handle);
        assert!(contents.contains("DEBUG kv payload received"));
        assert!(contents.contains("user=\"tom\""));
        assert!(contents.contains("attempts=3"));
        assert!(contents.contains("success=true"));
    }

    #[test]
    fn log_json_metadata_populates_entry_list_sink() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let set_level = lookup_host_function(LOG_SET_LEVEL).expect("log set_level not registered");
        let add_sink = lookup_host_function(LOG_ADD_SINK).expect("log add_sink not registered");
        let clear_sinks =
            lookup_host_function(LOG_CLEAR_SINKS).expect("log clear_sinks not registered");
        let record_fn = lookup_host_function(LOG_RECORD).expect("log record not registered");

        let mut clear_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(clear_sinks(&mut clear_ctx), HOST_STATUS_SUCCESS);

        let list_handle = list_from_bytes(&[]);

        let sink_args = [
            4,
            list_handle as SpectraHostValue,
            LogLevel::Debug.to_value(),
        ];
        let mut sink_ctx = SpectraHostCallContext {
            args: sink_args.as_ptr(),
            arg_len: sink_args.len(),
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(add_sink(&mut sink_ctx), HOST_STATUS_SUCCESS);

        let level_args = [LogLevel::Trace.to_value()];
        let mut level_ctx = SpectraHostCallContext {
            args: level_args.as_ptr(),
            arg_len: 1,
            results: ptr::null_mut(),
            result_len: 0,
        };
        assert_eq!(set_level(&mut level_ctx), HOST_STATUS_SUCCESS);

        let message_handle = list_from_bytes(b"structured login");
        let metadata_handle = list_from_bytes(b"{\"user\":\"tom\",\"attempts\":3}");
        let args = [
            LogLevel::Debug.to_value(),
            message_handle as SpectraHostValue,
            metadata_handle as SpectraHostValue,
        ];
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(record_fn(&mut ctx), HOST_STATUS_SUCCESS);
        assert_eq!(results[0], 1);

        let entry_handles = list_values(list_handle);
        assert_eq!(entry_handles.len(), 1);
        let entry_handle = usize::try_from(entry_handles[0]).expect("entry handle must be non-negative");
        let entry_json = list_to_string(entry_handle);
        let parsed: JsonValue = serde_json::from_str(&entry_json).expect("entry must be valid JSON");

        assert_eq!(parsed["level"], JsonValue::String("DEBUG".to_string()));
        assert_eq!(parsed["message"], JsonValue::String("structured login".to_string()));
        let fields = parsed["fields"].as_object().expect("fields must be an object");
        assert_eq!(fields["user"], JsonValue::String("tom".to_string()));
        assert_eq!(fields["attempts"].as_i64(), Some(3));
    }

    #[test]
    fn text_new_and_len_return_zero_length() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let (status, results) = invoke(TEXT_NEW, &[], 1);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let handle = results[0] as usize;

        let args = [handle as SpectraHostValue];
        let (len_status, len_results) = invoke(TEXT_LEN, &args, 1);
        assert_eq!(len_status, HOST_STATUS_SUCCESS);
        assert_eq!(len_results[0], 0);

        let _ = invoke(TEXT_FREE, &args, 0);
    }

    #[test]
    fn text_from_list_and_to_list_roundtrip_utf8() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let source = "Ol\u{00E1}";
        let list_handle = list_from_bytes(source.as_bytes());
        let args = [list_handle as SpectraHostValue];

        let (status, results) = invoke(TEXT_FROM_LIST, &args, 1);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let text_handle = results[0] as usize;

        let text_args = [text_handle as SpectraHostValue];
        let (to_list_status, to_list_results) = invoke(TEXT_TO_LIST, &text_args, 1);
        assert_eq!(to_list_status, HOST_STATUS_SUCCESS);
        let roundtrip_list = to_list_results[0] as usize;
        assert_eq!(list_to_string(roundtrip_list), source);

        let _ = invoke(TEXT_FREE, &text_args, 0);
    }

    #[test]
    fn text_len_counts_unicode_scalars() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let source = "a\u{1F31F}b";
        let list_handle = list_from_bytes(source.as_bytes());
        let args = [list_handle as SpectraHostValue];
        let (create_status, create_results) = invoke(TEXT_FROM_LIST, &args, 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let text_handle = create_results[0] as usize;

        let len_args = [text_handle as SpectraHostValue];
        let (len_status, len_results) = invoke(TEXT_LEN, &len_args, 1);
        assert_eq!(len_status, HOST_STATUS_SUCCESS);
        assert_eq!(len_results[0], 3);

        let _ = invoke(TEXT_FREE, &len_args, 0);
    }

    #[test]
    fn text_concat_produces_new_combined_string() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let left = list_from_bytes("foo".as_bytes());
        let right = list_from_bytes("bar".as_bytes());

        let (left_status, left_results) = invoke(TEXT_FROM_LIST, &[left as SpectraHostValue], 1);
        assert_eq!(left_status, HOST_STATUS_SUCCESS);
        let left_handle = left_results[0] as usize;

        let (right_status, right_results) = invoke(TEXT_FROM_LIST, &[right as SpectraHostValue], 1);
        assert_eq!(right_status, HOST_STATUS_SUCCESS);
        let right_handle = right_results[0] as usize;

        let args = [left_handle as SpectraHostValue, right_handle as SpectraHostValue];
        let (concat_status, concat_results) = invoke(TEXT_CONCAT, &args, 1);
        assert_eq!(concat_status, HOST_STATUS_SUCCESS);
        let combined_handle = concat_results[0] as usize;

        let (to_list_status, to_list_results) = invoke(TEXT_TO_LIST, &[combined_handle as SpectraHostValue], 1);
        assert_eq!(to_list_status, HOST_STATUS_SUCCESS);
        let list_handle = to_list_results[0] as usize;
        assert_eq!(list_to_string(list_handle), "foobar");

        let _ = invoke(TEXT_FREE, &[left_handle as SpectraHostValue], 0);
        let _ = invoke(TEXT_FREE, &[right_handle as SpectraHostValue], 0);
        let _ = invoke(TEXT_FREE, &[combined_handle as SpectraHostValue], 0);
    }

    #[test]
    fn text_substring_defaults_to_remainder() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let base_list = list_from_bytes(b"spectra");
        let (create_status, create_results) = invoke(TEXT_FROM_LIST, &[base_list as SpectraHostValue], 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let source_handle = create_results[0] as usize;

        let args = [source_handle as SpectraHostValue, 3];
        let (status, results) = invoke(TEXT_SUBSTRING, &args, 1);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let slice_handle = results[0] as usize;

        let (to_list_status, to_list_results) = invoke(TEXT_TO_LIST, &[slice_handle as SpectraHostValue], 1);
        assert_eq!(to_list_status, HOST_STATUS_SUCCESS);
        let list_handle = to_list_results[0] as usize;
        assert_eq!(list_to_string(list_handle), "ctra");

        let _ = invoke(TEXT_FREE, &[source_handle as SpectraHostValue], 0);
        let _ = invoke(TEXT_FREE, &[slice_handle as SpectraHostValue], 0);
    }

    #[test]
    fn text_substring_with_explicit_length() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let base_list = list_from_bytes(b"spectra");
        let (create_status, create_results) = invoke(TEXT_FROM_LIST, &[base_list as SpectraHostValue], 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let source_handle = create_results[0] as usize;

        let args = [source_handle as SpectraHostValue, 1, 2];
        let (status, results) = invoke(TEXT_SUBSTRING, &args, 1);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let slice_handle = results[0] as usize;

        let (to_list_status, to_list_results) = invoke(TEXT_TO_LIST, &[slice_handle as SpectraHostValue], 1);
        assert_eq!(to_list_status, HOST_STATUS_SUCCESS);
        let list_handle = to_list_results[0] as usize;
        assert_eq!(list_to_string(list_handle), "pe");

        let _ = invoke(TEXT_FREE, &[source_handle as SpectraHostValue], 0);
        let _ = invoke(TEXT_FREE, &[slice_handle as SpectraHostValue], 0);
    }

    #[test]
    fn text_substring_out_of_bounds_returns_error() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let base_list = list_from_bytes(b"ab");
        let (create_status, create_results) = invoke(TEXT_FROM_LIST, &[base_list as SpectraHostValue], 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let source_handle = create_results[0] as usize;

        let args = [source_handle as SpectraHostValue, 5];
        let (status, _) = invoke(TEXT_SUBSTRING, &args, 1);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);

        let _ = invoke(TEXT_FREE, &[source_handle as SpectraHostValue], 0);
    }

    #[test]
    fn text_from_int_and_parse_int_roundtrip() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let value = 12345;
        let (create_status, create_results) = invoke(TEXT_FROM_INT, &[value], 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let handle = create_results[0] as usize;

        let (to_list_status, to_list_results) = invoke(TEXT_TO_LIST, &[handle as SpectraHostValue], 1);
        assert_eq!(to_list_status, HOST_STATUS_SUCCESS);
        let list_handle = to_list_results[0] as usize;
        assert_eq!(list_to_string(list_handle), "12345");

        let (parse_status, parse_results) = invoke(TEXT_PARSE_INT, &[handle as SpectraHostValue], 1);
        assert_eq!(parse_status, HOST_STATUS_SUCCESS);
        assert_eq!(parse_results[0], value);

        let _ = invoke(TEXT_FREE, &[handle as SpectraHostValue], 0);
    }

    #[test]
    fn text_parse_int_rejects_invalid_payload() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let list_handle = list_from_bytes(b"not_a_number");
        let (create_status, create_results) = invoke(TEXT_FROM_LIST, &[list_handle as SpectraHostValue], 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let text_handle = create_results[0] as usize;

        let (parse_status, _) = invoke(TEXT_PARSE_INT, &[text_handle as SpectraHostValue], 1);
        assert_eq!(parse_status, HOST_STATUS_INVALID_ARGUMENT);

        let _ = invoke(TEXT_FREE, &[text_handle as SpectraHostValue], 0);
    }

    #[test]
    fn text_from_float_and_parse_float_roundtrip() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let float_value = 3.5f64;
        let packed = encode_float(float_value);

        let (create_status, create_results) = invoke(TEXT_FROM_FLOAT, &[packed], 1);
        assert_eq!(create_status, HOST_STATUS_SUCCESS);
        let handle = create_results[0] as usize;

        let (parse_status, parse_results) = invoke(TEXT_PARSE_FLOAT, &[handle as SpectraHostValue], 1);
        assert_eq!(parse_status, HOST_STATUS_SUCCESS);
        let parsed = decode_float(parse_results[0]);
        assert!((parsed - float_value).abs() < f64::EPSILON);

        let _ = invoke(TEXT_FREE, &[handle as SpectraHostValue], 0);
    }

    #[test]
    fn time_now_returns_unix_epoch_pair() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let now_fn = lookup_host_function(TIME_NOW).expect("time now not registered");

        let mut results = [0, 0];
        let mut ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: results.as_mut_ptr(),
            result_len: results.len(),
        };

        assert_eq!(now_fn(&mut ctx), HOST_STATUS_SUCCESS);
        assert!(results[0] >= 0);
        assert!(results[1] >= 0);
        assert!(results[1] < 1_000_000_000);
    }

    #[test]
    fn time_now_monotonic_is_non_decreasing() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let monotonic_fn =
            lookup_host_function(TIME_NOW_MONOTONIC).expect("time now_monotonic not registered");

        let mut first = [0, 0];
        let mut first_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: first.as_mut_ptr(),
            result_len: first.len(),
        };
        assert_eq!(monotonic_fn(&mut first_ctx), HOST_STATUS_SUCCESS);

        std::thread::sleep(Duration::from_millis(5));

        let mut second = [0, 0];
        let mut second_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: second.as_mut_ptr(),
            result_len: second.len(),
        };
        assert_eq!(monotonic_fn(&mut second_ctx), HOST_STATUS_SUCCESS);

        let first_duration = Duration::new(
            u64::try_from(first[0]).expect("monotonic seconds must be non-negative"),
            u32::try_from(first[1]).expect("monotonic nanos must be non-negative"),
        );
        let second_duration = Duration::new(
            u64::try_from(second[0]).expect("monotonic seconds must be non-negative"),
            u32::try_from(second[1]).expect("monotonic nanos must be non-negative"),
        );

        assert!(second_duration >= first_duration);
    }

    #[test]
    fn time_sleep_blocks_for_requested_duration() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let sleep_fn = lookup_host_function(TIME_SLEEP).expect("time sleep not registered");

        let args: [SpectraHostValue; 2] = [0, 20_000_000];
        let mut results = [1];
        let mut ctx = SpectraHostCallContext {
            args: args.as_ptr(),
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: results.len(),
        };

        let before = Instant::now();
        assert_eq!(sleep_fn(&mut ctx), HOST_STATUS_SUCCESS);
        let elapsed = before.elapsed();

        assert!(elapsed >= Duration::from_millis(10));
        assert_eq!(results[0], 0);
    }

    #[test]
    fn math_rng_seed_is_deterministic() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        let seed_fn = lookup_host_function(MATH_RNG_SEED).expect("rng_seed not registered");
        let next_fn = lookup_host_function(MATH_RNG_NEXT).expect("rng_next not registered");

        let seed_args = [12345];
        let mut handle_result = [0];
        let mut seed_ctx = SpectraHostCallContext {
            args: seed_args.as_ptr(),
            arg_len: 1,
            results: handle_result.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(seed_fn(&mut seed_ctx), HOST_STATUS_SUCCESS);
        let mut next_results = [0];
        let mut next_ctx = SpectraHostCallContext {
            args: handle_result.as_ptr(),
            arg_len: 1,
            results: next_results.as_mut_ptr(),
            result_len: 1,
        };

        assert_eq!(next_fn(&mut next_ctx), HOST_STATUS_SUCCESS);
        let first = next_results[0];
        assert_eq!(next_fn(&mut next_ctx), HOST_STATUS_SUCCESS);
        let second = next_results[0];

        let seed_args_again = [12345];
        let mut handle_again = [0];
        let mut seed_ctx_again = SpectraHostCallContext {
            args: seed_args_again.as_ptr(),
            arg_len: 1,
            results: handle_again.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(seed_fn(&mut seed_ctx_again), HOST_STATUS_SUCCESS);

        let mut next_ctx_again = SpectraHostCallContext {
            args: handle_again.as_ptr(),
            arg_len: 1,
            results: next_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(next_fn(&mut next_ctx_again), HOST_STATUS_SUCCESS);
        assert_eq!(next_results[0], first);
        assert_eq!(next_fn(&mut next_ctx_again), HOST_STATUS_SUCCESS);
        assert_eq!(next_results[0], second);

        let mut free_ctx = SpectraHostCallContext {
            args: handle_result.as_ptr(),
            arg_len: 1,
            results: ptr::null_mut(),
            result_len: 0,
        };
        let free_fn = lookup_host_function(MATH_RNG_FREE).expect("rng_free not registered");
        assert_eq!(free_fn(&mut free_ctx), HOST_STATUS_SUCCESS);
    }

    #[test]
    fn math_rng_next_range_obeys_bounds() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        let seed_fn = lookup_host_function(MATH_RNG_SEED).expect("rng_seed not registered");
        let range_fn =
            lookup_host_function(MATH_RNG_NEXT_RANGE).expect("rng_next_range not registered");

        let seed_args = [2024];
        let mut handle_result = [0];
        let mut seed_ctx = SpectraHostCallContext {
            args: seed_args.as_ptr(),
            arg_len: 1,
            results: handle_result.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(seed_fn(&mut seed_ctx), HOST_STATUS_SUCCESS);

        let handle = handle_result[0];
        let mut range_results = [0];
        let range_args = [handle, -5, 5];
        let mut range_ctx = SpectraHostCallContext {
            args: range_args.as_ptr(),
            arg_len: 3,
            results: range_results.as_mut_ptr(),
            result_len: 1,
        };

        for _ in 0..10 {
            assert_eq!(range_fn(&mut range_ctx), HOST_STATUS_SUCCESS);
            assert!((-5..=5).contains(&range_results[0]));
        }

        let invalid_args = [handle, 10, -10];
        let mut invalid_ctx = SpectraHostCallContext {
            args: invalid_args.as_ptr(),
            arg_len: 3,
            results: range_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(range_fn(&mut invalid_ctx), HOST_STATUS_INVALID_ARGUMENT);

        let free_all = lookup_host_function(MATH_RNG_FREE_ALL).expect("rng_free_all not registered");
        let mut free_all_results = [0];
        let mut free_all_ctx = SpectraHostCallContext {
            args: ptr::null(),
            arg_len: 0,
            results: free_all_results.as_mut_ptr(),
            result_len: 1,
        };
        assert_eq!(free_all(&mut free_all_ctx), HOST_STATUS_SUCCESS);
        assert!(free_all_results[0] >= 1);
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
        };
        assert_eq!(clear_fn(&mut clear_ctx), HOST_STATUS_SUCCESS);

        let free_fn = lookup_host_function(LIST_FREE).expect("list_free not registered");
        let free_args = [handle as SpectraHostValue];
        let mut free_ctx = SpectraHostCallContext {
            args: free_args.as_ptr(),
            arg_len: 1,
            results: ptr::null_mut(),
            result_len: 0,
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
        };
        assert_eq!(free_all_fn(&mut free_all_ctx), HOST_STATUS_SUCCESS);
        assert_eq!(free_all_results[0], 0);
    }
}
