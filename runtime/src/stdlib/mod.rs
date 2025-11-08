use crate::ffi::{
    register_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_ARITHMETIC_ERROR,
    HOST_STATUS_INTERNAL_ERROR, HOST_STATUS_INVALID_ARGUMENT, HOST_STATUS_NOT_FOUND,
    HOST_STATUS_SUCCESS,
};
use crate::initialize;
use crate::memory::ManualBox;
use std::collections::HashMap;
use std::io::{self, Write};
use std::slice;
use std::sync::{Mutex, OnceLock};

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

const IO_PRINT: &str = "spectra.std.io.print";
const IO_FLUSH: &str = "spectra.std.io.flush";

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
}

fn register_io() {
    register_host_function(IO_PRINT, std_io_print);
    register_host_function(IO_FLUSH, std_io_flush);
}

fn register_collections() {
    register_host_function(LIST_NEW, std_list_new);
    register_host_function(LIST_PUSH, std_list_push);
    register_host_function(LIST_LEN, std_list_len);
    register_host_function(LIST_CLEAR, std_list_clear);
    register_host_function(LIST_FREE, std_list_free);
    register_host_function(LIST_FREE_ALL, std_list_free_all);
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
