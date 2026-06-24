use crate::ffi::{
    register_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INTERNAL_ERROR,
    HOST_STATUS_INVALID_ARGUMENT, HOST_STATUS_NOT_FOUND, HOST_STATUS_SUCCESS,
};
use crate::initialize;
use crate::memory::ManualBox;
use crate::reactor::{self, Interest, ReactorEvent};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::slice;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant as StdInstant, SystemTime, UNIX_EPOCH};

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

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

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
const STR_EQ: &str = "spectra.std.string.eq";
const STR_CONCAT: &str = "spectra.std.string.concat";
const STR_REPEAT: &str = "spectra.std.string.repeat_str";
const STR_BUILDER_NEW: &str = "spectra.std.string.builder_new";
const STR_BUILDER_PUSH: &str = "spectra.std.string.builder_push";
const STR_BUILDER_LEN: &str = "spectra.std.string.builder_len";
const STR_BUILDER_FINISH: &str = "spectra.std.string.builder_finish";
const STR_BUILDER_FREE: &str = "spectra.std.string.builder_free";
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
const TIME_MONOTONIC_MILLIS: &str = "spectra.std.time.monotonic_millis";
const TIME_MONOTONIC_NANOS: &str = "spectra.std.time.monotonic_nanos";
const TIME_DURATION_MS: &str = "spectra.std.time.duration_ms";
const TIME_DURATION_SECS: &str = "spectra.std.time.duration_secs";
const TIME_DURATION_MILLIS: &str = "spectra.std.time.duration_millis";
const TIME_DURATION_SECS_VALUE: &str = "spectra.std.time.duration_secs_value";
const TIME_DURATION_ADD: &str = "spectra.std.time.duration_add";
const TIME_DURATION_SUB: &str = "spectra.std.time.duration_sub";
const TIME_INSTANT_NOW: &str = "spectra.std.time.instant_now";
const TIME_INSTANT_ELAPSED_MS: &str = "spectra.std.time.instant_elapsed_ms";
const TIME_INSTANT_ADD: &str = "spectra.std.time.instant_add";
const TIME_INSTANT_HAS_ELAPSED: &str = "spectra.std.time.instant_has_elapsed";
const TIME_SLEEP: &str = "spectra.std.time.sleep";
const TIME_UNIX_TO_UTC: &str = "spectra.std.time.unix_to_utc";
const TIME_UTC_YEAR: &str = "spectra.std.time.utc_year";
const TIME_UTC_MONTH: &str = "spectra.std.time.utc_month";
const TIME_UTC_DAY: &str = "spectra.std.time.utc_day";
const TIME_UTC_HOUR: &str = "spectra.std.time.utc_hour";
const TIME_UTC_MINUTE: &str = "spectra.std.time.utc_minute";
const TIME_UTC_SECOND: &str = "spectra.std.time.utc_second";

// ── std.range ────────────────────────────────────────────────────────────────
const RANGE_CREATE: &str = "spectra.std.range.create";
const RANGE_LEN: &str = "spectra.std.range.len";
const RANGE_AT: &str = "spectra.std.range.at";
const RANGE_EQ: &str = "spectra.std.range.eq";
const RANGE_START: &str = "spectra.std.range.start";
const RANGE_END: &str = "spectra.std.range.end";
const RANGE_IS_INCLUSIVE: &str = "spectra.std.range.is_inclusive";

// ── std.tensor ──────────────────────────────────────────────────────────────
const TENSOR_ZEROS: &str = "spectra.std.tensor.zeros";
const TENSOR_ONES: &str = "spectra.std.tensor.ones";
const TENSOR_FULL: &str = "spectra.std.tensor.full";
const TENSOR_FULL_F: &str = "spectra.std.tensor.full_f";
const TENSOR_LITERAL: &str = "spectra.std.tensor.literal";
const TENSOR_LITERAL_F: &str = "spectra.std.tensor.literal_f";
const TENSOR_LITERAL2: &str = "spectra.std.tensor.literal2";
const TENSOR_LITERAL2_F: &str = "spectra.std.tensor.literal2_f";
const TENSOR_ARANGE: &str = "spectra.std.tensor.arange";
const TENSOR_ZEROS2: &str = "spectra.std.tensor.zeros2";
const TENSOR_ONES2: &str = "spectra.std.tensor.ones2";
const TENSOR_FULL2: &str = "spectra.std.tensor.full2";
const TENSOR_FULL2_F: &str = "spectra.std.tensor.full2_f";
const TENSOR_LEN: &str = "spectra.std.tensor.len";
const TENSOR_RANK: &str = "spectra.std.tensor.rank";
const TENSOR_DIM: &str = "spectra.std.tensor.dim";
const TENSOR_ROWS: &str = "spectra.std.tensor.rows";
const TENSOR_COLS: &str = "spectra.std.tensor.cols";
const TENSOR_IS_VALID: &str = "spectra.std.tensor.is_valid";
const TENSOR_GET: &str = "spectra.std.tensor.get";
const TENSOR_GET_F: &str = "spectra.std.tensor.get_f";
const TENSOR_SET: &str = "spectra.std.tensor.set";
const TENSOR_SET_F: &str = "spectra.std.tensor.set_f";
const TENSOR_GET2: &str = "spectra.std.tensor.get2";
const TENSOR_GET2_F: &str = "spectra.std.tensor.get2_f";
const TENSOR_SET2: &str = "spectra.std.tensor.set2";
const TENSOR_SET2_F: &str = "spectra.std.tensor.set2_f";
const TENSOR_RESHAPE: &str = "spectra.std.tensor.reshape";
const TENSOR_FLATTEN: &str = "spectra.std.tensor.flatten";
const TENSOR_PERMUTE: &str = "spectra.std.tensor.permute";
const TENSOR_SLICE: &str = "spectra.std.tensor.slice";
const TENSOR_CONCAT: &str = "spectra.std.tensor.concat";
const TENSOR_STACK: &str = "spectra.std.tensor.stack";
const TENSOR_ADD: &str = "spectra.std.tensor.add";
const TENSOR_SUB: &str = "spectra.std.tensor.sub";
const TENSOR_MUL: &str = "spectra.std.tensor.mul";
const TENSOR_DIV: &str = "spectra.std.tensor.div";
const TENSOR_SUM: &str = "spectra.std.tensor.sum";
const TENSOR_SUM_F: &str = "spectra.std.tensor.sum_f";
const TENSOR_SUM_T: &str = "spectra.std.tensor.sum_t";
const TENSOR_MEAN_F: &str = "spectra.std.tensor.mean_f";
const TENSOR_MEAN_T: &str = "spectra.std.tensor.mean_t";
const TENSOR_MAX: &str = "spectra.std.tensor.max";
const TENSOR_MIN: &str = "spectra.std.tensor.min";
const TENSOR_ARGMAX: &str = "spectra.std.tensor.argmax";
const TENSOR_MATMUL: &str = "spectra.std.tensor.matmul";
const TENSOR_MATMUL_BATCHED: &str = "spectra.std.tensor.matmul_batched";
const TENSOR_TRANSPOSE: &str = "spectra.std.tensor.transpose";
const TENSOR_DOT: &str = "spectra.std.tensor.dot";
const TENSOR_DOT_T: &str = "spectra.std.tensor.dot_t";
const TENSOR_NEG: &str = "spectra.std.tensor.neg";
const TENSOR_EXP_F: &str = "spectra.std.tensor.exp_f";
const TENSOR_LOG_F: &str = "spectra.std.tensor.log_f";
const TENSOR_SQRT_F: &str = "spectra.std.tensor.sqrt_f";
const TENSOR_RELU: &str = "spectra.std.tensor.relu";
const TENSOR_SIGMOID_F: &str = "spectra.std.tensor.sigmoid_f";
const TENSOR_TANH_F: &str = "spectra.std.tensor.tanh_f";
const TENSOR_SEED: &str = "spectra.std.tensor.seed";
const TENSOR_UNIFORM: &str = "spectra.std.tensor.uniform";
const TENSOR_UNIFORM_F: &str = "spectra.std.tensor.uniform_f";
const TENSOR_NORMAL_F: &str = "spectra.std.tensor.normal_f";
const TENSOR_BERNOULLI: &str = "spectra.std.tensor.bernoulli";
const TENSOR_CATEGORICAL: &str = "spectra.std.tensor.categorical";
const TENSOR_SET_DETERMINISTIC_MODE: &str = "spectra.std.tensor.set_deterministic_mode";
const TENSOR_DETERMINISTIC_MODE: &str = "spectra.std.tensor.deterministic_mode";
const TENSOR_TOLERANCE_ABS: &str = "spectra.std.tensor.tolerance_abs";
const TENSOR_TOLERANCE_REL: &str = "spectra.std.tensor.tolerance_rel";
const TENSOR_DEVICE: &str = "spectra.std.tensor.device";
const TENSOR_DEVICE_AVAILABLE: &str = "spectra.std.tensor.device_available";
const TENSOR_DEVICE_STATUS: &str = "spectra.std.tensor.device_status";
const TENSOR_TO_DEVICE: &str = "spectra.std.tensor.to_device";
const TENSOR_CPU: &str = "spectra.std.tensor.cpu";
const TENSOR_SYNC: &str = "spectra.std.tensor.sync";
const TENSOR_PRECISION: &str = "spectra.std.tensor.precision";
const TENSOR_TO_PRECISION: &str = "spectra.std.tensor.to_precision";
const TENSOR_STATS_ALLOCATIONS: &str = "spectra.std.tensor.stats_allocations";
const TENSOR_STATS_ACTIVE: &str = "spectra.std.tensor.stats_active";
const TENSOR_STATS_PEAK_BYTES: &str = "spectra.std.tensor.stats_peak_bytes";
const TENSOR_STATS_REUSED_BUFFERS: &str = "spectra.std.tensor.stats_reused_buffers";
const TENSOR_STATS_POOL_HITS: &str = "spectra.std.tensor.stats_pool_hits";
const TENSOR_STATS_POOL_MISSES: &str = "spectra.std.tensor.stats_pool_misses";
const TENSOR_STATS_ACTIVE_BYTES: &str = "spectra.std.tensor.stats_active_bytes";
const TENSOR_STATS_SCRATCH_REUSES: &str = "spectra.std.tensor.stats_scratch_reuses";
const TENSOR_KERNEL_STRATEGY: &str = "spectra.std.tensor.kernel_strategy";
const TENSOR_STATS_KERNEL_OPS: &str = "spectra.std.tensor.stats_kernel_ops";
const TENSOR_STATS_KERNEL_ELEMENTS: &str = "spectra.std.tensor.stats_kernel_elements";
const TENSOR_STATS_DEVICE_TRANSFERS: &str = "spectra.std.tensor.stats_device_transfers";
const TENSOR_STATS_GPU_KERNEL_OPS: &str = "spectra.std.tensor.stats_gpu_kernel_ops";
const TENSOR_STATS_CPU_FALLBACKS: &str = "spectra.std.tensor.stats_cpu_fallbacks";
const TENSOR_STATS_GRAPH_NODES: &str = "spectra.std.tensor.stats_graph_nodes";
const TENSOR_STATS_LIFETIME_RECORDS: &str = "spectra.std.tensor.stats_lifetime_records";
const TENSOR_STATS_RELEASED_LIFETIMES: &str = "spectra.std.tensor.stats_released_lifetimes";
const TENSOR_STATS_ALLOCATION_SITES: &str = "spectra.std.tensor.stats_allocation_sites";
const TENSOR_STATS_REUSE_RATE_PER_MILLE: &str = "spectra.std.tensor.stats_reuse_rate_per_mille";
const TENSOR_MEMORY_REPORT: &str = "spectra.std.tensor.memory_report";
const TENSOR_RESET_STATS: &str = "spectra.std.tensor.reset_stats";
const TENSOR_REQUIRES_GRAD: &str = "spectra.std.tensor.requires_grad";
const TENSOR_BACKWARD: &str = "spectra.std.tensor.backward";
const TENSOR_GRAD: &str = "spectra.std.tensor.grad";
const TENSOR_ZERO_GRAD: &str = "spectra.std.tensor.zero_grad";
const TENSOR_SET_GRAD_ENABLED: &str = "spectra.std.tensor.set_grad_enabled";
const TENSOR_GRAD_ENABLED: &str = "spectra.std.tensor.grad_enabled";
const TENSOR_FREE: &str = "spectra.std.tensor.free";
const TENSOR_FREE_ALL: &str = "spectra.std.tensor.free_all";
const TENSOR_REFILL: &str = "spectra.std.tensor.refill";

// ── std.ml ──────────────────────────────────────────────────────────────────
const ML_MODULE_NEW: &str = "spectra.std.ml.module_new";
const ML_MODULE_ADD_PARAMETER: &str = "spectra.std.ml.module_add_parameter";
const ML_MODULE_PARAMETER_COUNT: &str = "spectra.std.ml.module_parameter_count";
const ML_MODULE_PARAMETER: &str = "spectra.std.ml.module_parameter";
const ML_MODULE_SET_TRAINING: &str = "spectra.std.ml.module_set_training";
const ML_MODULE_IS_TRAINING: &str = "spectra.std.ml.module_is_training";
const ML_LINEAR: &str = "spectra.std.ml.linear";
const ML_CONV2D: &str = "spectra.std.ml.conv2d";
const ML_DROPOUT: &str = "spectra.std.ml.dropout";
const ML_MAX_POOL2D: &str = "spectra.std.ml.max_pool2d";
const ML_MSE_LOSS: &str = "spectra.std.ml.mse_loss";
const ML_BCE_LOSS: &str = "spectra.std.ml.bce_loss";
const ML_CROSS_ENTROPY_LOSS: &str = "spectra.std.ml.cross_entropy_loss";
const ML_NLL_LOSS: &str = "spectra.std.ml.nll_loss";
const ML_SGD_STEP: &str = "spectra.std.ml.sgd_step";
const ML_SGD_MOMENTUM_STEP: &str = "spectra.std.ml.sgd_momentum_step";
const ML_ADAM_STEP: &str = "spectra.std.ml.adam_step";
const ML_ADAMW_STEP: &str = "spectra.std.ml.adamw_step";
const ML_EXP_LR: &str = "spectra.std.ml.exp_lr";
const ML_UNSCALE_GRAD: &str = "spectra.std.ml.unscale_grad";
const ML_DATASET_FROM_TENSORS: &str = "spectra.std.ml.dataset_from_tensors";
const ML_DATASET_FROM_CSV: &str = "spectra.std.ml.dataset_from_csv";
const ML_DATASET_FROM_JSONL: &str = "spectra.std.ml.dataset_from_jsonl";
const ML_DATASET_FROM_NPY: &str = "spectra.std.ml.dataset_from_npy";
const ML_DATASET_FROM_DIRECTORY: &str = "spectra.std.ml.dataset_from_directory";
const ML_DATASET_LEN: &str = "spectra.std.ml.dataset_len";
const ML_DATASET_MAP_FEATURES: &str = "spectra.std.ml.dataset_map_features";
const ML_DATASET_FILTER_LABEL_MIN: &str = "spectra.std.ml.dataset_filter_label_min";
const ML_DATASET_TRAIN_SPLIT: &str = "spectra.std.ml.dataset_train_split";
const ML_DATASET_TEST_SPLIT: &str = "spectra.std.ml.dataset_test_split";
const ML_DATALOADER_NEW: &str = "spectra.std.ml.dataloader_new";
const ML_DATALOADER_BATCH_COUNT: &str = "spectra.std.ml.dataloader_batch_count";
const ML_DATALOADER_BATCH_FEATURES: &str = "spectra.std.ml.dataloader_batch_features";
const ML_DATALOADER_BATCH_LABELS: &str = "spectra.std.ml.dataloader_batch_labels";
const ML_DATAFRAME_FROM_CSV: &str = "spectra.std.ml.dataframe_from_csv";
const ML_DATAFRAME_ROWS: &str = "spectra.std.ml.dataframe_rows";
const ML_DATAFRAME_COLS: &str = "spectra.std.ml.dataframe_cols";
const ML_DATAFRAME_COLUMN: &str = "spectra.std.ml.dataframe_column";
const ML_EXPERIMENT_START: &str = "spectra.std.ml.experiment_start";
const ML_EXPERIMENT_SET_CONFIG: &str = "spectra.std.ml.experiment_set_config";
const ML_EXPERIMENT_LOG_METRIC: &str = "spectra.std.ml.experiment_log_metric";
const ML_EXPERIMENT_LOG_ARTIFACT: &str = "spectra.std.ml.experiment_log_artifact";
const ML_EXPERIMENT_SET_LOCKFILE: &str = "spectra.std.ml.experiment_set_lockfile";
const ML_EXPERIMENT_SET_MODEL_OUTPUT: &str = "spectra.std.ml.experiment_set_model_output";
const ML_EXPERIMENT_FINISH: &str = "spectra.std.ml.experiment_finish";
const ML_EXPERIMENT_MANIFEST_PATH: &str = "spectra.std.ml.experiment_manifest_path";
const ML_EXPERIMENT_REPRO_COMMAND: &str = "spectra.std.ml.experiment_repro_command";
const ML_EXPERIMENT_COMPARE_MANIFESTS: &str = "spectra.std.ml.experiment_compare_manifests";
const ML_DISTRIBUTED_SESSION_START: &str = "spectra.std.ml.distributed_session_start";
const ML_DISTRIBUTED_WORKER_STEP: &str = "spectra.std.ml.distributed_worker_step";
const ML_DISTRIBUTED_GLOBAL_STEP: &str = "spectra.std.ml.distributed_global_step";
const ML_DISTRIBUTED_WORKER_STEP_COUNT: &str = "spectra.std.ml.distributed_worker_step_count";
const ML_DISTRIBUTED_CHECKPOINT_SAVE: &str = "spectra.std.ml.distributed_checkpoint_save";
const ML_DISTRIBUTED_RESUME: &str = "spectra.std.ml.distributed_resume";
const ML_DISTRIBUTED_SUMMARY: &str = "spectra.std.ml.distributed_summary";
const ML_ONNX_EXPORT: &str = "spectra.std.ml.onnx_export";
const ML_ONNX_IMPORT_SUMMARY: &str = "spectra.std.ml.onnx_import_summary";
const ML_ONNX_VALIDATE: &str = "spectra.std.ml.onnx_validate";
const ML_ONNX_ROUNDTRIP: &str = "spectra.std.ml.onnx_roundtrip";
const ML_EMBEDDING_LOOKUP: &str = "spectra.std.ml.embedding_lookup";
const ML_POSITIONAL_ENCODING: &str = "spectra.std.ml.positional_encoding";
const ML_LAYER_NORM: &str = "spectra.std.ml.layer_norm";
const ML_GELU: &str = "spectra.std.ml.gelu";
const ML_SWIGLU: &str = "spectra.std.ml.swiglu";
const ML_ATTENTION: &str = "spectra.std.ml.attention";
const ML_KV_CACHE_NEW: &str = "spectra.std.ml.kv_cache_new";
const ML_KV_CACHE_APPEND: &str = "spectra.std.ml.kv_cache_append";
const ML_KV_CACHE_KEYS: &str = "spectra.std.ml.kv_cache_keys";
const ML_KV_CACHE_VALUES: &str = "spectra.std.ml.kv_cache_values";
const ML_KV_CACHE_LEN: &str = "spectra.std.ml.kv_cache_len";
const ML_LOGITS_SAMPLE: &str = "spectra.std.ml.logits_sample";
const ML_TOKENIZER_WORDPIECE: &str = "spectra.std.ml.tokenizer_wordpiece";
const ML_TOKENIZER_ENCODE: &str = "spectra.std.ml.tokenizer_encode";
const ML_TOKENIZER_DECODE: &str = "spectra.std.ml.tokenizer_decode";
const ML_TEXT_EMBED: &str = "spectra.std.ml.text_embed";
const ML_VECTOR_INDEX_NEW: &str = "spectra.std.ml.vector_index_new";
const ML_VECTOR_INDEX_INSERT: &str = "spectra.std.ml.vector_index_insert";
const ML_VECTOR_INDEX_QUERY: &str = "spectra.std.ml.vector_index_query";
const ML_VECTOR_INDEX_PERSIST: &str = "spectra.std.ml.vector_index_persist";
const ML_VECTOR_INDEX_LOAD: &str = "spectra.std.ml.vector_index_load";
const ML_RAG_CHUNK_TEXT: &str = "spectra.std.ml.rag_chunk_text";
const ML_RAG_BUILD_PROMPT: &str = "spectra.std.ml.rag_build_prompt";
const ML_RAG_EVALUATE_ANSWER: &str = "spectra.std.ml.rag_evaluate_answer";
const ML_METRICS_CLASSIFICATION: &str = "spectra.std.ml.metrics_classification";
const ML_METRICS_REGRESSION: &str = "spectra.std.ml.metrics_regression";
const ML_METRICS_RANKING: &str = "spectra.std.ml.metrics_ranking";
const ML_METRICS_GENERATION: &str = "spectra.std.ml.metrics_generation";
const ML_SERVING_METRICS: &str = "spectra.std.ml.serving_metrics";
const ML_EVALUATION_REPORT: &str = "spectra.std.ml.evaluation_report";

const CONCURRENT_TASK_SPAWN: &str = "spectra.std.concurrent.task_spawn";
const CONCURRENT_TASK_JOIN: &str = "spectra.std.concurrent.task_join";
const CONCURRENT_TASK_IS_DONE: &str = "spectra.std.concurrent.task_is_done";
const CONCURRENT_CHANNEL_NEW: &str = "spectra.std.concurrent.channel_new";
const CONCURRENT_CHANNEL_SEND: &str = "spectra.std.concurrent.channel_send";
const CONCURRENT_CHANNEL_RECV: &str = "spectra.std.concurrent.channel_recv";
const CONCURRENT_CHANNEL_LEN: &str = "spectra.std.concurrent.channel_len";
const CONCURRENT_CHANNEL_CLOSE: &str = "spectra.std.concurrent.channel_close";
const CONCURRENT_COUNTER_NEW: &str = "spectra.std.concurrent.counter_new";
const CONCURRENT_COUNTER_ADD: &str = "spectra.std.concurrent.counter_add";
const CONCURRENT_COUNTER_GET: &str = "spectra.std.concurrent.counter_get";
const CONCURRENT_PIPELINE_SUM: &str = "spectra.std.concurrent.pipeline_sum";
const CONCURRENT_STATS_TASKS_SPAWNED: &str = "spectra.std.concurrent.stats_tasks_spawned";
const CONCURRENT_STATS_CHANNELS: &str = "spectra.std.concurrent.stats_channels";
const CONCURRENT_RESET: &str = "spectra.std.concurrent.reset";

const ASYNC_TASK_READY: &str = "spectra.async.task.ready";
const ASYNC_TASK_READY_BATCH: &str = "spectra.async.task.ready_batch";
const ASYNC_TASK_BATCH_CHECKSUM: &str = "spectra.async.task.batch_checksum";
const ASYNC_TASK_POLL: &str = "spectra.async.task.poll";
const ASYNC_TASK_RESULT: &str = "spectra.async.task.result";
const ASYNC_TASK_JOIN: &str = "spectra.async.task.join";
const ASYNC_TASK_JOIN_STATUS: &str = "spectra.async.task.join_status";
const ASYNC_TASK_CANCEL: &str = "spectra.async.task.cancel";
const ASYNC_TASK_IS_CANCELLED: &str = "spectra.async.task.is_cancelled";
const ASYNC_TASK_CANCEL_HANDLE: &str = "spectra.async.task.cancel_handle";
const ASYNC_TASK_WITH_TIMEOUT: &str = "spectra.async.task.with_timeout";
const ASYNC_TASK_FAIL: &str = "spectra.async.task.fail";
const ASYNC_TASK_JOIN_ORDER: &str = "spectra.async.task.join_order";
const ASYNC_TASK_RESET: &str = "spectra.async.task.reset";
const ASYNC_CANCEL_HANDLE_CANCEL: &str = "spectra.async.cancel_handle.cancel";
const ASYNC_SCHEDULER_ADVANCE_TIME: &str = "spectra.async.scheduler.advance_time";
const ASYNC_SCOPE_NEW: &str = "spectra.async.scope.new";
const ASYNC_SCOPE_CHILD: &str = "spectra.async.scope.child";
const ASYNC_SCOPE_ATTACH: &str = "spectra.async.scope.attach";
const ASYNC_SCOPE_SPAWN_READY: &str = "spectra.async.scope.spawn_ready";
const ASYNC_SCOPE_CANCEL: &str = "spectra.async.scope.cancel";
const ASYNC_SCOPE_JOIN: &str = "spectra.async.scope.join";
const ASYNC_SCOPE_JOINED_COUNT: &str = "spectra.async.scope.joined_count";
const ASYNC_SCOPE_FAILURES: &str = "spectra.async.scope.failures";
const ASYNC_STREAM_NEW: &str = "spectra.async.stream.new";
const ASYNC_STREAM_PUSH: &str = "spectra.async.stream.push";
const ASYNC_STREAM_DONE: &str = "spectra.async.stream.done";
const ASYNC_STREAM_NEXT: &str = "spectra.async.stream.next";
const ASYNC_STREAM_NEXT_STATUS: &str = "spectra.async.stream.next_status";
const ASYNC_STREAM_CANCEL: &str = "spectra.async.stream.cancel";
const ASYNC_STREAM_LEN: &str = "spectra.async.stream.len";
const ASYNC_STREAM_CAPACITY: &str = "spectra.async.stream.capacity";
const ASYNC_STREAM_MAP: &str = "spectra.async.stream.map";
const ASYNC_STREAM_FILTER: &str = "spectra.async.stream.filter";
const ASYNC_STREAM_FOLD: &str = "spectra.async.stream.fold";
const ASYNC_STREAM_TAKE: &str = "spectra.async.stream.take";
const ASYNC_STREAM_SKIP: &str = "spectra.async.stream.skip";
const ASYNC_STREAM_CHUNKS: &str = "spectra.async.stream.chunks";
const ASYNC_STREAM_FUSE: &str = "spectra.async.stream.fuse";
const ASYNC_FS_READ: &str = "spectra.async.fs.read_async";
const ASYNC_FS_WRITE: &str = "spectra.async.fs.write_async";
const ASYNC_TCP_LISTEN: &str = "spectra.async.tcp.listen";
const ASYNC_TCP_LISTENER_PORT: &str = "spectra.async.tcp.listener_port";
const ASYNC_TCP_CONNECT: &str = "spectra.async.tcp.connect_async";
const ASYNC_TCP_ACCEPT: &str = "spectra.async.tcp.accept_async";
const ASYNC_TCP_READ: &str = "spectra.async.tcp.read_async";
const ASYNC_TCP_WRITE: &str = "spectra.async.tcp.write_async";
const ASYNC_TCP_CLOSE: &str = "spectra.async.tcp.close";
const ASYNC_UDP_BIND: &str = "spectra.async.udp.bind";
const ASYNC_UDP_PORT: &str = "spectra.async.udp.port";
const ASYNC_UDP_SEND_TO: &str = "spectra.async.udp.send_to_async";
const ASYNC_UDP_RECV: &str = "spectra.async.udp.recv_async";
const ASYNC_UDP_CLOSE: &str = "spectra.async.udp.close";
const ASYNC_CHANNEL_NEW: &str = "spectra.async.channel.new";
const ASYNC_CHANNEL_SEND: &str = "spectra.async.channel.send";
const ASYNC_CHANNEL_RECV: &str = "spectra.async.channel.recv";
const ASYNC_CHANNEL_CLOSE: &str = "spectra.async.channel.close";
const ASYNC_CHANNEL_LEN: &str = "spectra.async.channel.len";

const ASYNC_REACTOR_BACKEND: &str = "spectra.async.reactor.backend";
const ASYNC_REACTOR_WAKE: &str = "spectra.async.reactor.wake";
const ASYNC_REACTOR_TIMER: &str = "spectra.async.reactor.timer";
const ASYNC_REACTOR_IO_REGISTER: &str = "spectra.async.reactor.io_register";
const ASYNC_REACTOR_IO_NOTIFY: &str = "spectra.async.reactor.io_notify";
const ASYNC_REACTOR_POLL: &str = "spectra.async.reactor.poll";
const ASYNC_REACTOR_LAST_KIND: &str = "spectra.async.reactor.last_kind";
const ASYNC_REACTOR_LAST_READINESS: &str = "spectra.async.reactor.last_readiness";
const ASYNC_REACTOR_STATS_QUEUED: &str = "spectra.async.reactor.stats_queued";
const ASYNC_REACTOR_STATS_TASK_WAKEUPS: &str = "spectra.async.reactor.stats_task_wakeups";
const ASYNC_REACTOR_STATS_TIMER_EVENTS: &str = "spectra.async.reactor.stats_timer_events";
const ASYNC_REACTOR_STATS_IO_EVENTS: &str = "spectra.async.reactor.stats_io_events";
const ASYNC_REACTOR_STATS_IO_REGISTRATIONS: &str = "spectra.async.reactor.stats_io_registrations";
const ASYNC_REACTOR_RESET: &str = "spectra.async.reactor.reset";

const SERVE_SERVER_NEW: &str = "spectra.std.serve.server_new";
const SERVE_SERVER_WARMUP: &str = "spectra.std.serve.server_warmup";
const SERVE_SERVER_IS_WARM: &str = "spectra.std.serve.server_is_warm";
const SERVE_SERVER_ENQUEUE: &str = "spectra.std.serve.server_enqueue";
const SERVE_SERVER_CANCEL: &str = "spectra.std.serve.server_cancel";
const SERVE_SERVER_PROCESS_BATCH: &str = "spectra.std.serve.server_process_batch";
const SERVE_SERVER_RESULT: &str = "spectra.std.serve.server_result";
const SERVE_SERVER_PENDING: &str = "spectra.std.serve.server_pending";
const SERVE_SERVER_SET_TIMEOUT: &str = "spectra.std.serve.server_set_timeout";
const SERVE_SERVER_RESIDENT_MODEL: &str = "spectra.std.serve.server_resident_model";
const SERVE_SERVER_BENCHMARK: &str = "spectra.std.serve.server_benchmark";
const SERVE_SERVER_SET_INPUT_POLICY: &str = "spectra.std.serve.server_set_input_policy";
const SERVE_SERVER_SET_OUTPUT_POLICY: &str = "spectra.std.serve.server_set_output_policy";
const SERVE_SERVER_SET_RATE_LIMIT: &str = "spectra.std.serve.server_set_rate_limit";
const SERVE_SERVER_SET_FALLBACK: &str = "spectra.std.serve.server_set_fallback";
const SERVE_SERVER_LAST_DIAGNOSTIC: &str = "spectra.std.serve.server_last_diagnostic";
const SERVE_SERVER_AUDIT_LOG: &str = "spectra.std.serve.server_audit_log";
const SERVE_SERVER_SET_MODEL_VERSION: &str = "spectra.std.serve.server_set_model_version";
const SERVE_SERVER_MONITORING_SNAPSHOT: &str = "spectra.std.serve.server_monitoring_snapshot";
const SERVE_SERVER_DISTRIBUTION_SUMMARY: &str = "spectra.std.serve.server_distribution_summary";
const SERVE_DRIFT_CHECK: &str = "spectra.std.serve.drift_check";
const SERVE_EXPORT_MONITORING: &str = "spectra.std.serve.export_monitoring";
const SERVE_RESET: &str = "spectra.std.serve.reset";

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
    register_range();
    register_tensor();
    register_ml();
    register_concurrent();
    register_async();
    register_serve();
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

fn register_tensor() {
    register_host_function(TENSOR_ZEROS, std_tensor_zeros);
    register_host_function(TENSOR_ONES, std_tensor_ones);
    register_host_function(TENSOR_FULL, std_tensor_full);
    register_host_function(TENSOR_FULL_F, std_tensor_full_f);
    register_host_function(TENSOR_LITERAL, std_tensor_literal);
    register_host_function(TENSOR_LITERAL_F, std_tensor_literal_f);
    register_host_function(TENSOR_LITERAL2, std_tensor_literal2);
    register_host_function(TENSOR_LITERAL2_F, std_tensor_literal2_f);
    register_host_function(TENSOR_ARANGE, std_tensor_arange);
    register_host_function(TENSOR_ZEROS2, std_tensor_zeros2);
    register_host_function(TENSOR_ONES2, std_tensor_ones2);
    register_host_function(TENSOR_FULL2, std_tensor_full2);
    register_host_function(TENSOR_FULL2_F, std_tensor_full2_f);
    register_host_function(TENSOR_LEN, std_tensor_len);
    register_host_function(TENSOR_RANK, std_tensor_rank);
    register_host_function(TENSOR_DIM, std_tensor_dim);
    register_host_function(TENSOR_ROWS, std_tensor_rows);
    register_host_function(TENSOR_COLS, std_tensor_cols);
    register_host_function(TENSOR_IS_VALID, std_tensor_is_valid);
    register_host_function(TENSOR_GET, std_tensor_get);
    register_host_function(TENSOR_GET_F, std_tensor_get_f);
    register_host_function(TENSOR_SET, std_tensor_set);
    register_host_function(TENSOR_SET_F, std_tensor_set_f);
    register_host_function(TENSOR_GET2, std_tensor_get2);
    register_host_function(TENSOR_GET2_F, std_tensor_get2_f);
    register_host_function(TENSOR_SET2, std_tensor_set2);
    register_host_function(TENSOR_SET2_F, std_tensor_set2_f);
    register_host_function(TENSOR_RESHAPE, std_tensor_reshape);
    register_host_function(TENSOR_FLATTEN, std_tensor_flatten);
    register_host_function(TENSOR_PERMUTE, std_tensor_permute);
    register_host_function(TENSOR_SLICE, std_tensor_slice);
    register_host_function(TENSOR_CONCAT, std_tensor_concat);
    register_host_function(TENSOR_STACK, std_tensor_stack);
    register_host_function(TENSOR_ADD, std_tensor_add);
    register_host_function(TENSOR_SUB, std_tensor_sub);
    register_host_function(TENSOR_MUL, std_tensor_mul);
    register_host_function(TENSOR_DIV, std_tensor_div);
    register_host_function(TENSOR_SUM, std_tensor_sum);
    register_host_function(TENSOR_SUM_F, std_tensor_sum_f);
    register_host_function(TENSOR_SUM_T, std_tensor_sum_t);
    register_host_function(TENSOR_MEAN_F, std_tensor_mean_f);
    register_host_function(TENSOR_MEAN_T, std_tensor_mean_t);
    register_host_function(TENSOR_MAX, std_tensor_max);
    register_host_function(TENSOR_MIN, std_tensor_min);
    register_host_function(TENSOR_ARGMAX, std_tensor_argmax);
    register_host_function(TENSOR_MATMUL, std_tensor_matmul);
    register_host_function(TENSOR_MATMUL_BATCHED, std_tensor_matmul_batched);
    register_host_function(TENSOR_TRANSPOSE, std_tensor_transpose);
    register_host_function(TENSOR_DOT, std_tensor_dot);
    register_host_function(TENSOR_DOT_T, std_tensor_dot_t);
    register_host_function(TENSOR_NEG, std_tensor_neg);
    register_host_function(TENSOR_EXP_F, std_tensor_exp_f);
    register_host_function(TENSOR_LOG_F, std_tensor_log_f);
    register_host_function(TENSOR_SQRT_F, std_tensor_sqrt_f);
    register_host_function(TENSOR_RELU, std_tensor_relu);
    register_host_function(TENSOR_SIGMOID_F, std_tensor_sigmoid_f);
    register_host_function(TENSOR_TANH_F, std_tensor_tanh_f);
    register_host_function(TENSOR_SEED, std_tensor_seed);
    register_host_function(TENSOR_UNIFORM, std_tensor_uniform);
    register_host_function(TENSOR_UNIFORM_F, std_tensor_uniform_f);
    register_host_function(TENSOR_NORMAL_F, std_tensor_normal_f);
    register_host_function(TENSOR_BERNOULLI, std_tensor_bernoulli);
    register_host_function(TENSOR_CATEGORICAL, std_tensor_categorical);
    register_host_function(
        TENSOR_SET_DETERMINISTIC_MODE,
        std_tensor_set_deterministic_mode,
    );
    register_host_function(TENSOR_DETERMINISTIC_MODE, std_tensor_deterministic_mode);
    register_host_function(TENSOR_TOLERANCE_ABS, std_tensor_tolerance_abs);
    register_host_function(TENSOR_TOLERANCE_REL, std_tensor_tolerance_rel);
    register_host_function(TENSOR_DEVICE, std_tensor_device);
    register_host_function(TENSOR_DEVICE_AVAILABLE, std_tensor_device_available);
    register_host_function(TENSOR_DEVICE_STATUS, std_tensor_device_status);
    register_host_function(TENSOR_TO_DEVICE, std_tensor_to_device);
    register_host_function(TENSOR_CPU, std_tensor_cpu);
    register_host_function(TENSOR_SYNC, std_tensor_sync);
    register_host_function(TENSOR_PRECISION, std_tensor_precision);
    register_host_function(TENSOR_TO_PRECISION, std_tensor_to_precision);
    register_host_function(TENSOR_STATS_ALLOCATIONS, std_tensor_stats_allocations);
    register_host_function(TENSOR_STATS_ACTIVE, std_tensor_stats_active);
    register_host_function(TENSOR_STATS_PEAK_BYTES, std_tensor_stats_peak_bytes);
    register_host_function(TENSOR_STATS_REUSED_BUFFERS, std_tensor_stats_reused_buffers);
    register_host_function(TENSOR_STATS_POOL_HITS, std_tensor_stats_pool_hits);
    register_host_function(TENSOR_STATS_POOL_MISSES, std_tensor_stats_pool_misses);
    register_host_function(TENSOR_STATS_ACTIVE_BYTES, std_tensor_stats_active_bytes);
    register_host_function(TENSOR_STATS_SCRATCH_REUSES, std_tensor_stats_scratch_reuses);
    register_host_function(TENSOR_KERNEL_STRATEGY, std_tensor_kernel_strategy);
    register_host_function(TENSOR_STATS_KERNEL_OPS, std_tensor_stats_kernel_ops);
    register_host_function(
        TENSOR_STATS_KERNEL_ELEMENTS,
        std_tensor_stats_kernel_elements,
    );
    register_host_function(
        TENSOR_STATS_DEVICE_TRANSFERS,
        std_tensor_stats_device_transfers,
    );
    register_host_function(TENSOR_STATS_GPU_KERNEL_OPS, std_tensor_stats_gpu_kernel_ops);
    register_host_function(TENSOR_STATS_CPU_FALLBACKS, std_tensor_stats_cpu_fallbacks);
    register_host_function(TENSOR_STATS_GRAPH_NODES, std_tensor_stats_graph_nodes);
    register_host_function(
        TENSOR_STATS_LIFETIME_RECORDS,
        std_tensor_stats_lifetime_records,
    );
    register_host_function(
        TENSOR_STATS_RELEASED_LIFETIMES,
        std_tensor_stats_released_lifetimes,
    );
    register_host_function(
        TENSOR_STATS_ALLOCATION_SITES,
        std_tensor_stats_allocation_sites,
    );
    register_host_function(
        TENSOR_STATS_REUSE_RATE_PER_MILLE,
        std_tensor_stats_reuse_rate_per_mille,
    );
    register_host_function(TENSOR_MEMORY_REPORT, std_tensor_memory_report);
    register_host_function(TENSOR_RESET_STATS, std_tensor_reset_stats);
    register_host_function(TENSOR_REQUIRES_GRAD, std_tensor_requires_grad);
    register_host_function(TENSOR_BACKWARD, std_tensor_backward);
    register_host_function(TENSOR_GRAD, std_tensor_grad);
    register_host_function(TENSOR_ZERO_GRAD, std_tensor_zero_grad);
    register_host_function(TENSOR_SET_GRAD_ENABLED, std_tensor_set_grad_enabled);
    register_host_function(TENSOR_GRAD_ENABLED, std_tensor_grad_enabled);
    register_host_function(TENSOR_FREE, std_tensor_free);
    register_host_function(TENSOR_FREE_ALL, std_tensor_free_all);
    register_host_function(TENSOR_REFILL, std_tensor_refill);
}

fn register_ml() {
    register_host_function(ML_MODULE_NEW, std_ml_module_new);
    register_host_function(ML_MODULE_ADD_PARAMETER, std_ml_module_add_parameter);
    register_host_function(ML_MODULE_PARAMETER_COUNT, std_ml_module_parameter_count);
    register_host_function(ML_MODULE_PARAMETER, std_ml_module_parameter);
    register_host_function(ML_MODULE_SET_TRAINING, std_ml_module_set_training);
    register_host_function(ML_MODULE_IS_TRAINING, std_ml_module_is_training);
    register_host_function(ML_LINEAR, std_ml_linear);
    register_host_function(ML_CONV2D, std_ml_conv2d);
    register_host_function(ML_DROPOUT, std_ml_dropout);
    register_host_function(ML_MAX_POOL2D, std_ml_max_pool2d);
    register_host_function(ML_MSE_LOSS, std_ml_mse_loss);
    register_host_function(ML_BCE_LOSS, std_ml_bce_loss);
    register_host_function(ML_CROSS_ENTROPY_LOSS, std_ml_cross_entropy_loss);
    register_host_function(ML_NLL_LOSS, std_ml_nll_loss);
    register_host_function(ML_SGD_STEP, std_ml_sgd_step);
    register_host_function(ML_SGD_MOMENTUM_STEP, std_ml_sgd_momentum_step);
    register_host_function(ML_ADAM_STEP, std_ml_adam_step);
    register_host_function(ML_ADAMW_STEP, std_ml_adamw_step);
    register_host_function(ML_EXP_LR, std_ml_exp_lr);
    register_host_function(ML_UNSCALE_GRAD, std_ml_unscale_grad);
    register_host_function(ML_DATASET_FROM_TENSORS, std_ml_dataset_from_tensors);
    register_host_function(ML_DATASET_FROM_CSV, std_ml_dataset_from_csv);
    register_host_function(ML_DATASET_FROM_JSONL, std_ml_dataset_from_jsonl);
    register_host_function(ML_DATASET_FROM_NPY, std_ml_dataset_from_npy);
    register_host_function(ML_DATASET_FROM_DIRECTORY, std_ml_dataset_from_directory);
    register_host_function(ML_DATASET_LEN, std_ml_dataset_len);
    register_host_function(ML_DATASET_MAP_FEATURES, std_ml_dataset_map_features);
    register_host_function(ML_DATASET_FILTER_LABEL_MIN, std_ml_dataset_filter_label_min);
    register_host_function(ML_DATASET_TRAIN_SPLIT, std_ml_dataset_train_split);
    register_host_function(ML_DATASET_TEST_SPLIT, std_ml_dataset_test_split);
    register_host_function(ML_DATALOADER_NEW, std_ml_dataloader_new);
    register_host_function(ML_DATALOADER_BATCH_COUNT, std_ml_dataloader_batch_count);
    register_host_function(
        ML_DATALOADER_BATCH_FEATURES,
        std_ml_dataloader_batch_features,
    );
    register_host_function(ML_DATALOADER_BATCH_LABELS, std_ml_dataloader_batch_labels);
    register_host_function(ML_DATAFRAME_FROM_CSV, std_ml_dataframe_from_csv);
    register_host_function(ML_DATAFRAME_ROWS, std_ml_dataframe_rows);
    register_host_function(ML_DATAFRAME_COLS, std_ml_dataframe_cols);
    register_host_function(ML_DATAFRAME_COLUMN, std_ml_dataframe_column);
    register_host_function(ML_EXPERIMENT_START, std_ml_experiment_start);
    register_host_function(ML_EXPERIMENT_SET_CONFIG, std_ml_experiment_set_config);
    register_host_function(ML_EXPERIMENT_LOG_METRIC, std_ml_experiment_log_metric);
    register_host_function(ML_EXPERIMENT_LOG_ARTIFACT, std_ml_experiment_log_artifact);
    register_host_function(ML_EXPERIMENT_SET_LOCKFILE, std_ml_experiment_set_lockfile);
    register_host_function(
        ML_EXPERIMENT_SET_MODEL_OUTPUT,
        std_ml_experiment_set_model_output,
    );
    register_host_function(ML_EXPERIMENT_FINISH, std_ml_experiment_finish);
    register_host_function(ML_EXPERIMENT_MANIFEST_PATH, std_ml_experiment_manifest_path);
    register_host_function(ML_EXPERIMENT_REPRO_COMMAND, std_ml_experiment_repro_command);
    register_host_function(
        ML_EXPERIMENT_COMPARE_MANIFESTS,
        std_ml_experiment_compare_manifests,
    );
    register_host_function(
        ML_DISTRIBUTED_SESSION_START,
        std_ml_distributed_session_start,
    );
    register_host_function(ML_DISTRIBUTED_WORKER_STEP, std_ml_distributed_worker_step);
    register_host_function(ML_DISTRIBUTED_GLOBAL_STEP, std_ml_distributed_global_step);
    register_host_function(
        ML_DISTRIBUTED_WORKER_STEP_COUNT,
        std_ml_distributed_worker_step_count,
    );
    register_host_function(
        ML_DISTRIBUTED_CHECKPOINT_SAVE,
        std_ml_distributed_checkpoint_save,
    );
    register_host_function(ML_DISTRIBUTED_RESUME, std_ml_distributed_resume);
    register_host_function(ML_DISTRIBUTED_SUMMARY, std_ml_distributed_summary);
    register_host_function(ML_ONNX_EXPORT, std_ml_onnx_export);
    register_host_function(ML_ONNX_IMPORT_SUMMARY, std_ml_onnx_import_summary);
    register_host_function(ML_ONNX_VALIDATE, std_ml_onnx_validate);
    register_host_function(ML_ONNX_ROUNDTRIP, std_ml_onnx_roundtrip);
    register_host_function(ML_EMBEDDING_LOOKUP, std_ml_embedding_lookup);
    register_host_function(ML_POSITIONAL_ENCODING, std_ml_positional_encoding);
    register_host_function(ML_LAYER_NORM, std_ml_layer_norm);
    register_host_function(ML_GELU, std_ml_gelu);
    register_host_function(ML_SWIGLU, std_ml_swiglu);
    register_host_function(ML_ATTENTION, std_ml_attention);
    register_host_function(ML_KV_CACHE_NEW, std_ml_kv_cache_new);
    register_host_function(ML_KV_CACHE_APPEND, std_ml_kv_cache_append);
    register_host_function(ML_KV_CACHE_KEYS, std_ml_kv_cache_keys);
    register_host_function(ML_KV_CACHE_VALUES, std_ml_kv_cache_values);
    register_host_function(ML_KV_CACHE_LEN, std_ml_kv_cache_len);
    register_host_function(ML_LOGITS_SAMPLE, std_ml_logits_sample);
    register_host_function(ML_TOKENIZER_WORDPIECE, std_ml_tokenizer_wordpiece);
    register_host_function(ML_TOKENIZER_ENCODE, std_ml_tokenizer_encode);
    register_host_function(ML_TOKENIZER_DECODE, std_ml_tokenizer_decode);
    register_host_function(ML_TEXT_EMBED, std_ml_text_embed);
    register_host_function(ML_VECTOR_INDEX_NEW, std_ml_vector_index_new);
    register_host_function(ML_VECTOR_INDEX_INSERT, std_ml_vector_index_insert);
    register_host_function(ML_VECTOR_INDEX_QUERY, std_ml_vector_index_query);
    register_host_function(ML_VECTOR_INDEX_PERSIST, std_ml_vector_index_persist);
    register_host_function(ML_VECTOR_INDEX_LOAD, std_ml_vector_index_load);
    register_host_function(ML_RAG_CHUNK_TEXT, std_ml_rag_chunk_text);
    register_host_function(ML_RAG_BUILD_PROMPT, std_ml_rag_build_prompt);
    register_host_function(ML_RAG_EVALUATE_ANSWER, std_ml_rag_evaluate_answer);
    register_host_function(ML_METRICS_CLASSIFICATION, std_ml_metrics_classification);
    register_host_function(ML_METRICS_REGRESSION, std_ml_metrics_regression);
    register_host_function(ML_METRICS_RANKING, std_ml_metrics_ranking);
    register_host_function(ML_METRICS_GENERATION, std_ml_metrics_generation);
    register_host_function(ML_SERVING_METRICS, std_ml_serving_metrics);
    register_host_function(ML_EVALUATION_REPORT, std_ml_evaluation_report);
}

fn register_concurrent() {
    register_host_function(CONCURRENT_TASK_SPAWN, std_concurrent_task_spawn);
    register_host_function(CONCURRENT_TASK_JOIN, std_concurrent_task_join);
    register_host_function(CONCURRENT_TASK_IS_DONE, std_concurrent_task_is_done);
    register_host_function(CONCURRENT_CHANNEL_NEW, std_concurrent_channel_new);
    register_host_function(CONCURRENT_CHANNEL_SEND, std_concurrent_channel_send);
    register_host_function(CONCURRENT_CHANNEL_RECV, std_concurrent_channel_recv);
    register_host_function(CONCURRENT_CHANNEL_LEN, std_concurrent_channel_len);
    register_host_function(CONCURRENT_CHANNEL_CLOSE, std_concurrent_channel_close);
    register_host_function(CONCURRENT_COUNTER_NEW, std_concurrent_counter_new);
    register_host_function(CONCURRENT_COUNTER_ADD, std_concurrent_counter_add);
    register_host_function(CONCURRENT_COUNTER_GET, std_concurrent_counter_get);
    register_host_function(CONCURRENT_PIPELINE_SUM, std_concurrent_pipeline_sum);
    register_host_function(
        CONCURRENT_STATS_TASKS_SPAWNED,
        std_concurrent_stats_tasks_spawned,
    );
    register_host_function(CONCURRENT_STATS_CHANNELS, std_concurrent_stats_channels);
    register_host_function(CONCURRENT_RESET, std_concurrent_reset);
}

fn register_async() {
    register_host_function(ASYNC_TASK_READY, std_async_task_ready);
    register_host_function(ASYNC_TASK_READY_BATCH, std_async_task_ready_batch);
    register_host_function(ASYNC_TASK_BATCH_CHECKSUM, std_async_task_batch_checksum);
    register_host_function(ASYNC_TASK_POLL, std_async_task_poll);
    register_host_function(ASYNC_TASK_RESULT, std_async_task_result);
    register_host_function(ASYNC_TASK_JOIN, std_async_task_join);
    register_host_function(ASYNC_TASK_JOIN_STATUS, std_async_task_join_status);
    register_host_function(ASYNC_TASK_CANCEL, std_async_task_cancel);
    register_host_function(ASYNC_TASK_IS_CANCELLED, std_async_task_is_cancelled);
    register_host_function(ASYNC_TASK_CANCEL_HANDLE, std_async_task_cancel_handle);
    register_host_function(ASYNC_TASK_WITH_TIMEOUT, std_async_task_with_timeout);
    register_host_function(ASYNC_TASK_FAIL, std_async_task_fail);
    register_host_function(ASYNC_TASK_JOIN_ORDER, std_async_task_join_order);
    register_host_function(ASYNC_TASK_RESET, std_async_task_reset);
    register_host_function(ASYNC_CANCEL_HANDLE_CANCEL, std_async_cancel_handle_cancel);
    register_host_function(
        ASYNC_SCHEDULER_ADVANCE_TIME,
        std_async_scheduler_advance_time,
    );
    register_host_function(ASYNC_SCOPE_NEW, std_async_scope_new);
    register_host_function(ASYNC_SCOPE_CHILD, std_async_scope_child);
    register_host_function(ASYNC_SCOPE_ATTACH, std_async_scope_attach);
    register_host_function(ASYNC_SCOPE_SPAWN_READY, std_async_scope_spawn_ready);
    register_host_function(ASYNC_SCOPE_CANCEL, std_async_scope_cancel);
    register_host_function(ASYNC_SCOPE_JOIN, std_async_scope_join);
    register_host_function(ASYNC_SCOPE_JOINED_COUNT, std_async_scope_joined_count);
    register_host_function(ASYNC_SCOPE_FAILURES, std_async_scope_failures);
    register_host_function(ASYNC_STREAM_NEW, std_async_stream_new);
    register_host_function(ASYNC_STREAM_PUSH, std_async_stream_push);
    register_host_function(ASYNC_STREAM_DONE, std_async_stream_done);
    register_host_function(ASYNC_STREAM_NEXT, std_async_stream_next);
    register_host_function(ASYNC_STREAM_NEXT_STATUS, std_async_stream_next_status);
    register_host_function(ASYNC_STREAM_CANCEL, std_async_stream_cancel);
    register_host_function(ASYNC_STREAM_LEN, std_async_stream_len);
    register_host_function(ASYNC_STREAM_CAPACITY, std_async_stream_capacity);
    register_host_function(ASYNC_STREAM_MAP, std_async_stream_map);
    register_host_function(ASYNC_STREAM_FILTER, std_async_stream_filter);
    register_host_function(ASYNC_STREAM_FOLD, std_async_stream_fold);
    register_host_function(ASYNC_STREAM_TAKE, std_async_stream_take);
    register_host_function(ASYNC_STREAM_SKIP, std_async_stream_skip);
    register_host_function(ASYNC_STREAM_CHUNKS, std_async_stream_chunks);
    register_host_function(ASYNC_STREAM_FUSE, std_async_stream_fuse);
    register_host_function(ASYNC_FS_READ, std_async_fs_read);
    register_host_function(ASYNC_FS_WRITE, std_async_fs_write);
    register_host_function(ASYNC_TCP_LISTEN, std_async_tcp_listen);
    register_host_function(ASYNC_TCP_LISTENER_PORT, std_async_tcp_listener_port);
    register_host_function(ASYNC_TCP_CONNECT, std_async_tcp_connect);
    register_host_function(ASYNC_TCP_ACCEPT, std_async_tcp_accept);
    register_host_function(ASYNC_TCP_READ, std_async_tcp_read);
    register_host_function(ASYNC_TCP_WRITE, std_async_tcp_write);
    register_host_function(ASYNC_TCP_CLOSE, std_async_tcp_close);
    register_host_function(ASYNC_UDP_BIND, std_async_udp_bind);
    register_host_function(ASYNC_UDP_PORT, std_async_udp_port);
    register_host_function(ASYNC_UDP_SEND_TO, std_async_udp_send_to);
    register_host_function(ASYNC_UDP_RECV, std_async_udp_recv);
    register_host_function(ASYNC_UDP_CLOSE, std_async_udp_close);
    register_host_function(ASYNC_CHANNEL_NEW, std_async_channel_new);
    register_host_function(ASYNC_CHANNEL_SEND, std_async_channel_send);
    register_host_function(ASYNC_CHANNEL_RECV, std_async_channel_recv);
    register_host_function(ASYNC_CHANNEL_CLOSE, std_async_channel_close);
    register_host_function(ASYNC_CHANNEL_LEN, std_async_channel_len);
    register_host_function(ASYNC_REACTOR_BACKEND, std_async_reactor_backend);
    register_host_function(ASYNC_REACTOR_WAKE, std_async_reactor_wake);
    register_host_function(ASYNC_REACTOR_TIMER, std_async_reactor_timer);
    register_host_function(ASYNC_REACTOR_IO_REGISTER, std_async_reactor_io_register);
    register_host_function(ASYNC_REACTOR_IO_NOTIFY, std_async_reactor_io_notify);
    register_host_function(ASYNC_REACTOR_POLL, std_async_reactor_poll);
    register_host_function(ASYNC_REACTOR_LAST_KIND, std_async_reactor_last_kind);
    register_host_function(
        ASYNC_REACTOR_LAST_READINESS,
        std_async_reactor_last_readiness,
    );
    register_host_function(ASYNC_REACTOR_STATS_QUEUED, std_async_reactor_stats_queued);
    register_host_function(
        ASYNC_REACTOR_STATS_TASK_WAKEUPS,
        std_async_reactor_stats_task_wakeups,
    );
    register_host_function(
        ASYNC_REACTOR_STATS_TIMER_EVENTS,
        std_async_reactor_stats_timer_events,
    );
    register_host_function(
        ASYNC_REACTOR_STATS_IO_EVENTS,
        std_async_reactor_stats_io_events,
    );
    register_host_function(
        ASYNC_REACTOR_STATS_IO_REGISTRATIONS,
        std_async_reactor_stats_io_registrations,
    );
    register_host_function(ASYNC_REACTOR_RESET, std_async_reactor_reset);
}

fn register_serve() {
    register_host_function(SERVE_SERVER_NEW, std_serve_server_new);
    register_host_function(SERVE_SERVER_WARMUP, std_serve_server_warmup);
    register_host_function(SERVE_SERVER_IS_WARM, std_serve_server_is_warm);
    register_host_function(SERVE_SERVER_ENQUEUE, std_serve_server_enqueue);
    register_host_function(SERVE_SERVER_CANCEL, std_serve_server_cancel);
    register_host_function(SERVE_SERVER_PROCESS_BATCH, std_serve_server_process_batch);
    register_host_function(SERVE_SERVER_RESULT, std_serve_server_result);
    register_host_function(SERVE_SERVER_PENDING, std_serve_server_pending);
    register_host_function(SERVE_SERVER_SET_TIMEOUT, std_serve_server_set_timeout);
    register_host_function(SERVE_SERVER_RESIDENT_MODEL, std_serve_server_resident_model);
    register_host_function(SERVE_SERVER_BENCHMARK, std_serve_server_benchmark);
    register_host_function(
        SERVE_SERVER_SET_INPUT_POLICY,
        std_serve_server_set_input_policy,
    );
    register_host_function(
        SERVE_SERVER_SET_OUTPUT_POLICY,
        std_serve_server_set_output_policy,
    );
    register_host_function(SERVE_SERVER_SET_RATE_LIMIT, std_serve_server_set_rate_limit);
    register_host_function(SERVE_SERVER_SET_FALLBACK, std_serve_server_set_fallback);
    register_host_function(
        SERVE_SERVER_LAST_DIAGNOSTIC,
        std_serve_server_last_diagnostic,
    );
    register_host_function(SERVE_SERVER_AUDIT_LOG, std_serve_server_audit_log);
    register_host_function(
        SERVE_SERVER_SET_MODEL_VERSION,
        std_serve_server_set_model_version,
    );
    register_host_function(
        SERVE_SERVER_MONITORING_SNAPSHOT,
        std_serve_server_monitoring_snapshot,
    );
    register_host_function(
        SERVE_SERVER_DISTRIBUTION_SUMMARY,
        std_serve_server_distribution_summary,
    );
    register_host_function(SERVE_DRIFT_CHECK, std_serve_drift_check);
    register_host_function(SERVE_EXPORT_MONITORING, std_serve_export_monitoring);
    register_host_function(SERVE_RESET, std_serve_reset);
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
                            if b == 0 {
                                break;
                            }
                            bytes.push(b);
                            offset += 1;
                        }
                        match String::from_utf8(bytes) {
                            Ok(s) => write!(stdout, "{}", s).is_ok(),
                            Err(_) => write!(stdout, "(invalid utf8)").is_ok(),
                        }
                    }
                }
                PRINT_TAG_BOOL => {
                    write!(stdout, "{}", if value != 0 { "true" } else { "false" }).is_ok()
                }
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
                            if b == 0 {
                                break;
                            }
                            bytes.push(b);
                            offset += 1;
                        }
                        match String::from_utf8(bytes) {
                            Ok(s) => write!(stderr, "{}", s).is_ok(),
                            Err(_) => write!(stderr, "(invalid utf8)").is_ok(),
                        }
                    }
                }
                PRINT_TAG_BOOL => {
                    write!(stderr, "{}", if value != 0 { "true" } else { "false" }).is_ok()
                }
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
    let mut guard = lock_unpoisoned(registry);
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

// ── std.string string builder (R-3108) ──────────────────────────────────────

struct StringBuilder {
    buf: Vec<u8>,
    len: usize,
}

impl StringBuilder {
    fn new(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity.max(256)),
            len: 0,
        }
    }

    #[allow(dead_code)]
    fn push_bytes(&mut self, bytes: &[u8]) {
        let needed = self.len + bytes.len();
        if needed > self.buf.len() {
            self.buf.resize(needed, 0);
        }
        self.buf[self.len..needed].copy_from_slice(bytes);
        self.len = needed;
    }

    fn push_spectra_string(&mut self, str_ptr: i64) {
        let raw = str_ptr as *const i64;
        if raw.is_null() {
            return;
        }
        let mut offset = 0;
        loop {
            let slot = unsafe { *raw.add(offset) };
            // Spectra string: one byte per i64 slot, byte is in the lowest byte.
            let byte = (slot & 0xFF) as u8;
            if byte == 0 {
                break;
            }
            self.buf.push(byte);
            self.len += 1;
            offset += 1;
        }
    }

    fn current_len(&self) -> usize {
        self.len
    }

    fn finish(&mut self) -> String {
        let s = String::from_utf8(self.buf[..self.len].to_vec()).unwrap_or_default();
        self.len = 0;
        s
    }
}

struct StringBuilderRegistry {
    builders: Vec<Option<ManualBox<StringBuilder>>>,
    free: Vec<usize>,
    next_fresh: usize,
}

impl StringBuilderRegistry {
    fn new() -> Self {
        Self {
            builders: Vec::new(),
            free: Vec::new(),
            next_fresh: 0,
        }
    }

    fn insert(&mut self, builder: ManualBox<StringBuilder>) -> usize {
        let handle = if let Some(idx) = self.free.pop() {
            self.builders[idx] = Some(builder);
            idx
        } else {
            let idx = self.next_fresh;
            self.next_fresh += 1;
            self.builders.push(Some(builder));
            idx
        };
        // Handle 0 is reserved as invalid (matches concurrent task convention).
        handle + 1
    }

    fn push_spectra_string(&mut self, handle: usize, str_ptr: i64) -> Result<(), i32> {
        let idx = handle.checked_sub(1).ok_or(HOST_STATUS_NOT_FOUND)?;
        match self.builders.get_mut(idx) {
            Some(Some(b)) => {
                b.push_spectra_string(str_ptr);
                Ok(())
            }
            _ => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn len(&self, handle: usize) -> Result<usize, i32> {
        let idx = handle.checked_sub(1).ok_or(HOST_STATUS_NOT_FOUND)?;
        match self.builders.get(idx) {
            Some(Some(b)) => Ok(b.current_len()),
            _ => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn finish(&mut self, handle: usize) -> Result<String, i32> {
        let idx = handle.checked_sub(1).ok_or(HOST_STATUS_NOT_FOUND)?;
        match self.builders.get_mut(idx) {
            Some(slot) => match slot.take() {
                Some(mut b) => {
                    self.free.push(idx);
                    Ok(b.finish())
                }
                None => Err(HOST_STATUS_NOT_FOUND),
            },
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }

    fn discard(&mut self, handle: usize) -> Result<(), i32> {
        let idx = handle.checked_sub(1).ok_or(HOST_STATUS_NOT_FOUND)?;
        match self.builders.get_mut(idx) {
            Some(slot) => match slot.take() {
                Some(_) => {
                    self.free.push(idx);
                    Ok(())
                }
                None => Err(HOST_STATUS_NOT_FOUND),
            },
            None => Err(HOST_STATUS_NOT_FOUND),
        }
    }
}

fn string_builder_registry() -> &'static Mutex<StringBuilderRegistry> {
    static REGISTRY: OnceLock<Mutex<StringBuilderRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(StringBuilderRegistry::new()))
}

fn with_string_builder_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut StringBuilderRegistry) -> R,
{
    let registry = string_builder_registry();
    let mut guard = lock_unpoisoned(registry);
    action(&mut guard)
}

#[allow(dead_code)]
fn lock_string_builder_registry() -> Result<std::sync::MutexGuard<'static, StringBuilderRegistry>, i32> {
    string_builder_registry()
        .lock()
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)
}

/// Fast-path helper for `str.builder_new(capacity)` called from JIT code
/// via the `spectra_rt_builder_new` fast ABI entry.
pub fn string_builder_new_fast(capacity: usize) -> SpectraHostValue {
    with_string_builder_registry(|reg| {
        let builder = match initialize().memory().allocate_manual(StringBuilder::new(capacity)) {
            Ok(b) => b,
            Err(_) => return 0,
        };
        let handle = reg.insert(builder);
        handle as SpectraHostValue
    })
}

/// Fast-path helper for `str.builder_push(handle, str_ptr)`. Reads the
/// Spectra string directly into the builder buffer without allocating an
/// intermediate `String`.
pub fn string_builder_push_fast(handle: usize, str_ptr: SpectraHostValue) {
    with_string_builder_registry(|reg| {
        let _ = reg.push_spectra_string(handle, str_ptr);
    });
}

/// Fast-path helper for `str.builder_len(handle)`.
pub fn string_builder_len_fast(handle: usize) -> SpectraHostValue {
    with_string_builder_registry(|reg| match reg.len(handle) {
        Ok(n) => n as SpectraHostValue,
        Err(_) => 0,
    })
}

/// Fast-path helper for `str.builder_finish(handle)`. Returns a Spectra
/// string handle.
pub fn string_builder_finish_fast(handle: usize) -> SpectraHostValue {
    with_string_builder_registry(|reg| match reg.finish(handle) {
        Ok(s) => unsafe { alloc_spectra_string(&s) },
        Err(_) => 0,
    })
}

/// Fast-path helper for `str.builder_free(handle)`.
pub fn string_builder_free_fast(handle: usize) {
    with_string_builder_registry(|reg| {
        let _ = reg.discard(handle);
    });
}

/// Fast-path helper for `col.map_set(handle, key, value)`.
///
/// Returns 0 on success, `HOST_STATUS_NOT_FOUND` if the handle is invalid.
/// Handle 0 is a sentinel for "no map" and is a no-op (returns NOT_FOUND).
pub fn map_set_fast(handle: usize, key: i64, value: i64) -> i32 {
    with_map_registry(|reg| match reg.maps.get_mut(&handle) {
        Some(m) => {
            m.data.insert(key, value);
            HOST_STATUS_SUCCESS
        }
        None => HOST_STATUS_NOT_FOUND,
    })
}

/// Fast-path helper for `col.map_get(handle, key)`.
///
/// Returns the value for the key, or 0 if the key is absent or the handle
/// is invalid. Note: cannot distinguish "stored value is 0" from "key
/// absent / invalid handle".
pub fn map_get_fast(handle: usize, key: i64) -> i64 {
    with_map_registry(|reg| {
        reg.maps
            .get(&handle)
            .and_then(|m| m.data.get(&key).copied())
            .unwrap_or(0)
    })
}

/// Fast-path helper for `col.map_contains(handle, key)`.
///
/// Returns 1 if the key is present in the map, 0 otherwise (including
/// invalid handle).
pub fn map_contains_fast(handle: usize, key: i64) -> i64 {
    with_map_registry(|reg| {
        reg.maps
            .get(&handle)
            .map(|m| if m.data.contains_key(&key) { 1 } else { 0 })
            .unwrap_or(0)
    })
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

// ── std.tensor runtime ──────────────────────────────────────────────────────

pub const NUMERICAL_TOLERANCE_ABS: f64 = 1.0e-9;
pub const NUMERICAL_TOLERANCE_REL: f64 = 1.0e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TensorDType {
    Int,
    Float,
}

impl TensorDType {
    fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TensorLayout {
    Contiguous,
    View,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TensorDevice {
    Cpu,
    Cuda,
    Rocm,
    Metal,
    DirectMl,
    Vulkan,
    Wgpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TensorPrecision {
    F64,
    F32,
    F16,
    Bf16,
}

impl TensorPrecision {
    fn from_code(code: SpectraHostValue) -> Option<Self> {
        match code {
            0 => Some(Self::F64),
            1 => Some(Self::F32),
            2 => Some(Self::F16),
            3 => Some(Self::Bf16),
            _ => None,
        }
    }

    fn code(self) -> SpectraHostValue {
        match self {
            Self::F64 => 0,
            Self::F32 => 1,
            Self::F16 => 2,
            Self::Bf16 => 3,
        }
    }

    fn quantize(self, value: f64) -> f64 {
        match self {
            Self::F64 => value,
            Self::F32 => value as f32 as f64,
            Self::F16 => half::f16::from_f64(value).to_f64(),
            Self::Bf16 => half::bf16::from_f64(value).to_f64(),
        }
    }
}

impl TensorDevice {
    fn from_code(code: SpectraHostValue) -> Option<Self> {
        match code {
            0 => Some(Self::Cpu),
            1 => Some(Self::Cuda),
            2 => Some(Self::Rocm),
            3 => Some(Self::Metal),
            4 => Some(Self::DirectMl),
            5 => Some(Self::Vulkan),
            6 => Some(Self::Wgpu),
            _ => None,
        }
    }

    fn code(self) -> SpectraHostValue {
        match self {
            Self::Cpu => 0,
            Self::Cuda => 1,
            Self::Rocm => 2,
            Self::Metal => 3,
            Self::DirectMl => 4,
            Self::Vulkan => 5,
            Self::Wgpu => 6,
        }
    }

    fn is_available(self) -> bool {
        match self {
            Self::Cpu => true,
            Self::Wgpu => {
                #[cfg(feature = "gpu")]
                {
                    crate::gpu::is_available()
                }
                #[cfg(not(feature = "gpu"))]
                {
                    false
                }
            }
            _ => false,
        }
    }

    fn is_accelerator(self) -> bool {
        !matches!(self, Self::Cpu)
    }

    fn status_code(self) -> SpectraHostValue {
        if self.is_available() {
            return 0;
        }
        match self {
            Self::Cpu => 0,
            Self::Wgpu => 1,
            Self::Cuda | Self::Rocm | Self::Metal | Self::DirectMl | Self::Vulkan => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutogradOp {
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Relu,
    Exp,
    Log,
    Sqrt,
    Sigmoid,
    Tanh,
    SumTensor,
    MeanTensor,
    Matmul,
    Transpose,
    DotTensor,
    View,
    MlLinear,
    MlConv2d,
    MlMse,
    MlBce,
    MlCrossEntropy,
    MlNll,
}

#[derive(Debug, Clone)]
struct AutogradNode {
    op: AutogradOp,
    parents: Vec<usize>,
    input_shape: Vec<usize>,
    left_shape: Vec<usize>,
    right_shape: Vec<usize>,
    input: Vec<f64>,
    output: Vec<f64>,
    left: Vec<f64>,
    right: Vec<f64>,
    aux: Vec<usize>,
}

impl AutogradNode {
    fn unary(
        op: AutogradOp,
        parent: usize,
        input_shape: Vec<usize>,
        input: Vec<f64>,
        output: Vec<f64>,
    ) -> Self {
        Self {
            op,
            parents: vec![parent],
            input_shape: input_shape.clone(),
            left_shape: Vec::new(),
            right_shape: Vec::new(),
            input,
            output,
            left: Vec::new(),
            right: Vec::new(),
            aux: Vec::new(),
        }
    }

    fn binary(
        op: AutogradOp,
        left_parent: usize,
        right_parent: usize,
        shape: Vec<usize>,
        left: Vec<f64>,
        right: Vec<f64>,
    ) -> Self {
        Self {
            op,
            parents: vec![left_parent, right_parent],
            input_shape: shape.clone(),
            left_shape: Vec::new(),
            right_shape: Vec::new(),
            input: Vec::new(),
            output: Vec::new(),
            left,
            right,
            aux: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct StdTensor {
    dtype: TensorDType,
    shape: Vec<usize>,
    strides: Vec<usize>,
    storage: Arc<Vec<SpectraHostValue>>,
    offset: usize,
    layout: TensorLayout,
    device: TensorDevice,
    precision: TensorPrecision,
    requires_grad: bool,
    grad: Option<Vec<f64>>,
    creator: Option<AutogradNode>,
}

impl StdTensor {
    fn new(dtype: TensorDType, shape: Vec<usize>, data: Vec<SpectraHostValue>) -> Option<Self> {
        let expected_len = shape
            .iter()
            .try_fold(1usize, |acc, dim| acc.checked_mul(*dim))?;
        if expected_len != data.len() {
            return None;
        }
        Self::from_storage(
            dtype,
            shape.clone(),
            tensor_strides(&shape),
            Arc::new(data),
            0,
            TensorLayout::Contiguous,
        )
    }

    fn from_storage(
        dtype: TensorDType,
        shape: Vec<usize>,
        strides: Vec<usize>,
        storage: Arc<Vec<SpectraHostValue>>,
        offset: usize,
        layout: TensorLayout,
    ) -> Option<Self> {
        if shape.is_empty() || shape.iter().any(|&dim| dim == 0) {
            return None;
        }
        let expected_len = shape
            .iter()
            .try_fold(1usize, |acc, dim| acc.checked_mul(*dim))?;
        if strides.len() != shape.len() {
            return None;
        }
        if expected_len == 0 {
            return None;
        }
        let max_offset = max_tensor_offset(&shape, &strides, offset)?;
        if max_offset >= storage.len() {
            return None;
        }
        Some(Self {
            dtype,
            shape,
            strides,
            storage,
            offset,
            layout,
            device: TensorDevice::Cpu,
            precision: TensorPrecision::F64,
            requires_grad: false,
            grad: None,
            creator: None,
        })
    }

    fn len(&self) -> usize {
        self.shape
            .iter()
            .fold(1usize, |acc, dim| acc.saturating_mul(*dim))
    }

    fn offset(&self, indices: &[usize]) -> Option<usize> {
        if indices.len() != self.shape.len() {
            return None;
        }
        let mut offset = self.offset;
        for ((idx, dim), stride) in indices
            .iter()
            .zip(self.shape.iter())
            .zip(self.strides.iter())
        {
            if *idx >= *dim {
                return None;
            }
            offset = offset.checked_add(idx.checked_mul(*stride)?)?;
        }
        Some(offset)
    }

    fn linear_offset(&self, index: usize) -> Option<usize> {
        if index >= self.len() {
            return None;
        }
        if self.layout == TensorLayout::Contiguous {
            return self.offset.checked_add(index);
        }

        let mut remaining = index;
        let mut offset = self.offset;
        for axis in (0..self.shape.len()).rev() {
            let dim = self.shape[axis];
            let axis_index = remaining % dim;
            remaining /= dim;
            offset = offset.checked_add(axis_index.checked_mul(self.strides[axis])?)?;
        }
        Some(offset)
    }

    fn value_at_linear(&self, index: usize) -> Option<SpectraHostValue> {
        let offset = self.linear_offset(index)?;
        self.storage.get(offset).copied()
    }

    fn materialize(&self) -> Vec<SpectraHostValue> {
        if self.layout == TensorLayout::Contiguous
            && self.offset == 0
            && self.storage.len() == self.len()
        {
            return self.storage.as_ref().clone();
        }
        (0..self.len())
            .filter_map(|index| self.value_at_linear(index))
            .collect()
    }

    fn set_linear(&mut self, index: usize, value: SpectraHostValue) -> bool {
        let Some(offset) = self.linear_offset(index) else {
            return false;
        };
        let storage = Arc::make_mut(&mut self.storage);
        if offset >= storage.len() {
            return false;
        }
        storage[offset] = value;
        true
    }

    fn storage_bytes(&self) -> usize {
        self.len()
            .saturating_mul(std::mem::size_of::<SpectraHostValue>())
    }

    fn is_contiguous(&self) -> bool {
        self.layout == TensorLayout::Contiguous && self.strides == tensor_strides(&self.shape)
    }
}

struct TensorRegistry {
    next_id: usize,
    tensors: HashMap<usize, ManualBox<StdTensor>>,
    pool: Vec<Vec<SpectraHostValue>>,
    metrics: TensorMetrics,
    memory_step: usize,
    lifetimes: Vec<TensorLifetimeRecord>,
    active_lifetimes: HashMap<usize, usize>,
}

#[derive(Debug, Clone)]
struct TensorLifetimeRecord {
    handle: usize,
    dtype: TensorDType,
    shape: Vec<usize>,
    bytes: usize,
    allocation_step: usize,
    release_step: Option<usize>,
    allocation_site: String,
}

impl TensorRegistry {
    fn new() -> Self {
        Self {
            next_id: 1,
            tensors: HashMap::new(),
            pool: Vec::new(),
            metrics: TensorMetrics::default(),
            memory_step: 0,
            lifetimes: Vec::new(),
            active_lifetimes: HashMap::new(),
        }
    }

    fn insert(
        &mut self,
        tensor: ManualBox<StdTensor>,
        allocation_site: impl Into<String>,
    ) -> usize {
        let bytes = tensor
            .len()
            .saturating_mul(std::mem::size_of::<SpectraHostValue>());
        self.metrics.allocations = self.metrics.allocations.saturating_add(1);
        self.metrics.active_tensors = self.metrics.active_tensors.saturating_add(1);
        self.metrics.active_bytes = self.metrics.active_bytes.saturating_add(bytes);
        self.metrics.peak_bytes = self.metrics.peak_bytes.max(self.metrics.active_bytes);

        let mut handle = self.next_id.max(1);
        while self.tensors.contains_key(&handle) {
            handle = handle.wrapping_add(1).max(1);
        }
        self.next_id = handle.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        self.memory_step = self.memory_step.saturating_add(1);
        let record = TensorLifetimeRecord {
            handle,
            dtype: tensor.dtype,
            shape: tensor.shape.clone(),
            bytes,
            allocation_step: self.memory_step,
            release_step: None,
            allocation_site: allocation_site.into(),
        };
        let record_index = self.lifetimes.len();
        self.lifetimes.push(record);
        self.active_lifetimes.insert(handle, record_index);
        self.tensors.insert(handle, tensor);
        handle
    }

    fn remove(&mut self, handle: usize) -> Result<(), i32> {
        if let Some(tensor) = self.tensors.remove(&handle) {
            self.mark_released(handle);
            self.recycle_tensor(tensor);
            Ok(())
        } else {
            Err(HOST_STATUS_NOT_FOUND)
        }
    }

    fn clear_all(&mut self) -> usize {
        let count = self.tensors.len();
        let tensors: Vec<_> = self.tensors.drain().map(|(_, tensor)| tensor).collect();
        let handles = self.active_lifetimes.keys().copied().collect::<Vec<_>>();
        for handle in handles {
            self.mark_released(handle);
        }
        for tensor in tensors {
            self.recycle_tensor(tensor);
        }
        self.next_id = 1;
        count
    }

    fn get(&self, handle: usize) -> Option<&StdTensor> {
        self.tensors.get(&handle).map(|boxed| boxed.as_ref())
    }

    fn get_mut(&mut self, handle: usize) -> Option<&mut StdTensor> {
        self.tensors.get_mut(&handle).map(|boxed| boxed.as_mut())
    }

    fn mark_released(&mut self, handle: usize) {
        self.memory_step = self.memory_step.saturating_add(1);
        if let Some(index) = self.active_lifetimes.remove(&handle) {
            if let Some(record) = self.lifetimes.get_mut(index) {
                record.release_step = Some(self.memory_step);
            }
        }
    }

    fn recycle_tensor(&mut self, tensor: ManualBox<StdTensor>) {
        let tensor = tensor.into_inner();
        let bytes = tensor.storage_bytes();
        self.metrics.active_tensors = self.metrics.active_tensors.saturating_sub(1);
        self.metrics.active_bytes = self.metrics.active_bytes.saturating_sub(bytes);
        if tensor.offset == 0 && tensor.is_contiguous() && Arc::strong_count(&tensor.storage) == 1 {
            if let Ok(data) = Arc::try_unwrap(tensor.storage) {
                let capacity = data.capacity();
                if self.pool.len() < 32 {
                    self.pool.push(data);
                } else if let Some((replace_index, _)) = self
                    .pool
                    .iter()
                    .enumerate()
                    .filter(|(_, buffer)| buffer.capacity() < capacity)
                    .min_by_key(|(_, buffer)| buffer.capacity())
                {
                    self.pool[replace_index] = data;
                }
            }
        }
    }

    fn take_buffer(&mut self, len: usize) -> Vec<SpectraHostValue> {
        if let Some(buffer) = self.take_buffer_unfilled(len) {
            self.metrics.reused_buffers = self.metrics.reused_buffers.saturating_add(1);
            self.metrics.pool_hits = self.metrics.pool_hits.saturating_add(1);
            buffer
        } else {
            self.metrics.pool_misses = self.metrics.pool_misses.saturating_add(1);
            vec![0; len]
        }
    }

    fn take_buffer_unfilled(&mut self, len: usize) -> Option<Vec<SpectraHostValue>> {
        let index = self.pool.iter().position(|buffer| buffer.capacity() >= len)?;
        let mut buffer = self.pool.swap_remove(index);
        if buffer.capacity() >= len {
            unsafe {
                buffer.set_len(len);
            }
        } else {
            buffer.resize(len, 0);
        }
        Some(buffer)
    }

    #[allow(dead_code)]
    fn reset_pool(&mut self) {
        self.pool.clear();
    }

    fn note_kernel(&mut self, elements: usize) {
        self.metrics.kernel_ops = self.metrics.kernel_ops.saturating_add(1);
        self.metrics.kernel_elements = self.metrics.kernel_elements.saturating_add(elements);
    }

    fn note_scratch_reuse(&mut self) {
        self.metrics.scratch_reuses = self.metrics.scratch_reuses.saturating_add(1);
    }

    fn note_device_transfer(&mut self) {
        self.metrics.device_transfers = self.metrics.device_transfers.saturating_add(1);
    }

    #[allow(dead_code)]
    fn note_gpu_kernel(&mut self) {
        self.metrics.gpu_kernel_ops = self.metrics.gpu_kernel_ops.saturating_add(1);
    }

    #[allow(dead_code)]
    fn note_cpu_fallback(&mut self) {
        self.metrics.cpu_fallbacks = self.metrics.cpu_fallbacks.saturating_add(1);
    }

    fn reset_metrics(&mut self) {
        let active_tensors = self.tensors.len();
        let active_bytes = self
            .tensors
            .values()
            .map(|tensor| tensor.storage_bytes())
            .sum();
        self.metrics = TensorMetrics {
            active_tensors,
            active_bytes,
            peak_bytes: active_bytes,
            ..Default::default()
        };
        self.memory_step = 0;
        self.lifetimes.clear();
        self.active_lifetimes.clear();
        let snapshots = self
            .tensors
            .iter()
            .map(|(handle, tensor)| {
                (
                    *handle,
                    tensor.dtype,
                    tensor.shape.clone(),
                    tensor.storage_bytes(),
                )
            })
            .collect::<Vec<_>>();
        for (handle, dtype, shape, bytes) in snapshots {
            self.memory_step = self.memory_step.saturating_add(1);
            let index = self.lifetimes.len();
            self.lifetimes.push(TensorLifetimeRecord {
                handle,
                dtype,
                shape,
                bytes,
                allocation_step: self.memory_step,
                release_step: None,
                allocation_site: "reset_stats.active_snapshot".to_string(),
            });
            self.active_lifetimes.insert(handle, index);
        }
    }

    fn allocation_site_count(&self) -> usize {
        self.lifetimes
            .iter()
            .map(|record| record.allocation_site.as_str())
            .collect::<HashSet<_>>()
            .len()
    }

    fn released_lifetime_count(&self) -> usize {
        self.lifetimes
            .iter()
            .filter(|record| record.release_step.is_some())
            .count()
    }

    fn reuse_rate_per_mille(&self) -> usize {
        let total = self
            .metrics
            .pool_hits
            .saturating_add(self.metrics.pool_misses);
        if total == 0 {
            return 0;
        }
        self.metrics.pool_hits.saturating_mul(1000) / total
    }

    fn memory_report_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\"schema\":\"spectra.tensor.memory_report.v1\"");
        out.push_str(&format!(
            ",\"allocations\":{},\"active_tensors\":{},\"active_bytes\":{},\"peak_bytes\":{},\"reused_buffers\":{},\"pool_hits\":{},\"pool_misses\":{},\"reuse_rate_per_mille\":{},\"scratch_reuses\":{},\"allocation_sites\":{},\"lifetime_records\":{},\"released_lifetimes\":{}",
            self.metrics.allocations,
            self.metrics.active_tensors,
            self.metrics.active_bytes,
            self.metrics.peak_bytes,
            self.metrics.reused_buffers,
            self.metrics.pool_hits,
            self.metrics.pool_misses,
            self.reuse_rate_per_mille(),
            self.metrics.scratch_reuses,
            self.allocation_site_count(),
            self.lifetimes.len(),
            self.released_lifetime_count()
        ));
        out.push_str(&format!(
            ",\"kernel_ops\":{},\"kernel_elements\":{},\"device_transfers\":{},\"gpu_kernel_ops\":{},\"cpu_fallbacks\":{}",
            self.metrics.kernel_ops,
            self.metrics.kernel_elements,
            self.metrics.device_transfers,
            self.metrics.gpu_kernel_ops,
            self.metrics.cpu_fallbacks
        ));
        out.push_str(",\"tensors\":[");
        for (index, record) in self.lifetimes.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"handle\":{},\"dtype\":\"{}\",\"shape\":[{}],\"bytes\":{},\"allocation_step\":{},\"release_step\":{},\"active\":{},\"allocation_site\":\"{}\"}}",
                record.handle,
                record.dtype.name(),
                record
                    .shape
                    .iter()
                    .map(|dim| dim.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
                record.bytes,
                record.allocation_step,
                record
                    .release_step
                    .map(|step| step.to_string())
                    .unwrap_or_else(|| "null".to_string()),
                record.release_step.is_none(),
                json_escape(&record.allocation_site)
            ));
        }
        out.push_str("]}");
        out
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TensorMetrics {
    allocations: usize,
    active_tensors: usize,
    active_bytes: usize,
    peak_bytes: usize,
    reused_buffers: usize,
    pool_hits: usize,
    pool_misses: usize,
    scratch_reuses: usize,
    kernel_ops: usize,
    kernel_elements: usize,
    device_transfers: usize,
    gpu_kernel_ops: usize,
    cpu_fallbacks: usize,
}

fn tensor_registry() -> &'static Mutex<TensorRegistry> {
    static REGISTRY: OnceLock<Mutex<TensorRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(TensorRegistry::new()))
}

fn tensor_grad_enabled() -> &'static Mutex<bool> {
    static ENABLED: OnceLock<Mutex<bool>> = OnceLock::new();
    ENABLED.get_or_init(|| Mutex::new(true))
}

fn tensor_deterministic_mode() -> &'static Mutex<bool> {
    static ENABLED: OnceLock<Mutex<bool>> = OnceLock::new();
    ENABLED.get_or_init(|| Mutex::new(false))
}

fn with_tensor_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut TensorRegistry) -> R,
{
    let mut guard = lock_unpoisoned(tensor_registry());
    action(&mut guard)
}

fn tensor_strides(shape: &[usize]) -> Vec<usize> {
    let mut strides = vec![1; shape.len()];
    if shape.len() > 1 {
        for idx in (0..shape.len() - 1).rev() {
            strides[idx] = strides[idx + 1] * shape[idx + 1];
        }
    }
    strides
}

fn tensor_is_grad_enabled() -> bool {
    *lock_unpoisoned(tensor_grad_enabled())
}

fn tensor_values_as_f64(tensor: &StdTensor) -> Vec<f64> {
    tensor
        .materialize()
        .iter()
        .map(|raw| match tensor.dtype {
            TensorDType::Int => *raw as f64,
            TensorDType::Float => f64::from_bits(*raw as u64),
        })
        .collect()
}

fn f64_values_to_host(values: &[f64]) -> Vec<SpectraHostValue> {
    values
        .iter()
        .map(|value| value.to_bits() as SpectraHostValue)
        .collect()
}

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(feature = "gpu")]
fn tensor_values_as_f32(tensor: &StdTensor) -> Option<Vec<f32>> {
    if tensor.dtype != TensorDType::Float {
        return None;
    }
    Some(
        tensor
            .materialize()
            .iter()
            .map(|raw| f64::from_bits(*raw as u64) as f32)
            .collect(),
    )
}

#[cfg(feature = "gpu")]
fn f32_values_to_host(values: &[f32]) -> Vec<SpectraHostValue> {
    values
        .iter()
        .map(|value| (*value as f64).to_bits() as SpectraHostValue)
        .collect()
}

#[cfg(feature = "gpu")]
fn gpu_binary_float(
    left: &StdTensor,
    right: &StdTensor,
    op: crate::gpu::GpuBinaryOp,
) -> Result<Option<Vec<SpectraHostValue>>, ()> {
    if left.device != TensorDevice::Wgpu
        || right.device != TensorDevice::Wgpu
        || left.dtype != TensorDType::Float
        || right.dtype != TensorDType::Float
    {
        return Ok(None);
    }
    let Some(left_data) = tensor_values_as_f32(left) else {
        return Ok(None);
    };
    let Some(right_data) = tensor_values_as_f32(right) else {
        return Ok(None);
    };
    match crate::gpu::binary(&left_data, &right_data, op) {
        Ok(values) => Ok(Some(f32_values_to_host(&values))),
        Err(_) => Err(()),
    }
}

#[cfg(feature = "gpu")]
fn gpu_unary_float(
    tensor: &StdTensor,
    op: crate::gpu::GpuUnaryOp,
) -> Result<Option<Vec<SpectraHostValue>>, ()> {
    if tensor.device != TensorDevice::Wgpu || tensor.dtype != TensorDType::Float {
        return Ok(None);
    }
    let Some(data) = tensor_values_as_f32(tensor) else {
        return Ok(None);
    };
    match crate::gpu::unary(&data, op) {
        Ok(values) => Ok(Some(f32_values_to_host(&values))),
        Err(_) => Err(()),
    }
}

fn tensor_requires_autograd(registry: &TensorRegistry, parents: &[usize]) -> bool {
    if !tensor_is_grad_enabled() {
        return false;
    }
    parents.iter().any(|handle| {
        registry
            .get(*handle)
            .map(|tensor| tensor.requires_grad && tensor.dtype == TensorDType::Float)
            .unwrap_or(false)
    })
}

fn max_tensor_offset(shape: &[usize], strides: &[usize], base_offset: usize) -> Option<usize> {
    let mut max_offset = base_offset;
    for (dim, stride) in shape.iter().zip(strides.iter()) {
        max_offset = max_offset.checked_add(dim.saturating_sub(1).checked_mul(*stride)?)?;
    }
    Some(max_offset)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TensorKernelStrategy {
    Scalar = 1,
    #[allow(dead_code)]
    Avx2 = 2,
    #[allow(dead_code)]
    Neon = 3,
    #[allow(dead_code)]
    Blas = 4,
    #[allow(dead_code)]
    Gpu = 5,
}

impl TensorKernelStrategy {
    fn current() -> Self {
        #[cfg(feature = "blas")]
        {
            return Self::Blas;
        }
        #[cfg(feature = "gpu")]
        {
            if crate::gpu::is_available() {
                return Self::Gpu;
            }
        }
        Self::Scalar
    }

    fn code(self) -> SpectraHostValue {
        self as SpectraHostValue
    }
}

fn kernel_dot_i64(left: &[SpectraHostValue], right: &[SpectraHostValue]) -> SpectraHostValue {
    debug_assert_eq!(left.len(), right.len());
    let len = left.len();
    let mut acc0 = 0i64;
    let mut acc1 = 0i64;
    let mut acc2 = 0i64;
    let mut acc3 = 0i64;
    let mut idx = 0usize;
    unsafe {
        let left_ptr = left.as_ptr();
        let right_ptr = right.as_ptr();
        while idx + 4 <= len {
            acc0 += *left_ptr.add(idx) * *right_ptr.add(idx);
            acc1 += *left_ptr.add(idx + 1) * *right_ptr.add(idx + 1);
            acc2 += *left_ptr.add(idx + 2) * *right_ptr.add(idx + 2);
            acc3 += *left_ptr.add(idx + 3) * *right_ptr.add(idx + 3);
            idx += 4;
        }
        let mut acc = acc0 + acc1 + acc2 + acc3;
        while idx < len {
            acc += *left_ptr.add(idx) * *right_ptr.add(idx);
            idx += 1;
        }
        acc
    }
}

fn kernel_dot_f64_bits(left: &[SpectraHostValue], right: &[SpectraHostValue]) -> SpectraHostValue {
    let mut acc0 = 0.0f64;
    let mut acc1 = 0.0f64;
    let mut acc2 = 0.0f64;
    let mut acc3 = 0.0f64;
    let chunks = left.chunks_exact(4);
    let remainder = chunks.remainder();
    for (a, b) in chunks.zip(right.chunks_exact(4)) {
        acc0 += f64::from_bits(a[0] as u64) * f64::from_bits(b[0] as u64);
        acc1 += f64::from_bits(a[1] as u64) * f64::from_bits(b[1] as u64);
        acc2 += f64::from_bits(a[2] as u64) * f64::from_bits(b[2] as u64);
        acc3 += f64::from_bits(a[3] as u64) * f64::from_bits(b[3] as u64);
    }
    let mut acc = acc0 + acc1 + acc2 + acc3;
    let offset = left.len() - remainder.len();
    for idx in 0..remainder.len() {
        acc +=
            f64::from_bits(left[offset + idx] as u64) * f64::from_bits(right[offset + idx] as u64);
    }
    acc.to_bits() as i64
}

fn kernel_transpose_i64(
    data: &[SpectraHostValue],
    rows: usize,
    cols: usize,
) -> Vec<SpectraHostValue> {
    let mut out = vec![0; data.len()];
    for row in 0..rows {
        for col in 0..cols {
            out[col * rows + row] = data[row * cols + col];
        }
    }
    out
}

fn kernel_matmul_i64(
    left: &[SpectraHostValue],
    right: &[SpectraHostValue],
    m: usize,
    k: usize,
    n: usize,
) -> Vec<SpectraHostValue> {
    let right_t = kernel_transpose_i64(right, k, n);
    let mut out = vec![0; m * n];
    for row in 0..m {
        let lhs = &left[row * k..row * k + k];
        for col in 0..n {
            let rhs = &right_t[col * k..col * k + k];
            out[row * n + col] = kernel_dot_i64(lhs, rhs);
        }
    }
    out
}

fn kernel_matmul_f64_bits(
    left: &[SpectraHostValue],
    right: &[SpectraHostValue],
    m: usize,
    k: usize,
    n: usize,
) -> Vec<SpectraHostValue> {
    let right_t = kernel_transpose_i64(right, k, n);
    let mut out = vec![0; m * n];
    for row in 0..m {
        let lhs = &left[row * k..row * k + k];
        for col in 0..n {
            let rhs = &right_t[col * k..col * k + k];
            out[row * n + col] = kernel_dot_f64_bits(lhs, rhs);
        }
    }
    out
}

#[doc(hidden)]
pub fn tensor_bench_kernel_dot_i64(left: &[i64], right: &[i64]) -> i64 {
    kernel_dot_i64(left, right)
}

#[doc(hidden)]
pub fn tensor_bench_kernel_matmul_i64(
    left: &[i64],
    right: &[i64],
    m: usize,
    k: usize,
    n: usize,
) -> Vec<i64> {
    kernel_matmul_i64(left, right, m, k, n)
}

#[track_caller]
fn tensor_alloc(
    dtype: TensorDType,
    shape: Vec<usize>,
    data: Vec<SpectraHostValue>,
) -> Result<usize, i32> {
    let data = with_tensor_registry(|registry| {
        let mut buffer = registry.take_buffer(data.len());
        buffer.copy_from_slice(&data);
        buffer
    });
    let Some(tensor) = StdTensor::new(dtype, shape, data) else {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    };
    let memory = initialize().memory();
    let tensor = memory
        .allocate_manual(tensor)
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;
    let site = tensor_allocation_site(std::panic::Location::caller());
    Ok(with_tensor_registry(|registry| {
        registry.insert(tensor, site)
    }))
}

#[track_caller]
fn tensor_alloc_buffered(
    dtype: TensorDType,
    shape: Vec<usize>,
    buffer: Vec<SpectraHostValue>,
) -> Result<usize, i32> {
    let Some(tensor) = StdTensor::new(dtype, shape, buffer) else {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    };
    let memory = initialize().memory();
    let tensor = memory
        .allocate_manual(tensor)
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;
    let site = tensor_allocation_site(std::panic::Location::caller());
    Ok(with_tensor_registry(|registry| {
        registry.insert(tensor, site)
    }))
}

#[track_caller]
fn tensor_insert(tensor: StdTensor) -> Result<usize, i32> {
    let memory = initialize().memory();
    let tensor = memory
        .allocate_manual(tensor)
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;
    let site = tensor_allocation_site(std::panic::Location::caller());
    Ok(with_tensor_registry(|registry| {
        registry.insert(tensor, site)
    }))
}

#[track_caller]
fn tensor_alloc_autograd(
    dtype: TensorDType,
    shape: Vec<usize>,
    data: Vec<SpectraHostValue>,
    requires_grad: bool,
    creator: Option<AutogradNode>,
) -> Result<usize, i32> {
    let data = with_tensor_registry(|registry| {
        let mut buffer = registry.take_buffer(data.len());
        buffer.copy_from_slice(&data);
        buffer
    });
    let Some(mut tensor) = StdTensor::new(dtype, shape, data) else {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    };
    tensor.requires_grad = requires_grad && dtype == TensorDType::Float;
    tensor.creator = if tensor.requires_grad { creator } else { None };
    tensor_insert(tensor)
}

#[track_caller]
fn tensor_alloc_autograd_on_device(
    dtype: TensorDType,
    shape: Vec<usize>,
    data: Vec<SpectraHostValue>,
    requires_grad: bool,
    creator: Option<AutogradNode>,
    device: TensorDevice,
    precision: TensorPrecision,
) -> Result<usize, i32> {
    let data = with_tensor_registry(|registry| {
        let mut buffer = registry.take_buffer(data.len());
        buffer.copy_from_slice(&data);
        buffer
    });
    let Some(mut tensor) = StdTensor::new(dtype, shape, data) else {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    };
    tensor.device = device;
    tensor.precision = precision;
    tensor.requires_grad = requires_grad && dtype == TensorDType::Float;
    tensor.creator = if tensor.requires_grad { creator } else { None };
    tensor_insert(tensor)
}

fn tensor_allocation_site(location: &'static std::panic::Location<'static>) -> String {
    format!("{}:{}", location.file(), location.line())
}

fn tensor_result(ctx_ref: &mut SpectraHostCallContext, value: SpectraHostValue) -> i32 {
    if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = value;
    }
    HOST_STATUS_SUCCESS
}

fn tensor_optional_result(ctx_ref: &mut SpectraHostCallContext, value: SpectraHostValue) -> i32 {
    if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
        return HOST_STATUS_SUCCESS;
    }
    unsafe {
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = value;
    }
    HOST_STATUS_SUCCESS
}

#[inline]
fn fill_i64_pattern(buffer: &mut [SpectraHostValue], value: SpectraHostValue) {
    for slot in buffer.iter_mut() {
        *slot = value;
    }
}

unsafe fn tensor_args<'a>(
    ctx: *mut SpectraHostCallContext,
    expected: usize,
) -> Result<(&'a mut SpectraHostCallContext, &'a [SpectraHostValue]), i32> {
    if ctx.is_null() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let ctx_ref = &mut *ctx;
    if ctx_ref.arg_len != expected || (expected > 0 && ctx_ref.args.is_null()) {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let args = if expected == 0 {
        &[] as &[SpectraHostValue]
    } else {
        slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len)
    };
    Ok((ctx_ref, args))
}

fn tensor_create_1d(
    ctx: *mut SpectraHostCallContext,
    value: SpectraHostValue,
    dtype: TensorDType,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let size = args[0];
        if size <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let data = vec![value; size as usize];
        match tensor_alloc(dtype, vec![size as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

fn tensor_create_2d(
    ctx: *mut SpectraHostCallContext,
    value: SpectraHostValue,
    dtype: TensorDType,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let rows = args[0];
        let cols = args[1];
        if rows <= 0 || cols <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(len) = (rows as usize).checked_mul(cols as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let data = vec![value; len];
        match tensor_alloc(dtype, vec![rows as usize, cols as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_zeros(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_create_1d(ctx, 0, TensorDType::Int)
}

extern "C" fn std_tensor_ones(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_create_1d(ctx, 1, TensorDType::Int)
}

extern "C" fn std_tensor_full(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[0] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let data = vec![args[1]; args[0] as usize];
        match tensor_alloc(TensorDType::Int, vec![args[0] as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_full_f(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let n = args[0];
        if n <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let len = n as usize;
        let value = args[1];
        let buffer = with_tensor_registry(|registry| {
            if let Some(buffer) = registry.take_buffer_unfilled(len) {
                registry.metrics.reused_buffers =
                    registry.metrics.reused_buffers.saturating_add(1);
                registry.metrics.pool_hits = registry.metrics.pool_hits.saturating_add(1);
                Some(buffer)
            } else {
                registry.metrics.pool_misses =
                    registry.metrics.pool_misses.saturating_add(1);
                None
            }
        });
        let mut buffer = buffer.unwrap_or_else(|| Vec::with_capacity(len));
        if buffer.len() < len {
            buffer.resize(len, 0);
        }
        fill_i64_pattern(&mut buffer, value);
        match tensor_alloc_buffered(TensorDType::Float, vec![len], buffer) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_refill(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let handle = args[0] as usize;
        let value = args[1];
        let result = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(handle) else {
                return Err(HOST_STATUS_NOT_FOUND);
            };
            if tensor.dtype != TensorDType::Float {
                return Err(HOST_STATUS_INVALID_ARGUMENT);
            }
            if tensor.requires_grad {
                return Err(HOST_STATUS_INVALID_ARGUMENT);
            }
            if !tensor.is_contiguous() || tensor.offset != 0 {
                return Err(HOST_STATUS_INVALID_ARGUMENT);
            }
            let len = tensor.len();
            let storage = Arc::make_mut(&mut tensor.storage);
            if storage.len() < len {
                storage.resize(len, 0);
            }
            fill_i64_pattern(storage, value);
            Ok(())
        });
        match result {
            Ok(()) => tensor_optional_result(ctx_ref, 0),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_literal(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_literal_1d(ctx, TensorDType::Int)
}

extern "C" fn std_tensor_literal_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_literal_1d(ctx, TensorDType::Float)
}

extern "C" fn std_tensor_literal2(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_literal_2d(ctx, TensorDType::Int)
}

extern "C" fn std_tensor_literal2_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_literal_2d(ctx, TensorDType::Float)
}

fn tensor_literal_1d(ctx: *mut SpectraHostCallContext, dtype: TensorDType) -> i32 {
    unsafe {
        if ctx.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len == 0 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let len = args[0];
        if len < 0 || args.len() != (len as usize).saturating_add(1) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let data = args[1..].to_vec();
        match tensor_alloc(dtype, vec![len as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

fn tensor_literal_2d(ctx: *mut SpectraHostCallContext, dtype: TensorDType) -> i32 {
    unsafe {
        if ctx.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 2 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let rows = args[0];
        let cols = args[1];
        if rows < 0 || cols < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(len) = (rows as usize).checked_mul(cols as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args.len() != len.saturating_add(2) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let data = args[2..].to_vec();
        match tensor_alloc(dtype, vec![rows as usize, cols as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_arange(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (start, end, step) = (args[0], args[1], args[2]);
        if step == 0 || (step > 0 && start >= end) || (step < 0 && start <= end) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mut data = Vec::new();
        let mut current = start;
        while (step > 0 && current < end) || (step < 0 && current > end) {
            data.push(current);
            current = current.saturating_add(step);
            if data.len() > 10_000_000 {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
        }
        match tensor_alloc(TensorDType::Int, vec![data.len()], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_zeros2(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_create_2d(ctx, 0, TensorDType::Int)
}

extern "C" fn std_tensor_ones2(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_create_2d(ctx, 1, TensorDType::Int)
}

extern "C" fn std_tensor_full2(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[0] <= 0 || args[1] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(len) = (args[0] as usize).checked_mul(args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let data = vec![args[2]; len];
        match tensor_alloc(
            TensorDType::Int,
            vec![args[0] as usize, args[1] as usize],
            data,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_full2_f(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[0] <= 0 || args[1] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(len) = (args[0] as usize).checked_mul(args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let data = vec![args[2]; len];
        match tensor_alloc(
            TensorDType::Float,
            vec![args[0] as usize, args[1] as usize],
            data,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_len(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| tensor.len() as SpectraHostValue)
}

extern "C" fn std_tensor_rank(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| tensor.shape.len() as SpectraHostValue)
}

extern "C" fn std_tensor_dim(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = with_tensor_registry(|registry| {
            registry
                .get(args[0] as usize)
                .and_then(|tensor| tensor.shape.get(args[1] as usize).copied())
                .map(|dim| dim as SpectraHostValue)
                .unwrap_or(-1)
        });
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_rows(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| {
        tensor.shape.first().copied().unwrap_or(0) as i64
    })
}

extern "C" fn std_tensor_cols(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| {
        tensor.shape.get(1).copied().unwrap_or(1) as i64
    })
}

extern "C" fn std_tensor_is_valid(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let valid = with_tensor_registry(|registry| registry.get(args[0] as usize).is_some());
        tensor_result(ctx_ref, valid as SpectraHostValue)
    }
}

fn tensor_query_i64(
    ctx: *mut SpectraHostCallContext,
    query: impl FnOnce(&StdTensor) -> SpectraHostValue,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(value) =
            with_tensor_registry(|registry| registry.get(args[0] as usize).map(query))
        else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_get(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_get_linear(ctx, false)
}

extern "C" fn std_tensor_get_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_get_linear(ctx, true)
}

fn tensor_get_linear(ctx: *mut SpectraHostCallContext, as_float: bool) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let index = args[1];
        if index < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(value) = with_tensor_registry(|registry| {
            registry
                .get(args[0] as usize)
                .and_then(|tensor| tensor.value_at_linear(index as usize))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let value = if as_float {
            value
        } else {
            f64_bits_to_i64_if_needed(value)
        };
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_set(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_set_linear(ctx, false)
}

extern "C" fn std_tensor_set_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_set_linear(ctx, true)
}

fn tensor_set_linear(ctx: *mut SpectraHostCallContext, is_float: bool) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ok = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(args[0] as usize) else {
                return false;
            };
            if (args[1] as usize) >= tensor.len() {
                return false;
            }
            tensor.dtype = if is_float {
                TensorDType::Float
            } else {
                TensorDType::Int
            };
            tensor.set_linear(args[1] as usize, args[2])
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_get2(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_get_2d(ctx, false)
}

extern "C" fn std_tensor_get2_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_get_2d(ctx, true)
}

fn tensor_get_2d(ctx: *mut SpectraHostCallContext, as_float: bool) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 || args[2] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(value) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            let offset = tensor.offset(&[args[1] as usize, args[2] as usize])?;
            tensor.storage.get(offset).copied()
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let value = if as_float {
            value
        } else {
            f64_bits_to_i64_if_needed(value)
        };
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_set2(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_set_2d(ctx, false)
}

extern "C" fn std_tensor_set2_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_set_2d(ctx, true)
}

fn tensor_set_2d(ctx: *mut SpectraHostCallContext, is_float: bool) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 4) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 || args[2] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ok = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(args[0] as usize) else {
                return false;
            };
            let Some(offset) = tensor.offset(&[args[1] as usize, args[2] as usize]) else {
                return false;
            };
            tensor.dtype = if is_float {
                TensorDType::Float
            } else {
                TensorDType::Int
            };
            let storage = Arc::make_mut(&mut tensor.storage);
            if offset >= storage.len() {
                return false;
            }
            storage[offset] = args[3];
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_reshape(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (handle, rows, cols) = (args[0] as usize, args[1], args[2]);
        if rows <= 0 || cols <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(new_len) = (rows as usize).checked_mul(cols as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(tensor) = with_tensor_registry(|registry| {
            registry.get(handle).and_then(|tensor| {
                if tensor.len() != new_len {
                    return None;
                }
                let mut result = StdTensor::from_storage(
                    tensor.dtype,
                    vec![rows as usize, cols as usize],
                    tensor_strides(&[rows as usize, cols as usize]),
                    tensor.storage.clone(),
                    tensor.offset,
                    if tensor.is_contiguous() {
                        TensorLayout::Contiguous
                    } else {
                        TensorLayout::View
                    },
                )?;
                result.device = tensor.device;
                result.precision = tensor.precision;
                let requires_grad = tensor.dtype == TensorDType::Float
                    && tensor_requires_autograd(registry, &[handle]);
                if requires_grad {
                    result.requires_grad = true;
                    result.creator = Some(AutogradNode {
                        op: AutogradOp::View,
                        parents: vec![handle],
                        input_shape: tensor.shape.clone(),
                        left_shape: Vec::new(),
                        right_shape: Vec::new(),
                        input: tensor_values_as_f64(tensor),
                        output: Vec::new(),
                        left: Vec::new(),
                        right: Vec::new(),
                        aux: Vec::new(),
                    });
                }
                Some(result)
            })
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_insert(tensor) {
            Ok(new_handle) => tensor_result(ctx_ref, new_handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_flatten(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(tensor) = with_tensor_registry(|registry| {
            registry.get(args[0] as usize).and_then(|tensor| {
                let mut result = if tensor.is_contiguous() {
                    StdTensor::from_storage(
                        tensor.dtype,
                        vec![tensor.len()],
                        vec![1],
                        tensor.storage.clone(),
                        tensor.offset,
                        TensorLayout::Contiguous,
                    )
                } else {
                    let data = tensor.materialize();
                    StdTensor::new(tensor.dtype, vec![data.len()], data)
                }?;
                result.device = tensor.device;
                result.precision = tensor.precision;
                let requires_grad = tensor.dtype == TensorDType::Float
                    && tensor_requires_autograd(registry, &[args[0] as usize]);
                if requires_grad {
                    result.requires_grad = true;
                    result.creator = Some(AutogradNode {
                        op: AutogradOp::View,
                        parents: vec![args[0] as usize],
                        input_shape: tensor.shape.clone(),
                        left_shape: Vec::new(),
                        right_shape: Vec::new(),
                        input: tensor_values_as_f64(tensor),
                        output: Vec::new(),
                        left: Vec::new(),
                        right: Vec::new(),
                        aux: Vec::new(),
                    });
                }
                Some(result)
            })
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        match tensor_insert(tensor) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_permute(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 || args[2] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(tensor) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            let (axis_a, axis_b) = (args[1] as usize, args[2] as usize);
            if axis_a >= tensor.shape.len() || axis_b >= tensor.shape.len() {
                return None;
            }
            let mut shape = tensor.shape.clone();
            let mut strides = tensor.strides.clone();
            shape.swap(axis_a, axis_b);
            strides.swap(axis_a, axis_b);
            let mut result = StdTensor::from_storage(
                tensor.dtype,
                shape,
                strides,
                tensor.storage.clone(),
                tensor.offset,
                TensorLayout::View,
            )?;
            result.device = tensor.device;
            result.precision = tensor.precision;
            Some(result)
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_insert(tensor) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_slice(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (handle, start, end) = (args[0] as usize, args[1], args[2]);
        if start < 0 || end < start {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(tensor) = with_tensor_registry(|registry| {
            let tensor = registry.get(handle)?;
            if tensor.shape.len() != 1 || end as usize > tensor.len() || start == end {
                return None;
            }
            let base_offset = tensor.linear_offset(start as usize)?;
            let mut result = StdTensor::from_storage(
                tensor.dtype,
                vec![(end - start) as usize],
                vec![tensor.strides[0]],
                tensor.storage.clone(),
                base_offset,
                TensorLayout::View,
            )?;
            result.device = tensor.device;
            result.precision = tensor.precision;
            Some(result)
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_insert(tensor) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_concat(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((dtype, shape, data)) = with_tensor_registry(|registry| {
            let left = registry.get(args[0] as usize)?;
            let right = registry.get(args[1] as usize)?;
            if left.dtype != right.dtype || left.shape.len() != right.shape.len() {
                return None;
            }
            let mut shape = left.shape.clone();
            if left.shape.len() == 1 {
                shape[0] = left.shape[0].checked_add(right.shape[0])?;
            } else {
                if left.shape[1..] != right.shape[1..] {
                    return None;
                }
                shape[0] = left.shape[0].checked_add(right.shape[0])?;
            }
            let mut data = left.materialize();
            data.extend(right.materialize());
            let dtype = left.dtype;
            registry.note_kernel(data.len());
            Some((dtype, shape, data))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc(dtype, shape, data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_stack(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((dtype, shape, data)) = with_tensor_registry(|registry| {
            let left = registry.get(args[0] as usize)?;
            let right = registry.get(args[1] as usize)?;
            if left.dtype != right.dtype || left.shape != right.shape {
                return None;
            }
            let mut shape = Vec::with_capacity(left.shape.len() + 1);
            shape.push(2);
            shape.extend(left.shape.iter().copied());
            let mut data = left.materialize();
            data.extend(right.materialize());
            let dtype = left.dtype;
            registry.note_kernel(data.len());
            Some((dtype, shape, data))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc(dtype, shape, data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_add(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_binary(ctx, AutogradOp::Add, |a, b| a + b, |a, b| a + b)
}

extern "C" fn std_tensor_sub(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_binary(ctx, AutogradOp::Sub, |a, b| a - b, |a, b| a - b)
}

extern "C" fn std_tensor_mul(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_binary(ctx, AutogradOp::Mul, |a, b| a * b, |a, b| a * b)
}

extern "C" fn std_tensor_div(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_binary(
        ctx,
        AutogradOp::Div,
        |a, b| if b == 0 { 0 } else { a / b },
        |a, b| if b == 0.0 { f64::NAN } else { a / b },
    )
}

fn tensor_binary(
    ctx: *mut SpectraHostCallContext,
    op: AutogradOp,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((dtype, shape, data, requires_grad, creator, device, precision)) =
            with_tensor_registry(|registry| {
                let left = registry.get(args[0] as usize)?.clone();
                let right = registry.get(args[1] as usize)?.clone();
                if left.shape != right.shape || left.dtype != right.dtype {
                    return None;
                }
                if left.device != right.device {
                    return None;
                }
                let element_count = left.len();
                let left_data = left.materialize();
                let right_data = right.materialize();
                #[cfg(feature = "gpu")]
                let gpu_data = if left.device == TensorDevice::Wgpu {
                    let gpu_op = match op {
                        AutogradOp::Add => Some(crate::gpu::GpuBinaryOp::Add),
                        AutogradOp::Sub => Some(crate::gpu::GpuBinaryOp::Sub),
                        AutogradOp::Mul => Some(crate::gpu::GpuBinaryOp::Mul),
                        AutogradOp::Div => Some(crate::gpu::GpuBinaryOp::Div),
                        _ => None,
                    }?;
                    match gpu_binary_float(&left, &right, gpu_op) {
                        Ok(Some(data)) => {
                            registry.note_gpu_kernel();
                            Some(data)
                        }
                        Ok(None) => None,
                        Err(()) => {
                            registry.note_cpu_fallback();
                            None
                        }
                    }
                } else {
                    None
                };
                let data = match left.dtype {
                    TensorDType::Int => {
                        if left.device.is_accelerator() {
                            return None;
                        }
                        left_data
                            .iter()
                            .zip(right_data.iter())
                            .map(|(a, b)| int_op(*a, *b))
                            .collect()
                    }
                    TensorDType::Float => {
                        #[cfg(feature = "gpu")]
                        if let Some(data) = gpu_data {
                            data
                        } else {
                            left_data
                                .iter()
                                .zip(right_data.iter())
                                .map(|(a, b)| {
                                    float_op(f64::from_bits(*a as u64), f64::from_bits(*b as u64))
                                        .to_bits() as i64
                                })
                                .collect()
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            if left.device.is_accelerator() {
                                return None;
                            }
                            left_data
                                .iter()
                                .zip(right_data.iter())
                                .map(|(a, b)| {
                                    float_op(f64::from_bits(*a as u64), f64::from_bits(*b as u64))
                                        .to_bits() as i64
                                })
                                .collect()
                        }
                    }
                };
                let requires_grad = left.dtype == TensorDType::Float
                    && tensor_requires_autograd(registry, &[args[0] as usize, args[1] as usize]);
                let creator = requires_grad.then(|| {
                    AutogradNode::binary(
                        op,
                        args[0] as usize,
                        args[1] as usize,
                        left.shape.clone(),
                        left_data
                            .iter()
                            .map(|raw| f64::from_bits(*raw as u64))
                            .collect(),
                        right_data
                            .iter()
                            .map(|raw| f64::from_bits(*raw as u64))
                            .collect(),
                    )
                });
                let result = Some((
                    left.dtype,
                    left.shape.clone(),
                    data,
                    requires_grad,
                    creator,
                    left.device,
                    left.precision,
                ));
                registry.note_kernel(element_count);
                result
            })
        else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc_autograd_on_device(
            dtype,
            shape,
            data,
            requires_grad,
            creator,
            device,
            precision,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_sum(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(value) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            let data = tensor.materialize();
            let value = match tensor.dtype {
                TensorDType::Int => {
                    if tensor.device.is_accelerator() {
                        return None;
                    }
                    data.iter().sum()
                }
                TensorDType::Float => {
                    #[cfg(feature = "gpu")]
                    if tensor.device == TensorDevice::Wgpu {
                        let gpu_data = tensor_values_as_f32(tensor)?;
                        match crate::gpu::sum(&gpu_data) {
                            Ok(value) => {
                                registry.note_gpu_kernel();
                                value as i64
                            }
                            Err(_) => {
                                registry.note_cpu_fallback();
                                data.iter()
                                    .map(|bits| f64::from_bits(*bits as u64))
                                    .sum::<f64>() as i64
                            }
                        }
                    } else {
                        data.iter()
                            .map(|bits| f64::from_bits(*bits as u64))
                            .sum::<f64>() as i64
                    }
                    #[cfg(not(feature = "gpu"))]
                    {
                        if tensor.device.is_accelerator() {
                            return None;
                        }
                        data.iter()
                            .map(|bits| f64::from_bits(*bits as u64))
                            .sum::<f64>() as i64
                    }
                }
            };
            Some(value)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_sum_f(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(sum) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            let data = tensor.materialize();
            let sum = match tensor.dtype {
                TensorDType::Int => {
                    if tensor.device.is_accelerator() {
                        return None;
                    }
                    data.iter().map(|v| *v as f64).sum::<f64>()
                }
                TensorDType::Float => {
                    #[cfg(feature = "gpu")]
                    if tensor.device == TensorDevice::Wgpu {
                        let gpu_data = tensor_values_as_f32(tensor)?;
                        match crate::gpu::sum(&gpu_data) {
                            Ok(value) => {
                                registry.note_gpu_kernel();
                                value as f64
                            }
                            Err(_) => {
                                registry.note_cpu_fallback();
                                data.iter()
                                    .map(|bits| f64::from_bits(*bits as u64))
                                    .sum::<f64>()
                            }
                        }
                    } else {
                        data.iter()
                            .map(|bits| f64::from_bits(*bits as u64))
                            .sum::<f64>()
                    }
                    #[cfg(not(feature = "gpu"))]
                    {
                        if tensor.device.is_accelerator() {
                            return None;
                        }
                        data.iter()
                            .map(|bits| f64::from_bits(*bits as u64))
                            .sum::<f64>()
                    }
                }
            };
            Some(sum)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, sum.to_bits() as i64)
    }
}

extern "C" fn std_tensor_sum_t(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_reduction_tensor(ctx, AutogradOp::SumTensor, |values| values.iter().sum())
}

extern "C" fn std_tensor_mean_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| {
        let data = tensor.materialize();
        if data.is_empty() {
            return f64::NAN.to_bits() as i64;
        }
        let sum = match tensor.dtype {
            TensorDType::Int => data.iter().map(|v| *v as f64).sum::<f64>(),
            TensorDType::Float => data
                .iter()
                .map(|bits| f64::from_bits(*bits as u64))
                .sum::<f64>(),
        };
        (sum / data.len() as f64).to_bits() as i64
    })
}

extern "C" fn std_tensor_mean_t(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_reduction_tensor(ctx, AutogradOp::MeanTensor, |values| {
        values.iter().sum::<f64>() / values.len() as f64
    })
}

fn tensor_reduction_tensor(
    ctx: *mut SpectraHostCallContext,
    op: AutogradOp,
    reduce: impl Fn(&[f64]) -> f64,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((value, requires_grad, creator)) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            let values = tensor_values_as_f64(tensor);
            if values.is_empty() {
                return None;
            }
            let value = reduce(&values);
            let requires_grad = tensor.dtype == TensorDType::Float
                && tensor_requires_autograd(registry, &[args[0] as usize]);
            let creator = requires_grad.then(|| AutogradNode {
                op,
                parents: vec![args[0] as usize],
                input_shape: tensor.shape.clone(),
                left_shape: Vec::new(),
                right_shape: Vec::new(),
                input: values,
                output: vec![value],
                left: Vec::new(),
                right: Vec::new(),
                aux: Vec::new(),
            });
            registry.note_kernel(tensor.len());
            Some((value, requires_grad, creator))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc_autograd(
            TensorDType::Float,
            vec![1],
            vec![value.to_bits() as SpectraHostValue],
            requires_grad,
            creator,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_max(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| {
        tensor.materialize().iter().copied().max().unwrap_or(0)
    })
}

extern "C" fn std_tensor_min(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| {
        tensor.materialize().iter().copied().min().unwrap_or(0)
    })
}

extern "C" fn std_tensor_argmax(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_query_i64(ctx, |tensor| {
        let data = tensor.materialize();
        if data.is_empty() {
            return -1;
        }
        let mut best_index = 0usize;
        match tensor.dtype {
            TensorDType::Int => {
                let mut best = data[0];
                for (index, value) in data.iter().copied().enumerate().skip(1) {
                    if value > best {
                        best = value;
                        best_index = index;
                    }
                }
            }
            TensorDType::Float => {
                let mut best = f64::from_bits(data[0] as u64);
                for (index, raw) in data.iter().copied().enumerate().skip(1) {
                    let value = f64::from_bits(raw as u64);
                    if value > best {
                        best = value;
                        best_index = index;
                    }
                }
            }
        }
        best_index as SpectraHostValue
    })
}

extern "C" fn std_tensor_transpose(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(tensor) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            if tensor.shape.len() != 2 {
                return None;
            }
            let element_count = tensor.len();
            let mut result = StdTensor::from_storage(
                tensor.dtype,
                vec![tensor.shape[1], tensor.shape[0]],
                vec![tensor.strides[1], tensor.strides[0]],
                tensor.storage.clone(),
                tensor.offset,
                TensorLayout::View,
            )?;
            result.device = tensor.device;
            result.precision = tensor.precision;
            let requires_grad = tensor.dtype == TensorDType::Float
                && tensor_requires_autograd(registry, &[args[0] as usize]);
            if requires_grad {
                result.requires_grad = true;
                result.creator = Some(AutogradNode {
                    op: AutogradOp::Transpose,
                    parents: vec![args[0] as usize],
                    input_shape: tensor.shape.clone(),
                    left_shape: Vec::new(),
                    right_shape: Vec::new(),
                    input: tensor_values_as_f64(tensor),
                    output: Vec::new(),
                    left: Vec::new(),
                    right: Vec::new(),
                    aux: Vec::new(),
                });
            }
            registry.note_kernel(element_count);
            Some(result)
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_insert(tensor) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_dot(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(value) = with_tensor_registry(|registry| {
            let left = registry.get(args[0] as usize)?;
            let right = registry.get(args[1] as usize)?;
            if left.shape.len() != 1 || left.shape != right.shape || left.dtype != right.dtype {
                return None;
            }
            let element_count = left.len();
            let left_data = left.materialize();
            let right_data = right.materialize();
            let value = match left.dtype {
                TensorDType::Int => Some(kernel_dot_i64(&left_data, &right_data)),
                TensorDType::Float => Some(kernel_dot_f64_bits(&left_data, &right_data)),
            };
            registry.note_kernel(element_count);
            value
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_dot_t(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((value, requires_grad, creator)) = with_tensor_registry(|registry| {
            let left = registry.get(args[0] as usize)?;
            let right = registry.get(args[1] as usize)?;
            if left.shape.len() != 1
                || left.shape != right.shape
                || left.dtype != TensorDType::Float
                || right.dtype != TensorDType::Float
            {
                return None;
            }
            let left_values = tensor_values_as_f64(left);
            let right_values = tensor_values_as_f64(right);
            let value = left_values
                .iter()
                .zip(right_values.iter())
                .map(|(a, b)| a * b)
                .sum::<f64>();
            let requires_grad =
                tensor_requires_autograd(registry, &[args[0] as usize, args[1] as usize]);
            let creator = requires_grad.then(|| AutogradNode {
                op: AutogradOp::DotTensor,
                parents: vec![args[0] as usize, args[1] as usize],
                input_shape: vec![1],
                left_shape: left.shape.clone(),
                right_shape: right.shape.clone(),
                input: Vec::new(),
                output: vec![value],
                left: left_values,
                right: right_values,
                aux: Vec::new(),
            });
            registry.note_kernel(left.len());
            Some((value, requires_grad, creator))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc_autograd(
            TensorDType::Float,
            vec![1],
            vec![value.to_bits() as SpectraHostValue],
            requires_grad,
            creator,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_neg(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_unary(ctx, AutogradOp::Neg, |v| v.saturating_neg(), |v| -v)
}

extern "C" fn std_tensor_exp_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_float_unary(ctx, AutogradOp::Exp, f64::exp)
}

extern "C" fn std_tensor_log_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_float_unary(ctx, AutogradOp::Log, f64::ln)
}

extern "C" fn std_tensor_sqrt_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_float_unary(ctx, AutogradOp::Sqrt, f64::sqrt)
}

extern "C" fn std_tensor_relu(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_unary(ctx, AutogradOp::Relu, |v| v.max(0), |v| v.max(0.0))
}

extern "C" fn std_tensor_sigmoid_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_float_unary(ctx, AutogradOp::Sigmoid, |v| 1.0 / (1.0 + (-v).exp()))
}

extern "C" fn std_tensor_tanh_f(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_float_unary(ctx, AutogradOp::Tanh, f64::tanh)
}

fn tensor_unary(
    ctx: *mut SpectraHostCallContext,
    op: AutogradOp,
    int_op: impl Fn(i64) -> i64,
    float_op: impl Fn(f64) -> f64,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((dtype, shape, data, requires_grad, creator, device, precision)) =
            with_tensor_registry(|registry| {
                let tensor = registry.get(args[0] as usize)?.clone();
                let element_count = tensor.len();
                let source = tensor.materialize();
                #[cfg(feature = "gpu")]
                let gpu_data = if tensor.device == TensorDevice::Wgpu {
                    let gpu_op = match op {
                        AutogradOp::Neg => Some(crate::gpu::GpuUnaryOp::Neg),
                        AutogradOp::Relu => Some(crate::gpu::GpuUnaryOp::Relu),
                        _ => None,
                    }?;
                    match gpu_unary_float(&tensor, gpu_op) {
                        Ok(Some(data)) => {
                            registry.note_gpu_kernel();
                            Some(data)
                        }
                        Ok(None) => None,
                        Err(()) => {
                            registry.note_cpu_fallback();
                            None
                        }
                    }
                } else {
                    None
                };
                let data: Vec<SpectraHostValue> = match tensor.dtype {
                    TensorDType::Int => {
                        if tensor.device.is_accelerator() {
                            return None;
                        }
                        source.iter().map(|value| int_op(*value)).collect()
                    }
                    TensorDType::Float => {
                        #[cfg(feature = "gpu")]
                        if let Some(data) = gpu_data {
                            data
                        } else {
                            source
                                .iter()
                                .map(|bits| float_op(f64::from_bits(*bits as u64)).to_bits() as i64)
                                .collect()
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            if tensor.device.is_accelerator() {
                                return None;
                            }
                            source
                                .iter()
                                .map(|bits| float_op(f64::from_bits(*bits as u64)).to_bits() as i64)
                                .collect()
                        }
                    }
                };
                let requires_grad = tensor.dtype == TensorDType::Float
                    && tensor_requires_autograd(registry, &[args[0] as usize]);
                let creator = requires_grad.then(|| {
                    AutogradNode::unary(
                        op,
                        args[0] as usize,
                        tensor.shape.clone(),
                        source
                            .iter()
                            .map(|raw| f64::from_bits(*raw as u64))
                            .collect(),
                        data.iter().map(|raw| f64::from_bits(*raw as u64)).collect(),
                    )
                });
                let result = Some((
                    tensor.dtype,
                    tensor.shape.clone(),
                    data,
                    requires_grad,
                    creator,
                    tensor.device,
                    tensor.precision,
                ));
                registry.note_kernel(element_count);
                result
            })
        else {
            return HOST_STATUS_NOT_FOUND;
        };
        match tensor_alloc_autograd_on_device(
            dtype,
            shape,
            data,
            requires_grad,
            creator,
            device,
            precision,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

fn tensor_float_unary(
    ctx: *mut SpectraHostCallContext,
    autograd_op: AutogradOp,
    op: impl Fn(f64) -> f64,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, data, requires_grad, creator)) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            let element_count = tensor.len();
            let source = tensor.materialize();
            let data: Vec<SpectraHostValue> = source
                .iter()
                .map(|bits| {
                    let value = match tensor.dtype {
                        TensorDType::Int => *bits as f64,
                        TensorDType::Float => f64::from_bits(*bits as u64),
                    };
                    op(value).to_bits() as i64
                })
                .collect();
            let input = tensor_values_as_f64(tensor);
            let output = data
                .iter()
                .map(|raw| f64::from_bits(*raw as u64))
                .collect::<Vec<_>>();
            let requires_grad = tensor_requires_autograd(registry, &[args[0] as usize]);
            let creator = requires_grad.then(|| {
                AutogradNode::unary(
                    autograd_op,
                    args[0] as usize,
                    tensor.shape.clone(),
                    input,
                    output,
                )
            });
            let result = Some((tensor.shape.clone(), data, requires_grad, creator));
            registry.note_kernel(element_count);
            result
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        match tensor_alloc_autograd(TensorDType::Float, shape, data, requires_grad, creator) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_matmul(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((dtype, shape, data, requires_grad, creator, device, precision)) =
            with_tensor_registry(|registry| {
                let a = registry.get(args[0] as usize)?.clone();
                let b = registry.get(args[1] as usize)?.clone();
                if a.shape.len() != 2 || b.shape.len() != 2 || a.dtype != b.dtype {
                    return None;
                }
                if a.device != b.device {
                    return None;
                }
                let (m, k) = (a.shape[0], a.shape[1]);
                let (bk, n) = (b.shape[0], b.shape[1]);
                if k != bk {
                    return None;
                }
                let element_count = m.saturating_mul(n).saturating_mul(k);
                let a_data = a.materialize();
                let b_data = b.materialize();
                let out = match a.dtype {
                    TensorDType::Int => {
                        if a.device.is_accelerator() {
                            return None;
                        }
                        kernel_matmul_i64(&a_data, &b_data, m, k, n)
                    }
                    TensorDType::Float => {
                        #[cfg(feature = "gpu")]
                        if a.device == TensorDevice::Wgpu {
                            let a_gpu = tensor_values_as_f32(&a)?;
                            let b_gpu = tensor_values_as_f32(&b)?;
                            match crate::gpu::matmul(&a_gpu, &b_gpu, m, k, n) {
                                Ok(values) => {
                                    registry.note_gpu_kernel();
                                    f32_values_to_host(&values)
                                }
                                Err(_) => {
                                    registry.note_cpu_fallback();
                                    kernel_matmul_f64_bits(&a_data, &b_data, m, k, n)
                                }
                            }
                        } else {
                            kernel_matmul_f64_bits(&a_data, &b_data, m, k, n)
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            if a.device.is_accelerator() {
                                return None;
                            }
                            kernel_matmul_f64_bits(&a_data, &b_data, m, k, n)
                        }
                    }
                };
                let requires_grad = a.dtype == TensorDType::Float
                    && tensor_requires_autograd(registry, &[args[0] as usize, args[1] as usize]);
                let creator = requires_grad.then(|| AutogradNode {
                    op: AutogradOp::Matmul,
                    parents: vec![args[0] as usize, args[1] as usize],
                    input_shape: Vec::new(),
                    left_shape: a.shape.clone(),
                    right_shape: b.shape.clone(),
                    input: Vec::new(),
                    output: out.iter().map(|raw| f64::from_bits(*raw as u64)).collect(),
                    left: a_data
                        .iter()
                        .map(|raw| f64::from_bits(*raw as u64))
                        .collect(),
                    right: b_data
                        .iter()
                        .map(|raw| f64::from_bits(*raw as u64))
                        .collect(),
                    aux: Vec::new(),
                });
                let result = Some((
                    a.dtype,
                    vec![m, n],
                    out,
                    requires_grad,
                    creator,
                    a.device,
                    a.precision,
                ));
                registry.note_scratch_reuse();
                registry.note_kernel(element_count);
                result
            })
        else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc_autograd_on_device(
            dtype,
            shape,
            data,
            requires_grad,
            creator,
            device,
            precision,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_matmul_batched(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((dtype, shape, data)) = with_tensor_registry(|registry| {
            let a = registry.get(args[0] as usize)?;
            let b = registry.get(args[1] as usize)?;
            if a.shape.len() != 3 || b.shape.len() != 3 || a.dtype != b.dtype {
                return None;
            }
            let (batch, m, k) = (a.shape[0], a.shape[1], a.shape[2]);
            let (bbatch, bk, n) = (b.shape[0], b.shape[1], b.shape[2]);
            if batch != bbatch || k != bk {
                return None;
            }
            let dtype = a.dtype;
            let a_data = a.materialize();
            let b_data = b.materialize();
            let mut out = Vec::with_capacity(batch * m * n);
            for batch_index in 0..batch {
                let a_start = batch_index * m * k;
                let b_start = batch_index * k * n;
                let batch_out = match dtype {
                    TensorDType::Int => kernel_matmul_i64(
                        &a_data[a_start..a_start + m * k],
                        &b_data[b_start..b_start + k * n],
                        m,
                        k,
                        n,
                    ),
                    TensorDType::Float => kernel_matmul_f64_bits(
                        &a_data[a_start..a_start + m * k],
                        &b_data[b_start..b_start + k * n],
                        m,
                        k,
                        n,
                    ),
                };
                out.extend(batch_out);
            }
            registry.note_scratch_reuse();
            registry.note_kernel(batch.saturating_mul(m).saturating_mul(n).saturating_mul(k));
            Some((dtype, vec![batch, m, n], out))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc(dtype, shape, data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

fn matmul_f64(left: &[f64], right: &[f64], m: usize, k: usize, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; m * n];
    for row in 0..m {
        for col in 0..n {
            let mut acc = 0.0;
            for inner in 0..k {
                acc += left[row * k + inner] * right[inner * n + col];
            }
            out[row * n + col] = acc;
        }
    }
    out
}

fn transpose_f64(data: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    let mut out = vec![0.0; data.len()];
    for row in 0..rows {
        for col in 0..cols {
            out[col * rows + row] = data[row * cols + col];
        }
    }
    out
}

fn accumulate_tensor_grad(tensor: &mut StdTensor, grad: &[f64]) -> bool {
    if tensor.dtype != TensorDType::Float || tensor.len() != grad.len() {
        return false;
    }
    let target = tensor.grad.get_or_insert_with(|| vec![0.0; grad.len()]);
    for (slot, value) in target.iter_mut().zip(grad.iter()) {
        *slot += *value;
    }
    true
}

fn autograd_parent_grads(node: &AutogradNode, grad: &[f64]) -> Option<Vec<(usize, Vec<f64>)>> {
    match node.op {
        AutogradOp::Add => Some(vec![
            (node.parents[0], grad.to_vec()),
            (node.parents[1], grad.to_vec()),
        ]),
        AutogradOp::Sub => Some(vec![
            (node.parents[0], grad.to_vec()),
            (node.parents[1], grad.iter().map(|v| -*v).collect()),
        ]),
        AutogradOp::Mul => Some(vec![
            (
                node.parents[0],
                grad.iter()
                    .zip(node.right.iter())
                    .map(|(g, r)| g * r)
                    .collect(),
            ),
            (
                node.parents[1],
                grad.iter()
                    .zip(node.left.iter())
                    .map(|(g, l)| g * l)
                    .collect(),
            ),
        ]),
        AutogradOp::Div => Some(vec![
            (
                node.parents[0],
                grad.iter()
                    .zip(node.right.iter())
                    .map(|(g, r)| g / r)
                    .collect(),
            ),
            (
                node.parents[1],
                grad.iter()
                    .zip(node.left.iter().zip(node.right.iter()))
                    .map(|(g, (l, r))| -(g * l) / (r * r))
                    .collect(),
            ),
        ]),
        AutogradOp::Neg => Some(vec![(node.parents[0], grad.iter().map(|v| -*v).collect())]),
        AutogradOp::Relu => Some(vec![(
            node.parents[0],
            grad.iter()
                .zip(node.input.iter())
                .map(|(g, x)| if *x > 0.0 { *g } else { 0.0 })
                .collect(),
        )]),
        AutogradOp::Exp => Some(vec![(
            node.parents[0],
            grad.iter()
                .zip(node.output.iter())
                .map(|(g, y)| g * y)
                .collect(),
        )]),
        AutogradOp::Log => Some(vec![(
            node.parents[0],
            grad.iter()
                .zip(node.input.iter())
                .map(|(g, x)| g / x)
                .collect(),
        )]),
        AutogradOp::Sqrt => Some(vec![(
            node.parents[0],
            grad.iter()
                .zip(node.output.iter())
                .map(|(g, y)| g * 0.5 / y)
                .collect(),
        )]),
        AutogradOp::Sigmoid => Some(vec![(
            node.parents[0],
            grad.iter()
                .zip(node.output.iter())
                .map(|(g, y)| g * y * (1.0 - y))
                .collect(),
        )]),
        AutogradOp::Tanh => Some(vec![(
            node.parents[0],
            grad.iter()
                .zip(node.output.iter())
                .map(|(g, y)| g * (1.0 - y * y))
                .collect(),
        )]),
        AutogradOp::SumTensor => Some(vec![(node.parents[0], vec![grad[0]; node.input.len()])]),
        AutogradOp::MeanTensor => Some(vec![(
            node.parents[0],
            vec![grad[0] / node.input.len() as f64; node.input.len()],
        )]),
        AutogradOp::Transpose => {
            let rows = node.input_shape[0];
            let cols = node.input_shape[1];
            Some(vec![(node.parents[0], transpose_f64(grad, cols, rows))])
        }
        AutogradOp::View => Some(vec![(node.parents[0], grad.to_vec())]),
        AutogradOp::DotTensor => Some(vec![
            (
                node.parents[0],
                node.right.iter().map(|value| grad[0] * value).collect(),
            ),
            (
                node.parents[1],
                node.left.iter().map(|value| grad[0] * value).collect(),
            ),
        ]),
        AutogradOp::Matmul => {
            let (m, k) = (node.left_shape[0], node.left_shape[1]);
            let n = node.right_shape[1];
            let right_t = transpose_f64(&node.right, k, n);
            let left_t = transpose_f64(&node.left, m, k);
            Some(vec![
                (node.parents[0], matmul_f64(grad, &right_t, m, n, k)),
                (node.parents[1], matmul_f64(&left_t, grad, k, m, n)),
            ])
        }
        AutogradOp::MlLinear => {
            let (batch, in_features, out_features) = (node.aux[0], node.aux[1], node.aux[2]);
            let weight_t = transpose_f64(&node.right, in_features, out_features);
            let input_t = transpose_f64(&node.left, batch, in_features);
            let grad_input = matmul_f64(grad, &weight_t, batch, out_features, in_features);
            let grad_weight = matmul_f64(&input_t, grad, in_features, batch, out_features);
            let mut grad_bias = vec![0.0; out_features];
            for row in 0..batch {
                for col in 0..out_features {
                    grad_bias[col] += grad[row * out_features + col];
                }
            }
            Some(vec![
                (node.parents[0], grad_input),
                (node.parents[1], grad_weight),
                (node.parents[2], grad_bias),
            ])
        }
        AutogradOp::MlMse => {
            let n = node.left.len() as f64;
            Some(vec![(
                node.parents[0],
                node.left
                    .iter()
                    .zip(node.right.iter())
                    .map(|(p, t)| grad[0] * 2.0 * (p - t) / n)
                    .collect(),
            )])
        }
        AutogradOp::MlBce => {
            let n = node.left.len() as f64;
            Some(vec![(
                node.parents[0],
                node.left
                    .iter()
                    .zip(node.right.iter())
                    .map(|(p, t)| {
                        let p = p.clamp(1e-7, 1.0 - 1e-7);
                        grad[0] * (p - t) / (p * (1.0 - p) * n)
                    })
                    .collect(),
            )])
        }
        AutogradOp::MlCrossEntropy | AutogradOp::MlNll => {
            let batch = node.aux[0];
            Some(vec![(
                node.parents[0],
                node.output
                    .iter()
                    .map(|v| grad[0] * v / batch as f64)
                    .collect(),
            )])
        }
        AutogradOp::MlConv2d => {
            let (batch, in_ch, h, w, out_ch, kh, kw, out_h, out_w) = (
                node.aux[0],
                node.aux[1],
                node.aux[2],
                node.aux[3],
                node.aux[4],
                node.aux[5],
                node.aux[6],
                node.aux[7],
                node.aux[8],
            );
            let mut grad_input = vec![0.0; batch * in_ch * h * w];
            let mut grad_kernel = vec![0.0; out_ch * in_ch * kh * kw];
            let mut grad_bias = vec![0.0; out_ch];
            for n in 0..batch {
                for oc in 0..out_ch {
                    for oy in 0..out_h {
                        for ox in 0..out_w {
                            let g = grad[((n * out_ch + oc) * out_h + oy) * out_w + ox];
                            grad_bias[oc] += g;
                            for ic in 0..in_ch {
                                for ky in 0..kh {
                                    for kx in 0..kw {
                                        let iy = oy + ky;
                                        let ix = ox + kx;
                                        let input_idx = ((n * in_ch + ic) * h + iy) * w + ix;
                                        let kernel_idx = ((oc * in_ch + ic) * kh + ky) * kw + kx;
                                        grad_input[input_idx] += g * node.right[kernel_idx];
                                        grad_kernel[kernel_idx] += g * node.left[input_idx];
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(vec![
                (node.parents[0], grad_input),
                (node.parents[1], grad_kernel),
                (node.parents[2], grad_bias),
            ])
        }
    }
}

fn tensor_backward_impl(loss_handle: usize) -> Result<(), i32> {
    let mut stack = with_tensor_registry(|registry| {
        let loss = registry.get(loss_handle).ok_or(HOST_STATUS_NOT_FOUND)?;
        if loss.dtype != TensorDType::Float || loss.len() != 1 {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        Ok::<_, i32>(vec![(loss_handle, vec![1.0])])
    })?;
    let mut visited = Vec::new();

    while let Some((handle, grad)) = stack.pop() {
        let next = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(handle) else {
                return Ok::<Vec<(usize, Vec<f64>)>, i32>(Vec::new());
            };
            if !accumulate_tensor_grad(tensor, &grad) {
                return Err(HOST_STATUS_INVALID_ARGUMENT);
            }
            visited.push(handle);
            let Some(node) = tensor.creator.clone() else {
                return Ok(Vec::new());
            };
            Ok(autograd_parent_grads(&node, &grad).unwrap_or_default())
        })?;
        stack.extend(next);
    }

    with_tensor_registry(|registry| {
        for handle in visited {
            if let Some(tensor) = registry.get_mut(handle) {
                tensor.creator = None;
            }
        }
    });
    Ok(())
}

extern "C" fn std_tensor_requires_grad(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(args[0] as usize) else {
                return false;
            };
            if tensor.dtype != TensorDType::Float {
                return false;
            }
            tensor.requires_grad = args[1] != 0;
            if !tensor.requires_grad {
                tensor.grad = None;
                tensor.creator = None;
            }
            true
        });
        if !ok {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        tensor_result(ctx_ref, args[0])
    }
}

extern "C" fn std_tensor_backward(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_backward_impl(args[0] as usize) {
            Ok(()) => tensor_optional_result(ctx_ref, 0),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_grad(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, grad)) = with_tensor_registry(|registry| {
            let tensor = registry.get(args[0] as usize)?;
            Some((tensor.shape.clone(), tensor.grad.clone()?))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        match tensor_alloc(TensorDType::Float, shape, f64_values_to_host(&grad)) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_zero_grad(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(args[0] as usize) else {
                return false;
            };
            tensor.grad = None;
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_set_grad_enabled(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        *lock_unpoisoned(tensor_grad_enabled()) = args[0] != 0;
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_grad_enabled(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, tensor_is_grad_enabled() as SpectraHostValue)
    }
}

// ── std.ml runtime ──────────────────────────────────────────────────────────

#[derive(Default)]
struct MlModule {
    parameters: Vec<usize>,
    training: bool,
}

#[derive(Clone, Copy)]
struct MlDataset {
    features: usize,
    labels: usize,
    len: usize,
}

#[derive(Clone, Copy)]
struct MlDataLoader {
    dataset: usize,
    batch_size: usize,
    shuffle_seed: u64,
}

#[derive(Clone)]
struct MlDataFrame {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

#[derive(Clone)]
struct MlMetricRecord {
    name: String,
    value: f64,
    step: i64,
}

#[derive(Clone)]
struct MlArtifactRecord {
    path: String,
    size: u64,
    fnv64: String,
}

#[derive(Clone)]
struct MlExperiment {
    name: String,
    out_dir: String,
    seed: i64,
    configs: Vec<(String, String)>,
    metrics: Vec<MlMetricRecord>,
    artifacts: Vec<MlArtifactRecord>,
    lockfile: Option<MlArtifactRecord>,
    model_output: Option<MlArtifactRecord>,
    manifest_path: String,
    reproduction_command: String,
    finished: bool,
}

#[derive(Clone)]
struct MlDistributedWorker {
    worker_id: usize,
    step_count: i64,
    sample_count: i64,
    accumulator: f64,
    active: bool,
}

#[derive(Clone)]
struct MlDistributedSession {
    name: String,
    out_dir: String,
    worker_count: usize,
    seed: i64,
    global_step: i64,
    interrupted_worker: Option<usize>,
    workers: Vec<MlDistributedWorker>,
    last_checkpoint_path: Option<String>,
}

#[derive(Clone)]
struct MlKvCache {
    max_tokens: usize,
    dim: usize,
    keys: Vec<f64>,
    values: Vec<f64>,
}

#[derive(Clone)]
struct MlWordpieceTokenizer {
    token_to_id: HashMap<String, i64>,
    id_to_token: HashMap<i64, String>,
    unk_id: i64,
    max_token_chars: usize,
}

#[derive(Clone)]
struct MlVectorIndexEntry {
    id: String,
    vector: Vec<f64>,
}

#[derive(Clone)]
struct MlVectorIndex {
    dim: usize,
    entries: Vec<MlVectorIndexEntry>,
}

struct MlRegistry {
    next_id: usize,
    modules: HashMap<usize, MlModule>,
    datasets: HashMap<usize, MlDataset>,
    loaders: HashMap<usize, MlDataLoader>,
    dataframes: HashMap<usize, MlDataFrame>,
    experiments: HashMap<usize, MlExperiment>,
    distributed_sessions: HashMap<usize, MlDistributedSession>,
    kv_caches: HashMap<usize, MlKvCache>,
    tokenizers: HashMap<usize, MlWordpieceTokenizer>,
    vector_indexes: HashMap<usize, MlVectorIndex>,
}

impl MlRegistry {
    fn new() -> Self {
        Self {
            next_id: 1,
            modules: HashMap::new(),
            datasets: HashMap::new(),
            loaders: HashMap::new(),
            dataframes: HashMap::new(),
            experiments: HashMap::new(),
            distributed_sessions: HashMap::new(),
            kv_caches: HashMap::new(),
            tokenizers: HashMap::new(),
            vector_indexes: HashMap::new(),
        }
    }

    fn next_handle(&mut self) -> usize {
        let handle = self.next_id.max(1);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        handle
    }
}

fn ml_registry() -> &'static Mutex<MlRegistry> {
    static REGISTRY: OnceLock<Mutex<MlRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(MlRegistry::new()))
}

fn with_ml_registry<F, R>(action: F) -> R
where
    F: FnOnce(&mut MlRegistry) -> R,
{
    let mut guard = lock_unpoisoned(ml_registry());
    action(&mut guard)
}

unsafe fn ml_args<'a>(
    ctx: *mut SpectraHostCallContext,
    expected: usize,
) -> Result<(&'a mut SpectraHostCallContext, &'a [SpectraHostValue]), i32> {
    tensor_args(ctx, expected)
}

fn ml_tensor_float_data(handle: usize) -> Option<(Vec<usize>, Vec<f64>, bool)> {
    with_tensor_registry(|registry| {
        let tensor = registry.get(handle)?;
        if tensor.dtype != TensorDType::Float {
            return None;
        }
        Some((
            tensor.shape.clone(),
            tensor_values_as_f64(tensor),
            tensor.requires_grad,
        ))
    })
}

fn ml_tensor_int_data(handle: usize) -> Option<Vec<i64>> {
    with_tensor_registry(|registry| {
        let tensor = registry.get(handle)?;
        Some(tensor.materialize())
    })
}

fn ml_store_float_tensor(handle: usize, values: Vec<f64>) -> bool {
    with_tensor_registry(|registry| {
        let Some(tensor) = registry.get_mut(handle) else {
            return false;
        };
        if tensor.dtype != TensorDType::Float || tensor.len() != values.len() {
            return false;
        }
        tensor.storage = Arc::new(f64_values_to_host(&values));
        tensor.offset = 0;
        tensor.layout = TensorLayout::Contiguous;
        tensor.strides = tensor_strides(&tensor.shape);
        true
    })
}

fn ml_alloc_float_tensor(shape: Vec<usize>, values: Vec<f64>) -> Result<usize, i32> {
    if shape.iter().product::<usize>() != values.len() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    tensor_alloc(TensorDType::Float, shape, f64_values_to_host(&values))
}

fn ml_sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        let z = (-value).exp();
        1.0 / (1.0 + z)
    } else {
        let z = value.exp();
        z / (1.0 + z)
    }
}

fn ml_softmax_row(values: &[f64]) -> Option<Vec<f64>> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exp = values
        .iter()
        .map(|value| (value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return None;
    }
    Some(exp.into_iter().map(|value| value / sum).collect())
}

fn ml_parse_wordpiece_vocab(spec: &str) -> Option<MlWordpieceTokenizer> {
    let mut token_to_id = HashMap::new();
    let mut id_to_token = HashMap::new();
    let mut next_id = 0i64;
    for raw_line in spec.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let (token, id) = if let Some((token, id)) = line.split_once(':') {
            (token.trim().to_string(), id.trim().parse::<i64>().ok()?)
        } else {
            let id = next_id;
            (line.to_string(), id)
        };
        if token.is_empty() || token_to_id.contains_key(&token) || id_to_token.contains_key(&id) {
            return None;
        }
        next_id = next_id.max(id + 1);
        token_to_id.insert(token.clone(), id);
        id_to_token.insert(id, token);
    }
    if token_to_id.is_empty() {
        return None;
    }
    let unk_id = *token_to_id
        .get("[UNK]")
        .or_else(|| token_to_id.get("<unk>"))
        .unwrap_or(&0);
    let max_token_chars = token_to_id
        .keys()
        .map(|token| token.len())
        .max()
        .unwrap_or(1);
    Some(MlWordpieceTokenizer {
        token_to_id,
        id_to_token,
        unk_id,
        max_token_chars,
    })
}

fn ml_wordpiece_encode(tokenizer: &MlWordpieceTokenizer, text: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    for word in text.split_whitespace() {
        let normalized = word
            .trim_matches(|ch: char| ch.is_ascii_punctuation())
            .to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        let chars = normalized.chars().collect::<Vec<_>>();
        let mut start = 0usize;
        let mut word_ids = Vec::new();
        let mut failed = false;
        while start < chars.len() {
            let mut end = chars.len().min(start + tokenizer.max_token_chars);
            let mut found = None;
            while end > start {
                let piece = chars[start..end].iter().collect::<String>();
                let candidate = if start == 0 {
                    piece
                } else {
                    format!("##{}", piece)
                };
                if let Some(id) = tokenizer.token_to_id.get(&candidate) {
                    found = Some((*id, end));
                    break;
                }
                end -= 1;
            }
            if let Some((id, next)) = found {
                word_ids.push(id);
                start = next;
            } else {
                failed = true;
                break;
            }
        }
        if failed || word_ids.is_empty() {
            ids.push(tokenizer.unk_id);
        } else {
            ids.extend(word_ids);
        }
    }
    ids
}

fn ml_wordpiece_decode(tokenizer: &MlWordpieceTokenizer, ids: &[i64]) -> String {
    let mut words = Vec::<String>::new();
    for id in ids {
        let token = tokenizer
            .id_to_token
            .get(id)
            .cloned()
            .unwrap_or_else(|| "[UNK]".to_string());
        if let Some(piece) = token.strip_prefix("##") {
            if let Some(last) = words.last_mut() {
                last.push_str(piece);
            } else {
                words.push(piece.to_string());
            }
        } else if token == "[PAD]" {
            continue;
        } else {
            words.push(token);
        }
    }
    words.join(" ")
}

fn ml_hash_text_to_embedding(text: &str, dim: usize) -> Option<Vec<f64>> {
    if dim == 0 {
        return None;
    }
    let mut values = vec![0.0f64; dim];
    for token in text.split_whitespace() {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in token.to_ascii_lowercase().as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let idx = (hash as usize) % dim;
        let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
        values[idx] += sign;
    }
    let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm > 0.0 {
        for value in &mut values {
            *value /= norm;
        }
    }
    Some(values)
}

fn ml_cosine_similarity(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let dot = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| a * b)
        .sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        return Some(0.0);
    }
    Some(dot / (left_norm * right_norm))
}

fn ml_vector_index_json(index: &MlVectorIndex) -> String {
    let entries = index
        .entries
        .iter()
        .map(|entry| {
            let values = entry
                .vector
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"id\":{},\"vector\":[{}]}}",
                ml_json_string(&entry.id),
                values
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"spectra.ml.vector_index.v1\",\"dim\":{},\"entries\":[{}]}}",
        index.dim, entries
    )
}

fn ml_json_array_section(source: &str, key: &str) -> Option<String> {
    ml_manifest_section(source, key)
}

fn ml_vector_index_from_json(source: &str) -> Option<MlVectorIndex> {
    if !source.contains("\"schema\":\"spectra.ml.vector_index.v1\"") {
        return None;
    }
    let dim = ml_checkpoint_number(source, "\"dim\"")? as usize;
    let entries_section = ml_json_array_section(source, "\"entries\"")?;
    let mut entries = Vec::new();
    let mut offset = 0usize;
    while let Some(relative_start) = entries_section[offset..].find('{') {
        let start = offset + relative_start;
        let end = entries_section[start..].find('}')? + start;
        let item = &entries_section[start..=end];
        let id = ml_checkpoint_string(item, "\"id\"")?;
        let vector_section = ml_manifest_section(item, "\"vector\"")?;
        let vector = vector_section
            .trim_matches(|ch| ch == '[' || ch == ']')
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .map(|part| part.trim().parse::<f64>().ok())
            .collect::<Option<Vec<_>>>()?;
        if vector.len() != dim {
            return None;
        }
        entries.push(MlVectorIndexEntry { id, vector });
        offset = end + 1;
    }
    Some(MlVectorIndex { dim, entries })
}

fn ml_token_set(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|token| {
            token
                .trim_matches(|ch: char| ch.is_ascii_punctuation())
                .to_ascii_lowercase()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

fn ml_f1_overlap(answer: &str, expected: &str) -> f64 {
    let answer_tokens = ml_token_set(answer);
    let expected_tokens = ml_token_set(expected);
    if answer_tokens.is_empty() || expected_tokens.is_empty() {
        return 0.0;
    }
    let overlap = answer_tokens.intersection(&expected_tokens).count() as f64;
    if overlap == 0.0 {
        return 0.0;
    }
    let precision = overlap / answer_tokens.len() as f64;
    let recall = overlap / expected_tokens.len() as f64;
    2.0 * precision * recall / (precision + recall)
}

fn ml_metrics_json(kind: &str, fields: &[(&str, String)]) -> String {
    let fields = fields
        .iter()
        .map(|(key, value)| format!("\"{}\":{}", key, value))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"spectra.ml.metric.v1\",\"kind\":\"{}\",{}}}",
        kind, fields
    )
}

fn ml_float_json(value: f64) -> String {
    if value.is_finite() {
        format!("{:.6}", value)
    } else {
        "null".to_string()
    }
}

fn ml_json_payload_arg(value: SpectraHostValue) -> Option<String> {
    let payload = ml_read_path_arg(value)?;
    let trimmed = payload.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        Some(trimmed.to_string())
    } else {
        Some(ml_json_string(trimmed))
    }
}

fn ml_loss_tensor(
    ctx_ref: &mut SpectraHostCallContext,
    value: f64,
    requires_grad: bool,
    creator: Option<AutogradNode>,
) -> i32 {
    match tensor_alloc_autograd(
        TensorDType::Float,
        vec![1],
        vec![value.to_bits() as SpectraHostValue],
        requires_grad,
        creator,
    ) {
        Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
        Err(code) => code,
    }
}

extern "C" fn std_ml_module_new(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = ml_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.modules.insert(
                handle,
                MlModule {
                    parameters: Vec::new(),
                    training: true,
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_module_add_parameter(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_ml_registry(|registry| {
            let Some(module) = registry.modules.get_mut(&(args[0] as usize)) else {
                return false;
            };
            module.parameters.push(args[1] as usize);
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_module_parameter_count(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(count) = with_ml_registry(|registry| {
            registry
                .modules
                .get(&(args[0] as usize))
                .map(|module| module.parameters.len())
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, count as SpectraHostValue)
    }
}

extern "C" fn std_ml_module_parameter(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(param) = with_ml_registry(|registry| {
            registry
                .modules
                .get(&(args[0] as usize))
                .and_then(|module| module.parameters.get(args[1] as usize).copied())
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, param as SpectraHostValue)
    }
}

extern "C" fn std_ml_module_set_training(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_ml_registry(|registry| {
            let Some(module) = registry.modules.get_mut(&(args[0] as usize)) else {
                return false;
            };
            module.training = args[1] != 0;
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_module_is_training(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(training) = with_ml_registry(|registry| {
            registry
                .modules
                .get(&(args[0] as usize))
                .map(|module| module.training)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, training as SpectraHostValue)
    }
}

extern "C" fn std_ml_linear(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (input_h, weight_h, bias_h) = (args[0] as usize, args[1] as usize, args[2] as usize);
        let Some((shape, out, requires_grad, creator)) = with_tensor_registry(|registry| {
            let input = registry.get(input_h)?;
            let weight = registry.get(weight_h)?;
            let bias = registry.get(bias_h)?;
            if input.dtype != TensorDType::Float
                || weight.dtype != TensorDType::Float
                || bias.dtype != TensorDType::Float
                || input.shape.len() != 2
                || weight.shape.len() != 2
                || bias.shape.len() != 1
            {
                return None;
            }
            let (batch, in_features) = (input.shape[0], input.shape[1]);
            let (w_in, out_features) = (weight.shape[0], weight.shape[1]);
            if in_features != w_in || bias.shape[0] != out_features {
                return None;
            }
            let x = tensor_values_as_f64(input);
            let w = tensor_values_as_f64(weight);
            let b = tensor_values_as_f64(bias);
            let mut out = matmul_f64(&x, &w, batch, in_features, out_features);
            for row in 0..batch {
                for col in 0..out_features {
                    out[row * out_features + col] += b[col];
                }
            }
            let requires_grad = tensor_requires_autograd(registry, &[input_h, weight_h, bias_h]);
            let creator = requires_grad.then(|| AutogradNode {
                op: AutogradOp::MlLinear,
                parents: vec![input_h, weight_h, bias_h],
                input_shape: input.shape.clone(),
                left_shape: input.shape.clone(),
                right_shape: weight.shape.clone(),
                input: b,
                output: out.clone(),
                left: x,
                right: w,
                aux: vec![batch, in_features, out_features],
            });
            registry.note_kernel(batch * in_features * out_features);
            Some((vec![batch, out_features], out, requires_grad, creator))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc_autograd(
            TensorDType::Float,
            shape,
            f64_values_to_host(&out),
            requires_grad,
            creator,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_conv2d(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 10) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (input_h, kernel_h, bias_h) = (args[0] as usize, args[1] as usize, args[2] as usize);
        let dims = [
            args[3] as usize,
            args[4] as usize,
            args[5] as usize,
            args[6] as usize,
            args[7] as usize,
            args[8] as usize,
            args[9] as usize,
        ];
        if args[3..].iter().any(|v| *v <= 0) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let (batch, in_ch, h, w, out_ch, kh, kw) = (
            dims[0], dims[1], dims[2], dims[3], dims[4], dims[5], dims[6],
        );
        if h < kh || w < kw {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some((out, requires_grad, creator, device)) = with_tensor_registry(|registry| {
            let input = registry.get(input_h)?.clone();
            let kernel = registry.get(kernel_h)?.clone();
            let bias = registry.get(bias_h)?.clone();
            if input.dtype != TensorDType::Float
                || kernel.dtype != TensorDType::Float
                || bias.dtype != TensorDType::Float
                || input.device != kernel.device
                || input.device != bias.device
                || input.len() != batch * in_ch * h * w
                || kernel.len() != out_ch * in_ch * kh * kw
                || bias.len() != out_ch
            {
                return None;
            }
            let x = tensor_values_as_f64(&input);
            let k = tensor_values_as_f64(&kernel);
            let b = tensor_values_as_f64(&bias);
            let device = input.device;
            let (out_h, out_w) = (h - kh + 1, w - kw + 1);
            let cpu_conv2d = || {
                let mut out = vec![0.0; batch * out_ch * out_h * out_w];
                for n in 0..batch {
                    for oc in 0..out_ch {
                        for oy in 0..out_h {
                            for ox in 0..out_w {
                                let mut acc = b[oc];
                                for ic in 0..in_ch {
                                    for ky in 0..kh {
                                        for kx in 0..kw {
                                            let input_idx =
                                                ((n * in_ch + ic) * h + oy + ky) * w + ox + kx;
                                            let kernel_idx =
                                                ((oc * in_ch + ic) * kh + ky) * kw + kx;
                                            acc += x[input_idx] * k[kernel_idx];
                                        }
                                    }
                                }
                                out[((n * out_ch + oc) * out_h + oy) * out_w + ox] = acc;
                            }
                        }
                    }
                }
                out
            };
            #[cfg(feature = "gpu")]
            let out = if input.device == TensorDevice::Wgpu {
                let x_gpu = tensor_values_as_f32(&input)?;
                let k_gpu = tensor_values_as_f32(&kernel)?;
                let b_gpu = tensor_values_as_f32(&bias)?;
                match crate::gpu::conv2d(&x_gpu, &k_gpu, &b_gpu, dims) {
                    Ok(values) => {
                        registry.note_gpu_kernel();
                        values.iter().map(|value| *value as f64).collect::<Vec<_>>()
                    }
                    Err(_) => {
                        registry.note_cpu_fallback();
                        cpu_conv2d()
                    }
                }
            } else {
                cpu_conv2d()
            };
            #[cfg(not(feature = "gpu"))]
            let out = {
                if input.device.is_accelerator() {
                    return None;
                }
                cpu_conv2d()
            };
            let requires_grad = tensor_requires_autograd(registry, &[input_h, kernel_h, bias_h]);
            let creator = requires_grad.then(|| AutogradNode {
                op: AutogradOp::MlConv2d,
                parents: vec![input_h, kernel_h, bias_h],
                input_shape: input.shape.clone(),
                left_shape: input.shape.clone(),
                right_shape: kernel.shape.clone(),
                input: b,
                output: out.clone(),
                left: x,
                right: k,
                aux: vec![batch, in_ch, h, w, out_ch, kh, kw, out_h, out_w],
            });
            registry.note_kernel(batch * out_ch * out_h * out_w * in_ch * kh * kw);
            Some((out, requires_grad, creator, device))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match tensor_alloc_autograd_on_device(
            TensorDType::Float,
            vec![out.len()],
            f64_values_to_host(&out),
            requires_grad,
            creator,
            device,
            TensorPrecision::F32,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dropout(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (shape, data, requires_grad) = match ml_tensor_float_data(args[0] as usize) {
            Some(v) => v,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let p = f64::from_bits(args[1] as u64);
        let training = args[2] != 0;
        if !(0.0..1.0).contains(&p) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let out = if training {
            data.into_iter()
                .enumerate()
                .map(|(idx, v)| if idx % 2 == 0 { v / (1.0 - p) } else { 0.0 })
                .collect::<Vec<_>>()
        } else {
            data
        };
        match tensor_alloc_autograd(
            TensorDType::Float,
            shape,
            f64_values_to_host(&out),
            requires_grad,
            None,
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_max_pool2d(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 7) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (input_h, batch, channels, h, w, pool_h, pool_w) = (
            args[0] as usize,
            args[1] as usize,
            args[2] as usize,
            args[3] as usize,
            args[4] as usize,
            args[5] as usize,
            args[6] as usize,
        );
        if pool_h == 0 || pool_w == 0 || h % pool_h != 0 || w % pool_w != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some((_shape, data, _requires_grad)) = ml_tensor_float_data(input_h) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if data.len() != batch * channels * h * w {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let (out_h, out_w) = (h / pool_h, w / pool_w);
        let mut out = vec![0.0; batch * channels * out_h * out_w];
        for n in 0..batch {
            for c in 0..channels {
                for oy in 0..out_h {
                    for ox in 0..out_w {
                        let mut best = f64::NEG_INFINITY;
                        for py in 0..pool_h {
                            for px in 0..pool_w {
                                let iy = oy * pool_h + py;
                                let ix = ox * pool_w + px;
                                best = best.max(data[((n * channels + c) * h + iy) * w + ix]);
                            }
                        }
                        out[((n * channels + c) * out_h + oy) * out_w + ox] = best;
                    }
                }
            }
        }
        match tensor_alloc(
            TensorDType::Float,
            vec![out.len()],
            f64_values_to_host(&out),
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

fn ml_two_tensor_loss(
    ctx: *mut SpectraHostCallContext,
    op: AutogradOp,
    value_and_grad: impl Fn(&[f64], &[f64]) -> Option<(f64, Vec<f64>)>,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_pred_shape, pred, pred_requires_grad)) = ml_tensor_float_data(args[0] as usize)
        else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_target_shape, target, _target_requires_grad)) =
            ml_tensor_float_data(args[1] as usize)
        else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if pred.len() != target.len() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some((value, grad_pred)) = value_and_grad(&pred, &target) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let requires_grad = pred_requires_grad && tensor_is_grad_enabled();
        let creator = requires_grad.then(|| AutogradNode {
            op,
            parents: vec![args[0] as usize],
            input_shape: vec![pred.len()],
            left_shape: Vec::new(),
            right_shape: Vec::new(),
            input: Vec::new(),
            output: grad_pred,
            left: pred,
            right: target,
            aux: Vec::new(),
        });
        ml_loss_tensor(ctx_ref, value, requires_grad, creator)
    }
}

extern "C" fn std_ml_mse_loss(ctx: *mut SpectraHostCallContext) -> i32 {
    ml_two_tensor_loss(ctx, AutogradOp::MlMse, |pred, target| {
        let n = pred.len() as f64;
        let value = pred
            .iter()
            .zip(target.iter())
            .map(|(p, t)| (p - t) * (p - t))
            .sum::<f64>()
            / n;
        Some((value, Vec::new()))
    })
}

extern "C" fn std_ml_bce_loss(ctx: *mut SpectraHostCallContext) -> i32 {
    ml_two_tensor_loss(ctx, AutogradOp::MlBce, |pred, target| {
        let n = pred.len() as f64;
        let value = pred
            .iter()
            .zip(target.iter())
            .map(|(p, t)| {
                let p = p.clamp(1e-7, 1.0 - 1e-7);
                -(t * p.ln() + (1.0 - t) * (1.0 - p).ln())
            })
            .sum::<f64>()
            / n;
        Some((value, Vec::new()))
    })
}

fn ml_classification_loss(
    ctx: *mut SpectraHostCallContext,
    op: AutogradOp,
    from_logits: bool,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, scores, requires_grad)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(targets) = ml_tensor_int_data(args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if shape.len() != 2 || targets.len() != shape[0] {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let (batch, classes) = (shape[0], shape[1]);
        let mut loss = 0.0;
        let mut grad = vec![0.0; scores.len()];
        for row in 0..batch {
            let target = targets[row] as usize;
            if target >= classes {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            if from_logits {
                let row_scores = &scores[row * classes..row * classes + classes];
                let max_score = row_scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let denom = row_scores
                    .iter()
                    .map(|v| (v - max_score).exp())
                    .sum::<f64>();
                for col in 0..classes {
                    let prob = (row_scores[col] - max_score).exp() / denom;
                    grad[row * classes + col] = prob;
                }
                loss -= (grad[row * classes + target]).ln();
                grad[row * classes + target] -= 1.0;
            } else {
                loss -= scores[row * classes + target];
                grad[row * classes + target] = -1.0;
            }
        }
        loss /= batch as f64;
        let creator = (requires_grad && tensor_is_grad_enabled()).then(|| AutogradNode {
            op,
            parents: vec![args[0] as usize],
            input_shape: shape,
            left_shape: Vec::new(),
            right_shape: Vec::new(),
            input: Vec::new(),
            output: grad,
            left: scores,
            right: Vec::new(),
            aux: vec![batch, classes],
        });
        ml_loss_tensor(
            ctx_ref,
            loss,
            requires_grad && tensor_is_grad_enabled(),
            creator,
        )
    }
}

extern "C" fn std_ml_cross_entropy_loss(ctx: *mut SpectraHostCallContext) -> i32 {
    ml_classification_loss(ctx, AutogradOp::MlCrossEntropy, true)
}

extern "C" fn std_ml_nll_loss(ctx: *mut SpectraHostCallContext) -> i32 {
    ml_classification_loss(ctx, AutogradOp::MlNll, false)
}

fn ml_optimizer_update(param_handle: usize, update: impl Fn(f64, f64, usize) -> f64) -> bool {
    with_tensor_registry(|registry| {
        let Some(param) = registry.get_mut(param_handle) else {
            return false;
        };
        if param.dtype != TensorDType::Float {
            return false;
        }
        let Some(grad) = param.grad.clone() else {
            return false;
        };
        let mut values = tensor_values_as_f64(param);
        if values.len() != grad.len() {
            return false;
        }
        for (idx, value) in values.iter_mut().enumerate() {
            *value = update(*value, grad[idx], idx);
        }
        param.storage = Arc::new(f64_values_to_host(&values));
        param.offset = 0;
        param.layout = TensorLayout::Contiguous;
        param.strides = tensor_strides(&param.shape);
        param.grad = None;
        true
    })
}

extern "C" fn std_ml_sgd_step(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let lr = f64::from_bits(args[1] as u64);
        if !lr.is_finite() || lr < 0.0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if !ml_optimizer_update(args[0] as usize, |value, grad, _| value - lr * grad) {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_sgd_momentum_step(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 4) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (param_h, velocity_h) = (args[0] as usize, args[1] as usize);
        let (lr, momentum) = (
            f64::from_bits(args[2] as u64),
            f64::from_bits(args[3] as u64),
        );
        let Some((_shape, mut velocity, _)) = ml_tensor_float_data(velocity_h) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_tensor_registry(|registry| {
            let Some(param) = registry.get_mut(param_h) else {
                return false;
            };
            let Some(grad) = param.grad.clone() else {
                return false;
            };
            let mut values = tensor_values_as_f64(param);
            if values.len() != grad.len() || velocity.len() != grad.len() {
                return false;
            }
            for idx in 0..values.len() {
                velocity[idx] = momentum * velocity[idx] + grad[idx];
                values[idx] -= lr * velocity[idx];
            }
            param.storage = Arc::new(f64_values_to_host(&values));
            param.offset = 0;
            param.layout = TensorLayout::Contiguous;
            param.strides = tensor_strides(&param.shape);
            param.grad = None;
            true
        });
        if !ok || !ml_store_float_tensor(velocity_h, velocity) {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_adam_step(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 8) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (param_h, m_h, v_h) = (args[0] as usize, args[1] as usize, args[2] as usize);
        let lr = f64::from_bits(args[3] as u64);
        let beta1 = f64::from_bits(args[4] as u64);
        let beta2 = f64::from_bits(args[5] as u64);
        let eps = f64::from_bits(args[6] as u64);
        let step = args[7].max(1) as i32;
        let Some((_m_shape, mut m, _)) = ml_tensor_float_data(m_h) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_v_shape, mut v, _)) = ml_tensor_float_data(v_h) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_tensor_registry(|registry| {
            let Some(param) = registry.get_mut(param_h) else {
                return false;
            };
            let Some(grad) = param.grad.clone() else {
                return false;
            };
            let mut values = tensor_values_as_f64(param);
            if values.len() != grad.len() || m.len() != grad.len() || v.len() != grad.len() {
                return false;
            }
            for idx in 0..values.len() {
                m[idx] = beta1 * m[idx] + (1.0 - beta1) * grad[idx];
                v[idx] = beta2 * v[idx] + (1.0 - beta2) * grad[idx] * grad[idx];
                let m_hat = m[idx] / (1.0 - beta1.powi(step));
                let v_hat = v[idx] / (1.0 - beta2.powi(step));
                values[idx] -= lr * m_hat / (v_hat.sqrt() + eps);
            }
            param.storage = Arc::new(f64_values_to_host(&values));
            param.offset = 0;
            param.layout = TensorLayout::Contiguous;
            param.strides = tensor_strides(&param.shape);
            param.grad = None;
            true
        });
        if !ok || !ml_store_float_tensor(m_h, m) || !ml_store_float_tensor(v_h, v) {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_adamw_step(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 9) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (param_h, m_h, v_h) = (args[0] as usize, args[1] as usize, args[2] as usize);
        let lr = f64::from_bits(args[3] as u64);
        let beta1 = f64::from_bits(args[4] as u64);
        let beta2 = f64::from_bits(args[5] as u64);
        let eps = f64::from_bits(args[6] as u64);
        let step = args[7].max(1) as i32;
        let weight_decay = f64::from_bits(args[8] as u64);
        let Some((_m_shape, mut m, _)) = ml_tensor_float_data(m_h) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_v_shape, mut v, _)) = ml_tensor_float_data(v_h) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_tensor_registry(|registry| {
            let Some(param) = registry.get_mut(param_h) else {
                return false;
            };
            let Some(grad) = param.grad.clone() else {
                return false;
            };
            let mut values = tensor_values_as_f64(param);
            if values.len() != grad.len() || m.len() != grad.len() || v.len() != grad.len() {
                return false;
            }
            for idx in 0..values.len() {
                m[idx] = beta1 * m[idx] + (1.0 - beta1) * grad[idx];
                v[idx] = beta2 * v[idx] + (1.0 - beta2) * grad[idx] * grad[idx];
                let m_hat = m[idx] / (1.0 - beta1.powi(step));
                let v_hat = v[idx] / (1.0 - beta2.powi(step));
                values[idx] =
                    values[idx] * (1.0 - lr * weight_decay) - lr * m_hat / (v_hat.sqrt() + eps);
            }
            param.storage = Arc::new(f64_values_to_host(&values));
            param.offset = 0;
            param.layout = TensorLayout::Contiguous;
            param.strides = tensor_strides(&param.shape);
            param.grad = None;
            true
        });
        if !ok || !ml_store_float_tensor(m_h, m) || !ml_store_float_tensor(v_h, v) {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_exp_lr(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let base = f64::from_bits(args[0] as u64);
        let gamma = f64::from_bits(args[1] as u64);
        let step = args[2] as i32;
        tensor_result(
            ctx_ref,
            (base * gamma.powi(step)).to_bits() as SpectraHostValue,
        )
    }
}

extern "C" fn std_ml_unscale_grad(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let scale = f64::from_bits(args[1] as u64);
        if !scale.is_finite() || scale == 0.0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ok = with_tensor_registry(|registry| {
            let Some(tensor) = registry.get_mut(args[0] as usize) else {
                return false;
            };
            let Some(grad) = tensor.grad.as_mut() else {
                return false;
            };
            for value in grad.iter_mut() {
                *value /= scale;
            }
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

fn ml_read_path_arg(arg: SpectraHostValue) -> Option<String> {
    let path = unsafe { read_spectra_string(arg)? };
    if path.trim().is_empty() {
        return None;
    }
    Some(path)
}

fn ml_parse_csv_numeric(path: &str, has_header: bool) -> Result<(usize, usize, Vec<f64>), i32> {
    let content = std::fs::read_to_string(path).map_err(|_| HOST_STATUS_NOT_FOUND)?;
    let mut rows = 0usize;
    let mut cols = None;
    let mut values = Vec::new();
    for (line_index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if has_header && line_index == 0 {
            continue;
        }
        let parsed = line
            .split(',')
            .map(|part| part.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| HOST_STATUS_INVALID_ARGUMENT)?;
        if parsed.is_empty() {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        match cols {
            Some(expected) if expected != parsed.len() => return Err(HOST_STATUS_INVALID_ARGUMENT),
            None => cols = Some(parsed.len()),
            _ => {}
        }
        rows = rows.saturating_add(1);
        values.extend(parsed);
    }
    let cols = cols.ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    if rows == 0 {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    Ok((rows, cols, values))
}

fn ml_dataset_from_flat_parts(
    features: Vec<f64>,
    labels: Vec<f64>,
    len: usize,
) -> Result<usize, i32> {
    if len == 0 || features.is_empty() || labels.is_empty() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    if features.len() % len != 0 || labels.len() % len != 0 {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let features_handle = tensor_alloc(
        TensorDType::Float,
        vec![len, features.len() / len],
        f64_values_to_host(&features),
    )?;
    let labels_handle = tensor_alloc(
        TensorDType::Float,
        vec![len, labels.len() / len],
        f64_values_to_host(&labels),
    )?;
    let handle = with_ml_registry(|registry| {
        let handle = registry.next_handle();
        registry.datasets.insert(
            handle,
            MlDataset {
                features: features_handle,
                labels: labels_handle,
                len,
            },
        );
        handle
    });
    Ok(handle)
}

fn ml_dataset_from_csv_path(path: &str, label_col: usize, has_header: bool) -> Result<usize, i32> {
    let (rows, cols, values) = ml_parse_csv_numeric(path, has_header)?;
    if cols < 2 || label_col >= cols {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let mut features = Vec::with_capacity(rows * (cols - 1));
    let mut labels = Vec::with_capacity(rows);
    for row in 0..rows {
        for col in 0..cols {
            let value = values[row * cols + col];
            if col == label_col {
                labels.push(value);
            } else {
                features.push(value);
            }
        }
    }
    ml_dataset_from_flat_parts(features, labels, rows)
}

fn ml_parse_json_number_after(key: &str, input: &str) -> Option<f64> {
    let start = input.find(key)? + key.len();
    let rest = input[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let end = rest
        .find(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E')))
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
}

fn ml_parse_json_features(input: &str) -> Option<Vec<f64>> {
    let key_pos = input.find("\"features\"")?;
    let after_key = &input[key_pos..];
    let open = after_key.find('[')? + key_pos;
    let close = input[open..].find(']')? + open;
    input[open + 1..close]
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()
}

fn ml_dataset_from_jsonl_path(path: &str) -> Result<usize, i32> {
    let content = std::fs::read_to_string(path).map_err(|_| HOST_STATUS_NOT_FOUND)?;
    let mut features = Vec::new();
    let mut labels = Vec::new();
    let mut row_width = None;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let row = ml_parse_json_features(line).ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
        let label =
            ml_parse_json_number_after("\"label\"", line).ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
        if row.is_empty() {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        match row_width {
            Some(expected) if expected != row.len() => return Err(HOST_STATUS_INVALID_ARGUMENT),
            None => row_width = Some(row.len()),
            _ => {}
        }
        features.extend(row);
        labels.push(label);
    }
    ml_dataset_from_flat_parts(features, labels.clone(), labels.len())
}

fn ml_parse_npy_f64_1d(path: &str) -> Result<Vec<f64>, i32> {
    let bytes = std::fs::read(path).map_err(|_| HOST_STATUS_NOT_FOUND)?;
    if bytes.len() < 16 || &bytes[0..6] != b"\x93NUMPY" || bytes[6] != 1 || bytes[7] != 0 {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let header_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let header_start = 10usize;
    let data_start = header_start
        .checked_add(header_len)
        .ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    if data_start > bytes.len() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let header = std::str::from_utf8(&bytes[header_start..data_start])
        .map_err(|_| HOST_STATUS_INVALID_ARGUMENT)?;
    if !header.contains("'descr': '<f8'") && !header.contains("\"descr\": \"<f8\"") {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    if header.contains("True") {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let open = header.find('(').ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    let close = header[open..]
        .find(')')
        .map(|idx| open + idx)
        .ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    let shape_text = header[open + 1..close].trim().trim_end_matches(',');
    let len = shape_text
        .parse::<usize>()
        .map_err(|_| HOST_STATUS_INVALID_ARGUMENT)?;
    let expected_bytes = len
        .checked_mul(8)
        .and_then(|n| data_start.checked_add(n))
        .ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    if expected_bytes != bytes.len() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let mut values = Vec::with_capacity(len);
    for chunk in bytes[data_start..].chunks_exact(8) {
        values.push(f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]));
    }
    Ok(values)
}

fn ml_dataset_subset(dataset: MlDataset, start: usize, len: usize) -> Result<usize, i32> {
    let (feature_shape, feature_data, _) =
        ml_tensor_float_data(dataset.features).ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    let (label_shape, label_data, _) =
        ml_tensor_float_data(dataset.labels).ok_or(HOST_STATUS_INVALID_ARGUMENT)?;
    if feature_shape.is_empty() || label_shape.is_empty() || feature_shape[0] != dataset.len {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    if start.checked_add(len).unwrap_or(usize::MAX) > dataset.len {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let feature_width = feature_data.len() / dataset.len;
    let label_width = label_data.len() / dataset.len;
    let f_start = start * feature_width;
    let l_start = start * label_width;
    ml_dataset_from_flat_parts(
        feature_data[f_start..f_start + len * feature_width].to_vec(),
        label_data[l_start..l_start + len * label_width].to_vec(),
        len,
    )
}

fn ml_fnv64_file(path: &str) -> Result<(u64, String), i32> {
    let bytes = std::fs::read(path).map_err(|_| HOST_STATUS_NOT_FOUND)?;
    let mut hash = 0xcbf29ce484222325u64;
    for byte in &bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok((bytes.len() as u64, format!("{hash:016x}")))
}

fn ml_artifact_record(path: String) -> Result<MlArtifactRecord, i32> {
    if path.trim().is_empty() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    let (size, fnv64) = ml_fnv64_file(&path)?;
    Ok(MlArtifactRecord { path, size, fnv64 })
}

fn ml_json_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

fn ml_artifact_json(record: &MlArtifactRecord) -> String {
    format!(
        "{{\"path\":{},\"size\":{},\"fnv64\":{}}}",
        ml_json_string(&record.path),
        record.size,
        ml_json_string(&record.fnv64)
    )
}

fn ml_experiment_manifest_json(experiment: &MlExperiment) -> String {
    let mut configs = experiment.configs.clone();
    configs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let configs_json = configs
        .iter()
        .map(|(key, value)| {
            format!(
                "{{\"key\":{},\"value\":{}}}",
                ml_json_string(key),
                ml_json_string(value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let metrics_json = experiment
        .metrics
        .iter()
        .map(|metric| {
            format!(
                "{{\"name\":{},\"step\":{},\"value\":{}}}",
                ml_json_string(&metric.name),
                metric.step,
                metric.value
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let artifacts_json = experiment
        .artifacts
        .iter()
        .map(ml_artifact_json)
        .collect::<Vec<_>>()
        .join(",");
    let lockfile_json = experiment
        .lockfile
        .as_ref()
        .map(ml_artifact_json)
        .unwrap_or_else(|| "null".to_string());
    let model_output_json = experiment
        .model_output
        .as_ref()
        .map(ml_artifact_json)
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"schema\":\"spectra.ml.experiment.v1\",\"name\":{},\"seed\":{},\"configs\":[{}],\"metrics\":[{}],\"artifacts\":[{}],\"lockfile\":{},\"model_output\":{},\"manifest_path\":{},\"reproduction_command\":{}}}",
        ml_json_string(&experiment.name),
        experiment.seed,
        configs_json,
        metrics_json,
        artifacts_json,
        lockfile_json,
        model_output_json,
        ml_json_string(&experiment.manifest_path),
        ml_json_string(&experiment.reproduction_command)
    )
}

fn ml_manifest_section(source: &str, key: &str) -> Option<String> {
    let start = source.find(key)? + key.len();
    let bytes = source.as_bytes();
    let mut index = start;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index >= bytes.len() || bytes[index] != b':' {
        return None;
    }
    index += 1;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let open = *bytes.get(index)?;
    let close = match open {
        b'[' => b']',
        b'{' => b'}',
        b'"' => b'"',
        b'n' => return Some("null".to_string()),
        b'-' | b'0'..=b'9' => {
            let mut end = index + 1;
            while end < bytes.len()
                && (bytes[end].is_ascii_digit()
                    || matches!(bytes[end], b'.' | b'e' | b'E' | b'+' | b'-'))
            {
                end += 1;
            }
            return Some(source[index..end].to_string());
        }
        _ => return None,
    };
    if open == b'"' {
        let mut end = index + 1;
        while end < bytes.len() {
            if bytes[end] == b'"' && bytes[end - 1] != b'\\' {
                return Some(source[index..=end].to_string());
            }
            end += 1;
        }
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for end in index..bytes.len() {
        let byte = bytes[end];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if byte == b'"' {
            in_string = true;
        } else if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(source[index..=end].to_string());
            }
        }
    }
    None
}

fn ml_compare_manifest_payloads(left: &str, right: &str) -> bool {
    for key in [
        "\"configs\"",
        "\"metrics\"",
        "\"artifacts\"",
        "\"lockfile\"",
        "\"model_output\"",
        "\"seed\"",
    ] {
        if ml_manifest_section(left, key) != ml_manifest_section(right, key) {
            return false;
        }
    }
    true
}

fn ml_distributed_worker_json(worker: &MlDistributedWorker) -> String {
    format!(
        "{{\"worker_id\":{},\"step_count\":{},\"sample_count\":{},\"accumulator\":{},\"active\":{}}}",
        worker.worker_id,
        worker.step_count,
        worker.sample_count,
        worker.accumulator,
        if worker.active { "true" } else { "false" }
    )
}

fn ml_distributed_session_json(session: &MlDistributedSession) -> String {
    let workers_json = session
        .workers
        .iter()
        .map(ml_distributed_worker_json)
        .collect::<Vec<_>>()
        .join(",");
    let checkpoint_json = session
        .last_checkpoint_path
        .as_ref()
        .map(|path| ml_json_string(path))
        .unwrap_or_else(|| "null".to_string());
    let interrupted_json = session
        .interrupted_worker
        .map(|worker_id| worker_id.to_string())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"schema\":\"spectra.ml.distributed_checkpoint.v1\",\"name\":{},\"topology\":\"single-machine-simulated-workers\",\"seed\":{},\"worker_count\":{},\"global_step\":{},\"interrupted_worker\":{},\"last_checkpoint_path\":{},\"workers\":[{}]}}",
        ml_json_string(&session.name),
        session.seed,
        session.worker_count,
        session.global_step,
        interrupted_json,
        checkpoint_json,
        workers_json
    )
}

fn ml_distributed_summary_json(session: &MlDistributedSession) -> String {
    let total_samples: i64 = session
        .workers
        .iter()
        .map(|worker| worker.sample_count)
        .sum();
    let total_worker_steps: i64 = session.workers.iter().map(|worker| worker.step_count).sum();
    format!(
        "{{\"schema\":\"spectra.ml.distributed_summary.v1\",\"name\":{},\"topology\":\"single-machine-simulated-workers\",\"worker_count\":{},\"global_step\":{},\"total_worker_steps\":{},\"total_samples\":{},\"checkpoint\":{}}}",
        ml_json_string(&session.name),
        session.worker_count,
        session.global_step,
        total_worker_steps,
        total_samples,
        session
            .last_checkpoint_path
            .as_ref()
            .map(|path| ml_json_string(path))
            .unwrap_or_else(|| "null".to_string())
    )
}

fn ml_checkpoint_number(source: &str, key: &str) -> Option<i64> {
    ml_manifest_section(source, key)?.trim().parse::<i64>().ok()
}

fn ml_checkpoint_string(source: &str, key: &str) -> Option<String> {
    let encoded = ml_manifest_section(source, key)?;
    if encoded == "null" {
        return None;
    }
    let trimmed = encoded.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return None;
    }
    Some(
        trimmed[1..trimmed.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\"),
    )
}

fn ml_checkpoint_worker_number(worker_source: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{}\":", key);
    let start = worker_source.find(&needle)? + needle.len();
    let bytes = worker_source.as_bytes();
    let mut end = start;
    while end < bytes.len()
        && (bytes[end].is_ascii_digit() || matches!(bytes[end], b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        end += 1;
    }
    worker_source[start..end]
        .parse::<f64>()
        .ok()
        .map(|value| value as i64)
}

fn ml_checkpoint_worker_float(worker_source: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{}\":", key);
    let start = worker_source.find(&needle)? + needle.len();
    let bytes = worker_source.as_bytes();
    let mut end = start;
    while end < bytes.len()
        && (bytes[end].is_ascii_digit() || matches!(bytes[end], b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        end += 1;
    }
    worker_source[start..end].parse::<f64>().ok()
}

fn ml_checkpoint_worker_bool(worker_source: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{}\":", key);
    let start = worker_source.find(&needle)? + needle.len();
    if worker_source[start..].starts_with("true") {
        Some(true)
    } else if worker_source[start..].starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn ml_distributed_session_from_checkpoint(
    source: &str,
    checkpoint_path: String,
) -> Option<MlDistributedSession> {
    if !source.contains("\"schema\":\"spectra.ml.distributed_checkpoint.v1\"") {
        return None;
    }
    let name = ml_checkpoint_string(source, "\"name\"")?;
    let seed = ml_checkpoint_number(source, "\"seed\"")?;
    let worker_count = ml_checkpoint_number(source, "\"worker_count\"")? as usize;
    let global_step = ml_checkpoint_number(source, "\"global_step\"")?;
    let interrupted_worker = match ml_manifest_section(source, "\"interrupted_worker\"")?.trim() {
        "null" => None,
        value => Some(value.parse::<usize>().ok()?),
    };
    let workers_section = ml_manifest_section(source, "\"workers\"")?;
    let mut workers = Vec::new();
    let mut offset = 0usize;
    while let Some(relative_start) = workers_section[offset..].find('{') {
        let start = offset + relative_start;
        let end = workers_section[start..].find('}')? + start;
        let item = &workers_section[start..=end];
        workers.push(MlDistributedWorker {
            worker_id: ml_checkpoint_worker_number(item, "worker_id")? as usize,
            step_count: ml_checkpoint_worker_number(item, "step_count")?,
            sample_count: ml_checkpoint_worker_number(item, "sample_count")?,
            accumulator: ml_checkpoint_worker_float(item, "accumulator")?,
            active: ml_checkpoint_worker_bool(item, "active")?,
        });
        offset = end + 1;
    }
    if workers.len() != worker_count {
        return None;
    }
    Some(MlDistributedSession {
        name,
        out_dir: std::path::Path::new(&checkpoint_path)
            .parent()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string()),
        worker_count,
        seed,
        global_step,
        interrupted_worker,
        workers,
        last_checkpoint_path: Some(checkpoint_path),
    })
}

#[derive(Clone)]
struct MlOnnxValue {
    name: &'static str,
    dtype: &'static str,
    shape: &'static [i64],
}

#[derive(Clone)]
struct MlOnnxNode {
    name: &'static str,
    op_type: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
}

#[derive(Clone)]
struct MlOnnxModel {
    kind: &'static str,
    nodes: Vec<MlOnnxNode>,
    inputs: Vec<MlOnnxValue>,
    outputs: Vec<MlOnnxValue>,
}

fn pb_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn pb_key(field: u32, wire: u8, out: &mut Vec<u8>) {
    pb_varint(((field << 3) | wire as u32) as u64, out);
}

fn pb_i64(field: u32, value: i64, out: &mut Vec<u8>) {
    pb_key(field, 0, out);
    pb_varint(value as u64, out);
}

fn pb_i32(field: u32, value: i32, out: &mut Vec<u8>) {
    pb_key(field, 0, out);
    pb_varint(value as u64, out);
}

fn pb_string(field: u32, value: &str, out: &mut Vec<u8>) {
    pb_key(field, 2, out);
    pb_varint(value.len() as u64, out);
    out.extend_from_slice(value.as_bytes());
}

fn pb_message(field: u32, payload: Vec<u8>, out: &mut Vec<u8>) {
    pb_key(field, 2, out);
    pb_varint(payload.len() as u64, out);
    out.extend_from_slice(&payload);
}

fn ml_onnx_dimension(value: i64) -> Vec<u8> {
    let mut out = Vec::new();
    pb_i64(1, value, &mut out);
    out
}

fn ml_onnx_shape(shape: &[i64]) -> Vec<u8> {
    let mut out = Vec::new();
    for dim in shape {
        pb_message(1, ml_onnx_dimension(*dim), &mut out);
    }
    out
}

fn ml_onnx_type(value: &MlOnnxValue) -> Vec<u8> {
    let elem_type = match value.dtype {
        "float32" => 1,
        "int64" => 7,
        _ => 0,
    };
    let mut tensor_type = Vec::new();
    pb_i32(1, elem_type, &mut tensor_type);
    pb_message(2, ml_onnx_shape(value.shape), &mut tensor_type);
    let mut type_proto = Vec::new();
    pb_message(1, tensor_type, &mut type_proto);
    type_proto
}

fn ml_onnx_value_info(value: &MlOnnxValue) -> Vec<u8> {
    let mut out = Vec::new();
    pb_string(1, value.name, &mut out);
    pb_message(2, ml_onnx_type(value), &mut out);
    out
}

fn ml_onnx_node(node: &MlOnnxNode) -> Vec<u8> {
    let mut out = Vec::new();
    for input in node.inputs {
        pb_string(1, input, &mut out);
    }
    for output in node.outputs {
        pb_string(2, output, &mut out);
    }
    pb_string(3, node.name, &mut out);
    pb_string(4, node.op_type, &mut out);
    out
}

fn ml_onnx_model_spec(kind: &str) -> Option<MlOnnxModel> {
    match kind {
        "linear" => Some(MlOnnxModel {
            kind: "linear",
            nodes: vec![MlOnnxNode {
                name: "linear_gemm",
                op_type: "Gemm",
                inputs: &["input", "weight", "bias"],
                outputs: &["output"],
            }],
            inputs: vec![
                MlOnnxValue {
                    name: "input",
                    dtype: "float32",
                    shape: &[1, 4],
                },
                MlOnnxValue {
                    name: "weight",
                    dtype: "float32",
                    shape: &[4, 3],
                },
                MlOnnxValue {
                    name: "bias",
                    dtype: "float32",
                    shape: &[3],
                },
            ],
            outputs: vec![MlOnnxValue {
                name: "output",
                dtype: "float32",
                shape: &[1, 3],
            }],
        }),
        "conv" => Some(MlOnnxModel {
            kind: "conv",
            nodes: vec![MlOnnxNode {
                name: "conv2d",
                op_type: "Conv",
                inputs: &["input", "kernel", "bias"],
                outputs: &["output"],
            }],
            inputs: vec![
                MlOnnxValue {
                    name: "input",
                    dtype: "float32",
                    shape: &[1, 1, 4, 4],
                },
                MlOnnxValue {
                    name: "kernel",
                    dtype: "float32",
                    shape: &[1, 1, 3, 3],
                },
                MlOnnxValue {
                    name: "bias",
                    dtype: "float32",
                    shape: &[1],
                },
            ],
            outputs: vec![MlOnnxValue {
                name: "output",
                dtype: "float32",
                shape: &[1, 1, 2, 2],
            }],
        }),
        "activation" => Some(MlOnnxModel {
            kind: "activation",
            nodes: vec![MlOnnxNode {
                name: "relu",
                op_type: "Relu",
                inputs: &["input"],
                outputs: &["output"],
            }],
            inputs: vec![MlOnnxValue {
                name: "input",
                dtype: "float32",
                shape: &[1, 8],
            }],
            outputs: vec![MlOnnxValue {
                name: "output",
                dtype: "float32",
                shape: &[1, 8],
            }],
        }),
        "normalization" => Some(MlOnnxModel {
            kind: "normalization",
            nodes: vec![MlOnnxNode {
                name: "layer_norm",
                op_type: "LayerNormalization",
                inputs: &["input", "scale", "bias"],
                outputs: &["output"],
            }],
            inputs: vec![
                MlOnnxValue {
                    name: "input",
                    dtype: "float32",
                    shape: &[1, 8],
                },
                MlOnnxValue {
                    name: "scale",
                    dtype: "float32",
                    shape: &[8],
                },
                MlOnnxValue {
                    name: "bias",
                    dtype: "float32",
                    shape: &[8],
                },
            ],
            outputs: vec![MlOnnxValue {
                name: "output",
                dtype: "float32",
                shape: &[1, 8],
            }],
        }),
        "transformer" => Some(MlOnnxModel {
            kind: "transformer",
            nodes: vec![
                MlOnnxNode {
                    name: "qk",
                    op_type: "MatMul",
                    inputs: &["query", "key"],
                    outputs: &["scores"],
                },
                MlOnnxNode {
                    name: "attention",
                    op_type: "Softmax",
                    inputs: &["scores"],
                    outputs: &["weights"],
                },
                MlOnnxNode {
                    name: "context",
                    op_type: "MatMul",
                    inputs: &["weights", "value"],
                    outputs: &["context"],
                },
                MlOnnxNode {
                    name: "norm",
                    op_type: "LayerNormalization",
                    inputs: &["context", "scale", "bias"],
                    outputs: &["normed"],
                },
                MlOnnxNode {
                    name: "ffn",
                    op_type: "Gelu",
                    inputs: &["normed"],
                    outputs: &["output"],
                },
            ],
            inputs: vec![
                MlOnnxValue {
                    name: "query",
                    dtype: "float32",
                    shape: &[1, 4, 8],
                },
                MlOnnxValue {
                    name: "key",
                    dtype: "float32",
                    shape: &[1, 8, 4],
                },
                MlOnnxValue {
                    name: "value",
                    dtype: "float32",
                    shape: &[1, 4, 8],
                },
                MlOnnxValue {
                    name: "scale",
                    dtype: "float32",
                    shape: &[8],
                },
                MlOnnxValue {
                    name: "bias",
                    dtype: "float32",
                    shape: &[8],
                },
            ],
            outputs: vec![MlOnnxValue {
                name: "output",
                dtype: "float32",
                shape: &[1, 4, 8],
            }],
        }),
        _ => None,
    }
}

fn ml_onnx_model_proto(model: &MlOnnxModel) -> Vec<u8> {
    let mut graph = Vec::new();
    for node in &model.nodes {
        pb_message(1, ml_onnx_node(node), &mut graph);
    }
    pb_string(2, &format!("spectra_{}_graph", model.kind), &mut graph);
    for input in &model.inputs {
        pb_message(11, ml_onnx_value_info(input), &mut graph);
    }
    for output in &model.outputs {
        pb_message(12, ml_onnx_value_info(output), &mut graph);
    }

    let mut opset = Vec::new();
    pb_string(1, "", &mut opset);
    pb_i64(2, 18, &mut opset);

    let mut out = Vec::new();
    pb_i64(1, 9, &mut out);
    pb_string(2, "SpectraLang", &mut out);
    pb_string(5, "R-1801 ONNX subset", &mut out);
    pb_message(7, graph, &mut out);
    pb_message(8, opset, &mut out);
    out
}

fn pb_read_varint(bytes: &[u8], index: &mut usize) -> Option<u64> {
    let mut shift = 0u32;
    let mut value = 0u64;
    while *index < bytes.len() && shift < 64 {
        let byte = bytes[*index];
        *index += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
    }
    None
}

fn pb_read_len<'a>(bytes: &'a [u8], index: &mut usize) -> Option<&'a [u8]> {
    let len = pb_read_varint(bytes, index)? as usize;
    let end = index.checked_add(len)?;
    if end > bytes.len() {
        return None;
    }
    let slice = &bytes[*index..end];
    *index = end;
    Some(slice)
}

fn pb_skip(bytes: &[u8], index: &mut usize, wire: u64) -> Option<()> {
    match wire {
        0 => {
            pb_read_varint(bytes, index)?;
            Some(())
        }
        1 => {
            *index = index.checked_add(8)?;
            (*index <= bytes.len()).then_some(())
        }
        2 => {
            pb_read_len(bytes, index)?;
            Some(())
        }
        5 => {
            *index = index.checked_add(4)?;
            (*index <= bytes.len()).then_some(())
        }
        _ => None,
    }
}

fn ml_onnx_node_op_types(node: &[u8], ops: &mut Vec<String>) -> Option<()> {
    let mut index = 0usize;
    while index < node.len() {
        let key = pb_read_varint(node, &mut index)?;
        let field = key >> 3;
        let wire = key & 0x7;
        if field == 4 && wire == 2 {
            let raw = pb_read_len(node, &mut index)?;
            ops.push(String::from_utf8(raw.to_vec()).ok()?);
        } else {
            pb_skip(node, &mut index, wire)?;
        }
    }
    Some(())
}

fn ml_onnx_graph_ops(graph: &[u8], ops: &mut Vec<String>) -> Option<(usize, usize)> {
    let mut index = 0usize;
    let mut inputs = 0usize;
    let mut outputs = 0usize;
    while index < graph.len() {
        let key = pb_read_varint(graph, &mut index)?;
        let field = key >> 3;
        let wire = key & 0x7;
        if wire == 2 {
            let raw = pb_read_len(graph, &mut index)?;
            match field {
                1 => ml_onnx_node_op_types(raw, ops)?,
                11 => inputs += 1,
                12 => outputs += 1,
                _ => {}
            }
        } else {
            pb_skip(graph, &mut index, wire)?;
        }
    }
    Some((inputs, outputs))
}

fn ml_onnx_import_summary_from_bytes(bytes: &[u8]) -> Option<String> {
    let mut index = 0usize;
    let mut ops = Vec::new();
    let mut graph_count = 0usize;
    let mut input_count = 0usize;
    let mut output_count = 0usize;
    let mut opset_seen = false;
    while index < bytes.len() {
        let key = pb_read_varint(bytes, &mut index)?;
        let field = key >> 3;
        let wire = key & 0x7;
        if field == 7 && wire == 2 {
            graph_count += 1;
            let raw = pb_read_len(bytes, &mut index)?;
            let (inputs, outputs) = ml_onnx_graph_ops(raw, &mut ops)?;
            input_count += inputs;
            output_count += outputs;
        } else if field == 8 && wire == 2 {
            opset_seen = true;
            pb_read_len(bytes, &mut index)?;
        } else {
            pb_skip(bytes, &mut index, wire)?;
        }
    }
    if graph_count != 1 || ops.is_empty() || input_count == 0 || output_count == 0 || !opset_seen {
        return None;
    }
    let ops_json = ops
        .iter()
        .map(|op| ml_json_string(op))
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "{{\"schema\":\"spectra.onnx.subset.v1\",\"graphs\":{},\"nodes\":{},\"inputs\":{},\"outputs\":{},\"ops\":[{}],\"dtypes\":[\"float32\"],\"shapes\":\"ranked\"}}",
        graph_count,
        ops.len(),
        input_count,
        output_count,
        ops_json
    ))
}

fn ml_onnx_validate_summary(summary: &str) -> bool {
    summary.contains("\"schema\":\"spectra.onnx.subset.v1\"")
        && summary.contains("\"nodes\":")
        && summary.contains("\"inputs\":")
        && summary.contains("\"outputs\":")
        && summary.contains("\"float32\"")
        && summary.contains("\"ranked\"")
}

extern "C" fn std_ml_dataset_from_tensors(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[2] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let len = args[2] as usize;
        let valid = with_tensor_registry(|registry| {
            registry.get(args[0] as usize).is_some() && registry.get(args[1] as usize).is_some()
        });
        if !valid {
            return HOST_STATUS_NOT_FOUND;
        }
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.datasets.insert(
                handle,
                MlDataset {
                    features: args[0] as usize,
                    labels: args[1] as usize,
                    len,
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_dataset_from_csv(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match ml_dataset_from_csv_path(&path, args[1] as usize, args[2] != 0) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataset_from_jsonl(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match ml_dataset_from_jsonl_path(&path) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataset_from_npy(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[2] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(features_path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(labels_path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let features = match ml_parse_npy_f64_1d(&features_path) {
            Ok(values) => values,
            Err(code) => return code,
        };
        let labels = match ml_parse_npy_f64_1d(&labels_path) {
            Ok(values) => values,
            Err(code) => return code,
        };
        match ml_dataset_from_flat_parts(features, labels, args[2] as usize) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataset_from_directory(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let features_path = std::path::Path::new(&path).join("features.csv");
        let labels_path = std::path::Path::new(&path).join("labels.csv");
        let (rows, _feature_cols, features) =
            match ml_parse_csv_numeric(features_path.to_string_lossy().as_ref(), true) {
                Ok(parts) => parts,
                Err(code) => return code,
            };
        let (label_rows, _label_cols, labels) =
            match ml_parse_csv_numeric(labels_path.to_string_lossy().as_ref(), true) {
                Ok(parts) => parts,
                Err(code) => return code,
            };
        if label_rows != rows {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        match ml_dataset_from_flat_parts(features, labels, rows) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataset_len(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(len) = with_ml_registry(|registry| {
            registry
                .datasets
                .get(&(args[0] as usize))
                .map(|dataset| dataset.len)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, len as SpectraHostValue)
    }
}

extern "C" fn std_ml_dataset_map_features(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let dataset =
            with_ml_registry(|registry| registry.datasets.get(&(args[0] as usize)).copied());
        let Some(dataset) = dataset else {
            return HOST_STATUS_NOT_FOUND;
        };
        let (feature_shape, feature_data, _) = match ml_tensor_float_data(dataset.features) {
            Some(parts) => parts,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let (_label_shape, label_data, _) = match ml_tensor_float_data(dataset.labels) {
            Some(parts) => parts,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        if feature_shape.is_empty() || feature_shape[0] != dataset.len {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let scale = f64::from_bits(args[1] as u64);
        let bias = f64::from_bits(args[2] as u64);
        if !scale.is_finite() || !bias.is_finite() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mapped = feature_data
            .into_iter()
            .map(|value| value * scale + bias)
            .collect::<Vec<_>>();
        match ml_dataset_from_flat_parts(mapped, label_data, dataset.len) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataset_filter_label_min(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let dataset =
            with_ml_registry(|registry| registry.datasets.get(&(args[0] as usize)).copied());
        let Some(dataset) = dataset else {
            return HOST_STATUS_NOT_FOUND;
        };
        let (feature_shape, feature_data, _) = match ml_tensor_float_data(dataset.features) {
            Some(parts) => parts,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        let (_label_shape, label_data, _) = match ml_tensor_float_data(dataset.labels) {
            Some(parts) => parts,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        };
        if feature_shape.is_empty() || dataset.len == 0 || feature_data.len() % dataset.len != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let min_label = f64::from_bits(args[1] as u64);
        if !min_label.is_finite() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let feature_width = feature_data.len() / dataset.len;
        let label_width = label_data.len() / dataset.len;
        let mut out_features = Vec::new();
        let mut out_labels = Vec::new();
        let mut out_len = 0usize;
        for row in 0..dataset.len {
            let label = label_data[row * label_width];
            if label >= min_label {
                out_features.extend_from_slice(
                    &feature_data[row * feature_width..row * feature_width + feature_width],
                );
                out_labels.extend_from_slice(
                    &label_data[row * label_width..row * label_width + label_width],
                );
                out_len = out_len.saturating_add(1);
            }
        }
        match ml_dataset_from_flat_parts(out_features, out_labels, out_len) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

fn std_ml_dataset_split(ctx: *mut SpectraHostCallContext, train: bool) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let dataset =
            with_ml_registry(|registry| registry.datasets.get(&(args[0] as usize)).copied());
        let Some(dataset) = dataset else {
            return HOST_STATUS_NOT_FOUND;
        };
        let train_len = args[1] as usize;
        if train_len == 0 || train_len >= dataset.len {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let (start, len) = if train {
            (0, train_len)
        } else {
            (train_len, dataset.len - train_len)
        };
        match ml_dataset_subset(dataset, start, len) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataset_train_split(ctx: *mut SpectraHostCallContext) -> i32 {
    std_ml_dataset_split(ctx, true)
}

extern "C" fn std_ml_dataset_test_split(ctx: *mut SpectraHostCallContext) -> i32 {
    std_ml_dataset_split(ctx, false)
}

extern "C" fn std_ml_dataloader_new(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let dataset = args[0] as usize;
        let exists = with_ml_registry(|registry| registry.datasets.contains_key(&dataset));
        if !exists {
            return HOST_STATUS_NOT_FOUND;
        }
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.loaders.insert(
                handle,
                MlDataLoader {
                    dataset,
                    batch_size: args[1] as usize,
                    shuffle_seed: args[2] as u64,
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_dataloader_batch_count(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(count) = with_ml_registry(|registry| {
            let loader = registry.loaders.get(&(args[0] as usize))?;
            let dataset = registry.datasets.get(&loader.dataset)?;
            Some((dataset.len + loader.batch_size - 1) / loader.batch_size)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, count as SpectraHostValue)
    }
}

fn ml_batch_indices(len: usize, batch_size: usize, batch_index: usize, seed: u64) -> Vec<usize> {
    let start = batch_index.saturating_mul(batch_size);
    let end = (start + batch_size).min(len);
    let mut indices = (start..end).collect::<Vec<_>>();
    if seed != 0 {
        indices.sort_by_key(|idx| {
            ((*idx as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(seed))
                >> 32
        });
    }
    indices
}

fn std_ml_dataloader_batch(ctx: *mut SpectraHostCallContext, labels: bool) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some((tensor_handle, len, batch_size, seed)) = with_ml_registry(|registry| {
            let loader = registry.loaders.get(&(args[0] as usize))?;
            let dataset = registry.datasets.get(&loader.dataset)?;
            Some((
                if labels {
                    dataset.labels
                } else {
                    dataset.features
                },
                dataset.len,
                loader.batch_size,
                loader.shuffle_seed,
            ))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let indices = ml_batch_indices(len, batch_size, args[1] as usize, seed);
        let Some((shape, data, _requires_grad)) = ml_tensor_float_data(tensor_handle) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if shape.is_empty() || shape[0] != len {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let row_width = data.len() / len;
        let mut out = Vec::with_capacity(indices.len() * row_width);
        for index in indices {
            out.extend_from_slice(&data[index * row_width..index * row_width + row_width]);
        }
        match tensor_alloc(
            TensorDType::Float,
            vec![out.len()],
            f64_values_to_host(&out),
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_dataloader_batch_features(ctx: *mut SpectraHostCallContext) -> i32 {
    std_ml_dataloader_batch(ctx, false)
}

extern "C" fn std_ml_dataloader_batch_labels(ctx: *mut SpectraHostCallContext) -> i32 {
    std_ml_dataloader_batch(ctx, true)
}

extern "C" fn std_ml_dataframe_from_csv(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (rows, cols, data) = match ml_parse_csv_numeric(&path, args[1] != 0) {
            Ok(parts) => parts,
            Err(code) => return code,
        };
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry
                .dataframes
                .insert(handle, MlDataFrame { rows, cols, data });
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_dataframe_rows(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(rows) = with_ml_registry(|registry| {
            registry
                .dataframes
                .get(&(args[0] as usize))
                .map(|frame| frame.rows)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, rows as SpectraHostValue)
    }
}

extern "C" fn std_ml_dataframe_cols(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(cols) = with_ml_registry(|registry| {
            registry
                .dataframes
                .get(&(args[0] as usize))
                .map(|frame| frame.cols)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, cols as SpectraHostValue)
    }
}

extern "C" fn std_ml_dataframe_column(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(values) = with_ml_registry(|registry| {
            let frame = registry.dataframes.get(&(args[0] as usize))?;
            let col = args[1] as usize;
            if col >= frame.cols {
                return None;
            }
            Some(
                (0..frame.rows)
                    .map(|row| frame.data[row * frame.cols + col])
                    .collect::<Vec<_>>(),
            )
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        match tensor_alloc(
            TensorDType::Float,
            vec![values.len()],
            f64_values_to_host(&values),
        ) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_experiment_start(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(name) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(out_dir) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let manifest_path = std::path::Path::new(&out_dir)
            .join("experiment-manifest.json")
            .to_string_lossy()
            .to_string();
        let reproduction_command = format!(
            "spectralang run <training.spectra> --package-lock spectra.lock --experiment-manifest {}",
            manifest_path
        );
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.experiments.insert(
                handle,
                MlExperiment {
                    name,
                    out_dir,
                    seed: args[2],
                    configs: Vec::new(),
                    metrics: Vec::new(),
                    artifacts: Vec::new(),
                    lockfile: None,
                    model_output: None,
                    manifest_path,
                    reproduction_command,
                    finished: false,
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_experiment_set_config(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(key) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(value) = ml_read_path_arg(args[2]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let ok = with_ml_registry(|registry| {
            let Some(experiment) = registry.experiments.get_mut(&(args[0] as usize)) else {
                return false;
            };
            if let Some((_, existing)) = experiment
                .configs
                .iter_mut()
                .find(|(existing_key, _)| existing_key == &key)
            {
                *existing = value;
            } else {
                experiment.configs.push((key, value));
            }
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_experiment_log_metric(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 4) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(name) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = f64::from_bits(args[2] as u64);
        if !value.is_finite() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let ok = with_ml_registry(|registry| {
            let Some(experiment) = registry.experiments.get_mut(&(args[0] as usize)) else {
                return false;
            };
            experiment.metrics.push(MlMetricRecord {
                name,
                value,
                step: args[3],
            });
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_experiment_log_artifact(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let artifact = match ml_artifact_record(path) {
            Ok(record) => record,
            Err(code) => return code,
        };
        let ok = with_ml_registry(|registry| {
            let Some(experiment) = registry.experiments.get_mut(&(args[0] as usize)) else {
                return false;
            };
            experiment.artifacts.push(artifact);
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_experiment_set_lockfile(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let lockfile = match ml_artifact_record(path) {
            Ok(record) => record,
            Err(code) => return code,
        };
        let ok = with_ml_registry(|registry| {
            let Some(experiment) = registry.experiments.get_mut(&(args[0] as usize)) else {
                return false;
            };
            experiment.lockfile = Some(lockfile);
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_experiment_set_model_output(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let output = match ml_artifact_record(path) {
            Ok(record) => record,
            Err(code) => return code,
        };
        let ok = with_ml_registry(|registry| {
            let Some(experiment) = registry.experiments.get_mut(&(args[0] as usize)) else {
                return false;
            };
            experiment.model_output = Some(output);
            true
        });
        if !ok {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_experiment_finish(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((out_dir, manifest_path, payload)) = with_ml_registry(|registry| {
            let experiment = registry.experiments.get_mut(&(args[0] as usize))?;
            experiment.finished = true;
            Some((
                experiment.out_dir.clone(),
                experiment.manifest_path.clone(),
                ml_experiment_manifest_json(experiment),
            ))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if std::fs::create_dir_all(&out_dir).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        if std::fs::write(&manifest_path, payload.as_bytes()).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_ml_experiment_manifest_path(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = with_ml_registry(|registry| {
            registry
                .experiments
                .get(&(args[0] as usize))
                .map(|experiment| experiment.manifest_path.clone())
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, alloc_spectra_string(&path))
    }
}

extern "C" fn std_ml_experiment_repro_command(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(command) = with_ml_registry(|registry| {
            registry
                .experiments
                .get(&(args[0] as usize))
                .map(|experiment| experiment.reproduction_command.clone())
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, alloc_spectra_string(&command))
    }
}

extern "C" fn std_ml_experiment_compare_manifests(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(left_path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(right_path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let left = match std::fs::read_to_string(&left_path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        let right = match std::fs::read_to_string(&right_path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        tensor_result(
            ctx_ref,
            if ml_compare_manifest_payloads(&left, &right) {
                1
            } else {
                0
            },
        )
    }
}

extern "C" fn std_ml_distributed_session_start(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 4) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(name) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(out_dir) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let worker_count = args[2];
        if worker_count <= 0 || worker_count > 1024 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let worker_count = worker_count as usize;
        let seed = args[3];
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            let workers = (0..worker_count)
                .map(|worker_id| MlDistributedWorker {
                    worker_id,
                    step_count: 0,
                    sample_count: 0,
                    accumulator: seed as f64 + worker_id as f64,
                    active: true,
                })
                .collect();
            registry.distributed_sessions.insert(
                handle,
                MlDistributedSession {
                    name,
                    out_dir,
                    worker_count,
                    seed,
                    global_step: 0,
                    interrupted_worker: None,
                    workers,
                    last_checkpoint_path: None,
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_distributed_worker_step(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 4) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let session_handle = args[0] as usize;
        if args[1] < 0 || args[2] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let worker_id = args[1] as usize;
        let samples = args[2];
        let loss = f64::from_bits(args[3] as u64);
        if !loss.is_finite() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(step_count) = with_ml_registry(|registry| {
            let session = registry.distributed_sessions.get_mut(&session_handle)?;
            let worker = session.workers.get_mut(worker_id)?;
            worker.active = true;
            worker.step_count += 1;
            worker.sample_count += samples;
            worker.accumulator += loss * samples as f64;
            session.interrupted_worker = None;
            Some(worker.step_count)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, step_count)
    }
}

extern "C" fn std_ml_distributed_global_step(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(global_step) = with_ml_registry(|registry| {
            let session = registry.distributed_sessions.get_mut(&(args[0] as usize))?;
            if session
                .workers
                .iter()
                .all(|worker| worker.step_count > session.global_step)
            {
                session.global_step += 1;
            }
            Some(session.global_step)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, global_step)
    }
}

extern "C" fn std_ml_distributed_worker_step_count(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] < 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(step_count) = with_ml_registry(|registry| {
            let session = registry.distributed_sessions.get(&(args[0] as usize))?;
            Some(session.workers.get(args[1] as usize)?.step_count)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, step_count)
    }
}

extern "C" fn std_ml_distributed_checkpoint_save(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let interrupted_worker = if args[2] < 0 {
            None
        } else {
            Some(args[2] as usize)
        };
        let Some((out_dir, payload)) = with_ml_registry(|registry| {
            let session = registry.distributed_sessions.get_mut(&(args[0] as usize))?;
            if let Some(worker_id) = interrupted_worker {
                let worker = session.workers.get_mut(worker_id)?;
                worker.active = false;
                session.interrupted_worker = Some(worker_id);
            } else {
                session.interrupted_worker = None;
            }
            session.last_checkpoint_path = Some(path.clone());
            Some((
                session.out_dir.clone(),
                ml_distributed_session_json(session),
            ))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if std::fs::create_dir_all(&out_dir).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        if std::fs::write(&path, payload.as_bytes()).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_result(ctx_ref, alloc_spectra_string(&path))
    }
}

extern "C" fn std_ml_distributed_resume(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let payload = match std::fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        let Some(mut session) = ml_distributed_session_from_checkpoint(&payload, path) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        for worker in &mut session.workers {
            worker.active = true;
        }
        session.interrupted_worker = None;
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.distributed_sessions.insert(handle, session);
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_distributed_summary(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(summary) = with_ml_registry(|registry| {
            registry
                .distributed_sessions
                .get(&(args[0] as usize))
                .map(ml_distributed_summary_json)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, alloc_spectra_string(&summary))
    }
}

extern "C" fn std_ml_onnx_export(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(kind) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(model) = ml_onnx_model_spec(&kind) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        let payload = ml_onnx_model_proto(&model);
        if std::fs::write(&path, payload).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_result(ctx_ref, alloc_spectra_string(&path))
    }
}

extern "C" fn std_ml_onnx_import_summary(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let bytes = match std::fs::read(&path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        let Some(summary) = ml_onnx_import_summary_from_bytes(&bytes) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, alloc_spectra_string(&summary))
    }
}

extern "C" fn std_ml_onnx_validate(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let bytes = match std::fs::read(&path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        let valid = ml_onnx_import_summary_from_bytes(&bytes)
            .as_deref()
            .map(ml_onnx_validate_summary)
            .unwrap_or(false);
        tensor_result(ctx_ref, if valid { 1 } else { 0 })
    }
}

extern "C" fn std_ml_onnx_roundtrip(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(input_path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(output_path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let bytes = match std::fs::read(&input_path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        let Some(summary) = ml_onnx_import_summary_from_bytes(&bytes) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if !ml_onnx_validate_summary(&summary) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        if let Some(parent) = std::path::Path::new(&output_path).parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        if std::fs::write(&output_path, bytes).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_result(ctx_ref, alloc_spectra_string(&output_path))
    }
}

extern "C" fn std_ml_embedding_lookup(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((table_shape, table, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some(ids) = ml_tensor_int_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if table_shape.len() != 2 || table_shape[0] == 0 || table_shape[1] == 0 || ids.is_empty() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let vocab = table_shape[0];
        let dim = table_shape[1];
        let mut out = Vec::with_capacity(ids.len() * dim);
        for id in &ids {
            if *id < 0 || (*id as usize) >= vocab {
                return HOST_STATUS_INVALID_ARGUMENT;
            }
            let start = *id as usize * dim;
            out.extend_from_slice(&table[start..start + dim]);
        }
        match ml_alloc_float_tensor(vec![ids.len(), dim], out) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_positional_encoding(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[0] <= 0 || args[1] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let seq_len = args[0] as usize;
        let dim = args[1] as usize;
        let mut out = Vec::with_capacity(seq_len * dim);
        for pos in 0..seq_len {
            for i in 0..dim {
                let pair = (i / 2) * 2;
                let angle = pos as f64 / 10000f64.powf(pair as f64 / dim as f64);
                out.push(if i % 2 == 0 { angle.sin() } else { angle.cos() });
            }
        }
        match ml_alloc_float_tensor(vec![seq_len, dim], out) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_layer_norm(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 4) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((input_shape, input, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some((scale_shape, scale, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some((bias_shape, bias, _)) = ml_tensor_float_data(args[2] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let eps = f64::from_bits(args[3] as u64);
        if input_shape.is_empty()
            || !eps.is_finite()
            || eps <= 0.0
            || scale_shape.len() != 1
            || bias_shape.len() != 1
        {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(&dim) = input_shape.last() else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if dim == 0 || scale.len() != dim || bias.len() != dim || input.len() % dim != 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mut out = Vec::with_capacity(input.len());
        for row in input.chunks(dim) {
            let mean = row.iter().sum::<f64>() / dim as f64;
            let var = row
                .iter()
                .map(|value| {
                    let delta = value - mean;
                    delta * delta
                })
                .sum::<f64>()
                / dim as f64;
            let denom = (var + eps).sqrt();
            for idx in 0..dim {
                out.push(((row[idx] - mean) / denom) * scale[idx] + bias[idx]);
            }
        }
        match ml_alloc_float_tensor(input_shape, out) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_gelu(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, input, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let out = input
            .iter()
            .map(|x| {
                0.5 * x
                    * (1.0
                        + ((2.0 / std::f64::consts::PI).sqrt() * (x + 0.044715 * x.powi(3))).tanh())
            })
            .collect::<Vec<_>>();
        match ml_alloc_float_tensor(shape, out) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_swiglu(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, input, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some((gate_shape, gate, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if shape != gate_shape || input.len() != gate.len() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let out = input
            .iter()
            .zip(gate.iter())
            .map(|(x, g)| x * ml_sigmoid(*g))
            .collect::<Vec<_>>();
        match ml_alloc_float_tensor(shape, out) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_attention(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((q_shape, q, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some((k_shape, k, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some((v_shape, v, _)) = ml_tensor_float_data(args[2] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if q_shape.len() != 2 || k_shape.len() != 2 || v_shape.len() != 2 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let (q_len, dim) = (q_shape[0], q_shape[1]);
        let (k_len, k_dim) = (k_shape[0], k_shape[1]);
        let (v_len, v_dim) = (v_shape[0], v_shape[1]);
        if dim == 0 || k_dim != dim || v_len != k_len || q_len == 0 || k_len == 0 || v_dim == 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let scale = (dim as f64).sqrt();
        let mut out = vec![0.0; q_len * v_dim];
        for qi in 0..q_len {
            let mut scores = Vec::with_capacity(k_len);
            for ki in 0..k_len {
                let mut dot = 0.0;
                for d in 0..dim {
                    dot += q[qi * dim + d] * k[ki * dim + d];
                }
                scores.push(dot / scale);
            }
            let Some(weights) = ml_softmax_row(&scores) else {
                return HOST_STATUS_INVALID_ARGUMENT;
            };
            for vi in 0..v_dim {
                let mut value = 0.0;
                for ki in 0..k_len {
                    value += weights[ki] * v[ki * v_dim + vi];
                }
                out[qi * v_dim + vi] = value;
            }
        }
        match ml_alloc_float_tensor(vec![q_len, v_dim], out) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_kv_cache_new(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[0] <= 0 || args[1] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.kv_caches.insert(
                handle,
                MlKvCache {
                    max_tokens: args[0] as usize,
                    dim: args[1] as usize,
                    keys: Vec::new(),
                    values: Vec::new(),
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_kv_cache_append(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((key_shape, key, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some((value_shape, value, _)) = ml_tensor_float_data(args[2] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if key_shape != value_shape || key_shape.len() != 2 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let tokens = key_shape[0];
        let dim = key_shape[1];
        let Some(len) = with_ml_registry(|registry| {
            let cache = registry.kv_caches.get_mut(&(args[0] as usize))?;
            if cache.dim != dim || tokens == 0 || cache.len() + tokens > cache.max_tokens {
                return None;
            }
            cache.keys.extend_from_slice(&key);
            cache.values.extend_from_slice(&value);
            Some(cache.len() as SpectraHostValue)
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, len)
    }
}

impl MlKvCache {
    fn len(&self) -> usize {
        self.keys.len() / self.dim
    }
}

extern "C" fn std_ml_kv_cache_keys(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, values)) = with_ml_registry(|registry| {
            let cache = registry.kv_caches.get(&(args[0] as usize))?;
            Some((vec![cache.len(), cache.dim], cache.keys.clone()))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        match ml_alloc_float_tensor(shape, values) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_kv_cache_values(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((shape, values)) = with_ml_registry(|registry| {
            let cache = registry.kv_caches.get(&(args[0] as usize))?;
            Some((vec![cache.len(), cache.dim], cache.values.clone()))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        match ml_alloc_float_tensor(shape, values) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_kv_cache_len(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(len) = with_ml_registry(|registry| {
            registry
                .kv_caches
                .get(&(args[0] as usize))
                .map(|cache| cache.len() as SpectraHostValue)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, len)
    }
}

extern "C" fn std_ml_logits_sample(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_shape, logits, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let temperature = f64::from_bits(args[1] as u64);
        if logits.is_empty() || !temperature.is_finite() || temperature <= 0.0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let scaled = logits
            .iter()
            .map(|value| *value / temperature)
            .collect::<Vec<_>>();
        let Some(probs) = ml_softmax_row(&scaled) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let sample = {
            let mut state = lock_unpoisoned(random_state());
            random_unit_f64(&mut state)
        };
        let mut cumulative = 0.0;
        for (index, prob) in probs.iter().enumerate() {
            cumulative += *prob;
            if sample <= cumulative {
                return tensor_result(ctx_ref, index as SpectraHostValue);
            }
        }
        tensor_result(ctx_ref, (probs.len() - 1) as SpectraHostValue)
    }
}

extern "C" fn std_ml_tokenizer_wordpiece(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(spec) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(tokenizer) = ml_parse_wordpiece_vocab(&spec) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.tokenizers.insert(handle, tokenizer);
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_tokenizer_encode(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(text) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(ids) = with_ml_registry(|registry| {
            let tokenizer = registry.tokenizers.get(&(args[0] as usize))?;
            Some(ml_wordpiece_encode(tokenizer, &text))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if ids.is_empty() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        match tensor_alloc(TensorDType::Int, vec![ids.len()], ids) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_tokenizer_decode(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(ids) = ml_tensor_int_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some(text) = with_ml_registry(|registry| {
            let tokenizer = registry.tokenizers.get(&(args[0] as usize))?;
            Some(ml_wordpiece_decode(tokenizer, &ids))
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, alloc_spectra_string(&text))
    }
}

extern "C" fn std_ml_text_embed(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(text) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(values) = ml_hash_text_to_embedding(&text, args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match ml_alloc_float_tensor(vec![args[1] as usize], values) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_ml_vector_index_new(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[0] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.vector_indexes.insert(
                handle,
                MlVectorIndex {
                    dim: args[0] as usize,
                    entries: Vec::new(),
                },
            );
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_vector_index_insert(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(id) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_shape, vector, _)) = ml_tensor_float_data(args[2] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some(count) = with_ml_registry(|registry| {
            let index = registry.vector_indexes.get_mut(&(args[0] as usize))?;
            if vector.len() != index.dim {
                return None;
            }
            if let Some(existing) = index.entries.iter_mut().find(|entry| entry.id == id) {
                existing.vector = vector;
            } else {
                index.entries.push(MlVectorIndexEntry { id, vector });
            }
            Some(index.entries.len() as SpectraHostValue)
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, count)
    }
}

extern "C" fn std_ml_vector_index_query(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[2] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some((_shape, query, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_NOT_FOUND;
        };
        let Some(results) = with_ml_registry(|registry| {
            let index = registry.vector_indexes.get(&(args[0] as usize))?;
            if query.len() != index.dim {
                return None;
            }
            let mut scored = index
                .entries
                .iter()
                .filter_map(|entry| {
                    ml_cosine_similarity(&query, &entry.vector)
                        .map(|score| (entry.id.clone(), score))
                })
                .collect::<Vec<_>>();
            scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let top_k = (args[2] as usize).min(scored.len());
            let json = scored
                .into_iter()
                .take(top_k)
                .map(|(id, score)| {
                    format!("{{\"id\":{},\"score\":{}}}", ml_json_string(&id), score)
                })
                .collect::<Vec<_>>()
                .join(",");
            Some(format!(
                "{{\"schema\":\"spectra.ml.vector_query.v1\",\"results\":[{}]}}",
                json
            ))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, alloc_spectra_string(&results))
    }
}

extern "C" fn std_ml_vector_index_persist(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(payload) = with_ml_registry(|registry| {
            registry
                .vector_indexes
                .get(&(args[0] as usize))
                .map(ml_vector_index_json)
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        if std::fs::write(&path, payload.as_bytes()).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_result(ctx_ref, alloc_spectra_string(&path))
    }
}

extern "C" fn std_ml_vector_index_load(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let payload = match std::fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => return HOST_STATUS_NOT_FOUND,
        };
        let Some(index) = ml_vector_index_from_json(&payload) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let handle = with_ml_registry(|registry| {
            let handle = registry.next_handle();
            registry.vector_indexes.insert(handle, index);
            handle
        });
        tensor_result(ctx_ref, handle as SpectraHostValue)
    }
}

extern "C" fn std_ml_rag_chunk_text(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(text) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if args[1] <= 0 || args[2] < 0 || args[2] >= args[1] {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let max_chars = args[1] as usize;
        let overlap = args[2] as usize;
        let chars = text.chars().collect::<Vec<_>>();
        let mut chunks = Vec::new();
        let mut start = 0usize;
        while start < chars.len() {
            let end = (start + max_chars).min(chars.len());
            let chunk = chars[start..end].iter().collect::<String>();
            chunks.push(format!(
                "{{\"id\":\"chunk{}\",\"text\":{}}}",
                chunks.len(),
                ml_json_string(&chunk)
            ));
            if end == chars.len() {
                break;
            }
            start = end - overlap;
        }
        let payload = format!(
            "{{\"schema\":\"spectra.ml.rag_chunks.v1\",\"chunks\":[{}]}}",
            chunks.join(",")
        );
        tensor_result(ctx_ref, alloc_spectra_string(&payload))
    }
}

extern "C" fn std_ml_rag_build_prompt(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(context) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(question) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let prompt = format!(
            "Use the context to answer.\nContext:\n{}\nQuestion:\n{}\nAnswer:",
            context, question
        );
        tensor_result(ctx_ref, alloc_spectra_string(&prompt))
    }
}

extern "C" fn std_ml_rag_evaluate_answer(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(answer) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(expected) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let score = (ml_f1_overlap(&answer, &expected) * 1000.0).round() as SpectraHostValue;
        tensor_result(ctx_ref, score.clamp(0, 1000))
    }
}

extern "C" fn std_ml_metrics_classification(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(expected) = ml_tensor_int_data(args[0] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(predicted) = ml_tensor_int_data(args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if expected.is_empty() || expected.len() != predicted.len() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let mut correct = 0usize;
        let mut tp = 0usize;
        let mut fp = 0usize;
        let mut fn_count = 0usize;
        let positives = expected.iter().filter(|&&value| value == 1).count();
        let negatives = expected.len().saturating_sub(positives);
        let mut auc_pairs = 0usize;
        let mut auc_wins = 0.0f64;

        for (&actual, &pred) in expected.iter().zip(predicted.iter()) {
            if actual == pred {
                correct += 1;
            }
            match (actual == 1, pred == 1) {
                (true, true) => tp += 1,
                (false, true) => fp += 1,
                (true, false) => fn_count += 1,
                (false, false) => {}
            }
        }
        for (i, &actual_i) in expected.iter().enumerate() {
            if actual_i != 1 {
                continue;
            }
            for (j, &actual_j) in expected.iter().enumerate() {
                if actual_j == 1 {
                    continue;
                }
                auc_pairs += 1;
                let score_i = predicted[i] as f64;
                let score_j = predicted[j] as f64;
                if score_i > score_j {
                    auc_wins += 1.0;
                } else if (score_i - score_j).abs() < f64::EPSILON {
                    auc_wins += 0.5;
                }
            }
        }

        let accuracy = correct as f64 / expected.len() as f64;
        let precision = if tp + fp == 0 {
            0.0
        } else {
            tp as f64 / (tp + fp) as f64
        };
        let recall = if tp + fn_count == 0 {
            0.0
        } else {
            tp as f64 / (tp + fn_count) as f64
        };
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        let roc_auc_baseline = if positives == 0 || negatives == 0 || auc_pairs == 0 {
            1.0
        } else {
            auc_wins / auc_pairs as f64
        };
        let payload = ml_metrics_json(
            "classification",
            &[
                ("count", expected.len().to_string()),
                ("accuracy", ml_float_json(accuracy)),
                ("precision", ml_float_json(precision)),
                ("recall", ml_float_json(recall)),
                ("f1", ml_float_json(f1)),
                ("roc_auc_baseline", ml_float_json(roc_auc_baseline)),
                ("true_positive", tp.to_string()),
                ("false_positive", fp.to_string()),
                ("false_negative", fn_count.to_string()),
            ],
        );
        tensor_result(ctx_ref, alloc_spectra_string(&payload))
    }
}

extern "C" fn std_ml_metrics_regression(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_, expected, _)) = ml_tensor_float_data(args[0] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_, predicted, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if expected.is_empty() || expected.len() != predicted.len() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mut squared = 0.0;
        let mut absolute = 0.0;
        for (&actual, &pred) in expected.iter().zip(predicted.iter()) {
            let error = pred - actual;
            squared += error * error;
            absolute += error.abs();
        }
        let n = expected.len() as f64;
        let mse = squared / n;
        let payload = ml_metrics_json(
            "regression",
            &[
                ("count", expected.len().to_string()),
                ("mse", ml_float_json(mse)),
                ("mae", ml_float_json(absolute / n)),
                ("rmse", ml_float_json(mse.sqrt())),
            ],
        );
        tensor_result(ctx_ref, alloc_spectra_string(&payload))
    }
}

extern "C" fn std_ml_metrics_ranking(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(relevance) = ml_tensor_int_data(args[0] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some((_, scores, _)) = ml_tensor_float_data(args[1] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let top_k = args[2] as usize;
        if relevance.is_empty() || relevance.len() != scores.len() || top_k == 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let k = top_k.min(relevance.len());
        let mut order = (0..relevance.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| {
            scores[b]
                .partial_cmp(&scores[a])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(&b))
        });
        let mut dcg = 0.0;
        let mut hit = 0.0;
        let mut mrr = 0.0;
        for (rank, &idx) in order.iter().take(k).enumerate() {
            if relevance[idx] > 0 {
                hit = 1.0;
                if mrr == 0.0 {
                    mrr = 1.0 / (rank + 1) as f64;
                }
            }
            let gain = (2.0f64).powi(relevance[idx] as i32) - 1.0;
            dcg += gain / ((rank + 2) as f64).log2();
        }
        let mut ideal = relevance.clone();
        ideal.sort_by(|a, b| b.cmp(a));
        let mut idcg = 0.0;
        for (rank, rel) in ideal.iter().take(k).enumerate() {
            let gain = (2.0f64).powi(*rel as i32) - 1.0;
            idcg += gain / ((rank + 2) as f64).log2();
        }
        let ndcg = if idcg == 0.0 { 0.0 } else { dcg / idcg };
        let payload = ml_metrics_json(
            "ranking",
            &[
                ("count", relevance.len().to_string()),
                ("top_k", k.to_string()),
                ("hit_rate_at_k", ml_float_json(hit)),
                ("mrr", ml_float_json(mrr)),
                ("ndcg_at_k", ml_float_json(ndcg)),
            ],
        );
        tensor_result(ctx_ref, alloc_spectra_string(&payload))
    }
}

extern "C" fn std_ml_metrics_generation(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(output) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(reference) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let output_tokens = ml_token_set(&output);
        let reference_tokens = ml_token_set(&reference);
        let overlap_f1 = ml_f1_overlap(&output, &reference);
        let exact_match = if output.trim() == reference.trim() {
            1.0
        } else {
            0.0
        };
        let perplexity_proxy = if overlap_f1 <= 0.0 {
            f64::INFINITY
        } else {
            (-overlap_f1.ln()).exp()
        };
        let payload = ml_metrics_json(
            "generation",
            &[
                ("output_tokens", output_tokens.len().to_string()),
                ("reference_tokens", reference_tokens.len().to_string()),
                ("exact_match", ml_float_json(exact_match)),
                ("token_f1", ml_float_json(overlap_f1)),
                ("perplexity", ml_float_json(perplexity_proxy)),
            ],
        );
        tensor_result(ctx_ref, alloc_spectra_string(&payload))
    }
}

extern "C" fn std_ml_serving_metrics(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(mut latencies) = ml_tensor_int_data(args[0] as usize) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let request_count = args[1];
        let error_count = args[2];
        if latencies.is_empty()
            || request_count <= 0
            || error_count < 0
            || error_count > request_count
        {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        latencies.sort();
        let total_ms = latencies.iter().sum::<i64>().max(0) as f64;
        let avg_ms = total_ms / latencies.len() as f64;
        let p95_index = ((latencies.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        let p95_ms = latencies[p95_index.min(latencies.len() - 1)] as f64;
        let throughput = if total_ms <= 0.0 {
            request_count as f64
        } else {
            request_count as f64 / (total_ms / 1000.0)
        };
        let payload = ml_metrics_json(
            "serving",
            &[
                ("requests", request_count.to_string()),
                ("errors", error_count.to_string()),
                (
                    "error_rate",
                    ml_float_json(error_count as f64 / request_count as f64),
                ),
                ("latency_avg_ms", ml_float_json(avg_ms)),
                ("latency_p95_ms", ml_float_json(p95_ms)),
                ("throughput_per_second", ml_float_json(throughput)),
            ],
        );
        tensor_result(ctx_ref, alloc_spectra_string(&payload))
    }
}

extern "C" fn std_ml_evaluation_report(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = ml_args(ctx, 7) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(path) = ml_read_path_arg(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(name) = ml_read_path_arg(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(classification) = ml_json_payload_arg(args[2]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(regression) = ml_json_payload_arg(args[3]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(ranking) = ml_json_payload_arg(args[4]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(generation) = ml_json_payload_arg(args[5]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(serving) = ml_json_payload_arg(args[6]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() {
                return HOST_STATUS_INTERNAL_ERROR;
            }
        }
        let report = format!(
            "{{\"schema\":\"spectra.ml.evaluation_report.v1\",\"name\":{},\"classification\":{},\"regression\":{},\"ranking\":{},\"generation\":{},\"serving\":{}}}",
            ml_json_string(&name),
            classification,
            regression,
            ranking,
            generation,
            serving
        );
        if std::fs::write(&path, report).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        let human_path = format!("{path}.txt");
        let human = format!(
            "Spectra ML Evaluation Report\nname={}\nclassification={}\nregression={}\nranking={}\ngeneration={}\nserving={}\n",
            name, classification, regression, ranking, generation, serving
        );
        if std::fs::write(&human_path, human).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_result(ctx_ref, alloc_spectra_string(&path))
    }
}

extern "C" fn std_tensor_seed(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        *lock_unpoisoned(random_state()) = args[0] as u64;
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_uniform(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (size, min, max) = (args[0], args[1], args[2]);
        if size <= 0 || min >= max {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let range = (max - min) as u64;
        let mut state = lock_unpoisoned(random_state());
        let data = (0..size as usize)
            .map(|_| min + (lcg_next(&mut state) % range) as i64)
            .collect::<Vec<_>>();
        drop(state);
        with_tensor_registry(|registry| registry.note_kernel(data.len()));
        match tensor_alloc(TensorDType::Int, vec![size as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_uniform_f(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (size, min, max) = (
            args[0],
            f64::from_bits(args[1] as u64),
            f64::from_bits(args[2] as u64),
        );
        if size <= 0 || !min.is_finite() || !max.is_finite() || min >= max {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mut state = lock_unpoisoned(random_state());
        let data = (0..size as usize)
            .map(|_| {
                let unit = random_unit_f64(&mut state);
                (min + (max - min) * unit).to_bits() as i64
            })
            .collect::<Vec<_>>();
        drop(state);
        with_tensor_registry(|registry| registry.note_kernel(data.len()));
        match tensor_alloc(TensorDType::Float, vec![size as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_normal_f(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 3) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (size, mean, stddev) = (
            args[0],
            f64::from_bits(args[1] as u64),
            f64::from_bits(args[2] as u64),
        );
        if size <= 0 || !mean.is_finite() || !stddev.is_finite() || stddev < 0.0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mut state = lock_unpoisoned(random_state());
        let mut data = Vec::with_capacity(size as usize);
        while data.len() < size as usize {
            let u1 = random_unit_f64(&mut state).max(f64::MIN_POSITIVE);
            let u2 = random_unit_f64(&mut state);
            let radius = (-2.0 * u1.ln()).sqrt();
            let theta = std::f64::consts::TAU * u2;
            data.push((mean + stddev * radius * theta.cos()).to_bits() as i64);
            if data.len() < size as usize {
                data.push((mean + stddev * radius * theta.sin()).to_bits() as i64);
            }
        }
        drop(state);
        with_tensor_registry(|registry| registry.note_kernel(data.len()));
        match tensor_alloc(TensorDType::Float, vec![size as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_set_deterministic_mode(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let enabled = args[0] != 0;
        *lock_unpoisoned(tensor_deterministic_mode()) = enabled;
        if enabled {
            *lock_unpoisoned(random_state()) = 0x5350_4543_5452_4131;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_deterministic_mode(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let enabled = *lock_unpoisoned(tensor_deterministic_mode());
        tensor_result(ctx_ref, enabled as SpectraHostValue)
    }
}

extern "C" fn std_tensor_tolerance_abs(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(
            ctx_ref,
            NUMERICAL_TOLERANCE_ABS.to_bits() as SpectraHostValue,
        )
    }
}

extern "C" fn std_tensor_tolerance_rel(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(
            ctx_ref,
            NUMERICAL_TOLERANCE_REL.to_bits() as SpectraHostValue,
        )
    }
}

extern "C" fn std_tensor_bernoulli(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (size, p) = (args[0], f64::from_bits(args[1] as u64));
        if size <= 0 || !(0.0..=1.0).contains(&p) {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let mut state = lock_unpoisoned(random_state());
        let data = (0..size as usize)
            .map(|_| {
                if random_unit_f64(&mut state) < p {
                    1
                } else {
                    0
                }
            })
            .collect::<Vec<_>>();
        drop(state);
        with_tensor_registry(|registry| registry.note_kernel(data.len()));
        match tensor_alloc(TensorDType::Int, vec![size as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_categorical(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (size, probabilities_handle) = (args[0], args[1] as usize);
        if size <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let Some(probabilities) = with_tensor_registry(|registry| {
            let tensor = registry.get(probabilities_handle)?;
            let source = tensor.materialize();
            if tensor.shape.len() != 1 || source.is_empty() {
                return None;
            }
            let mut total = 0.0f64;
            let mut weights = Vec::with_capacity(source.len());
            for raw in &source {
                let weight = match tensor.dtype {
                    TensorDType::Int => *raw as f64,
                    TensorDType::Float => f64::from_bits(*raw as u64),
                };
                if !weight.is_finite() || weight < 0.0 {
                    return None;
                }
                total += weight;
                weights.push(weight);
            }
            if total <= 0.0 {
                return None;
            }
            Some((weights, total))
        }) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let (weights, total) = probabilities;
        let mut state = lock_unpoisoned(random_state());
        let mut data = Vec::with_capacity(size as usize);
        for _ in 0..size as usize {
            let mut sample = random_unit_f64(&mut state) * total;
            let mut selected = weights.len().saturating_sub(1);
            for (index, weight) in weights.iter().enumerate() {
                if sample < *weight {
                    selected = index;
                    break;
                }
                sample -= *weight;
            }
            data.push(selected as SpectraHostValue);
        }
        drop(state);
        with_tensor_registry(|registry| registry.note_kernel(data.len()));
        match tensor_alloc(TensorDType::Int, vec![size as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_device(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(device) = with_tensor_registry(|registry| {
            registry
                .get(args[0] as usize)
                .map(|tensor| tensor.device.code())
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, device)
    }
}

extern "C" fn std_tensor_device_available(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(device) = TensorDevice::from_code(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, if device.is_available() { 1 } else { 0 })
    }
}

extern "C" fn std_tensor_device_status(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(device) = TensorDevice::from_code(args[0]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, device.status_code())
    }
}

extern "C" fn std_tensor_to_device(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(target_device) = TensorDevice::from_code(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        if !target_device.is_available() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let Some(source) = with_tensor_registry(|registry| registry.get(args[0] as usize).cloned())
        else {
            return HOST_STATUS_NOT_FOUND;
        };
        if target_device.is_accelerator() && source.dtype != TensorDType::Float {
            return HOST_STATUS_INVALID_ARGUMENT;
        }

        let data = source.materialize();
        let Some(mut moved) = StdTensor::new(source.dtype, source.shape.clone(), data) else {
            return HOST_STATUS_INTERNAL_ERROR;
        };
        moved.device = target_device;
        moved.precision = if target_device == TensorDevice::Wgpu {
            TensorPrecision::F32
        } else {
            source.precision
        };
        moved.requires_grad = source.requires_grad;
        moved.grad = source.grad.clone();
        match tensor_insert(moved) {
            Ok(handle) => {
                with_tensor_registry(|registry| registry.note_device_transfer());
                tensor_result(ctx_ref, handle as SpectraHostValue)
            }
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_cpu(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let transfer_args = [args[0], TensorDevice::Cpu.code()];
        let mut transfer_ctx = SpectraHostCallContext {
            args: transfer_args.as_ptr(),
            arg_len: transfer_args.len(),
            results: ctx_ref.results,
            result_len: ctx_ref.result_len,
            invoke_fn: ctx_ref.invoke_fn,
        };
        std_tensor_to_device(&mut transfer_ctx)
    }
}

extern "C" fn std_tensor_sync(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let exists = with_tensor_registry(|registry| registry.get(args[0] as usize).is_some());
        if !exists {
            return HOST_STATUS_NOT_FOUND;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_precision(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(precision) = with_tensor_registry(|registry| {
            registry
                .get(args[0] as usize)
                .map(|tensor| tensor.precision.code())
        }) else {
            return HOST_STATUS_NOT_FOUND;
        };
        tensor_result(ctx_ref, precision)
    }
}

extern "C" fn std_tensor_to_precision(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 2) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(target_precision) = TensorPrecision::from_code(args[1]) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let Some(source) = with_tensor_registry(|registry| registry.get(args[0] as usize).cloned())
        else {
            return HOST_STATUS_NOT_FOUND;
        };
        if source.dtype != TensorDType::Float {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let data = source
            .materialize()
            .iter()
            .map(|raw| {
                target_precision
                    .quantize(f64::from_bits(*raw as u64))
                    .to_bits() as SpectraHostValue
            })
            .collect::<Vec<_>>();
        let Some(mut converted) = StdTensor::new(source.dtype, source.shape.clone(), data) else {
            return HOST_STATUS_INTERNAL_ERROR;
        };
        converted.device = source.device;
        converted.precision = target_precision;
        converted.requires_grad = source.requires_grad;
        converted.grad = source.grad.clone();
        match tensor_insert(converted) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_stats_allocations(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.allocations)
}

extern "C" fn std_tensor_stats_active(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.active_tensors)
}

extern "C" fn std_tensor_stats_peak_bytes(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.peak_bytes)
}

extern "C" fn std_tensor_stats_reused_buffers(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.reused_buffers)
}

extern "C" fn std_tensor_stats_pool_hits(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.pool_hits)
}

extern "C" fn std_tensor_stats_pool_misses(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.pool_misses)
}

extern "C" fn std_tensor_stats_active_bytes(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.active_bytes)
}

extern "C" fn std_tensor_stats_scratch_reuses(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.scratch_reuses)
}

extern "C" fn std_tensor_kernel_strategy(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        tensor_result(ctx_ref, TensorKernelStrategy::current().code())
    }
}

extern "C" fn std_tensor_stats_kernel_ops(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.kernel_ops)
}

extern "C" fn std_tensor_stats_kernel_elements(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.kernel_elements)
}

extern "C" fn std_tensor_stats_device_transfers(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.device_transfers)
}

extern "C" fn std_tensor_stats_gpu_kernel_ops(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.gpu_kernel_ops)
}

extern "C" fn std_tensor_stats_cpu_fallbacks(ctx: *mut SpectraHostCallContext) -> i32 {
    tensor_metric(ctx, |metrics| metrics.cpu_fallbacks)
}

extern "C" fn std_tensor_stats_graph_nodes(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let count = with_tensor_registry(|registry| {
            registry
                .tensors
                .values()
                .filter(|tensor| tensor.creator.is_some())
                .count()
        });
        tensor_result(ctx_ref, count as SpectraHostValue)
    }
}

extern "C" fn std_tensor_stats_lifetime_records(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = with_tensor_registry(|registry| registry.lifetimes.len());
        tensor_result(ctx_ref, value as SpectraHostValue)
    }
}

extern "C" fn std_tensor_stats_released_lifetimes(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = with_tensor_registry(|registry| registry.released_lifetime_count());
        tensor_result(ctx_ref, value as SpectraHostValue)
    }
}

extern "C" fn std_tensor_stats_allocation_sites(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = with_tensor_registry(|registry| registry.allocation_site_count());
        tensor_result(ctx_ref, value as SpectraHostValue)
    }
}

extern "C" fn std_tensor_stats_reuse_rate_per_mille(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = with_tensor_registry(|registry| registry.reuse_rate_per_mille());
        tensor_result(ctx_ref, value as SpectraHostValue)
    }
}

extern "C" fn std_tensor_memory_report(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let report = with_tensor_registry(|registry| registry.memory_report_json());
        let ptr = alloc_spectra_string(&report);
        if ptr == 0 {
            return HOST_STATUS_INTERNAL_ERROR;
        }
        tensor_result(ctx_ref, ptr)
    }
}

extern "C" fn std_tensor_reset_stats(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        with_tensor_registry(|registry| registry.reset_metrics());
        tensor_optional_result(ctx_ref, 0)
    }
}

fn tensor_metric(
    ctx: *mut SpectraHostCallContext,
    read: impl FnOnce(TensorMetrics) -> usize,
) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let value = with_tensor_registry(|registry| read(registry.metrics)) as SpectraHostValue;
        tensor_result(ctx_ref, value)
    }
}

extern "C" fn std_tensor_free(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        match with_tensor_registry(|registry| registry.remove(args[0] as usize)) {
            Ok(()) => tensor_optional_result(ctx_ref, 0),
            Err(code) => code,
        }
    }
}

extern "C" fn std_tensor_free_all(ctx: *mut SpectraHostCallContext) -> i32 {
    let freed = with_tensor_registry(|registry| registry.clear_all());
    if ctx.is_null() {
        return HOST_STATUS_SUCCESS;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        tensor_result(ctx_ref, freed as SpectraHostValue)
    }
}

fn f64_bits_to_i64_if_needed(value: SpectraHostValue) -> SpectraHostValue {
    let as_float = f64::from_bits(value as u64);
    if as_float.is_finite() && as_float.fract() == 0.0 && as_float.abs() <= i64::MAX as f64 {
        as_float as i64
    } else {
        value
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
    register_host_function(STR_EQ, std_string_eq);
    register_host_function(STR_CONCAT, std_string_concat);
    register_host_function(STR_REPEAT, std_string_repeat);
    register_host_function(STR_BUILDER_NEW, std_string_builder_new);
    register_host_function(STR_BUILDER_PUSH, std_string_builder_push);
    register_host_function(STR_BUILDER_LEN, std_string_builder_len);
    register_host_function(STR_BUILDER_FINISH, std_string_builder_finish);
    register_host_function(STR_BUILDER_FREE, std_string_builder_free);
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

fn fs_path_from_string(path: String) -> Option<PathBuf> {
    if path.trim().is_empty() || path.contains('\0') {
        return None;
    }
    Some(PathBuf::from(path))
}

unsafe fn read_fs_path_arg(arg: SpectraHostValue) -> Result<Option<PathBuf>, i32> {
    match read_spectra_string(arg) {
        Some(path) => Ok(fs_path_from_string(path)),
        None => Err(HOST_STATUS_INVALID_ARGUMENT),
    }
}

fn ensure_file_parent(path: &Path) -> bool {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => std::fs::create_dir_all(parent).is_ok(),
        _ => true,
    }
}

fn fs_write_text(path: &Path, content: &str, append: bool) -> bool {
    if !ensure_file_parent(path) {
        return false;
    }

    if append {
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .and_then(|mut file| file.write_all(content.as_bytes()))
            .is_ok()
    } else {
        std::fs::write(path, content.as_bytes()).is_ok()
    }
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

extern "C" fn std_string_eq(ctx: *mut SpectraHostCallContext) -> i32 {
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
            (Some(left), Some(right)) => (left == right) as SpectraHostValue,
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

// ── std.string string builder (R-3108) ──────────────────────────────────────
//
// Each builder function takes at least one argument (some a sentinel) so the
// parser produces a `Call` on a `FieldAccess` instead of a `MethodCall`. The
// midend currently has no representation for module/namespace types, and a
// `MethodCall` on a bare identifier falls through to `IRType::Int` and then
// fails to resolve a struct method. A `Call` on a qualified path routes
// through the existing qualified-call resolution path.

extern "C" fn std_string_builder_new(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let capacity_hint = unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        args[0].max(0) as usize
    };
    let memory = initialize().memory();
    let builder = match memory.allocate_manual(StringBuilder::new(capacity_hint)) {
        Ok(b) => b,
        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
    };
    let handle = with_string_builder_registry(|reg| reg.insert(builder));
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = handle as SpectraHostValue;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_builder_push(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let str_ptr = args[1];
        with_string_builder_registry(|reg| {
            let _ = reg.push_spectra_string(handle, str_ptr);
        });
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_builder_len(ctx: *mut SpectraHostCallContext) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let result = unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len < 1 || ctx_ref.args.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let args = slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len);
        let handle = args[0] as usize;
        with_string_builder_registry(|reg| match reg.len(handle) {
            Ok(n) => n as SpectraHostValue,
            Err(_) => -1,
        })
    };
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_builder_finish(ctx: *mut SpectraHostCallContext) -> i32 {
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
        let result = with_string_builder_registry(|reg| match reg.finish(handle) {
            Ok(s) => alloc_spectra_string(&s),
            Err(_) => 0,
        });
        if ctx_ref.result_len > 0 && !ctx_ref.results.is_null() {
            let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
            results[0] = result;
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_string_builder_free(ctx: *mut SpectraHostCallContext) -> i32 {
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
        with_string_builder_registry(|reg| {
            let _ = reg.discard(handle);
        });
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
                let slice = if s_start <= s_end {
                    &s[s_start..s_end]
                } else {
                    ""
                };
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
            (Some(s), Some(sub)) if !sub.is_empty() => {
                s.matches(sub.as_str()).count() as SpectraHostValue
            }
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
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs().wrapping_mul(6364136223846793005))
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

#[inline]
fn random_unit_f64(state: &mut u64) -> f64 {
    (lcg_next(state) >> 11) as f64 / (1u64 << 53) as f64
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
        *lock_unpoisoned(random_state()) = args[0] as u64;
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
            let rand = lcg_next(&mut *lock_unpoisoned(random_state()));
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
        let f = random_unit_f64(&mut *lock_unpoisoned(random_state()));
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
        let rand = lcg_next(&mut *lock_unpoisoned(random_state()));
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
        let path = match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = alloc_spectra_string("");
                return HOST_STATUS_SUCCESS;
            }
            Err(status) => return status,
        };
        let content = std::fs::read_to_string(path).unwrap_or_default();
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
        let path = match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = 0;
                return HOST_STATUS_SUCCESS;
            }
            Err(status) => return status,
        };
        let content = read_spectra_string(args[1]).unwrap_or_default();
        let ok = fs_write_text(&path, &content, false);
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
        let path = match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = 0;
                return HOST_STATUS_SUCCESS;
            }
            Err(status) => return status,
        };
        let content = read_spectra_string(args[1]).unwrap_or_default();
        let ok = fs_write_text(&path, &content, true);
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
        let path = match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = 0;
                return HOST_STATUS_SUCCESS;
            }
            Err(status) => return status,
        };
        let exists = path.exists();
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
        let path = match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => {
                let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
                results[0] = 0;
                return HOST_STATUS_SUCCESS;
            }
            Err(status) => return status,
        };
        let ok = std::fs::remove_file(path).is_ok();
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
            return HOST_STATUS_INVALID_ARGUMENT;
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
            return HOST_STATUS_INVALID_ARGUMENT;
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
            return HOST_STATUS_INVALID_ARGUMENT;
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
        let v = char::from_u32(args[0] as u32)
            .map(|c| c.is_alphabetic())
            .unwrap_or(false);
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
        let v = char::from_u32(args[0] as u32)
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false);
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
        let v = char::from_u32(args[0] as u32)
            .map(|c| c.is_whitespace())
            .unwrap_or(false);
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
        let v = char::from_u32(args[0] as u32)
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
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
        let v = char::from_u32(args[0] as u32)
            .map(|c| c.is_lowercase())
            .unwrap_or(false);
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
        let v = char::from_u32(args[0] as u32)
            .map(|c| c.is_alphanumeric())
            .unwrap_or(false);
        results[0] = v as i64;
    }
    HOST_STATUS_SUCCESS
}

// ── std.time register & host functions ──────────────────────────────────────

fn register_time() {
    register_host_function(TIME_NOW_MILLIS, std_time_now_millis);
    register_host_function(TIME_NOW_SECS, std_time_now_secs);
    register_host_function(TIME_SLEEP_MS, std_time_sleep_ms);
    register_host_function(TIME_MONOTONIC_MILLIS, std_time_monotonic_millis);
    register_host_function(TIME_MONOTONIC_NANOS, std_time_monotonic_nanos);
    register_host_function(TIME_DURATION_MS, std_time_duration_ms);
    register_host_function(TIME_DURATION_SECS, std_time_duration_secs);
    register_host_function(TIME_DURATION_MILLIS, std_time_duration_millis);
    register_host_function(TIME_DURATION_SECS_VALUE, std_time_duration_secs_value);
    register_host_function(TIME_DURATION_ADD, std_time_duration_add);
    register_host_function(TIME_DURATION_SUB, std_time_duration_sub);
    register_host_function(TIME_INSTANT_NOW, std_time_instant_now);
    register_host_function(TIME_INSTANT_ELAPSED_MS, std_time_instant_elapsed_ms);
    register_host_function(TIME_INSTANT_ADD, std_time_instant_add);
    register_host_function(TIME_INSTANT_HAS_ELAPSED, std_time_instant_has_elapsed);
    register_host_function(TIME_SLEEP, std_time_sleep);
    register_host_function(TIME_UNIX_TO_UTC, std_time_unix_to_utc);
    register_host_function(TIME_UTC_YEAR, std_time_utc_year);
    register_host_function(TIME_UTC_MONTH, std_time_utc_month);
    register_host_function(TIME_UTC_DAY, std_time_utc_day);
    register_host_function(TIME_UTC_HOUR, std_time_utc_hour);
    register_host_function(TIME_UTC_MINUTE, std_time_utc_minute);
    register_host_function(TIME_UTC_SECOND, std_time_utc_second);
}

const STD_TIME_MAX_SLEEP_MS: u128 = 86_400_000;

#[derive(Clone, Copy)]
struct UtcDateTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
}

struct TimeHandles<T> {
    next: SpectraHostValue,
    values: HashMap<SpectraHostValue, T>,
}

impl<T> TimeHandles<T> {
    fn new() -> Self {
        Self {
            next: 1,
            values: HashMap::new(),
        }
    }

    fn insert(&mut self, value: T) -> SpectraHostValue {
        let handle = self.next;
        self.next = self.next.saturating_add(1).max(1);
        self.values.insert(handle, value);
        handle
    }

    fn get(&self, handle: SpectraHostValue) -> Option<&T> {
        self.values.get(&handle)
    }
}

fn time_start() -> StdInstant {
    static START: OnceLock<StdInstant> = OnceLock::new();
    *START.get_or_init(StdInstant::now)
}

fn duration_handles() -> &'static Mutex<TimeHandles<Duration>> {
    static HANDLES: OnceLock<Mutex<TimeHandles<Duration>>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(TimeHandles::new()))
}

fn instant_handles() -> &'static Mutex<TimeHandles<StdInstant>> {
    static HANDLES: OnceLock<Mutex<TimeHandles<StdInstant>>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(TimeHandles::new()))
}

fn utc_handles() -> &'static Mutex<TimeHandles<UtcDateTime>> {
    static HANDLES: OnceLock<Mutex<TimeHandles<UtcDateTime>>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(TimeHandles::new()))
}

fn store_duration(duration: Duration) -> SpectraHostValue {
    lock_unpoisoned(duration_handles()).insert(duration)
}

fn load_duration(handle: SpectraHostValue) -> Option<Duration> {
    lock_unpoisoned(duration_handles()).get(handle).copied()
}

fn store_instant(instant: StdInstant) -> SpectraHostValue {
    lock_unpoisoned(instant_handles()).insert(instant)
}

fn load_instant(handle: SpectraHostValue) -> Option<StdInstant> {
    lock_unpoisoned(instant_handles()).get(handle).copied()
}

fn store_utc(datetime: UtcDateTime) -> SpectraHostValue {
    lock_unpoisoned(utc_handles()).insert(datetime)
}

fn load_utc(handle: SpectraHostValue) -> Option<UtcDateTime> {
    lock_unpoisoned(utc_handles()).get(handle).copied()
}

fn host_args<'a>(
    ctx: *mut SpectraHostCallContext,
    expected_len: usize,
) -> Result<&'a [SpectraHostValue], i32> {
    if ctx.is_null() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != expected_len || (expected_len > 0 && ctx_ref.args.is_null()) {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        Ok(slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len))
    }
}

fn write_host_result(ctx: *mut SpectraHostCallContext, value: SpectraHostValue) -> i32 {
    if ctx.is_null() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        results[0] = value;
    }
    HOST_STATUS_SUCCESS
}

fn duration_to_i64_millis(duration: Duration) -> Option<i64> {
    i64::try_from(duration.as_millis()).ok()
}

fn duration_from_millis_i64(ms: i64) -> Option<Duration> {
    (ms >= 0).then(|| Duration::from_millis(ms as u64))
}

fn utc_from_unix_seconds(secs: i64) -> UtcDateTime {
    let days = secs.div_euclid(86_400);
    let seconds_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    UtcDateTime {
        year,
        month,
        day,
        hour: seconds_of_day / 3_600,
        minute: (seconds_of_day % 3_600) / 60,
        second: seconds_of_day % 60,
    }
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
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

extern "C" fn std_time_monotonic_millis(ctx: *mut SpectraHostCallContext) -> i32 {
    let elapsed = time_start().elapsed();
    let Some(ms) = duration_to_i64_millis(elapsed) else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    write_host_result(ctx, ms)
}

extern "C" fn std_time_monotonic_nanos(ctx: *mut SpectraHostCallContext) -> i32 {
    let elapsed = time_start().elapsed();
    let Ok(ns) = i64::try_from(elapsed.as_nanos()) else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    write_host_result(ctx, ns)
}

extern "C" fn std_time_duration_ms(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(duration) = duration_from_millis_i64(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, store_duration(duration))
}

extern "C" fn std_time_duration_secs(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if args[0] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    write_host_result(ctx, store_duration(Duration::from_secs(args[0] as u64)))
}

extern "C" fn std_time_duration_millis(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(duration) = load_duration(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(ms) = duration_to_i64_millis(duration) else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    write_host_result(ctx, ms)
}

extern "C" fn std_time_duration_secs_value(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(duration) = load_duration(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Ok(secs) = i64::try_from(duration.as_secs()) else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    write_host_result(ctx, secs)
}

extern "C" fn std_time_duration_add(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(lhs), Some(rhs)) = (load_duration(args[0]), load_duration(args[1])) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(sum) = lhs.checked_add(rhs) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, store_duration(sum))
}

extern "C" fn std_time_duration_sub(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(lhs), Some(rhs)) = (load_duration(args[0]), load_duration(args[1])) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(diff) = lhs.checked_sub(rhs) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, store_duration(diff))
}

extern "C" fn std_time_instant_now(ctx: *mut SpectraHostCallContext) -> i32 {
    write_host_result(ctx, store_instant(StdInstant::now()))
}

extern "C" fn std_time_instant_elapsed_ms(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(instant) = load_instant(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(ms) = duration_to_i64_millis(instant.elapsed()) else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    write_host_result(ctx, ms)
}

extern "C" fn std_time_instant_add(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(instant), Some(duration)) = (load_instant(args[0]), load_duration(args[1])) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(deadline) = instant.checked_add(duration) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, store_instant(deadline))
}

extern "C" fn std_time_instant_has_elapsed(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(instant) = load_instant(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, (StdInstant::now() >= instant) as i64)
}

extern "C" fn std_time_sleep(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(duration) = load_duration(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    if duration.as_millis() > STD_TIME_MAX_SLEEP_MS {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    std::thread::sleep(duration);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_time_unix_to_utc(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, store_utc(utc_from_unix_seconds(args[0])))
}

fn std_time_utc_field(ctx: *mut SpectraHostCallContext, field: fn(UtcDateTime) -> i64) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(datetime) = load_utc(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, field(datetime))
}

extern "C" fn std_time_utc_year(ctx: *mut SpectraHostCallContext) -> i32 {
    std_time_utc_field(ctx, |dt| dt.year)
}

extern "C" fn std_time_utc_month(ctx: *mut SpectraHostCallContext) -> i32 {
    std_time_utc_field(ctx, |dt| dt.month)
}

extern "C" fn std_time_utc_day(ctx: *mut SpectraHostCallContext) -> i32 {
    std_time_utc_field(ctx, |dt| dt.day)
}

extern "C" fn std_time_utc_hour(ctx: *mut SpectraHostCallContext) -> i32 {
    std_time_utc_field(ctx, |dt| dt.hour)
}

extern "C" fn std_time_utc_minute(ctx: *mut SpectraHostCallContext) -> i32 {
    std_time_utc_field(ctx, |dt| dt.minute)
}

extern "C" fn std_time_utc_second(ctx: *mut SpectraHostCallContext) -> i32 {
    std_time_utc_field(ctx, |dt| dt.second)
}

// ── std.range register & host functions ─────────────────────────────────────

fn register_range() {
    register_host_function(RANGE_CREATE, std_range_create);
    register_host_function(RANGE_LEN, std_range_len);
    register_host_function(RANGE_AT, std_range_at);
    register_host_function(RANGE_EQ, std_range_eq);
    register_host_function(RANGE_START, std_range_start);
    register_host_function(RANGE_END, std_range_end);
    register_host_function(RANGE_IS_INCLUSIVE, std_range_is_inclusive);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IntRange {
    start: i64,
    end: i64,
    inclusive: bool,
}

fn range_handles() -> &'static Mutex<TimeHandles<IntRange>> {
    static HANDLES: OnceLock<Mutex<TimeHandles<IntRange>>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(TimeHandles::new()))
}

fn store_range(range: IntRange) -> SpectraHostValue {
    lock_unpoisoned(range_handles()).insert(range)
}

fn load_range(handle: SpectraHostValue) -> Option<IntRange> {
    lock_unpoisoned(range_handles()).get(handle).copied()
}

fn range_len_value(range: IntRange) -> Option<i64> {
    if range.start > range.end {
        return Some(0);
    }
    let raw = i128::from(range.end) - i128::from(range.start);
    let len = raw + if range.inclusive { 1 } else { 0 };
    if len < 0 {
        return Some(0);
    }
    i64::try_from(len).ok()
}

fn range_at_value(range: IntRange, index: i64) -> Option<i64> {
    if index < 0 {
        return None;
    }
    let len = range_len_value(range)?;
    if index >= len {
        return None;
    }
    let value = i128::from(range.start) + i128::from(index);
    i64::try_from(value).ok()
}

extern "C" fn std_range_create(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 3) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let inclusive = match args[2] {
        0 => false,
        1 => true,
        _ => return HOST_STATUS_INVALID_ARGUMENT,
    };
    write_host_result(
        ctx,
        store_range(IntRange {
            start: args[0],
            end: args[1],
            inclusive,
        }),
    )
}

extern "C" fn std_range_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(range) = load_range(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(len) = range_len_value(range) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, len)
}

extern "C" fn std_range_at(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(range) = load_range(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(value) = range_at_value(range, args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, value)
}

extern "C" fn std_range_eq(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 2) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let (Some(lhs), Some(rhs)) = (load_range(args[0]), load_range(args[1])) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, (lhs == rhs) as i64)
}

extern "C" fn std_range_start(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(range) = load_range(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, range.start)
}

extern "C" fn std_range_end(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(range) = load_range(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, range.end)
}

extern "C" fn std_range_is_inclusive(ctx: *mut SpectraHostCallContext) -> i32 {
    let Ok(args) = host_args(ctx, 1) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(range) = load_range(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    write_host_result(ctx, range.inclusive as i64)
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
        Self {
            next_id: 1,
            maps: HashMap::new(),
        }
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
    let mut guard = lock_unpoisoned(registry);
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
            Some(m) => {
                m.data.insert(key, value);
                true
            }
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
            reg.maps
                .get(&handle)
                .and_then(|m| m.data.get(&key).copied())
                .unwrap_or(0)
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
            reg.maps
                .get(&handle)
                .map(|m| m.data.contains_key(&key))
                .unwrap_or(false)
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
            reg.maps
                .get_mut(&handle)
                .and_then(|m| m.data.remove(&key))
                .unwrap_or(0)
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
        let len = with_map_registry(|reg| reg.maps.get(&handle).map(|m| m.data.len()).unwrap_or(0));
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
        with_map_registry(|reg| {
            reg.maps.remove(&handle);
        });
    }
    HOST_STATUS_SUCCESS
}

fn host_call_args<'a>(
    ctx: *mut SpectraHostCallContext,
    expected_args: usize,
) -> Result<(&'a [SpectraHostValue], &'a mut [SpectraHostValue]), i32> {
    if ctx.is_null() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != expected_args {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        if expected_args > 0 && ctx_ref.args.is_null() {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        if ctx_ref.result_len == 0 || ctx_ref.results.is_null() {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }

        let args = if expected_args == 0 {
            &[]
        } else {
            slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len)
        };
        let results = slice::from_raw_parts_mut(ctx_ref.results, ctx_ref.result_len);
        Ok((args, results))
    }
}

fn host_call_void_args<'a>(
    ctx: *mut SpectraHostCallContext,
    expected_args: usize,
) -> Result<&'a [SpectraHostValue], i32> {
    if ctx.is_null() {
        return Err(HOST_STATUS_INVALID_ARGUMENT);
    }

    unsafe {
        let ctx_ref = &mut *ctx;
        if ctx_ref.arg_len != expected_args {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }
        if expected_args > 0 && ctx_ref.args.is_null() {
            return Err(HOST_STATUS_INVALID_ARGUMENT);
        }

        if expected_args == 0 {
            Ok(&[])
        } else {
            Ok(slice::from_raw_parts(ctx_ref.args, ctx_ref.arg_len))
        }
    }
}

#[derive(Clone, Copy)]
struct AsyncTask {
    value: SpectraHostValue,
    cancelled: bool,
    failed: bool,
    completed: bool,
    parent_scope: Option<SpectraHostValue>,
    cancel_handle: SpectraHostValue,
    timeout_inner: Option<SpectraHostValue>,
    deadline_ms: Option<SpectraHostValue>,
    join_order: Option<SpectraHostValue>,
}

struct AsyncScope {
    _parent: Option<SpectraHostValue>,
    child_scopes: Vec<SpectraHostValue>,
    children: Vec<SpectraHostValue>,
    cancelled: bool,
    joined_count: SpectraHostValue,
    failures: SpectraHostValue,
}

#[derive(Clone, Copy)]
enum AsyncStreamKind {
    Source,
    Map {
        upstream: SpectraHostValue,
        op: SpectraHostValue,
        arg: SpectraHostValue,
    },
    Filter {
        upstream: SpectraHostValue,
        predicate: SpectraHostValue,
        arg: SpectraHostValue,
    },
    Take {
        upstream: SpectraHostValue,
        remaining: SpectraHostValue,
    },
    Skip {
        upstream: SpectraHostValue,
        remaining: SpectraHostValue,
    },
    Chunks {
        upstream: SpectraHostValue,
        size: SpectraHostValue,
    },
    Fuse {
        upstream: SpectraHostValue,
        fused_done: bool,
    },
}

struct AsyncStream {
    kind: AsyncStreamKind,
    buffer: VecDeque<SpectraHostValue>,
    capacity: usize,
    pending_next: VecDeque<SpectraHostValue>,
    done: bool,
    cancelled: bool,
    failed: bool,
    last_next_status: SpectraHostValue,
    chunk_items: Vec<SpectraHostValue>,
}

struct AsyncTcpListenerState {
    listener: TcpListener,
    pending_accepts: VecDeque<SpectraHostValue>,
}

struct AsyncTcpStreamState {
    stream: TcpStream,
    pending_reads: VecDeque<SpectraHostValue>,
    closed: bool,
}

struct AsyncUdpSocketState {
    socket: UdpSocket,
    pending_recvs: VecDeque<SpectraHostValue>,
    closed: bool,
}

struct AsyncChannelState {
    queue: VecDeque<SpectraHostValue>,
    capacity: usize,
    pending_sends: VecDeque<(SpectraHostValue, SpectraHostValue)>,
    pending_recvs: VecDeque<SpectraHostValue>,
    closed: bool,
}

enum AsyncStreamPull {
    Pending,
    Item(SpectraHostValue),
    Done,
    Failed,
    Cancelled,
}

struct AsyncTaskRegistry {
    next_task: SpectraHostValue,
    next_scope: SpectraHostValue,
    next_cancel_handle: SpectraHostValue,
    next_stream: SpectraHostValue,
    next_tcp_listener: SpectraHostValue,
    next_tcp_stream: SpectraHostValue,
    next_udp_socket: SpectraHostValue,
    next_async_channel: SpectraHostValue,
    now_ms: SpectraHostValue,
    next_join_order: SpectraHostValue,
    tasks: HashMap<SpectraHostValue, AsyncTask>,
    scopes: HashMap<SpectraHostValue, AsyncScope>,
    cancel_handles: HashMap<SpectraHostValue, SpectraHostValue>,
    streams: HashMap<SpectraHostValue, AsyncStream>,
    tcp_listeners: HashMap<SpectraHostValue, AsyncTcpListenerState>,
    tcp_streams: HashMap<SpectraHostValue, AsyncTcpStreamState>,
    udp_sockets: HashMap<SpectraHostValue, AsyncUdpSocketState>,
    async_channels: HashMap<SpectraHostValue, AsyncChannelState>,
}

impl AsyncTaskRegistry {
    fn new() -> Self {
        Self {
            next_task: 1,
            next_scope: 1,
            next_cancel_handle: 1,
            next_stream: 1,
            next_tcp_listener: 1,
            next_tcp_stream: 1,
            next_udp_socket: 1,
            next_async_channel: 1,
            now_ms: 0,
            next_join_order: 1,
            tasks: HashMap::new(),
            scopes: HashMap::new(),
            cancel_handles: HashMap::new(),
            streams: HashMap::new(),
            tcp_listeners: HashMap::new(),
            tcp_streams: HashMap::new(),
            udp_sockets: HashMap::new(),
            async_channels: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }

    fn allocate_task(
        &mut self,
        value: SpectraHostValue,
        parent_scope: Option<SpectraHostValue>,
        timeout_inner: Option<SpectraHostValue>,
        deadline_ms: Option<SpectraHostValue>,
    ) -> SpectraHostValue {
        self.allocate_task_with_completion(
            value,
            parent_scope,
            timeout_inner,
            deadline_ms,
            true,
            true,
        )
    }

    fn allocate_task_with_completion(
        &mut self,
        value: SpectraHostValue,
        parent_scope: Option<SpectraHostValue>,
        timeout_inner: Option<SpectraHostValue>,
        deadline_ms: Option<SpectraHostValue>,
        completed: bool,
        wake: bool,
    ) -> SpectraHostValue {
        let task_id = self.next_task;
        self.next_task += 1;
        let cancel_handle = self.next_cancel_handle;
        self.next_cancel_handle += 1;
        self.cancel_handles.insert(cancel_handle, task_id);
        self.tasks.insert(
            task_id,
            AsyncTask {
                value,
                cancelled: false,
                failed: false,
                completed,
                parent_scope,
                cancel_handle,
                timeout_inner,
                deadline_ms,
                join_order: None,
            },
        );
        if let Some(scope_id) = parent_scope {
            if let Some(scope) = self.scopes.get_mut(&scope_id) {
                scope.children.push(task_id);
            }
        }
        if wake {
            reactor::global().wake_task(task_id);
        }
        task_id
    }

    fn complete_task(&mut self, task_id: SpectraHostValue, value: SpectraHostValue) -> Option<()> {
        let task = self.tasks.get_mut(&task_id)?;
        if task.cancelled {
            return Some(());
        }
        task.value = value;
        task.completed = true;
        reactor::global().wake_task(task_id);
        Some(())
    }

    fn fail_task(&mut self, task_id: SpectraHostValue) -> Option<()> {
        let task = self.tasks.get_mut(&task_id)?;
        task.failed = true;
        task.completed = true;
        reactor::global().wake_task(task_id);
        Some(())
    }

    fn allocate_failed_task(&mut self) -> SpectraHostValue {
        let task_id = self.allocate_task(-2, None, None, None);
        let _ = self.fail_task(task_id);
        task_id
    }

    fn task_is_cancelled(&self, task_id: SpectraHostValue) -> bool {
        self.tasks
            .get(&task_id)
            .map(|task| task.cancelled)
            .unwrap_or(true)
    }

    fn create_scope(&mut self, parent: Option<SpectraHostValue>) -> Option<SpectraHostValue> {
        if let Some(parent_id) = parent {
            self.scopes.get(&parent_id)?;
        }
        let scope_id = self.next_scope;
        self.next_scope += 1;
        self.scopes.insert(
            scope_id,
            AsyncScope {
                _parent: parent,
                child_scopes: Vec::new(),
                children: Vec::new(),
                cancelled: false,
                joined_count: 0,
                failures: 0,
            },
        );
        if let Some(parent_id) = parent {
            if let Some(parent_scope) = self.scopes.get_mut(&parent_id) {
                parent_scope.child_scopes.push(scope_id);
            }
        }
        Some(scope_id)
    }

    fn attach_task_to_scope(
        &mut self,
        scope_id: SpectraHostValue,
        task_id: SpectraHostValue,
    ) -> Option<()> {
        if !self.scopes.contains_key(&scope_id) {
            return None;
        }
        let task = self.tasks.get_mut(&task_id)?;
        if let Some(old_scope) = task.parent_scope {
            if let Some(scope) = self.scopes.get_mut(&old_scope) {
                scope.children.retain(|child| *child != task_id);
            }
        }
        task.parent_scope = Some(scope_id);
        if let Some(scope) = self.scopes.get_mut(&scope_id) {
            if !scope.children.contains(&task_id) {
                scope.children.push(task_id);
            }
        }
        Some(())
    }

    fn cancel_task(&mut self, task_id: SpectraHostValue) -> Option<()> {
        let inner = {
            let task = self.tasks.get_mut(&task_id)?;
            task.cancelled = true;
            task.timeout_inner
        };
        if let Some(inner) = inner {
            let _ = self.cancel_task(inner);
        }
        reactor::global().wake_task(task_id);
        Some(())
    }

    fn cancel_scope(&mut self, scope_id: SpectraHostValue) -> Option<()> {
        let (children, child_scopes) = {
            let scope = self.scopes.get_mut(&scope_id)?;
            scope.cancelled = true;
            (scope.children.clone(), scope.child_scopes.clone())
        };
        for task_id in children {
            let _ = self.cancel_task(task_id);
        }
        for child_scope in child_scopes {
            let _ = self.cancel_scope(child_scope);
        }
        Some(())
    }

    fn process_due_timeouts(&mut self) {
        let due_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter_map(|(task_id, task)| {
                let deadline = task.deadline_ms?;
                (deadline <= self.now_ms && !task.cancelled).then_some(*task_id)
            })
            .collect();
        for task_id in due_tasks {
            let _ = self.cancel_task(task_id);
        }
    }

    fn join_scope(&mut self, scope_id: SpectraHostValue) -> Option<SpectraHostValue> {
        self.process_due_timeouts();
        let (children, child_scopes, scope_cancelled) = {
            let scope = self.scopes.get(&scope_id)?;
            (
                scope.children.clone(),
                scope.child_scopes.clone(),
                scope.cancelled,
            )
        };

        let mut joined = 0;
        let mut failures = 0;
        let mut cancelled = scope_cancelled;
        for child_scope in child_scopes {
            let status = self.join_scope(child_scope)?;
            joined += self
                .scopes
                .get(&child_scope)
                .map(|scope| scope.joined_count)
                .unwrap_or(0);
            failures += self
                .scopes
                .get(&child_scope)
                .map(|scope| scope.failures)
                .unwrap_or(0);
            if status == 1 {
                cancelled = true;
            }
        }

        for task_id in children {
            let Some(task) = self.tasks.get_mut(&task_id) else {
                continue;
            };
            if task.join_order.is_none() {
                task.join_order = Some(self.next_join_order);
                self.next_join_order += 1;
            }
            joined += 1;
            if task.failed {
                failures += 1;
            }
            if task.cancelled {
                cancelled = true;
            }
        }

        let scope = self.scopes.get_mut(&scope_id)?;
        scope.joined_count = joined;
        scope.failures = failures;
        if failures > 0 {
            Some(2)
        } else if cancelled {
            Some(1)
        } else {
            Some(0)
        }
    }

    fn create_stream(&mut self, kind: AsyncStreamKind, capacity: usize) -> SpectraHostValue {
        let stream_id = self.next_stream;
        self.next_stream += 1;
        self.streams.insert(
            stream_id,
            AsyncStream {
                kind,
                buffer: VecDeque::new(),
                capacity,
                pending_next: VecDeque::new(),
                done: false,
                cancelled: false,
                failed: false,
                last_next_status: 0,
                chunk_items: Vec::new(),
            },
        );
        stream_id
    }

    fn push_stream_value(
        &mut self,
        stream_id: SpectraHostValue,
        value: SpectraHostValue,
    ) -> Option<SpectraHostValue> {
        let stream = self.streams.get_mut(&stream_id)?;
        if stream.cancelled || stream.done || stream.failed {
            return Some(-1);
        }
        if let Some(task_id) = stream.pending_next.pop_front() {
            stream.last_next_status = 1;
            let _ = self.complete_task(task_id, value);
            self.drive_streams();
            return Some(1);
        }
        if stream.buffer.len() >= stream.capacity {
            return Some(0);
        }
        stream.buffer.push_back(value);
        stream.last_next_status = 1;
        self.drive_streams();
        Some(1)
    }

    fn mark_stream_done(&mut self, stream_id: SpectraHostValue) -> Option<()> {
        let stream = self.streams.get_mut(&stream_id)?;
        stream.done = true;
        stream.last_next_status = 2;
        self.drive_streams();
        Some(())
    }

    fn cancel_stream(&mut self, stream_id: SpectraHostValue) -> Option<()> {
        let pending = {
            let stream = self.streams.get_mut(&stream_id)?;
            stream.cancelled = true;
            stream.last_next_status = 4;
            stream.pending_next.drain(..).collect::<Vec<_>>()
        };
        for task_id in pending {
            let _ = self.cancel_task(task_id);
        }
        Some(())
    }

    fn drive_streams(&mut self) {
        loop {
            let stream_ids = self.streams.keys().copied().collect::<Vec<_>>();
            let mut progressed = false;
            for stream_id in stream_ids {
                progressed |= self.drive_stream_pending(stream_id);
            }
            if !progressed {
                break;
            }
        }
    }

    fn drive_stream_pending(&mut self, stream_id: SpectraHostValue) -> bool {
        let mut progressed = false;
        loop {
            let Some(task_id) = self
                .streams
                .get(&stream_id)
                .and_then(|stream| stream.pending_next.front().copied())
            else {
                break;
            };
            match self.pull_stream_value(stream_id) {
                Some(AsyncStreamPull::Item(value)) => {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.pending_next.pop_front();
                        stream.last_next_status = 1;
                    }
                    let _ = self.complete_task(task_id, value);
                    progressed = true;
                }
                Some(AsyncStreamPull::Done) => {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.pending_next.pop_front();
                        stream.last_next_status = 2;
                    }
                    let _ = self.complete_task(task_id, -1);
                    progressed = true;
                }
                Some(AsyncStreamPull::Failed) => {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.pending_next.pop_front();
                        stream.last_next_status = 3;
                    }
                    if let Some(task) = self.tasks.get_mut(&task_id) {
                        task.completed = true;
                        task.failed = true;
                    }
                    progressed = true;
                }
                Some(AsyncStreamPull::Cancelled) => {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.pending_next.pop_front();
                        stream.last_next_status = 4;
                    }
                    let _ = self.cancel_task(task_id);
                    progressed = true;
                }
                Some(AsyncStreamPull::Pending) | None => break,
            }
        }
        progressed
    }

    fn pull_stream_value(&mut self, stream_id: SpectraHostValue) -> Option<AsyncStreamPull> {
        let kind = {
            let stream = self.streams.get_mut(&stream_id)?;
            if stream.cancelled {
                stream.last_next_status = 4;
                return Some(AsyncStreamPull::Cancelled);
            }
            if stream.failed {
                stream.last_next_status = 3;
                return Some(AsyncStreamPull::Failed);
            }
            if let Some(value) = stream.buffer.pop_front() {
                stream.last_next_status = 1;
                return Some(AsyncStreamPull::Item(value));
            }
            if stream.done {
                stream.last_next_status = 2;
                return Some(AsyncStreamPull::Done);
            }
            stream.kind
        };

        match kind {
            AsyncStreamKind::Source => {
                if let Some(stream) = self.streams.get_mut(&stream_id) {
                    stream.last_next_status = 0;
                }
                Some(AsyncStreamPull::Pending)
            }
            AsyncStreamKind::Map { upstream, op, arg } => {
                let pulled = self.pull_stream_value(upstream)?;
                Some(match pulled {
                    AsyncStreamPull::Item(value) => {
                        AsyncStreamPull::Item(map_stream_value(value, op, arg)?)
                    }
                    other => other,
                })
            }
            AsyncStreamKind::Filter {
                upstream,
                predicate,
                arg,
            } => loop {
                let pulled = self.pull_stream_value(upstream)?;
                match pulled {
                    AsyncStreamPull::Item(value) => {
                        if filter_stream_value(value, predicate, arg)? {
                            break Some(AsyncStreamPull::Item(value));
                        }
                    }
                    other => break Some(other),
                }
            },
            AsyncStreamKind::Take {
                upstream,
                remaining,
            } => {
                if remaining <= 0 {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.done = true;
                        stream.last_next_status = 2;
                    }
                    return Some(AsyncStreamPull::Done);
                }
                let pulled = self.pull_stream_value(upstream)?;
                if matches!(pulled, AsyncStreamPull::Item(_)) {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        if let AsyncStreamKind::Take { remaining, .. } = &mut stream.kind {
                            *remaining -= 1;
                        }
                    }
                }
                Some(pulled)
            }
            AsyncStreamKind::Skip {
                upstream,
                remaining: _,
            } => loop {
                let current_remaining = match self.streams.get(&stream_id).map(|stream| stream.kind)
                {
                    Some(AsyncStreamKind::Skip { remaining, .. }) => remaining,
                    _ => return None,
                };
                if current_remaining <= 0 {
                    break self.pull_stream_value(upstream);
                }
                let pulled = self.pull_stream_value(upstream)?;
                match pulled {
                    AsyncStreamPull::Item(_) => {
                        if let Some(stream) = self.streams.get_mut(&stream_id) {
                            if let AsyncStreamKind::Skip { remaining, .. } = &mut stream.kind {
                                *remaining -= 1;
                            }
                        }
                    }
                    other => break Some(other),
                }
            },
            AsyncStreamKind::Chunks { upstream, size } => {
                if size <= 0 {
                    return None;
                }
                loop {
                    let current_len = self
                        .streams
                        .get(&stream_id)
                        .map(|stream| stream.chunk_items.len())
                        .unwrap_or(0);
                    if current_len >= size as usize {
                        let value = self.take_stream_chunk_sum(stream_id)?;
                        return Some(AsyncStreamPull::Item(value));
                    }
                    let pulled = self.pull_stream_value(upstream)?;
                    match pulled {
                        AsyncStreamPull::Item(value) => {
                            if let Some(stream) = self.streams.get_mut(&stream_id) {
                                stream.chunk_items.push(value);
                            }
                        }
                        AsyncStreamPull::Done => {
                            if self
                                .streams
                                .get(&stream_id)
                                .map(|stream| !stream.chunk_items.is_empty())
                                .unwrap_or(false)
                            {
                                let value = self.take_stream_chunk_sum(stream_id)?;
                                return Some(AsyncStreamPull::Item(value));
                            }
                            if let Some(stream) = self.streams.get_mut(&stream_id) {
                                stream.done = true;
                                stream.last_next_status = 2;
                            }
                            return Some(AsyncStreamPull::Done);
                        }
                        other => return Some(other),
                    }
                }
            }
            AsyncStreamKind::Fuse {
                upstream,
                fused_done,
            } => {
                if fused_done {
                    return Some(AsyncStreamPull::Done);
                }
                let pulled = self.pull_stream_value(upstream)?;
                if matches!(pulled, AsyncStreamPull::Done) {
                    if let Some(stream) = self.streams.get_mut(&stream_id) {
                        stream.done = true;
                        if let AsyncStreamKind::Fuse { fused_done, .. } = &mut stream.kind {
                            *fused_done = true;
                        }
                    }
                }
                Some(pulled)
            }
        }
    }

    fn take_stream_chunk_sum(&mut self, stream_id: SpectraHostValue) -> Option<SpectraHostValue> {
        let stream = self.streams.get_mut(&stream_id)?;
        let value = stream.chunk_items.iter().copied().sum();
        stream.chunk_items.clear();
        Some(value)
    }

    fn insert_tcp_stream(&mut self, stream: TcpStream) -> Option<SpectraHostValue> {
        if stream.set_nonblocking(true).is_err() {
            return None;
        }
        let stream_id = self.next_tcp_stream;
        self.next_tcp_stream += 1;
        self.tcp_streams.insert(
            stream_id,
            AsyncTcpStreamState {
                stream,
                pending_reads: VecDeque::new(),
                closed: false,
            },
        );
        Some(stream_id)
    }

    fn drive_tcp_accepts(&mut self) {
        let listener_ids = self.tcp_listeners.keys().copied().collect::<Vec<_>>();
        for listener_id in listener_ids {
            loop {
                let task_id = match self
                    .tcp_listeners
                    .get_mut(&listener_id)
                    .and_then(|listener| listener.pending_accepts.pop_front())
                {
                    Some(task_id) if self.task_is_cancelled(task_id) => continue,
                    Some(task_id) => task_id,
                    None => break,
                };

                let accepted = match self.tcp_listeners.get(&listener_id) {
                    Some(listener) => listener.listener.accept(),
                    None => break,
                };
                match accepted {
                    Ok((stream, _)) => {
                        let Some(stream_id) = self.insert_tcp_stream(stream) else {
                            let _ = self.fail_task(task_id);
                            continue;
                        };
                        let _ = self.complete_task(task_id, stream_id);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if let Some(listener) = self.tcp_listeners.get_mut(&listener_id) {
                            listener.pending_accepts.push_front(task_id);
                        }
                        break;
                    }
                    Err(_) => {
                        let _ = self.fail_task(task_id);
                    }
                }
            }
        }
    }

    fn drive_tcp_reads(&mut self) {
        let stream_ids = self.tcp_streams.keys().copied().collect::<Vec<_>>();
        for stream_id in stream_ids {
            loop {
                let task_id = match self
                    .tcp_streams
                    .get_mut(&stream_id)
                    .and_then(|stream| stream.pending_reads.pop_front())
                {
                    Some(task_id) if self.task_is_cancelled(task_id) => continue,
                    Some(task_id) => task_id,
                    None => break,
                };

                let mut byte = [0u8; 1];
                let read = match self.tcp_streams.get_mut(&stream_id) {
                    Some(state) if state.closed => Ok(0),
                    Some(state) => state.stream.read(&mut byte),
                    None => break,
                };
                match read {
                    Ok(0) => {
                        let _ = self.complete_task(task_id, -1);
                    }
                    Ok(_) => {
                        let _ = self.complete_task(task_id, byte[0] as SpectraHostValue);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if let Some(state) = self.tcp_streams.get_mut(&stream_id) {
                            state.pending_reads.push_front(task_id);
                        }
                        break;
                    }
                    Err(_) => {
                        let _ = self.fail_task(task_id);
                    }
                }
            }
        }
    }

    fn drive_udp_recvs(&mut self) {
        let socket_ids = self.udp_sockets.keys().copied().collect::<Vec<_>>();
        for socket_id in socket_ids {
            loop {
                let task_id = match self
                    .udp_sockets
                    .get_mut(&socket_id)
                    .and_then(|socket| socket.pending_recvs.pop_front())
                {
                    Some(task_id) if self.task_is_cancelled(task_id) => continue,
                    Some(task_id) => task_id,
                    None => break,
                };

                let mut byte = [0u8; 1];
                let recv = match self.udp_sockets.get_mut(&socket_id) {
                    Some(state) if state.closed => match "127.0.0.1:0".parse() {
                        Ok(addr) => Ok((0, addr)),
                        Err(_) => {
                            let _ = self.fail_task(task_id);
                            continue;
                        }
                    },
                    Some(state) => state.socket.recv_from(&mut byte),
                    None => break,
                };
                match recv {
                    Ok((0, _)) => {
                        let _ = self.complete_task(task_id, -1);
                    }
                    Ok((_, _)) => {
                        let _ = self.complete_task(task_id, byte[0] as SpectraHostValue);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if let Some(state) = self.udp_sockets.get_mut(&socket_id) {
                            state.pending_recvs.push_front(task_id);
                        }
                        break;
                    }
                    Err(_) => {
                        let _ = self.fail_task(task_id);
                    }
                }
            }
        }
    }

    fn drive_pending_io_for_task(&mut self, task_id: SpectraHostValue) {
        for _ in 0..16 {
            let pending = self
                .tasks
                .get(&task_id)
                .map(|task| !task.completed && !task.cancelled && !task.failed)
                .unwrap_or(false);
            if !pending {
                break;
            }
            self.drive_tcp_accepts();
            self.drive_tcp_reads();
            self.drive_udp_recvs();
            std::thread::yield_now();
        }
    }

    fn drive_async_channel(&mut self, channel_id: SpectraHostValue) -> Option<()> {
        loop {
            let recv_task = {
                let channel = self.async_channels.get_mut(&channel_id)?;
                channel.pending_recvs.pop_front()
            };
            let Some(recv_task) = recv_task else {
                break;
            };
            if self.task_is_cancelled(recv_task) {
                continue;
            }

            let delivered = {
                let channel = self.async_channels.get_mut(&channel_id)?;
                channel.queue.pop_front()
            };
            if let Some(value) = delivered {
                let _ = self.complete_task(recv_task, value);
                continue;
            }

            let pending_send = {
                let channel = self.async_channels.get_mut(&channel_id)?;
                channel.pending_sends.pop_front()
            };
            if let Some((send_task, value)) = pending_send {
                if !self.task_is_cancelled(send_task) {
                    let _ = self.complete_task(send_task, 1);
                    let _ = self.complete_task(recv_task, value);
                    continue;
                }
            }

            let closed = self
                .async_channels
                .get(&channel_id)
                .map(|channel| channel.closed)
                .unwrap_or(true);
            if closed {
                let _ = self.complete_task(recv_task, -1);
                continue;
            }

            if let Some(channel) = self.async_channels.get_mut(&channel_id) {
                channel.pending_recvs.push_front(recv_task);
            }
            break;
        }

        loop {
            let can_buffer = self
                .async_channels
                .get(&channel_id)
                .map(|channel| channel.queue.len() < channel.capacity)
                .unwrap_or(false);
            if !can_buffer {
                break;
            }
            let pending_send = {
                let channel = self.async_channels.get_mut(&channel_id)?;
                channel.pending_sends.pop_front()
            };
            let Some((send_task, value)) = pending_send else {
                break;
            };
            if self.task_is_cancelled(send_task) {
                continue;
            }
            if let Some(channel) = self.async_channels.get_mut(&channel_id) {
                channel.queue.push_back(value);
            }
            let _ = self.complete_task(send_task, 1);
        }
        Some(())
    }
}

fn map_stream_value(
    value: SpectraHostValue,
    op: SpectraHostValue,
    arg: SpectraHostValue,
) -> Option<SpectraHostValue> {
    match op {
        0 => Some(value),
        1 => Some(value.saturating_add(arg)),
        2 => Some(value.saturating_sub(arg)),
        3 => Some(value.saturating_mul(arg)),
        4 if arg != 0 => Some(value / arg),
        5 => Some(value.saturating_neg()),
        _ => None,
    }
}

fn filter_stream_value(
    value: SpectraHostValue,
    predicate: SpectraHostValue,
    arg: SpectraHostValue,
) -> Option<bool> {
    match predicate {
        0 => Some(value != 0),
        1 => Some(value == arg),
        2 => Some(value != arg),
        3 => Some(value > arg),
        4 => Some(value >= arg),
        5 => Some(value < arg),
        6 => Some(value <= arg),
        7 if arg != 0 => Some(value % arg == 0),
        _ => None,
    }
}

fn fold_stream_value(
    accumulator: SpectraHostValue,
    value: SpectraHostValue,
    op: SpectraHostValue,
) -> Option<SpectraHostValue> {
    match op {
        0 => Some(accumulator.saturating_add(value)),
        1 => Some(accumulator.saturating_mul(value)),
        2 => Some(accumulator.min(value)),
        3 => Some(accumulator.max(value)),
        _ => None,
    }
}

fn async_task_registry() -> &'static Mutex<AsyncTaskRegistry> {
    static REGISTRY: OnceLock<Mutex<AsyncTaskRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(AsyncTaskRegistry::new()))
}

fn lock_async_task_registry() -> Result<std::sync::MutexGuard<'static, AsyncTaskRegistry>, i32> {
    async_task_registry()
        .lock()
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)
}

fn async_last_reactor_event() -> &'static Mutex<Option<ReactorEvent>> {
    static LAST: OnceLock<Mutex<Option<ReactorEvent>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
}

fn set_async_last_reactor_event(event: Option<ReactorEvent>) -> Result<(), i32> {
    let mut last = async_last_reactor_event()
        .lock()
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)?;
    *last = event;
    Ok(())
}

extern "C" fn std_async_task_ready(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let task_id = registry.allocate_task(args[0], None, None, None);
    results[0] = task_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_ready_batch(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let count = args[0];
    if count <= 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let mut first_task = 0;
    for offset in 0..count {
        let task_id = registry.allocate_task_with_completion(
            args[1].saturating_add(offset),
            None,
            None,
            None,
            true,
            false,
        );
        if offset == 0 {
            first_task = task_id;
        }
    }
    reactor::global().wake_task(first_task);
    results[0] = first_task;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_batch_checksum(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let count = args[1];
    if count <= 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.process_due_timeouts();
    let mut checksum = 0i64;
    for offset in 0..count {
        let Some(task) = registry.tasks.get(&args[0].saturating_add(offset)) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if task.cancelled || task.failed || !task.completed {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        checksum = checksum.wrapping_add(task.value);
    }
    results[0] = checksum;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_poll(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.process_due_timeouts();
    registry.drive_pending_io_for_task(args[0]);
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = i64::from(task.completed && !task.cancelled && !task.failed);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_result(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.process_due_timeouts();
    registry.drive_pending_io_for_task(args[0]);
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    if task.cancelled || task.failed {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    results[0] = task.value;
    HOST_STATUS_SUCCESS
}

fn async_task_join_status(task: &AsyncTask) -> SpectraHostValue {
    if task.cancelled {
        1
    } else if task.failed {
        2
    } else if !task.completed {
        3
    } else {
        0
    }
}

extern "C" fn std_async_task_join(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.process_due_timeouts();
    registry.drive_pending_io_for_task(args[0]);
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = match async_task_join_status(task) {
        0 => task.value,
        1 => -1,
        2 => -2,
        _ => -3,
    };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_join_status(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.process_due_timeouts();
    registry.drive_pending_io_for_task(args[0]);
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = async_task_join_status(task);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_cancel(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if registry.cancel_task(args[0]).is_none() {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_is_cancelled(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.process_due_timeouts();
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = i64::from(task.cancelled);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_cancel_handle(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = task.cancel_handle;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_with_timeout(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(inner) = registry.tasks.get(&args[0]).copied() else {
        return HOST_STATUS_NOT_FOUND;
    };
    let deadline = registry.now_ms.saturating_add(args[1]);
    let wrapper = registry.allocate_task(
        inner.value,
        inner.parent_scope,
        Some(args[0]),
        Some(deadline),
    );
    reactor::global().register_timer(wrapper, Duration::from_millis(args[1] as u64));
    registry.process_due_timeouts();
    results[0] = wrapper;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_fail(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(task) = registry.tasks.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    task.failed = true;
    reactor::global().wake_task(args[0]);
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_join_order(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = task.join_order.unwrap_or(0);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_cancel_handle_cancel(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(task_id) = registry.cancel_handles.get(&args[0]).copied() else {
        return HOST_STATUS_NOT_FOUND;
    };
    if registry.cancel_task(task_id).is_none() {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scheduler_advance_time(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[0] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.now_ms = registry.now_ms.saturating_add(args[0]);
    registry.process_due_timeouts();
    results[0] = registry.now_ms;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(scope) = registry.create_scope(None) else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    results[0] = scope;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_child(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(scope) = registry.create_scope(Some(args[0])) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = scope;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_attach(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if registry.attach_task_to_scope(args[0], args[1]).is_none() {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_spawn_ready(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.scopes.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.allocate_task(args[1], Some(args[0]), None, None);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_cancel(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if registry.cancel_scope(args[0]).is_none() {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_join(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(status) = registry.join_scope(args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = status;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_joined_count(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(scope) = registry.scopes.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = scope.joined_count;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_scope_failures(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(scope) = registry.scopes.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = scope.failures;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[0] <= 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    results[0] = registry.create_stream(AsyncStreamKind::Source, args[0] as usize);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_push(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(status) = registry.push_stream_value(args[0], args[1]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = status;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_done(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if registry.mark_stream_done(args[0]).is_none() {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_next(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(pulled) = registry.pull_stream_value(args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let (task, status) = match pulled {
        AsyncStreamPull::Pending => {
            let task = registry.allocate_task_with_completion(0, None, None, None, false, true);
            if let Some(stream) = registry.streams.get_mut(&args[0]) {
                stream.pending_next.push_back(task);
            }
            (task, 0)
        }
        AsyncStreamPull::Item(value) => (registry.allocate_task(value, None, None, None), 1),
        AsyncStreamPull::Done => (registry.allocate_task(-1, None, None, None), 2),
        AsyncStreamPull::Failed => {
            let task = registry.allocate_task(-2, None, None, None);
            if let Some(task_state) = registry.tasks.get_mut(&task) {
                task_state.failed = true;
            }
            (task, 3)
        }
        AsyncStreamPull::Cancelled => {
            let task = registry.allocate_task(-3, None, None, None);
            let _ = registry.cancel_task(task);
            (task, 4)
        }
    };
    if let Some(stream) = registry.streams.get_mut(&args[0]) {
        stream.last_next_status = status;
    }
    results[0] = task;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_next_status(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(stream) = registry.streams.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = stream.last_next_status;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_cancel(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if registry.cancel_stream(args[0]).is_none() {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(stream) = registry.streams.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = stream.buffer.len() as SpectraHostValue;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_capacity(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(stream) = registry.streams.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = stream.capacity as SpectraHostValue;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_map(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if map_stream_value(1, args[1], args[2]).is_none() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.create_stream(
        AsyncStreamKind::Map {
            upstream: args[0],
            op: args[1],
            arg: args[2],
        },
        1,
    );
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_filter(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if filter_stream_value(1, args[1], args[2]).is_none() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.create_stream(
        AsyncStreamKind::Filter {
            upstream: args[0],
            predicate: args[1],
            arg: args[2],
        },
        1,
    );
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_fold(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if fold_stream_value(args[1], 1, args[2]).is_none() {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    let mut accumulator = args[1];
    loop {
        match registry.pull_stream_value(args[0]) {
            Some(AsyncStreamPull::Item(value)) => {
                let Some(next) = fold_stream_value(accumulator, value, args[2]) else {
                    return HOST_STATUS_INVALID_ARGUMENT;
                };
                accumulator = next;
            }
            Some(AsyncStreamPull::Done) => {
                results[0] = registry.allocate_task(accumulator, None, None, None);
                return HOST_STATUS_SUCCESS;
            }
            Some(AsyncStreamPull::Pending) => {
                results[0] = registry.allocate_task_with_completion(
                    accumulator,
                    None,
                    None,
                    None,
                    false,
                    true,
                );
                return HOST_STATUS_SUCCESS;
            }
            Some(AsyncStreamPull::Cancelled) => {
                let task = registry.allocate_task(-3, None, None, None);
                let _ = registry.cancel_task(task);
                results[0] = task;
                return HOST_STATUS_SUCCESS;
            }
            Some(AsyncStreamPull::Failed) => {
                let task = registry.allocate_task(-2, None, None, None);
                if let Some(task_state) = registry.tasks.get_mut(&task) {
                    task_state.failed = true;
                }
                results[0] = task;
                return HOST_STATUS_SUCCESS;
            }
            None => return HOST_STATUS_NOT_FOUND,
        }
    }
}

extern "C" fn std_async_stream_take(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.create_stream(
        AsyncStreamKind::Take {
            upstream: args[0],
            remaining: args[1],
        },
        1,
    );
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_skip(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.create_stream(
        AsyncStreamKind::Skip {
            upstream: args[0],
            remaining: args[1],
        },
        1,
    );
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_chunks(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] <= 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.create_stream(
        AsyncStreamKind::Chunks {
            upstream: args[0],
            size: args[1],
        },
        1,
    );
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_stream_fuse(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.streams.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    results[0] = registry.create_stream(
        AsyncStreamKind::Fuse {
            upstream: args[0],
            fused_done: false,
        },
        1,
    );
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_fs_read(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let path = unsafe {
        match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => return HOST_STATUS_INVALID_ARGUMENT,
            Err(status) => return status,
        }
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let ptr = unsafe { alloc_spectra_string(&content) };
            results[0] = registry.allocate_task(ptr, None, None, None);
        }
        Err(_) => {
            results[0] = registry.allocate_failed_task();
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_fs_write(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let path = unsafe {
        match read_fs_path_arg(args[0]) {
            Ok(Some(path)) => path,
            Ok(None) => return HOST_STATUS_INVALID_ARGUMENT,
            Err(status) => return status,
        }
    };
    let content = unsafe {
        match read_spectra_string(args[1]) {
            Some(content) => content,
            None => return HOST_STATUS_INVALID_ARGUMENT,
        }
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if fs_write_text(&path, &content, false) {
        results[0] = registry.allocate_task(content.len() as SpectraHostValue, None, None, None);
    } else {
        results[0] = registry.allocate_failed_task();
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_listen(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if !(0..=65_535).contains(&args[0]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let listener = match TcpListener::bind(("127.0.0.1", args[0] as u16)) {
        Ok(listener) => listener,
        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
    };
    if listener.set_nonblocking(true).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let listener_id = registry.next_tcp_listener;
    registry.next_tcp_listener += 1;
    registry.tcp_listeners.insert(
        listener_id,
        AsyncTcpListenerState {
            listener,
            pending_accepts: VecDeque::new(),
        },
    );
    results[0] = listener_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_listener_port(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(listener) = registry.tcp_listeners.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let Ok(addr) = listener.listener.local_addr() else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    results[0] = addr.port() as SpectraHostValue;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_connect(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if !(1..=65_535).contains(&args[0]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    match TcpStream::connect(("127.0.0.1", args[0] as u16)) {
        Ok(stream) => {
            let Some(stream_id) = registry.insert_tcp_stream(stream) else {
                results[0] = registry.allocate_failed_task();
                return HOST_STATUS_SUCCESS;
            };
            results[0] = registry.allocate_task(stream_id, None, None, None);
            registry.drive_tcp_accepts();
        }
        Err(_) => {
            results[0] = registry.allocate_failed_task();
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_accept(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(listener) = registry.tcp_listeners.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    match listener.listener.accept() {
        Ok((stream, _)) => {
            let Some(stream_id) = registry.insert_tcp_stream(stream) else {
                results[0] = registry.allocate_failed_task();
                return HOST_STATUS_SUCCESS;
            };
            results[0] = registry.allocate_task(stream_id, None, None, None);
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            let task_id = registry.allocate_task_with_completion(0, None, None, None, false, true);
            if let Some(listener) = registry.tcp_listeners.get_mut(&args[0]) {
                listener.pending_accepts.push_back(task_id);
            }
            results[0] = task_id;
        }
        Err(_) => {
            results[0] = registry.allocate_failed_task();
        }
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_read(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let mut byte = [0u8; 1];
    let read = match registry.tcp_streams.get_mut(&args[0]) {
        Some(state) if state.closed => Ok(0),
        Some(state) => state.stream.read(&mut byte),
        None => return HOST_STATUS_NOT_FOUND,
    };
    match read {
        Ok(0) => results[0] = registry.allocate_task(-1, None, None, None),
        Ok(_) => results[0] = registry.allocate_task(byte[0] as SpectraHostValue, None, None, None),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            let task_id = registry.allocate_task_with_completion(0, None, None, None, false, true);
            if let Some(state) = registry.tcp_streams.get_mut(&args[0]) {
                state.pending_reads.push_back(task_id);
            }
            results[0] = task_id;
        }
        Err(_) => results[0] = registry.allocate_failed_task(),
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_write(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if !(0..=255).contains(&args[1]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let write = match registry.tcp_streams.get_mut(&args[0]) {
        Some(state) if state.closed => Ok(0),
        Some(state) => state.stream.write(&[args[1] as u8]),
        None => return HOST_STATUS_NOT_FOUND,
    };
    match write {
        Ok(count) => {
            results[0] = registry.allocate_task(count as SpectraHostValue, None, None, None)
        }
        Err(_) => results[0] = registry.allocate_failed_task(),
    }
    registry.drive_tcp_reads();
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_tcp_close(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if let Some(state) = registry.tcp_streams.get_mut(&args[0]) {
        state.closed = true;
        results[0] = 1;
        registry.drive_tcp_reads();
        return HOST_STATUS_SUCCESS;
    }
    if registry.tcp_listeners.remove(&args[0]).is_some() {
        results[0] = 1;
        return HOST_STATUS_SUCCESS;
    }
    HOST_STATUS_NOT_FOUND
}

extern "C" fn std_async_udp_bind(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if !(0..=65_535).contains(&args[0]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let socket = match UdpSocket::bind(("127.0.0.1", args[0] as u16)) {
        Ok(socket) => socket,
        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
    };
    if socket.set_nonblocking(true).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let socket_id = registry.next_udp_socket;
    registry.next_udp_socket += 1;
    registry.udp_sockets.insert(
        socket_id,
        AsyncUdpSocketState {
            socket,
            pending_recvs: VecDeque::new(),
            closed: false,
        },
    );
    results[0] = socket_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_udp_port(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(socket) = registry.udp_sockets.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let Ok(addr) = socket.socket.local_addr() else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    results[0] = addr.port() as SpectraHostValue;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_udp_send_to(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if !(1..=65_535).contains(&args[1]) || !(0..=255).contains(&args[2]) {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let send = match registry.udp_sockets.get(&args[0]) {
        Some(state) if state.closed => Ok(0),
        Some(state) => state
            .socket
            .send_to(&[args[2] as u8], ("127.0.0.1", args[1] as u16)),
        None => return HOST_STATUS_NOT_FOUND,
    };
    match send {
        Ok(count) => {
            results[0] = registry.allocate_task(count as SpectraHostValue, None, None, None)
        }
        Err(_) => results[0] = registry.allocate_failed_task(),
    }
    registry.drive_udp_recvs();
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_udp_recv(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let mut byte = [0u8; 1];
    let recv = match registry.udp_sockets.get_mut(&args[0]) {
        Some(state) if state.closed => match "127.0.0.1:0".parse() {
            Ok(addr) => Ok((0, addr)),
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        },
        Some(state) => state.socket.recv_from(&mut byte),
        None => return HOST_STATUS_NOT_FOUND,
    };
    match recv {
        Ok((0, _)) => results[0] = registry.allocate_task(-1, None, None, None),
        Ok((_, _)) => {
            results[0] = registry.allocate_task(byte[0] as SpectraHostValue, None, None, None)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            let task_id = registry.allocate_task_with_completion(0, None, None, None, false, true);
            if let Some(state) = registry.udp_sockets.get_mut(&args[0]) {
                state.pending_recvs.push_back(task_id);
            }
            results[0] = task_id;
        }
        Err(_) => results[0] = registry.allocate_failed_task(),
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_udp_close(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(state) = registry.udp_sockets.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    state.closed = true;
    results[0] = 1;
    registry.drive_udp_recvs();
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_channel_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[0] <= 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let channel_id = registry.next_async_channel;
    registry.next_async_channel += 1;
    registry.async_channels.insert(
        channel_id,
        AsyncChannelState {
            queue: VecDeque::new(),
            capacity: args[0] as usize,
            pending_sends: VecDeque::new(),
            pending_recvs: VecDeque::new(),
            closed: false,
        },
    );
    results[0] = channel_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_channel_send(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.async_channels.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    if channel.closed {
        results[0] = registry.allocate_task(0, None, None, None);
        return HOST_STATUS_SUCCESS;
    }
    if let Some(recv_task) = channel.pending_recvs.pop_front() {
        if !registry.task_is_cancelled(recv_task) {
            let _ = registry.complete_task(recv_task, args[1]);
            results[0] = registry.allocate_task(1, None, None, None);
            return HOST_STATUS_SUCCESS;
        }
    }
    let Some(channel) = registry.async_channels.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    if channel.queue.len() < channel.capacity {
        channel.queue.push_back(args[1]);
        results[0] = registry.allocate_task(1, None, None, None);
    } else {
        let task_id = registry.allocate_task_with_completion(0, None, None, None, false, true);
        if let Some(channel) = registry.async_channels.get_mut(&args[0]) {
            channel.pending_sends.push_back((task_id, args[1]));
        }
        results[0] = task_id;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_channel_recv(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    if !registry.async_channels.contains_key(&args[0]) {
        return HOST_STATUS_NOT_FOUND;
    }
    let queued = registry
        .async_channels
        .get_mut(&args[0])
        .and_then(|channel| channel.queue.pop_front());
    if let Some(value) = queued {
        results[0] = registry.allocate_task(value, None, None, None);
        let _ = registry.drive_async_channel(args[0]);
        return HOST_STATUS_SUCCESS;
    }
    let pending_send = registry
        .async_channels
        .get_mut(&args[0])
        .and_then(|channel| channel.pending_sends.pop_front());
    if let Some((send_task, value)) = pending_send {
        if !registry.task_is_cancelled(send_task) {
            let _ = registry.complete_task(send_task, 1);
            results[0] = registry.allocate_task(value, None, None, None);
            return HOST_STATUS_SUCCESS;
        }
    }
    let closed = registry
        .async_channels
        .get(&args[0])
        .map(|channel| channel.closed)
        .unwrap_or(true);
    if closed {
        results[0] = registry.allocate_task(-1, None, None, None);
        return HOST_STATUS_SUCCESS;
    }
    let task_id = registry.allocate_task_with_completion(0, None, None, None, false, true);
    if let Some(channel) = registry.async_channels.get_mut(&args[0]) {
        channel.pending_recvs.push_back(task_id);
    }
    results[0] = task_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_channel_close(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.async_channels.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    channel.closed = true;
    results[0] = 1;
    let _ = registry.drive_async_channel(args[0]);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_channel_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.async_channels.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = channel.queue.len() as SpectraHostValue;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_task_reset(ctx: *mut SpectraHostCallContext) -> i32 {
    let args = match host_call_void_args(ctx, 0) {
        Ok(args) => args,
        Err(status) => return status,
    };
    let _ = args;
    let mut registry = match lock_async_task_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.clear();
    reactor::global().reset();
    if set_async_last_reactor_event(None).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_backend(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    results[0] = reactor::global().backend().as_code();
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_wake(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    reactor::global().wake_task(args[0]);
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_timer(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    reactor::global().register_timer(args[0], Duration::from_millis(args[1] as u64));
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_io_register(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let Some(interest) = Interest::from_bits(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    results[0] = i64::from(reactor::global().register_io(args[0], interest));
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_io_notify(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let Some(readiness) = Interest::from_bits(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    results[0] = i64::from(reactor::global().notify_io(args[0], readiness));
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_poll(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[0] < -1 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let timeout = if args[0] < 0 {
        None
    } else {
        Some(Duration::from_millis(args[0] as u64))
    };
    let event = reactor::global().poll(timeout);
    if set_async_last_reactor_event(event).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    results[0] = event.map(|event| event.token).unwrap_or(-1);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_last_kind(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let last = match async_last_reactor_event().lock() {
        Ok(last) => last,
        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
    };
    results[0] = last.map(|event| event.kind.as_code()).unwrap_or(0);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_last_readiness(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let last = match async_last_reactor_event().lock() {
        Ok(last) => last,
        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
    };
    results[0] = last.map(|event| event.readiness.bits()).unwrap_or(0);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_stats_queued(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    results[0] = reactor::global().stats().queued as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_stats_task_wakeups(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    results[0] = reactor::global().stats().task_wakeups as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_stats_timer_events(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    results[0] = reactor::global().stats().timer_events as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_stats_io_events(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    results[0] = reactor::global().stats().io_events as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_async_reactor_stats_io_registrations(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    results[0] = reactor::global().stats().io_registrations as i64;
    HOST_STATUS_SUCCESS
}

/// Fast-path helper for `concurrent.task_spawn(value)` called from JIT code
/// via the `spectra_rt_concurrent_spawn_fast` fast ABI entry. Bypasses the
/// generic host-call dispatcher (no manual_alloc/free, no name lookup, no
/// catch_unwind, no host_registry lock). Returns the task_id, or 0 on
/// internal error (poisoned mutex).
pub fn concurrent_spawn_fast(value: SpectraHostValue) -> SpectraHostValue {
    let mut registry = match lock_concurrent_registry() {
        Ok(r) => r,
        Err(_) => return 0,
    };
    registry.spawn(value) as SpectraHostValue
}

/// Fast-path helper for `concurrent.task_join(task_id)`. Returns the value
/// written by the matching `task_spawn`, or 0 if the task_id is invalid
/// (out of range, recycled, or never existed).
pub fn concurrent_join_fast(task_id: SpectraHostValue) -> SpectraHostValue {
    let task_id = task_id as usize;
    let mut registry = match lock_concurrent_registry() {
        Ok(r) => r,
        Err(_) => return 0,
    };
    match registry.join(task_id) {
        Ok(v) => v,
        Err(_) => 0,
    }
}

extern "C" fn std_async_reactor_reset(ctx: *mut SpectraHostCallContext) -> i32 {
    let args = match host_call_void_args(ctx, 0) {
        Ok(args) => args,
        Err(status) => return status,
    };
    let _ = args;
    reactor::global().reset();
    if set_async_last_reactor_event(None).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    HOST_STATUS_SUCCESS
}

const CONCURRENT_POOL_INITIAL_CAPACITY: usize = 64;

struct ConcurrentChannel {
    queue: VecDeque<SpectraHostValue>,
    closed: bool,
}

struct ConcurrentRegistry {
    slots: Vec<Arc<OnceLock<SpectraHostValue>>>,
    free: Vec<usize>,
    next_fresh: usize,
    next_channel: SpectraHostValue,
    next_counter: SpectraHostValue,
    tasks_spawned: SpectraHostValue,
    channels: HashMap<SpectraHostValue, ConcurrentChannel>,
    counters: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl ConcurrentRegistry {
    fn new() -> Self {
        let mut slots = Vec::with_capacity(CONCURRENT_POOL_INITIAL_CAPACITY);
        slots.push(Arc::new(OnceLock::new()));
        Self {
            slots,
            free: Vec::new(),
            next_fresh: 1,
            next_channel: 1,
            next_counter: 1,
            tasks_spawned: 0,
            channels: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            if idx == 0 {
                continue;
            }
            *slot = Arc::new(OnceLock::new());
        }
        let total = self.slots.len();
        self.free.clear();
        self.free.extend(1..total);
        self.tasks_spawned = 0;
        self.channels.clear();
        self.counters.clear();
        self.next_channel = 1;
        self.next_counter = 1;
    }

    fn spawn(&mut self, value: SpectraHostValue) -> usize {
        self.tasks_spawned += 1;
        let slot_idx = if let Some(idx) = self.free.pop() {
            idx
        } else {
            let idx = self.next_fresh;
            self.next_fresh += 1;
            self.slots.push(Arc::new(OnceLock::new()));
            idx
        };
        let slot = &self.slots[slot_idx];
        debug_assert!(slot.get().is_none(), "slot pool invariant violated");
        let _ = slot.set(value);
        slot_idx
    }

    fn join(&mut self, task_id: usize) -> Result<SpectraHostValue, i32> {
        let slot = self.slots.get(task_id).ok_or(HOST_STATUS_NOT_FOUND)?;
        let value = slot.get().copied().ok_or(HOST_STATUS_NOT_FOUND)?;
        self.slots[task_id] = Arc::new(OnceLock::new());
        self.free.push(task_id);
        Ok(value)
    }

    fn is_done(&self, task_id: usize) -> Result<bool, i32> {
        let slot = self.slots.get(task_id).ok_or(HOST_STATUS_NOT_FOUND)?;
        Ok(slot.get().is_some())
    }
}

fn concurrent_registry() -> &'static Mutex<ConcurrentRegistry> {
    static REGISTRY: OnceLock<Mutex<ConcurrentRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ConcurrentRegistry::new()))
}

fn lock_concurrent_registry() -> Result<std::sync::MutexGuard<'static, ConcurrentRegistry>, i32> {
    concurrent_registry()
        .lock()
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)
}

extern "C" fn std_concurrent_task_spawn(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let value = args[0];
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let task_id = registry.spawn(value);
    results[0] = task_id as SpectraHostValue;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_task_join(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let task_id = args[0] as usize;
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    match registry.join(task_id) {
        Ok(value) => {
            results[0] = value;
            HOST_STATUS_SUCCESS
        }
        Err(code) => code,
    }
}

extern "C" fn std_concurrent_task_is_done(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let task_id = args[0] as usize;
    let registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    match registry.is_done(task_id) {
        Ok(done) => {
            results[0] = i64::from(done);
            HOST_STATUS_SUCCESS
        }
        Err(code) => code,
    }
}

extern "C" fn std_concurrent_channel_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let channel_id = registry.next_channel;
    registry.next_channel += 1;
    registry.channels.insert(
        channel_id,
        ConcurrentChannel {
            queue: VecDeque::new(),
            closed: false,
        },
    );
    results[0] = channel_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_channel_send(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.channels.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    if channel.closed {
        results[0] = 0;
        return HOST_STATUS_SUCCESS;
    }
    channel.queue.push_back(args[1]);
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_channel_recv(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.channels.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = channel.queue.pop_front().unwrap_or(-1);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_channel_len(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.channels.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = channel.queue.len() as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_channel_close(ctx: *mut SpectraHostCallContext) -> i32 {
    let args = match host_call_void_args(ctx, 1) {
        Ok(args) => args,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(channel) = registry.channels.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    channel.closed = true;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_counter_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let counter_id = registry.next_counter;
    registry.next_counter += 1;
    registry.counters.insert(counter_id, args[0]);
    results[0] = counter_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_counter_add(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(value) = registry.counters.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    *value += args[1];
    results[0] = *value;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_counter_get(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(value) = registry.counters.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = *value;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_pipeline_sum(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let start = args[0];
    let count = args[1].max(0);
    let workers = args[2].max(1).min(count.max(1));
    if count == 0 {
        results[0] = 0;
        return HOST_STATUS_SUCCESS;
    }

    let chunk_size = (count + workers - 1) / workers;
    let mut handles = Vec::new();
    for worker in 0..workers {
        let chunk_start = start + worker * chunk_size;
        let chunk_end = (chunk_start + chunk_size).min(start + count);
        if chunk_start >= chunk_end {
            continue;
        }
        handles.push(thread::spawn(move || {
            let mut sum = 0;
            for value in chunk_start..chunk_end {
                sum += value;
            }
            sum
        }));
    }

    let mut total = 0;
    for handle in handles {
        match handle.join() {
            Ok(partial) => total += partial,
            Err(_) => return HOST_STATUS_INTERNAL_ERROR,
        }
    }
    results[0] = total;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_stats_tasks_spawned(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    results[0] = registry.tasks_spawned;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_stats_channels(ctx: *mut SpectraHostCallContext) -> i32 {
    let (_, results) = match host_call_args(ctx, 0) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    results[0] = registry.channels.len() as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_reset(ctx: *mut SpectraHostCallContext) -> i32 {
    let _ = match host_call_void_args(ctx, 0) {
        Ok(args) => args,
        Err(status) => return status,
    };
    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.clear();
    HOST_STATUS_SUCCESS
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServeRequestState {
    Pending,
    Complete(SpectraHostValue),
    Cancelled,
}

struct ServeServer {
    model: SpectraHostValue,
    model_version: String,
    warm: bool,
    timeout: SpectraHostValue,
    queue: VecDeque<SpectraHostValue>,
    requests: HashMap<SpectraHostValue, (SpectraHostValue, ServeRequestState)>,
    input_policy: Option<(SpectraHostValue, SpectraHostValue)>,
    output_policy: Option<(SpectraHostValue, SpectraHostValue)>,
    rate_limit: Option<SpectraHostValue>,
    accepted_requests: SpectraHostValue,
    fallback: SpectraHostValue,
    last_diagnostic: String,
    audit_events: Vec<String>,
    total_requests: SpectraHostValue,
    completed_requests: SpectraHostValue,
    blocked_requests: SpectraHostValue,
    cancelled_requests: SpectraHostValue,
    error_count: SpectraHostValue,
    batch_count: SpectraHostValue,
    latency_samples_ms: Vec<SpectraHostValue>,
    observed_inputs: Vec<SpectraHostValue>,
    observed_outputs: Vec<SpectraHostValue>,
}

struct ServeRegistry {
    next_server: SpectraHostValue,
    next_request: SpectraHostValue,
    servers: HashMap<SpectraHostValue, ServeServer>,
}

impl ServeRegistry {
    fn new() -> Self {
        Self {
            next_server: 1,
            next_request: 1,
            servers: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }
}

fn serve_registry() -> &'static Mutex<ServeRegistry> {
    static REGISTRY: OnceLock<Mutex<ServeRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ServeRegistry::new()))
}

fn lock_serve_registry() -> Result<std::sync::MutexGuard<'static, ServeRegistry>, i32> {
    serve_registry()
        .lock()
        .map_err(|_| HOST_STATUS_INTERNAL_ERROR)
}

fn serve_guardrail_diagnostic(
    request_id: SpectraHostValue,
    stage: &str,
    policy: &str,
    value: SpectraHostValue,
    min: SpectraHostValue,
    max: SpectraHostValue,
    fallback: SpectraHostValue,
) -> String {
    format!(
        "{{\"schema\":\"spectra.serve.guardrail_diagnostic.v1\",\"request\":{},\"stage\":\"{}\",\"policy\":\"{}\",\"value\":{},\"min\":{},\"max\":{},\"fallback\":{}}}",
        request_id, stage, policy, value, min, max, fallback
    )
}

fn serve_audit_event(
    request_id: SpectraHostValue,
    event: &str,
    stage: &str,
    value: SpectraHostValue,
    result: SpectraHostValue,
) -> String {
    format!(
        "{{\"request\":{},\"event\":\"{}\",\"stage\":\"{}\",\"value\":{},\"result\":{}}}",
        request_id, event, stage, value, result
    )
}

fn serve_audit_json(server: &ServeServer) -> String {
    format!(
        "{{\"schema\":\"spectra.serve.audit.v1\",\"model\":{},\"warm\":{},\"accepted_requests\":{},\"events\":[{}]}}",
        server.model,
        i64::from(server.warm),
        server.accepted_requests,
        server.audit_events.join(",")
    )
}

fn serve_latency_for(input: SpectraHostValue, output: SpectraHostValue) -> SpectraHostValue {
    1 + (input.abs().saturating_add(output.abs()) % 17)
}

fn serve_record_block(server: &mut ServeServer, output: SpectraHostValue) {
    server.blocked_requests = server.blocked_requests.saturating_add(1);
    server.error_count = server.error_count.saturating_add(1);
    server.observed_outputs.push(output);
}

fn serve_record_complete(
    server: &mut ServeServer,
    input: SpectraHostValue,
    output: SpectraHostValue,
) {
    server.completed_requests = server.completed_requests.saturating_add(1);
    server.observed_outputs.push(output);
    server
        .latency_samples_ms
        .push(serve_latency_for(input, output));
}

fn serve_values_summary(values: &[SpectraHostValue]) -> (SpectraHostValue, SpectraHostValue, f64) {
    if values.is_empty() {
        return (0, 0, 0.0);
    }
    let min = *values.iter().min().unwrap_or(&0);
    let max = *values.iter().max().unwrap_or(&0);
    let sum = values.iter().map(|value| *value as f64).sum::<f64>();
    (min, max, sum / values.len() as f64)
}

fn serve_p95(values: &[SpectraHostValue]) -> SpectraHostValue {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let index = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

fn serve_distribution_summary_json(server: &ServeServer) -> String {
    let (input_min, input_max, input_mean) = serve_values_summary(&server.observed_inputs);
    let (output_min, output_max, output_mean) = serve_values_summary(&server.observed_outputs);
    format!(
        "{{\"schema\":\"spectra.serve.distribution_summary.v1\",\"model_version\":{},\"inputs\":{{\"count\":{},\"min\":{},\"max\":{},\"mean\":{}}},\"outputs\":{{\"count\":{},\"min\":{},\"max\":{},\"mean\":{}}}}}",
        ml_json_string(&server.model_version),
        server.observed_inputs.len(),
        input_min,
        input_max,
        ml_float_json(input_mean),
        server.observed_outputs.len(),
        output_min,
        output_max,
        ml_float_json(output_mean)
    )
}

fn serve_monitoring_snapshot_json(server: &ServeServer) -> String {
    let total_latency = server.latency_samples_ms.iter().sum::<SpectraHostValue>();
    let latency_avg = if server.latency_samples_ms.is_empty() {
        0.0
    } else {
        total_latency as f64 / server.latency_samples_ms.len() as f64
    };
    let error_rate = if server.total_requests <= 0 {
        0.0
    } else {
        server.error_count as f64 / server.total_requests as f64
    };
    let throughput = if total_latency <= 0 {
        server.completed_requests as f64
    } else {
        server.completed_requests as f64 / (total_latency as f64 / 1000.0)
    };
    format!(
        "{{\"schema\":\"spectra.serve.monitoring_snapshot.v1\",\"model\":{},\"model_version\":{},\"requests\":{},\"completed\":{},\"blocked\":{},\"cancelled\":{},\"errors\":{},\"error_rate\":{},\"batches\":{},\"pending\":{},\"latency_avg_ms\":{},\"latency_p95_ms\":{},\"throughput_per_second\":{}}}",
        server.model,
        ml_json_string(&server.model_version),
        server.total_requests,
        server.completed_requests,
        server.blocked_requests,
        server.cancelled_requests,
        server.error_count,
        ml_float_json(error_rate),
        server.batch_count,
        server.queue.len(),
        ml_float_json(latency_avg),
        serve_p95(&server.latency_samples_ms),
        ml_float_json(throughput)
    )
}

fn serve_json_number(source: &str, key: &str) -> Option<f64> {
    let start = source.find(key)? + key.len();
    let tail = &source[start..];
    let end =
        tail.find(|ch: char| !(ch.is_ascii_digit() || ch == '-' || ch == '.' || ch == '+'))?;
    tail[..end].parse::<f64>().ok()
}

fn serve_drift_json(
    reference: &str,
    live: &str,
    threshold_per_mille: SpectraHostValue,
) -> Option<String> {
    let ref_in = serve_json_number(reference, "\"inputs\":{\"count\":")?;
    let live_in = serve_json_number(live, "\"inputs\":{\"count\":")?;
    let ref_input_mean = serve_json_number(reference, "\"mean\":")?;
    let live_input_mean = serve_json_number(live, "\"mean\":")?;
    let ref_outputs = reference.find("\"outputs\"")?;
    let live_outputs = live.find("\"outputs\"")?;
    let ref_output_mean = serve_json_number(&reference[ref_outputs..], "\"mean\":")?;
    let live_output_mean = serve_json_number(&live[live_outputs..], "\"mean\":")?;
    let input_delta = (live_input_mean - ref_input_mean).abs();
    let output_delta = (live_output_mean - ref_output_mean).abs();
    let denom = ref_input_mean.abs().max(ref_output_mean.abs()).max(1.0);
    let score = ((input_delta + output_delta) / denom * 1000.0).round() as SpectraHostValue;
    let drifted = score > threshold_per_mille;
    Some(format!(
        "{{\"schema\":\"spectra.serve.drift_check.v1\",\"reference_count\":{},\"live_count\":{},\"input_mean_delta\":{},\"output_mean_delta\":{},\"score_per_mille\":{},\"threshold_per_mille\":{},\"drifted\":{}}}",
        ref_in as i64,
        live_in as i64,
        ml_float_json(input_delta),
        ml_float_json(output_delta),
        score.max(0),
        threshold_per_mille,
        if drifted { "true" } else { "false" }
    ))
}

extern "C" fn std_serve_server_new(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let server_id = registry.next_server;
    registry.next_server += 1;
    registry.servers.insert(
        server_id,
        ServeServer {
            model: args[0],
            model_version: format!("model-{}", args[0]),
            warm: false,
            timeout: 1,
            queue: VecDeque::new(),
            requests: HashMap::new(),
            input_policy: None,
            output_policy: None,
            rate_limit: None,
            accepted_requests: 0,
            fallback: -1,
            last_diagnostic:
                "{\"schema\":\"spectra.serve.guardrail_diagnostic.v1\",\"status\":\"ok\"}"
                    .to_string(),
            audit_events: Vec::new(),
            total_requests: 0,
            completed_requests: 0,
            blocked_requests: 0,
            cancelled_requests: 0,
            error_count: 0,
            batch_count: 0,
            latency_samples_ms: Vec::new(),
            observed_inputs: Vec::new(),
            observed_outputs: Vec::new(),
        },
    );
    results[0] = server_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_warmup(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.warm = true;
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_is_warm(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = i64::from(server.warm);
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_enqueue(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let request_id = registry.next_request;
    registry.next_request += 1;
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let input = args[1];
    server.total_requests = server.total_requests.saturating_add(1);
    server.observed_inputs.push(input);
    if let Some(limit) = server.rate_limit {
        if server.accepted_requests >= limit {
            server.last_diagnostic = serve_guardrail_diagnostic(
                request_id,
                "input",
                "rate_limit",
                server.accepted_requests + 1,
                0,
                limit,
                server.fallback,
            );
            server.audit_events.push(serve_audit_event(
                request_id,
                "blocked",
                "rate_limit",
                input,
                server.fallback,
            ));
            serve_record_block(server, server.fallback);
            server.requests.insert(
                request_id,
                (input, ServeRequestState::Complete(server.fallback)),
            );
            results[0] = request_id;
            return HOST_STATUS_SUCCESS;
        }
    }
    if let Some((min, max)) = server.input_policy {
        if input < min || input > max {
            server.last_diagnostic = serve_guardrail_diagnostic(
                request_id,
                "input",
                "range",
                input,
                min,
                max,
                server.fallback,
            );
            server.audit_events.push(serve_audit_event(
                request_id,
                "blocked",
                "input",
                input,
                server.fallback,
            ));
            serve_record_block(server, server.fallback);
            server.requests.insert(
                request_id,
                (input, ServeRequestState::Complete(server.fallback)),
            );
            results[0] = request_id;
            return HOST_STATUS_SUCCESS;
        }
    }
    server.accepted_requests += 1;
    server
        .requests
        .insert(request_id, (input, ServeRequestState::Pending));
    server.audit_events.push(serve_audit_event(
        request_id, "accepted", "input", input, input,
    ));
    server.queue.push_back(request_id);
    results[0] = request_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_cancel(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let Some((_, state)) = server.requests.get_mut(&args[1]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    if *state == ServeRequestState::Pending {
        *state = ServeRequestState::Cancelled;
        server.queue.retain(|request| *request != args[1]);
        server.cancelled_requests = server.cancelled_requests.saturating_add(1);
        server.error_count = server.error_count.saturating_add(1);
        server
            .audit_events
            .push(serve_audit_event(args[1], "cancelled", "request", 0, -1));
        results[0] = 1;
    } else {
        results[0] = 0;
    }
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_process_batch(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let max_batch = args[1].max(0) as usize;
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    if !server.warm || max_batch == 0 {
        results[0] = 0;
        return HOST_STATUS_SUCCESS;
    }
    server.batch_count = server.batch_count.saturating_add(1);

    let mut processed = 0;
    for _ in 0..max_batch {
        let Some(request_id) = server.queue.pop_front() else {
            break;
        };
        let Some((input, state)) = server.requests.get(&request_id) else {
            continue;
        };
        if *state != ServeRequestState::Pending {
            continue;
        }
        let input_value = *input;
        if server.timeout == 0 {
            if let Some((_, state)) = server.requests.get_mut(&request_id) {
                *state = ServeRequestState::Cancelled;
            }
            server.cancelled_requests = server.cancelled_requests.saturating_add(1);
            server.error_count = server.error_count.saturating_add(1);
            continue;
        }
        let output = input_value * server.model;
        if let Some((min, max)) = server.output_policy {
            if output < min || output > max {
                server.last_diagnostic = serve_guardrail_diagnostic(
                    request_id,
                    "output",
                    "range",
                    output,
                    min,
                    max,
                    server.fallback,
                );
                server.audit_events.push(serve_audit_event(
                    request_id,
                    "blocked",
                    "output",
                    output,
                    server.fallback,
                ));
                serve_record_block(server, server.fallback);
                if let Some((_, state)) = server.requests.get_mut(&request_id) {
                    *state = ServeRequestState::Complete(server.fallback);
                }
                processed += 1;
                continue;
            }
        }
        server.audit_events.push(serve_audit_event(
            request_id,
            "completed",
            "output",
            output,
            output,
        ));
        serve_record_complete(server, input_value, output);
        if let Some((_, state)) = server.requests.get_mut(&request_id) {
            *state = ServeRequestState::Complete(output);
        }
        processed += 1;
    }
    results[0] = processed;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_result(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let Some((_, state)) = server.requests.get(&args[1]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = match *state {
        ServeRequestState::Pending | ServeRequestState::Cancelled => -1,
        ServeRequestState::Complete(value) => value,
    };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_pending(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = server.queue.len() as i64;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_set_timeout(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.timeout = args[1].max(0);
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_resident_model(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    results[0] = server.model;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_benchmark(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let server_id = args[0];
    let requests = args[1].max(0);
    let batch = args[2].max(1);

    {
        let mut registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get_mut(&server_id) else {
            return HOST_STATUS_NOT_FOUND;
        };
        server.warm = true;
    }

    for input in 1..=requests {
        let mut registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let request_id = registry.next_request;
        registry.next_request += 1;
        let Some(server) = registry.servers.get_mut(&server_id) else {
            return HOST_STATUS_NOT_FOUND;
        };
        server.total_requests = server.total_requests.saturating_add(1);
        server.accepted_requests = server.accepted_requests.saturating_add(1);
        server.observed_inputs.push(input);
        server
            .requests
            .insert(request_id, (input, ServeRequestState::Pending));
        server.queue.push_back(request_id);
    }

    let mut processed_total = 0;
    loop {
        let mut registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get_mut(&server_id) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if server.queue.is_empty() {
            break;
        }
        server.batch_count = server.batch_count.saturating_add(1);
        let mut processed = 0;
        for _ in 0..batch {
            let Some(request_id) = server.queue.pop_front() else {
                break;
            };
            let Some((input, state)) = server.requests.get(&request_id) else {
                continue;
            };
            if *state != ServeRequestState::Pending {
                continue;
            }
            let input_value = *input;
            if server.timeout == 0 {
                if let Some((_, state)) = server.requests.get_mut(&request_id) {
                    *state = ServeRequestState::Cancelled;
                }
                server.cancelled_requests = server.cancelled_requests.saturating_add(1);
                server.error_count = server.error_count.saturating_add(1);
                continue;
            }
            let output = input_value * server.model;
            serve_record_complete(server, input_value, output);
            if let Some((_, state)) = server.requests.get_mut(&request_id) {
                *state = ServeRequestState::Complete(output);
            }
            processed += 1;
        }
        processed_total += processed;
    }
    results[0] = processed_total;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_set_input_policy(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] > args[2] {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.input_policy = Some((args[1], args[2]));
    server.audit_events.push(format!(
        "{{\"request\":0,\"event\":\"policy_attached\",\"stage\":\"input\",\"value\":{},\"result\":{}}}",
        args[1], args[2]
    ));
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_set_output_policy(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] > args[2] {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.output_policy = Some((args[1], args[2]));
    server.audit_events.push(format!(
        "{{\"request\":0,\"event\":\"policy_attached\",\"stage\":\"output\",\"value\":{},\"result\":{}}}",
        args[1], args[2]
    ));
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_set_rate_limit(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[1] <= 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.rate_limit = Some(args[1]);
    server.audit_events.push(format!(
        "{{\"request\":0,\"event\":\"policy_attached\",\"stage\":\"rate_limit\",\"value\":{},\"result\":{}}}",
        args[1], args[1]
    ));
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_set_fallback(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.fallback = args[1];
    server.audit_events.push(format!(
        "{{\"request\":0,\"event\":\"fallback_attached\",\"stage\":\"fallback\",\"value\":{},\"result\":{}}}",
        args[1], args[1]
    ));
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_last_diagnostic(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let diagnostic = {
        let registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get(&args[0]) else {
            return HOST_STATUS_NOT_FOUND;
        };
        server.last_diagnostic.clone()
    };
    results[0] = unsafe { alloc_spectra_string(&diagnostic) };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_audit_log(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let audit = {
        let registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get(&args[0]) else {
            return HOST_STATUS_NOT_FOUND;
        };
        serve_audit_json(server)
    };
    results[0] = unsafe { alloc_spectra_string(&audit) };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_set_model_version(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 2) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let Some(version) = ml_read_path_arg(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(server) = registry.servers.get_mut(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    server.model_version = version;
    server.audit_events.push(format!(
        "{{\"request\":0,\"event\":\"model_version_set\",\"stage\":\"monitoring\",\"value\":{},\"result\":{}}}",
        server.model, server.model
    ));
    results[0] = 1;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_monitoring_snapshot(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let snapshot = {
        let registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get(&args[0]) else {
            return HOST_STATUS_NOT_FOUND;
        };
        serve_monitoring_snapshot_json(server)
    };
    results[0] = unsafe { alloc_spectra_string(&snapshot) };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_server_distribution_summary(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let summary = {
        let registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get(&args[0]) else {
            return HOST_STATUS_NOT_FOUND;
        };
        serve_distribution_summary_json(server)
    };
    results[0] = unsafe { alloc_spectra_string(&summary) };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_drift_check(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 3) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    if args[2] < 0 {
        return HOST_STATUS_INVALID_ARGUMENT;
    }
    let Some(reference) = ml_read_path_arg(args[0]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(live) = ml_read_path_arg(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(drift) = serve_drift_json(&reference, &live, args[2]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    results[0] = unsafe { alloc_spectra_string(&drift) };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_export_monitoring(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 5) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let Some(path) = ml_read_path_arg(args[1]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(distribution) = ml_json_payload_arg(args[2]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(drift) = ml_json_payload_arg(args[3]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let Some(audit) = ml_json_payload_arg(args[4]) else {
        return HOST_STATUS_INVALID_ARGUMENT;
    };
    let snapshot = {
        let registry = match lock_serve_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(server) = registry.servers.get(&args[0]) else {
            return HOST_STATUS_NOT_FOUND;
        };
        serve_monitoring_snapshot_json(server)
    };
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() {
            return HOST_STATUS_INTERNAL_ERROR;
        }
    }
    let payload = format!(
        "{{\"schema\":\"spectra.serve.monitoring_export.v1\",\"snapshot\":{},\"distribution\":{},\"drift\":{},\"audit\":{}}}",
        snapshot, distribution, drift, audit
    );
    if std::fs::write(&path, payload).is_err() {
        return HOST_STATUS_INTERNAL_ERROR;
    }
    results[0] = unsafe { alloc_spectra_string(&path) };
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_reset(ctx: *mut SpectraHostCallContext) -> i32 {
    let _ = match host_call_void_args(ctx, 0) {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let mut registry = match lock_serve_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    registry.clear();
    HOST_STATUS_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::runtime_test_guard()
    }

    fn call_host(name: &str, args: &[SpectraHostValue]) -> (i32, SpectraHostValue) {
        let func = lookup_host_function(name).expect("host function not registered");
        let mut results = [0];
        let mut ctx = SpectraHostCallContext {
            args: if args.is_empty() {
                ptr::null()
            } else {
                args.as_ptr()
            },
            arg_len: args.len(),
            results: results.as_mut_ptr(),
            result_len: 1,
            invoke_fn: None,
        };
        let status = func(&mut ctx);
        (status, results[0])
    }

    fn call_host_without_results(name: &str, args: &[SpectraHostValue]) -> i32 {
        let func = lookup_host_function(name).expect("host function not registered");
        let mut ctx = SpectraHostCallContext {
            args: if args.is_empty() {
                ptr::null()
            } else {
                args.as_ptr()
            },
            arg_len: args.len(),
            results: ptr::null_mut(),
            result_len: 0,
            invoke_fn: None,
        };
        func(&mut ctx)
    }

    fn test_string(value: &str) -> SpectraHostValue {
        unsafe { alloc_spectra_string(value) }
    }

    fn write_test_npy(path: &std::path::Path, values: &[f64]) {
        let mut header = format!(
            "{{'descr': '<f8', 'fortran_order': False, 'shape': ({},), }}",
            values.len()
        );
        let preamble_len = 10usize;
        let padding = (16 - ((preamble_len + header.len() + 1) % 16)) % 16;
        header.push_str(&" ".repeat(padding));
        header.push('\n');
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x93NUMPY");
        bytes.push(1);
        bytes.push(0);
        bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
        bytes.extend_from_slice(header.as_bytes());
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).expect("write test npy");
    }

    fn temp_test_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "spectra_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ))
    }

    #[test]
    fn std_time_duration_and_instant_handles_are_real() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let (status, five_ms) = call_host(TIME_DURATION_MS, &[5]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(five_ms > 0);

        let (status, one_sec) = call_host(TIME_DURATION_SECS, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, sum) = call_host(TIME_DURATION_ADD, &[five_ms, one_sec]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, millis) = call_host(TIME_DURATION_MILLIS, &[sum]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(millis, 1_005);
        let (status, secs) = call_host(TIME_DURATION_SECS_VALUE, &[sum]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(secs, 1);

        let (status, start) = call_host(TIME_INSTANT_NOW, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        std::thread::sleep(Duration::from_millis(2));
        let (status, elapsed) = call_host(TIME_INSTANT_ELAPSED_MS, &[start]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(elapsed >= 1);

        let (status, one_ms) = call_host(TIME_DURATION_MS, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, deadline) = call_host(TIME_INSTANT_ADD, &[start, one_ms]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, elapsed) = call_host(TIME_INSTANT_HAS_ELAPSED, &[deadline]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(elapsed, 1);
    }

    #[test]
    fn std_time_invalid_handles_and_negative_durations_return_status() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let (status, _) = call_host(TIME_DURATION_MS, &[-1]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);
        let (status, _) = call_host(TIME_DURATION_MILLIS, &[999_999]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);
        let status = call_host_without_results(TIME_SLEEP, &[999_999]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);

        let (status, lhs) = call_host(TIME_DURATION_MS, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rhs) = call_host(TIME_DURATION_MS, &[2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TIME_DURATION_SUB, &[lhs, rhs]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn std_time_utc_calendar_boundaries_are_deterministic() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let (status, epoch) = call_host(TIME_UNIX_TO_UTC, &[0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TIME_UTC_YEAR, &[epoch]),
            (HOST_STATUS_SUCCESS, 1970)
        );
        assert_eq!(
            call_host(TIME_UTC_MONTH, &[epoch]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(call_host(TIME_UTC_DAY, &[epoch]), (HOST_STATUS_SUCCESS, 1));

        let leap_day = 1_582_934_400;
        let (status, leap) = call_host(TIME_UNIX_TO_UTC, &[leap_day]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TIME_UTC_YEAR, &[leap]),
            (HOST_STATUS_SUCCESS, 2020)
        );
        assert_eq!(call_host(TIME_UTC_MONTH, &[leap]), (HOST_STATUS_SUCCESS, 2));
        assert_eq!(call_host(TIME_UTC_DAY, &[leap]), (HOST_STATUS_SUCCESS, 29));

        let boundary = 1_609_459_199;
        let (status, end_2020) = call_host(TIME_UNIX_TO_UTC, &[boundary]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TIME_UTC_SECOND, &[end_2020]),
            (HOST_STATUS_SUCCESS, 59)
        );
    }

    #[test]
    fn std_range_handles_are_value_semantic_and_bounded() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let (status, exclusive) = call_host(RANGE_CREATE, &[2, 5, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(RANGE_LEN, &[exclusive]), (HOST_STATUS_SUCCESS, 3));
        assert_eq!(
            call_host(RANGE_AT, &[exclusive, 0]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(RANGE_AT, &[exclusive, 2]),
            (HOST_STATUS_SUCCESS, 4)
        );
        assert_eq!(
            call_host(RANGE_START, &[exclusive]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(call_host(RANGE_END, &[exclusive]), (HOST_STATUS_SUCCESS, 5));
        assert_eq!(
            call_host(RANGE_IS_INCLUSIVE, &[exclusive]),
            (HOST_STATUS_SUCCESS, 0)
        );

        let (status, inclusive) = call_host(RANGE_CREATE, &[2, 5, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(RANGE_LEN, &[inclusive]), (HOST_STATUS_SUCCESS, 4));
        assert_eq!(
            call_host(RANGE_AT, &[inclusive, 3]),
            (HOST_STATUS_SUCCESS, 5)
        );
        assert_eq!(
            call_host(RANGE_IS_INCLUSIVE, &[inclusive]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, same) = call_host(RANGE_CREATE, &[2, 5, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(RANGE_EQ, &[inclusive, same]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(RANGE_EQ, &[inclusive, exclusive]),
            (HOST_STATUS_SUCCESS, 0)
        );

        let (status, empty) = call_host(RANGE_CREATE, &[5, 2, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(RANGE_LEN, &[empty]), (HOST_STATUS_SUCCESS, 0));
    }

    #[test]
    fn std_range_invalid_handles_and_indexes_return_status() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(
            call_host(RANGE_LEN, &[999_999]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(RANGE_AT, &[999_999, 0]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );

        let (status, range) = call_host(RANGE_CREATE, &[0, 2, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(RANGE_AT, &[range, -1]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(RANGE_AT, &[range, 2]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(RANGE_CREATE, &[0, 1, 2]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
    }

    #[test]
    fn string_eq_host_function_compares_string_values() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let left = test_string("ok");
        let same_value = test_string("ok");
        let different = test_string("error");

        let (status, result) = call_host(STR_EQ, &[left, same_value]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(result, 1);

        let (status, result) = call_host(STR_EQ, &[left, different]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(result, 0);

        let func = lookup_host_function(STR_EQ).expect("string eq not registered");
        assert_eq!(func(ptr::null_mut()), HOST_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn r2005_invalid_host_contexts_return_status_without_panics() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(
            call_host_without_results(MAP_NEW, &[]),
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host_without_results(TENSOR_ONES, &[4]),
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host_without_results(ASYNC_CHANNEL_NEW, &[1]),
            HOST_STATUS_INVALID_ARGUMENT
        );

        let (status, value) = call_host(ASYNC_CHANNEL_SEND, &[999_999, 1]);
        assert_eq!(status, HOST_STATUS_NOT_FOUND);
        assert_eq!(value, 0);

        let (status, value) = call_host(TENSOR_GET, &[999_999, 0]);
        assert_eq!(status, HOST_STATUS_NOT_FOUND);
        assert_eq!(value, 0);
    }

    #[test]
    fn r2005_poisoned_runtime_locks_recover_without_panics() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let _ = std::thread::spawn(|| {
            let _guard = random_state()
                .lock()
                .expect("lock random state for poisoning");
            panic!("intentional R-2005 random-state poison");
        })
        .join();
        std::panic::set_hook(previous_hook);

        assert_eq!(
            call_host(TENSOR_SET_DETERMINISTIC_MODE, &[1]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_DETERMINISTIC_MODE, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(TENSOR_SET_DETERMINISTIC_MODE, &[0]),
            (HOST_STATUS_SUCCESS, 0)
        );
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
        let args = [0, 1, 0, 2, 0, 3];
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
    fn option_result_unwrap_wrong_variant_returns_host_status() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let none = [1_i64, 0_i64];
        let some = [0_i64, 42_i64];
        let ok = [0_i64, 7_i64];
        let err = [1_i64, 9_i64];

        assert_eq!(
            call_host(OPTION_UNWRAP, &[none.as_ptr() as SpectraHostValue]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(OPTION_UNWRAP, &[0]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(RESULT_UNWRAP, &[err.as_ptr() as SpectraHostValue]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(RESULT_UNWRAP_ERR, &[ok.as_ptr() as SpectraHostValue]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(RESULT_UNWRAP_ERR, &[0]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );

        assert_eq!(
            call_host(OPTION_UNWRAP, &[some.as_ptr() as SpectraHostValue]),
            (HOST_STATUS_SUCCESS, 42)
        );
        assert_eq!(
            call_host(RESULT_UNWRAP, &[ok.as_ptr() as SpectraHostValue]),
            (HOST_STATUS_SUCCESS, 7)
        );
        assert_eq!(
            call_host(RESULT_UNWRAP_ERR, &[err.as_ptr() as SpectraHostValue]),
            (HOST_STATUS_SUCCESS, 9)
        );
    }

    #[test]
    fn fs_write_append_and_overwrite_create_nested_parents() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let dir = temp_test_dir("fs_nested");
        let path = dir.join("level1").join("level2").join("artifact.txt");
        let path_arg = test_string(path.to_string_lossy().as_ref());

        assert_eq!(
            call_host(FS_WRITE, &[path_arg, test_string("first")]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(std::fs::read_to_string(&path).expect("read first"), "first");

        assert_eq!(
            call_host(FS_APPEND, &[path_arg, test_string("-second")]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read appended"),
            "first-second"
        );

        assert_eq!(
            call_host(FS_WRITE, &[path_arg, test_string("overwrite")]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read overwritten"),
            "overwrite"
        );

        let (status, read_ptr) = call_host(FS_READ, &[path_arg]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let read_back = unsafe { read_spectra_string(read_ptr) }.expect("fs read string");
        assert_eq!(read_back, "overwrite");

        assert_eq!(call_host(FS_EXISTS, &[path_arg]), (HOST_STATUS_SUCCESS, 1));
        assert_eq!(call_host(FS_REMOVE, &[path_arg]), (HOST_STATUS_SUCCESS, 1));
        assert_eq!(call_host(FS_EXISTS, &[path_arg]), (HOST_STATUS_SUCCESS, 0));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fs_invalid_paths_return_safe_values_without_panicking() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        let empty = test_string("");
        assert_eq!(
            call_host(FS_WRITE, &[empty, test_string("ignored")]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(FS_APPEND, &[empty, test_string("ignored")]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(call_host(FS_EXISTS, &[empty]), (HOST_STATUS_SUCCESS, 0));
        assert_eq!(call_host(FS_REMOVE, &[empty]), (HOST_STATUS_SUCCESS, 0));

        let dir = temp_test_dir("fs_blocked_parent");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "not a directory").expect("write blocker");
        let child = blocker.join("child.txt");
        let child_arg = test_string(child.to_string_lossy().as_ref());

        assert_eq!(
            call_host(FS_WRITE, &[child_arg, test_string("payload")]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(FS_APPEND, &[child_arg, test_string("payload")]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert!(!child.exists());
        std::fs::remove_dir_all(&dir).ok();
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

    #[test]
    fn tensor_runtime_lifecycle_and_elementwise_ops() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let (status, a) = call_host(TENSOR_FULL, &[4, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_ARANGE, &[1, 5, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        let (status, c) = call_host(TENSOR_ADD, &[a, b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, len) = call_host(TENSOR_LEN, &[c]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(len, 4);
        let (status, first) = call_host(TENSOR_GET, &[c, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(first, 3);
        let (status, sum) = call_host(TENSOR_SUM, &[c]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(sum, 18);

        let (status, freed) = call_host(TENSOR_FREE_ALL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(freed >= 3);
    }

    #[test]
    fn tensor_runtime_reshape_matmul_and_float_reduction() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let (status, a) = call_host(TENSOR_ARANGE, &[1, 7, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, a2) = call_host(TENSOR_RESHAPE, &[a, 2, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_ONES2, &[3, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, product) = call_host(TENSOR_MATMUL, &[a2, b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rows) = call_host(TENSOR_ROWS, &[product]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(rows, 2);
        let (status, cols) = call_host(TENSOR_COLS, &[product]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(cols, 2);
        let (status, p00) = call_host(TENSOR_GET2, &[product, 0, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(p00, 6);
        let (status, p10) = call_host(TENSOR_GET2, &[product, 1, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(p10, 15);

        let two = 2.0f64.to_bits() as i64;
        let (status, floats) = call_host(TENSOR_FULL_F, &[4, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, mean_bits) = call_host(TENSOR_MEAN_F, &[floats]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(f64::from_bits(mean_bits as u64), 2.0);

        let (status, freed) = call_host(TENSOR_FREE_ALL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(freed >= 5);
    }

    #[test]
    fn tensor_runtime_phase3_views_transforms_and_shape_errors() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let (status, base) = call_host(TENSOR_ARANGE, &[1, 7, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, matrix) = call_host(TENSOR_RESHAPE, &[base, 2, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, transposed) = call_host(TENSOR_TRANSPOSE, &[matrix]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, t01) = call_host(TENSOR_GET2, &[transposed, 0, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(t01, 4);

        let (status, permuted) = call_host(TENSOR_PERMUTE, &[matrix, 0, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, p10) = call_host(TENSOR_GET2, &[permuted, 1, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(p10, 2);

        let (status, slice) = call_host(TENSOR_SLICE, &[base, 2, 5]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_FREE, &[base]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, slice_sum) = call_host(TENSOR_SUM, &[slice]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(slice_sum, 12);

        let (status, _) = call_host(TENSOR_SET, &[slice, 0, 99]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, slice_first) = call_host(TENSOR_GET, &[slice, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(slice_first, 99);
        let (status, matrix_original_value) = call_host(TENSOR_GET2, &[matrix, 0, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(matrix_original_value, 3);

        let (status, invalid_axis) = call_host(TENSOR_PERMUTE, &[matrix, 0, 4]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);
        assert_eq!(invalid_axis, 0);
        let (status, invalid_shape) = call_host(TENSOR_RESHAPE, &[matrix, 4, 4]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);
        assert_eq!(invalid_shape, 0);

        let (status, freed) = call_host(TENSOR_FREE_ALL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(freed >= 4);
    }

    #[test]
    fn tensor_runtime_phase3_concat_stack_argmax_and_batched_matmul() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let (status, a) = call_host(TENSOR_ARANGE, &[1, 4, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_ARANGE, &[4, 7, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, joined) = call_host(TENSOR_CONCAT, &[a, b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, joined_len) = call_host(TENSOR_LEN, &[joined]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(joined_len, 6);
        let (status, max_index) = call_host(TENSOR_ARGMAX, &[joined]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(max_index, 5);

        let (status, left) = call_host(TENSOR_RESHAPE, &[a, 1, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, right) = call_host(TENSOR_RESHAPE, &[b, 1, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, stacked) = call_host(TENSOR_STACK, &[left, right]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rank) = call_host(TENSOR_RANK, &[stacked]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(rank, 3);
        let (status, dim0) = call_host(TENSOR_DIM, &[stacked, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(dim0, 2);

        let (status, lhs_flat) = call_host(TENSOR_ARANGE, &[1, 9, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rhs_flat) = call_host(TENSOR_ONES, &[8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, lhs_batch) = call_host(TENSOR_STACK, &[lhs_flat, lhs_flat]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _rhs_batch) = call_host(TENSOR_STACK, &[rhs_flat, rhs_flat]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, lhs3) = call_host(TENSOR_RESHAPE, &[lhs_batch, 3, 5]);
        assert_eq!(status, HOST_STATUS_INVALID_ARGUMENT);
        assert_eq!(lhs3, 0);

        let (status, lhs2) = call_host(TENSOR_RESHAPE, &[lhs_flat, 2, 4]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rhs2) = call_host(TENSOR_RESHAPE, &[rhs_flat, 4, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, lhs_stacked) = call_host(TENSOR_STACK, &[lhs2, lhs2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rhs_stacked) = call_host(TENSOR_STACK, &[rhs2, rhs2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, batched) = call_host(TENSOR_MATMUL_BATCHED, &[lhs_stacked, rhs_stacked]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, batched_rank) = call_host(TENSOR_RANK, &[batched]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(batched_rank, 3);
        let (status, first_value) = call_host(TENSOR_GET, &[batched, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(first_value, 10);

        let (status, freed) = call_host(TENSOR_FREE_ALL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(freed >= 10);
    }

    #[test]
    fn tensor_autodiff_elementwise_reduction_and_finite_difference() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);

        let three = 3.0f64.to_bits() as i64;
        let (status, x) = call_host(TENSOR_FULL_F, &[3, three]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, x) = call_host(TENSOR_REQUIRES_GRAD, &[x, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, y) = call_host(TENSOR_MUL, &[x, x]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss) = call_host(TENSOR_SUM_T, &[y]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_BACKWARD, &[loss]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad) = call_host(TENSOR_GRAD, &[x]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, g0_bits) = call_host(TENSOR_GET_F, &[grad, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(g0_bits as u64) - 6.0).abs() < 1e-12);
        let (status, graph_nodes) = call_host(TENSOR_STATS_GRAPH_NODES, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(graph_nodes, 0);

        let epsilon = 1e-4f64;
        let plus = (3.0 + epsilon).powi(2) * 3.0;
        let minus = (3.0 - epsilon).powi(2) * 3.0;
        let finite_difference = (plus - minus) / (2.0 * epsilon);
        assert!((finite_difference - 18.0).abs() < 1e-8);

        let (status, grad_sum_bits) = call_host(TENSOR_SUM_F, &[grad]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(grad_sum_bits as u64) - finite_difference).abs() < 1e-8);
    }

    #[test]
    fn tensor_autodiff_matmul_transpose_dot_and_inference_mode() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);

        let one = 1.0f64.to_bits() as i64;
        let two = 2.0f64.to_bits() as i64;
        let (status, a) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_FULL_F, &[4, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, a) = call_host(TENSOR_REQUIRES_GRAD, &[a, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_REQUIRES_GRAD, &[b, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, a2) = call_host(TENSOR_RESHAPE, &[a, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b2) = call_host(TENSOR_RESHAPE, &[b, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, product) = call_host(TENSOR_MATMUL, &[a2, b2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, transposed) = call_host(TENSOR_TRANSPOSE, &[product]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss) = call_host(TENSOR_SUM_T, &[transposed]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_BACKWARD, &[loss]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        let (status, grad_a) = call_host(TENSOR_GRAD, &[a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_b) = call_host(TENSOR_GRAD, &[b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_a0_bits) = call_host(TENSOR_GET_F, &[grad_a, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_b0_bits) = call_host(TENSOR_GET_F, &[grad_b, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(grad_a0_bits as u64) - 4.0).abs() < 1e-12);
        assert!((f64::from_bits(grad_b0_bits as u64) - 2.0).abs() < 1e-12);

        let (status, _) = call_host(TENSOR_ZERO_GRAD, &[a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_SET_GRAD_ENABLED, &[0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, enabled) = call_host(TENSOR_GRAD_ENABLED, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(enabled, 0);
        let (status, no_grad_product) = call_host(TENSOR_MUL, &[a, a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, no_grad_loss) = call_host(TENSOR_SUM_T, &[no_grad_product]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_BACKWARD, &[no_grad_loss]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_GRAD, &[a]);
        assert_eq!(status, HOST_STATUS_NOT_FOUND);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);

        let (status, v1) = call_host(TENSOR_FULL_F, &[2, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, v2) = call_host(TENSOR_FULL_F, &[2, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, v1) = call_host(TENSOR_REQUIRES_GRAD, &[v1, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, dot_loss) = call_host(TENSOR_DOT_T, &[v1, v2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_BACKWARD, &[dot_loss]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_v1) = call_host(TENSOR_GRAD, &[v1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_v1_0_bits) = call_host(TENSOR_GET_F, &[grad_v1, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(grad_v1_0_bits as u64) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn ml_phase6_mlp_layers_losses_optimizers_and_dataloader() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);

        let one = 1.0f64.to_bits() as i64;
        let two = 2.0f64.to_bits() as i64;
        let zero = 0.0f64.to_bits() as i64;
        let lr = 0.1f64.to_bits() as i64;

        let (status, module) = call_host(ML_MODULE_NEW, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, features) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, features2d) = call_host(TENSOR_RESHAPE, &[features, 4, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, target) = call_host(TENSOR_FULL_F, &[4, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, weight0) = call_host(
            TENSOR_REQUIRES_GRAD,
            &[call_host(TENSOR_FULL_F, &[1, zero]).1, 1],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, weight) = call_host(TENSOR_RESHAPE, &[weight0, 1, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bias) = call_host(
            TENSOR_REQUIRES_GRAD,
            &[call_host(TENSOR_FULL_F, &[1, zero]).1, 1],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_MODULE_ADD_PARAMETER, &[module, weight]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ML_MODULE_ADD_PARAMETER, &[module, bias]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ML_MODULE_PARAMETER_COUNT, &[module]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ML_MODULE_PARAMETER, &[module, 0]),
            (HOST_STATUS_SUCCESS, weight)
        );

        let (status, dataset) = call_host(ML_DATASET_FROM_TENSORS, &[features2d, target, 4]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[dataset]),
            (HOST_STATUS_SUCCESS, 4)
        );
        let (status, loader) = call_host(ML_DATALOADER_NEW, &[dataset, 2, 123]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATALOADER_BATCH_COUNT, &[loader]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ML_DATALOADER_BATCH_FEATURES, &[loader, 0]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ML_DATALOADER_BATCH_LABELS, &[loader, 0]).0,
            HOST_STATUS_SUCCESS
        );

        for _ in 0..40 {
            let (status, pred) = call_host(ML_LINEAR, &[features2d, weight, bias]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            let (status, loss) = call_host(ML_MSE_LOSS, &[pred, target]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            assert_eq!(call_host(TENSOR_BACKWARD, &[loss]).0, HOST_STATUS_SUCCESS);
            assert_eq!(call_host(ML_SGD_STEP, &[weight, lr]).0, HOST_STATUS_SUCCESS);
            assert_eq!(call_host(ML_SGD_STEP, &[bias, lr]).0, HOST_STATUS_SUCCESS);
        }
        let (status, pred) = call_host(ML_LINEAR, &[features2d, weight, bias]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss) = call_host(ML_MSE_LOSS, &[pred, target]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss_bits) = call_host(TENSOR_GET_F, &[loss, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(f64::from_bits(loss_bits as u64) < 0.001);

        let (status, probs) = call_host(TENSOR_FULL_F, &[4, 0.75f64.to_bits() as i64]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, labels) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_BCE_LOSS, &[probs, labels]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ML_EXP_LR, &[lr, 0.5f64.to_bits() as i64, 1]).0,
            HOST_STATUS_SUCCESS
        );
    }

    #[test]
    fn ml_phase6_cnn_conv2d_and_adamw_train_end_to_end() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);

        let one = 1.0f64.to_bits() as i64;
        let zero = 0.0f64.to_bits() as i64;
        let lr = 0.05f64.to_bits() as i64;

        let (status, input) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, target) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, kernel) = call_host(
            TENSOR_REQUIRES_GRAD,
            &[call_host(TENSOR_FULL_F, &[1, zero]).1, 1],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bias) = call_host(
            TENSOR_REQUIRES_GRAD,
            &[call_host(TENSOR_FULL_F, &[1, zero]).1, 1],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, m) = call_host(TENSOR_FULL_F, &[1, zero]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, v) = call_host(TENSOR_FULL_F, &[1, zero]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        for step in 1..=50 {
            let (status, pred) = call_host(ML_CONV2D, &[input, kernel, bias, 1, 1, 2, 2, 1, 1, 1]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            let (status, loss) = call_host(ML_MSE_LOSS, &[pred, target]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            assert_eq!(call_host(TENSOR_BACKWARD, &[loss]).0, HOST_STATUS_SUCCESS);
            assert_eq!(
                call_host(
                    ML_ADAMW_STEP,
                    &[
                        kernel,
                        m,
                        v,
                        lr,
                        0.9f64.to_bits() as i64,
                        0.999f64.to_bits() as i64,
                        1e-8f64.to_bits() as i64,
                        step,
                        0.0f64.to_bits() as i64,
                    ],
                )
                .0,
                HOST_STATUS_SUCCESS
            );
            assert_eq!(call_host(ML_SGD_STEP, &[bias, lr]).0, HOST_STATUS_SUCCESS);
        }

        let (status, pred) = call_host(ML_CONV2D, &[input, kernel, bias, 1, 1, 2, 2, 1, 1, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss) = call_host(ML_MSE_LOSS, &[pred, target]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss_bits) = call_host(TENSOR_GET_F, &[loss, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(f64::from_bits(loss_bits as u64) < 0.01);
        assert_eq!(
            call_host(ML_DROPOUT, &[pred, 0.5f64.to_bits() as i64, 0]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ML_MAX_POOL2D, &[input, 1, 1, 2, 2, 2, 2]).0,
            HOST_STATUS_SUCCESS
        );
    }

    #[test]
    fn tensor_runtime_phase4_kernels_rng_and_metrics() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_RESET_STATS, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);

        let (status, a) = call_host(TENSOR_ARANGE, &[1, 5, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_ARANGE, &[1, 5, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, dot) = call_host(TENSOR_DOT, &[a, b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(dot, 30);

        let (status, neg) = call_host(TENSOR_NEG, &[a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, first_neg) = call_host(TENSOR_GET, &[neg, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(first_neg, -1);

        let (status, matrix) = call_host(TENSOR_RESHAPE, &[a, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, transposed) = call_host(TENSOR_TRANSPOSE, &[matrix]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, t01) = call_host(TENSOR_GET2, &[transposed, 0, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(t01, 3);

        let _ = call_host(TENSOR_SEED, &[123]);
        let (status, r1) = call_host(TENSOR_UNIFORM, &[8, 0, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let _ = call_host(TENSOR_SEED, &[123]);
        let (status, r2) = call_host(TENSOR_UNIFORM, &[8, 0, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, r1_first) = call_host(TENSOR_GET, &[r1, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, r2_first) = call_host(TENSOR_GET, &[r2, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(r1_first, r2_first);

        let (status, kernel_ops) = call_host(TENSOR_STATS_KERNEL_OPS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(kernel_ops >= 5);
        let (status, peak_bytes) = call_host(TENSOR_STATS_PEAK_BYTES, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(peak_bytes > 0);
        let (status, strategy) = call_host(TENSOR_KERNEL_STRATEGY, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(strategy >= TensorKernelStrategy::Scalar.code());

        let (status, freed) = call_host(TENSOR_FREE_ALL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(freed >= 7);
        let (status, reused) = call_host(TENSOR_ZEROS, &[4]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, pool_hits) = call_host(TENSOR_STATS_POOL_HITS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(pool_hits > 0);
        let (status, active_bytes) = call_host(TENSOR_STATS_ACTIVE_BYTES, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(active_bytes > 0);
        let (status, _) = call_host(TENSOR_FREE, &[reused]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
    }

    #[test]
    fn tensor_runtime_phase15_deterministic_mode_and_tolerances() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        assert_eq!(
            call_host(TENSOR_SET_DETERMINISTIC_MODE, &[1]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_DETERMINISTIC_MODE, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, abs_bits) = call_host(TENSOR_TOLERANCE_ABS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(f64::from_bits(abs_bits as u64), NUMERICAL_TOLERANCE_ABS);
        let (status, rel_bits) = call_host(TENSOR_TOLERANCE_REL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(f64::from_bits(rel_bits as u64), NUMERICAL_TOLERANCE_REL);

        assert_eq!(
            call_host(TENSOR_SET_DETERMINISTIC_MODE, &[0]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_DETERMINISTIC_MODE, &[]),
            (HOST_STATUS_SUCCESS, 0)
        );
    }

    #[test]
    fn tensor_runtime_phase15_memory_report_tracks_lifetimes_sites_and_reuse() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_RESET_STATS, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[0]);

        let one = 1.0f64.to_bits() as i64;
        let (status, a) = call_host(TENSOR_FULL_F, &[16, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_RELU, &[a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_FREE, &[b]).0, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_FREE, &[a]).0, HOST_STATUS_SUCCESS);
        let (status, reused) = call_host(TENSOR_FULL_F, &[16, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_FREE, &[reused]).0, HOST_STATUS_SUCCESS);

        let (status, lifetimes) = call_host(TENSOR_STATS_LIFETIME_RECORDS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(lifetimes >= 3);
        let (status, released) = call_host(TENSOR_STATS_RELEASED_LIFETIMES, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(released >= 3);
        let (status, sites) = call_host(TENSOR_STATS_ALLOCATION_SITES, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(sites > 0);
        let (status, reuse_rate) = call_host(TENSOR_STATS_REUSE_RATE_PER_MILLE, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, pool_hits) = call_host(TENSOR_STATS_POOL_HITS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, pool_misses) = call_host(TENSOR_STATS_POOL_MISSES, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(
            reuse_rate > 0,
            "expected buffer reuse in inference-only memory planner test; pool_hits={pool_hits}, pool_misses={pool_misses}, reuse_rate={reuse_rate}"
        );

        let (status, report_ptr) = call_host(TENSOR_MEMORY_REPORT, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let report =
            unsafe { read_spectra_string(report_ptr) }.expect("valid memory report string");
        assert!(report.contains("\"schema\":\"spectra.tensor.memory_report.v1\""));
        assert!(report.contains("\"allocation_site\""));
        assert!(report.contains("\"release_step\""));
        assert!(report.contains("\"reuse_rate_per_mille\""));
        assert!(report.contains("\"tensors\""));
    }

    #[test]
    fn ml_phase17_dataset_dataframe_file_loaders_transforms_and_splits() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let dir = std::env::temp_dir().join(format!(
            "spectra_r1701_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp data dir");
        let csv = dir.join("tabular.csv");
        std::fs::write(
            &csv,
            "f0,f1,label\n1.0,2.0,0.0\n2.0,3.0,1.0\n3.0,4.0,1.0\n4.0,5.0,0.0\n",
        )
        .expect("write csv");
        let jsonl = dir.join("rows.jsonl");
        std::fs::write(
            &jsonl,
            "{\"features\":[1.0,2.0],\"label\":0.0}\n{\"features\":[2.0,3.0],\"label\":1.0}\n",
        )
        .expect("write jsonl");
        let features_npy = dir.join("features.npy");
        let labels_npy = dir.join("labels.npy");
        write_test_npy(&features_npy, &[1.0, 2.0, 2.0, 3.0]);
        write_test_npy(&labels_npy, &[0.0, 1.0]);
        let directory_dataset = dir.join("directory_dataset");
        std::fs::create_dir_all(&directory_dataset).expect("create directory dataset");
        std::fs::write(
            directory_dataset.join("features.csv"),
            "x0,x1\n1.0,2.0\n2.0,3.0\n3.0,4.0\n",
        )
        .expect("write directory features");
        std::fs::write(directory_dataset.join("labels.csv"), "y\n0.0\n1.0\n1.0\n")
            .expect("write directory labels");

        let csv_path = test_string(csv.to_string_lossy().as_ref());
        let (status, dataset) = call_host(ML_DATASET_FROM_CSV, &[csv_path, 2, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[dataset]),
            (HOST_STATUS_SUCCESS, 4)
        );

        let (status, mapped) = call_host(
            ML_DATASET_MAP_FEATURES,
            &[dataset, 2.0f64.to_bits() as i64, 1.0f64.to_bits() as i64],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, filtered) = call_host(
            ML_DATASET_FILTER_LABEL_MIN,
            &[mapped, 1.0f64.to_bits() as i64],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[filtered]),
            (HOST_STATUS_SUCCESS, 2)
        );
        let (status, train) = call_host(ML_DATASET_TRAIN_SPLIT, &[dataset, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, test) = call_host(ML_DATASET_TEST_SPLIT, &[dataset, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[train]),
            (HOST_STATUS_SUCCESS, 3)
        );
        assert_eq!(call_host(ML_DATASET_LEN, &[test]), (HOST_STATUS_SUCCESS, 1));

        let (status, loader) = call_host(ML_DATALOADER_NEW, &[filtered, 1, 123]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATALOADER_BATCH_COUNT, &[loader]),
            (HOST_STATUS_SUCCESS, 2)
        );
        let (status, batch_x) = call_host(ML_DATALOADER_BATCH_FEATURES, &[loader, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, first_feature_bits) = call_host(TENSOR_GET_F, &[batch_x, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(f64::from_bits(first_feature_bits as u64).is_finite());

        let jsonl_path = test_string(jsonl.to_string_lossy().as_ref());
        let (status, jsonl_dataset) = call_host(ML_DATASET_FROM_JSONL, &[jsonl_path]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[jsonl_dataset]),
            (HOST_STATUS_SUCCESS, 2)
        );

        let features_npy_path = test_string(features_npy.to_string_lossy().as_ref());
        let labels_npy_path = test_string(labels_npy.to_string_lossy().as_ref());
        let (status, npy_dataset) = call_host(
            ML_DATASET_FROM_NPY,
            &[features_npy_path, labels_npy_path, 2],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[npy_dataset]),
            (HOST_STATUS_SUCCESS, 2)
        );

        let directory_path = test_string(directory_dataset.to_string_lossy().as_ref());
        let (status, dir_dataset) = call_host(ML_DATASET_FROM_DIRECTORY, &[directory_path]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATASET_LEN, &[dir_dataset]),
            (HOST_STATUS_SUCCESS, 3)
        );

        let (status, frame) = call_host(ML_DATAFRAME_FROM_CSV, &[csv_path, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DATAFRAME_ROWS, &[frame]),
            (HOST_STATUS_SUCCESS, 4)
        );
        assert_eq!(
            call_host(ML_DATAFRAME_COLS, &[frame]),
            (HOST_STATUS_SUCCESS, 3)
        );
        let (status, column) = call_host(ML_DATAFRAME_COLUMN, &[frame, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, column_sum_bits) = call_host(TENSOR_SUM_F, &[column]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(column_sum_bits as u64) - 14.0).abs() < 1e-9);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ml_phase17_experiment_tracking_manifests_compare_and_repro_command() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        let dir = std::env::temp_dir().join(format!(
            "spectra_r1702_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let run_a = dir.join("run_a");
        let run_b = dir.join("run_b");
        let run_c = dir.join("run_c");
        std::fs::create_dir_all(&dir).expect("create temp experiment dir");
        let lockfile = dir.join("spectra.lock");
        let model = dir.join("model.txt");
        let artifact = dir.join("metrics.txt");
        std::fs::write(&lockfile, "package root 1.0.0\n").expect("write lockfile");
        std::fs::write(&model, "weights=2\n").expect("write model");
        std::fs::write(&artifact, "loss=0.25\n").expect("write artifact");

        let run_a_path = test_string(run_a.to_string_lossy().as_ref());
        let run_b_path = test_string(run_b.to_string_lossy().as_ref());
        let run_c_path = test_string(run_c.to_string_lossy().as_ref());
        let name = test_string("tabular-run");
        let (status, exp_a) = call_host(ML_EXPERIMENT_START, &[name, run_a_path, 123]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, exp_b) = call_host(ML_EXPERIMENT_START, &[name, run_b_path, 123]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, exp_c) = call_host(ML_EXPERIMENT_START, &[name, run_c_path, 123]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        for exp in [exp_a, exp_b, exp_c] {
            assert_eq!(
                call_host(
                    ML_EXPERIMENT_SET_CONFIG,
                    &[exp, test_string("lr"), test_string("0.01")]
                )
                .0,
                HOST_STATUS_SUCCESS
            );
            assert_eq!(
                call_host(
                    ML_EXPERIMENT_SET_LOCKFILE,
                    &[exp, test_string(lockfile.to_string_lossy().as_ref())]
                )
                .0,
                HOST_STATUS_SUCCESS
            );
            assert_eq!(
                call_host(
                    ML_EXPERIMENT_SET_MODEL_OUTPUT,
                    &[exp, test_string(model.to_string_lossy().as_ref())]
                )
                .0,
                HOST_STATUS_SUCCESS
            );
            assert_eq!(
                call_host(
                    ML_EXPERIMENT_LOG_ARTIFACT,
                    &[exp, test_string(artifact.to_string_lossy().as_ref())]
                )
                .0,
                HOST_STATUS_SUCCESS
            );
        }

        assert_eq!(
            call_host(
                ML_EXPERIMENT_LOG_METRIC,
                &[exp_a, test_string("loss"), 0.25f64.to_bits() as i64, 1]
            )
            .0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(
                ML_EXPERIMENT_LOG_METRIC,
                &[exp_b, test_string("loss"), 0.25f64.to_bits() as i64, 1]
            )
            .0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(
                ML_EXPERIMENT_LOG_METRIC,
                &[exp_c, test_string("loss"), 0.5f64.to_bits() as i64, 1]
            )
            .0,
            HOST_STATUS_SUCCESS
        );

        for exp in [exp_a, exp_b, exp_c] {
            assert_eq!(
                call_host(ML_EXPERIMENT_FINISH, &[exp]).0,
                HOST_STATUS_SUCCESS
            );
        }

        let (status, manifest_a_ptr) = call_host(ML_EXPERIMENT_MANIFEST_PATH, &[exp_a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let manifest_a =
            unsafe { read_spectra_string(manifest_a_ptr) }.expect("manifest path string");
        let manifest_text = std::fs::read_to_string(&manifest_a).expect("manifest exists");
        assert!(manifest_text.contains("\"schema\":\"spectra.ml.experiment.v1\""));
        assert!(manifest_text.contains("\"metrics\""));
        assert!(manifest_text.contains("\"lockfile\""));
        assert!(manifest_text.contains("\"model_output\""));

        let (status, command_ptr) = call_host(ML_EXPERIMENT_REPRO_COMMAND, &[exp_a]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let command = unsafe { read_spectra_string(command_ptr) }.expect("repro command string");
        assert!(command.contains("spectralang run"));
        assert!(command.contains("experiment-manifest.json"));

        let manifest_b = run_b.join("experiment-manifest.json");
        let manifest_c = run_c.join("experiment-manifest.json");
        assert_eq!(
            call_host(
                ML_EXPERIMENT_COMPARE_MANIFESTS,
                &[
                    test_string(&manifest_a),
                    test_string(manifest_b.to_string_lossy().as_ref())
                ],
            ),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(
                ML_EXPERIMENT_COMPARE_MANIFESTS,
                &[
                    test_string(&manifest_a),
                    test_string(manifest_c.to_string_lossy().as_ref())
                ],
            ),
            (HOST_STATUS_SUCCESS, 0)
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ml_phase17_distributed_training_checkpoint_resume() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        let dir = std::env::temp_dir().join(format!(
            "spectra_r1703_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let checkpoint = dir.join("checkpoint.json");
        std::fs::create_dir_all(&dir).expect("create temp distributed dir");

        let (status, session) = call_host(
            ML_DISTRIBUTED_SESSION_START,
            &[
                test_string("single-machine-reference"),
                test_string(dir.to_string_lossy().as_ref()),
                3,
                2026,
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);

        for worker_id in 0..3 {
            assert_eq!(
                call_host(
                    ML_DISTRIBUTED_WORKER_STEP,
                    &[
                        session,
                        worker_id,
                        8,
                        (0.25f64 + worker_id as f64).to_bits() as i64
                    ],
                ),
                (HOST_STATUS_SUCCESS, 1)
            );
        }
        assert_eq!(
            call_host(ML_DISTRIBUTED_GLOBAL_STEP, &[session]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, checkpoint_ptr) = call_host(
            ML_DISTRIBUTED_CHECKPOINT_SAVE,
            &[
                session,
                test_string(checkpoint.to_string_lossy().as_ref()),
                1,
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let checkpoint_path =
            unsafe { read_spectra_string(checkpoint_ptr) }.expect("checkpoint path string");
        let checkpoint_text = std::fs::read_to_string(&checkpoint_path).expect("checkpoint exists");
        assert!(checkpoint_text.contains("\"schema\":\"spectra.ml.distributed_checkpoint.v1\""));
        assert!(checkpoint_text.contains("\"interrupted_worker\":1"));
        assert!(checkpoint_text.contains("\"topology\":\"single-machine-simulated-workers\""));

        let (status, resumed) = call_host(
            ML_DISTRIBUTED_RESUME,
            &[test_string(checkpoint_path.as_str())],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_DISTRIBUTED_WORKER_STEP_COUNT, &[resumed, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        for worker_id in 0..3 {
            assert_eq!(
                call_host(
                    ML_DISTRIBUTED_WORKER_STEP,
                    &[resumed, worker_id, 4, 0.1f64.to_bits() as i64],
                ),
                (HOST_STATUS_SUCCESS, 2)
            );
        }
        assert_eq!(
            call_host(ML_DISTRIBUTED_GLOBAL_STEP, &[resumed]),
            (HOST_STATUS_SUCCESS, 2)
        );

        let (status, summary_ptr) = call_host(ML_DISTRIBUTED_SUMMARY, &[resumed]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let summary = unsafe { read_spectra_string(summary_ptr) }.expect("summary string");
        assert!(summary.contains("\"schema\":\"spectra.ml.distributed_summary.v1\""));
        assert!(summary.contains("\"global_step\":2"));
        assert!(summary.contains("\"total_samples\":36"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ml_phase18_onnx_subset_export_import_and_roundtrip() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        let dir = std::env::temp_dir().join(format!(
            "spectra_r1801_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp onnx dir");

        for (kind, expected_op) in [
            ("linear", "Gemm"),
            ("conv", "Conv"),
            ("activation", "Relu"),
            ("normalization", "LayerNormalization"),
            ("transformer", "Softmax"),
        ] {
            let path = dir.join(format!("{kind}.onnx"));
            let roundtrip = dir.join(format!("{kind}.roundtrip.onnx"));
            let (status, exported_ptr) = call_host(
                ML_ONNX_EXPORT,
                &[
                    test_string(path.to_string_lossy().as_ref()),
                    test_string(kind),
                ],
            );
            assert_eq!(status, HOST_STATUS_SUCCESS);
            let exported = unsafe { read_spectra_string(exported_ptr) }.expect("export path");
            assert!(std::fs::metadata(&exported).expect("onnx exists").len() > 16);
            assert_eq!(
                call_host(ML_ONNX_VALIDATE, &[test_string(&exported)]),
                (HOST_STATUS_SUCCESS, 1)
            );

            let (status, summary_ptr) =
                call_host(ML_ONNX_IMPORT_SUMMARY, &[test_string(&exported)]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            let summary = unsafe { read_spectra_string(summary_ptr) }.expect("summary");
            assert!(summary.contains("\"schema\":\"spectra.onnx.subset.v1\""));
            assert!(summary.contains(expected_op), "{summary}");
            assert!(summary.contains("\"float32\""));
            assert!(summary.contains("\"ranked\""));

            let (status, roundtrip_ptr) = call_host(
                ML_ONNX_ROUNDTRIP,
                &[
                    test_string(&exported),
                    test_string(roundtrip.to_string_lossy().as_ref()),
                ],
            );
            assert_eq!(status, HOST_STATUS_SUCCESS);
            let roundtrip_path =
                unsafe { read_spectra_string(roundtrip_ptr) }.expect("roundtrip path");
            assert_eq!(
                call_host(ML_ONNX_VALIDATE, &[test_string(&roundtrip_path)]),
                (HOST_STATUS_SUCCESS, 1)
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ml_phase18_transformer_primitives_and_sampling() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[0]);

        let one = 1.0f64.to_bits() as i64;
        let zero = 0.0f64.to_bits() as i64;
        let (status, table) = call_host(TENSOR_FULL_F, &[12, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, table) = call_host(TENSOR_RESHAPE, &[table, 4, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, ids) = call_host(TENSOR_ARANGE, &[0, 3, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, embedded) = call_host(ML_EMBEDDING_LOOKUP, &[table, ids]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TENSOR_ROWS, &[embedded]),
            (HOST_STATUS_SUCCESS, 3)
        );
        assert_eq!(
            call_host(TENSOR_COLS, &[embedded]),
            (HOST_STATUS_SUCCESS, 3)
        );

        let (status, pos) = call_host(ML_POSITIONAL_ENCODING, &[3, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_ROWS, &[pos]), (HOST_STATUS_SUCCESS, 3));
        let (status, pos00) = call_host(TENSOR_GET_F, &[pos, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(f64::from_bits(pos00 as u64).abs() < 1e-12);

        let (status, scale) = call_host(TENSOR_FULL_F, &[3, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bias) = call_host(TENSOR_FULL_F, &[3, zero]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, normed) = call_host(
            ML_LAYER_NORM,
            &[embedded, scale, bias, 1e-5f64.to_bits() as i64],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_LEN, &[normed]), (HOST_STATUS_SUCCESS, 9));

        let (status, gelu) = call_host(ML_GELU, &[normed]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, swiglu) = call_host(ML_SWIGLU, &[gelu, gelu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_LEN, &[swiglu]), (HOST_STATUS_SUCCESS, 9));

        let (status, query) = call_host(
            TENSOR_RESHAPE,
            &[call_host(TENSOR_FULL_F, &[6, one]).1, 2, 3],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, key) = call_host(
            TENSOR_RESHAPE,
            &[call_host(TENSOR_FULL_F, &[6, one]).1, 2, 3],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, value) = call_host(
            TENSOR_RESHAPE,
            &[
                call_host(TENSOR_FULL_F, &[4, 2.0f64.to_bits() as i64]).1,
                2,
                2,
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, attended) = call_host(ML_ATTENTION, &[query, key, value]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TENSOR_ROWS, &[attended]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(TENSOR_COLS, &[attended]),
            (HOST_STATUS_SUCCESS, 2)
        );

        let (status, query_cpu) = call_host(TENSOR_TO_DEVICE, &[query, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, key_cpu) = call_host(TENSOR_TO_DEVICE, &[key, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, value_cpu) = call_host(TENSOR_TO_DEVICE, &[value, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, attended_cpu) = call_host(ML_ATTENTION, &[query_cpu, key_cpu, value_cpu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, sum_a) = call_host(TENSOR_SUM_F, &[attended]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, sum_b) = call_host(TENSOR_SUM_F, &[attended_cpu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(
            (f64::from_bits(sum_a as u64) - f64::from_bits(sum_b as u64)).abs()
                <= NUMERICAL_TOLERANCE_ABS
        );

        let (status, cache) = call_host(ML_KV_CACHE_NEW, &[4, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_KV_CACHE_APPEND, &[cache, query, key]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ML_KV_CACHE_LEN, &[cache]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(call_host(ML_KV_CACHE_KEYS, &[cache]).0, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ML_KV_CACHE_VALUES, &[cache]).0,
            HOST_STATUS_SUCCESS
        );

        let (status, logits) = call_host(
            TENSOR_RESHAPE,
            &[call_host(TENSOR_FULL_F, &[3, one]).1, 1, 3],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let _ = call_host(TENSOR_SEED, &[123]);
        let (status, sample) = call_host(ML_LOGITS_SAMPLE, &[logits, 1.0f64.to_bits() as i64]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((0..3).contains(&sample));

        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
        let _ = call_host(TENSOR_FREE_ALL, &[]);
    }

    #[test]
    fn ml_phase18_rag_tokenizer_vector_index_and_prompt_eval() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let vocab = "[UNK]:0\nhello:1\nworld:2\nmachine:3\nlearning:4\nrag:5\nretrieval:6\n##s:7";
        let (status, tokenizer) = call_host(ML_TOKENIZER_WORDPIECE, &[test_string(vocab)]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, ids) = call_host(
            ML_TOKENIZER_ENCODE,
            &[tokenizer, test_string("hello world")],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_LEN, &[ids]), (HOST_STATUS_SUCCESS, 2));
        let (status, decoded_ptr) = call_host(ML_TOKENIZER_DECODE, &[tokenizer, ids]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let decoded = unsafe { read_spectra_string(decoded_ptr) }.expect("decoded string");
        assert_eq!(decoded, "hello world");

        let (status, index) = call_host(ML_VECTOR_INDEX_NEW, &[8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, rag_vec) = call_host(ML_TEXT_EMBED, &[test_string("rag retrieval"), 8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, ml_vec) = call_host(ML_TEXT_EMBED, &[test_string("machine learning"), 8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(
                ML_VECTOR_INDEX_INSERT,
                &[index, test_string("rag-doc"), rag_vec]
            ),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(
                ML_VECTOR_INDEX_INSERT,
                &[index, test_string("ml-doc"), ml_vec]
            ),
            (HOST_STATUS_SUCCESS, 2)
        );
        let (status, query_vec) = call_host(ML_TEXT_EMBED, &[test_string("rag retrieval"), 8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, query_ptr) = call_host(ML_VECTOR_INDEX_QUERY, &[index, query_vec, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let query = unsafe { read_spectra_string(query_ptr) }.expect("query json");
        assert!(query.contains("rag-doc"), "{query}");

        let dir = std::env::temp_dir().join(format!(
            "spectra_r1803_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let path = dir.join("index.json");
        let (status, persisted_ptr) = call_host(
            ML_VECTOR_INDEX_PERSIST,
            &[index, test_string(path.to_string_lossy().as_ref())],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let persisted = unsafe { read_spectra_string(persisted_ptr) }.expect("persisted path");
        let (status, loaded) = call_host(ML_VECTOR_INDEX_LOAD, &[test_string(&persisted)]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loaded_query_ptr) = call_host(ML_VECTOR_INDEX_QUERY, &[loaded, query_vec, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let loaded_query =
            unsafe { read_spectra_string(loaded_query_ptr) }.expect("loaded query json");
        assert!(loaded_query.contains("rag-doc"), "{loaded_query}");

        let (status, chunks_ptr) = call_host(
            ML_RAG_CHUNK_TEXT,
            &[test_string("RAG retrieval uses indexed chunks."), 12, 3],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let chunks = unsafe { read_spectra_string(chunks_ptr) }.expect("chunks json");
        assert!(chunks.contains("spectra.ml.rag_chunks.v1"));
        let (status, prompt_ptr) = call_host(
            ML_RAG_BUILD_PROMPT,
            &[
                test_string(&chunks),
                test_string("What does RAG retrieval use?"),
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let prompt = unsafe { read_spectra_string(prompt_ptr) }.expect("prompt");
        assert!(prompt.contains("Context:"));
        assert!(prompt.contains("Question:"));
        let (status, score_bits) = call_host(
            ML_RAG_EVALUATE_ANSWER,
            &[
                test_string("RAG retrieval uses indexed chunks"),
                test_string("retrieval uses indexed chunks"),
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(score_bits > 700);

        std::fs::remove_dir_all(&dir).ok();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
    }

    #[test]
    fn ml_phase19_evaluation_metrics_and_report() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let (status, labels) = call_host(TENSOR_ARANGE, &[0, 4, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, predicted) = call_host(TENSOR_ARANGE, &[0, 4, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, classification_ptr) =
            call_host(ML_METRICS_CLASSIFICATION, &[labels, predicted]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let classification =
            unsafe { read_spectra_string(classification_ptr) }.expect("classification json");
        assert!(classification.contains("\"accuracy\":1.000000"));
        assert!(classification.contains("\"roc_auc_baseline\""));

        let (status, regression_expected) = call_host(TENSOR_FULL_F, &[4, 1.0f64.to_bits() as i64]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, regression_predicted) =
            call_host(TENSOR_FULL_F, &[4, 1.0f64.to_bits() as i64]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, regression_ptr) = call_host(
            ML_METRICS_REGRESSION,
            &[regression_expected, regression_predicted],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let regression = unsafe { read_spectra_string(regression_ptr) }.expect("regression json");
        assert!(regression.contains("\"mse\":0.000000"));
        assert!(regression.contains("\"mae\":0.000000"));

        let (status, relevance) = call_host(TENSOR_ARANGE, &[0, 4, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, scores) = call_host(TENSOR_FULL_F, &[4, 1.0f64.to_bits() as i64]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, ranking_ptr) = call_host(ML_METRICS_RANKING, &[relevance, scores, 3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let ranking = unsafe { read_spectra_string(ranking_ptr) }.expect("ranking json");
        assert!(ranking.contains("\"ndcg_at_k\""));
        assert!(ranking.contains("\"hit_rate_at_k\""));

        let (status, generation_ptr) = call_host(
            ML_METRICS_GENERATION,
            &[
                test_string("the answer uses indexed retrieval"),
                test_string("answer uses retrieval"),
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let generation = unsafe { read_spectra_string(generation_ptr) }.expect("generation json");
        assert!(generation.contains("\"token_f1\""));
        assert!(generation.contains("\"perplexity\""));

        let (status, latencies) = call_host(TENSOR_ARANGE, &[10, 50, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, serving_ptr) = call_host(ML_SERVING_METRICS, &[latencies, 4, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let serving = unsafe { read_spectra_string(serving_ptr) }.expect("serving json");
        assert!(serving.contains("\"latency_p95_ms\""));
        assert!(serving.contains("\"throughput_per_second\""));

        let dir = std::env::temp_dir().join(format!(
            "spectra_r1901_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let path = dir.join("evaluation.json");
        let (status, report_ptr) = call_host(
            ML_EVALUATION_REPORT,
            &[
                test_string(path.to_string_lossy().as_ref()),
                test_string("phase19-eval"),
                test_string(&classification),
                test_string(&regression),
                test_string(&ranking),
                test_string(&generation),
                test_string(&serving),
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let report_path = unsafe { read_spectra_string(report_ptr) }.expect("report path");
        let report = std::fs::read_to_string(&report_path).expect("report file");
        assert!(report.contains("spectra.ml.evaluation_report.v1"));
        assert!(report.contains("\"classification\""));
        let human = std::fs::read_to_string(format!("{report_path}.txt")).expect("human report");
        assert!(human.contains("Spectra ML Evaluation Report"));

        std::fs::remove_dir_all(&dir).ok();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
    }

    #[test]
    fn tensor_runtime_phase7_device_placement_and_transfer_contract() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_RESET_STATS, &[]);

        let (status, tensor) = call_host(TENSOR_ONES, &[4]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TENSOR_DEVICE, &[tensor]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_DEVICE_AVAILABLE, &[0]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(TENSOR_DEVICE_AVAILABLE, &[1]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_TO_DEVICE, &[tensor, 99]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(TENSOR_TO_DEVICE, &[tensor, 1]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );

        let (status, moved) = call_host(TENSOR_TO_DEVICE, &[tensor, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_DEVICE, &[moved]), (HOST_STATUS_SUCCESS, 0));
        assert_eq!(call_host(TENSOR_SUM, &[moved]), (HOST_STATUS_SUCCESS, 4));
        assert_eq!(call_host(TENSOR_SYNC, &[moved]).0, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_CPU, &[moved]).0, HOST_STATUS_SUCCESS);
        let (status, transfers) = call_host(TENSOR_STATS_DEVICE_TRANSFERS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(transfers >= 2);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn tensor_runtime_phase7_wgpu_backend_float_kernels() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_RESET_STATS, &[]);

        if call_host(TENSOR_DEVICE_AVAILABLE, &[6]) != (HOST_STATUS_SUCCESS, 1) {
            return;
        }

        let one = 1.0f64.to_bits() as i64;
        let two = 2.0f64.to_bits() as i64;
        let (status, a) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_FULL_F, &[4, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, a_gpu) = call_host(TENSOR_TO_DEVICE, &[a, 6]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b_gpu) = call_host(TENSOR_TO_DEVICE, &[b, 6]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_DEVICE, &[a_gpu]), (HOST_STATUS_SUCCESS, 6));

        let (status, added) = call_host(TENSOR_ADD, &[a_gpu, b_gpu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_DEVICE, &[added]), (HOST_STATUS_SUCCESS, 6));
        let (status, added_sum_bits) = call_host(TENSOR_SUM_F, &[added]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(added_sum_bits as u64) - 12.0).abs() < 1e-5);

        let (status, relu) = call_host(TENSOR_RELU, &[added]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, relu_first_bits) = call_host(TENSOR_GET_F, &[relu, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(relu_first_bits as u64) - 3.0).abs() < 1e-5);

        let (status, left) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, right) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, two]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, left2) = call_host(TENSOR_RESHAPE, &[left, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, right2) = call_host(TENSOR_RESHAPE, &[right, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, product) = call_host(TENSOR_MATMUL, &[left2, right2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(TENSOR_DEVICE, &[product]),
            (HOST_STATUS_SUCCESS, 6)
        );
        let (status, p00_bits) = call_host(TENSOR_GET_F, &[product, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(p00_bits as u64) - 4.0).abs() < 1e-5);

        let (status, input) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, kernel) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[1, two]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bias) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[1, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, conv) = call_host(ML_CONV2D, &[input, kernel, bias, 1, 1, 2, 2, 1, 1, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_DEVICE, &[conv]), (HOST_STATUS_SUCCESS, 6));
        let (status, conv0_bits) = call_host(TENSOR_GET_F, &[conv, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(conv0_bits as u64) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tensor_runtime_r1603_default_cpu_fallback_and_diagnostics() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_RESET_STATS, &[]);

        assert_eq!(
            call_host(TENSOR_DEVICE_STATUS, &[0]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_DEVICE_AVAILABLE, &[0]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(TENSOR_DEVICE_STATUS, &[1]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(TENSOR_DEVICE_AVAILABLE, &[1]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_DEVICE_STATUS, &[99]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        #[cfg(not(feature = "gpu"))]
        assert_eq!(
            call_host(TENSOR_DEVICE_STATUS, &[6]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let one = 1.0f64.to_bits() as i64;
        let two = 2.0f64.to_bits() as i64;
        let (status, a) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(TENSOR_FULL_F, &[4, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, added) = call_host(TENSOR_ADD, &[a, b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, relu) = call_host(TENSOR_RELU, &[added]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, sum_bits) = call_host(TENSOR_SUM_F, &[relu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(sum_bits as u64) - 12.0).abs() < 1e-9);

        let (status, left) = call_host(TENSOR_RESHAPE, &[a, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, right) = call_host(TENSOR_RESHAPE, &[b, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, product) = call_host(TENSOR_MATMUL, &[left, right]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, p00_bits) = call_host(TENSOR_GET_F, &[product, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(p00_bits as u64) - 4.0).abs() < 1e-9);

        let (status, kernel) = call_host(TENSOR_FULL_F, &[1, two]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bias) = call_host(TENSOR_FULL_F, &[1, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, conv) = call_host(ML_CONV2D, &[a, kernel, bias, 1, 1, 2, 2, 1, 1, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, conv0_bits) = call_host(TENSOR_GET_F, &[conv, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(conv0_bits as u64) - 3.0).abs() < 1e-9);

        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
        let (status, backward_base) = call_host(TENSOR_FULL_F, &[4, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_source) = call_host(TENSOR_REQUIRES_GRAD, &[backward_base, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, backward_relu) = call_host(TENSOR_RELU, &[grad_source]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss) = call_host(TENSOR_SUM_T, &[backward_relu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_BACKWARD, &[loss]).0, HOST_STATUS_SUCCESS);
        let (status, grad) = call_host(TENSOR_GRAD, &[grad_source]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_sum_bits) = call_host(TENSOR_SUM_F, &[grad]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(grad_sum_bits as u64) - 4.0).abs() < 1e-9);

        assert_eq!(
            call_host(TENSOR_STATS_CPU_FALLBACKS, &[]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(TENSOR_STATS_GPU_KERNEL_OPS, &[]),
            (HOST_STATUS_SUCCESS, 0)
        );
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn tensor_runtime_r1603_wgpu_backend_diagnostics_and_backward() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);
        let _ = call_host(TENSOR_RESET_STATS, &[]);

        if call_host(TENSOR_DEVICE_AVAILABLE, &[6]) != (HOST_STATUS_SUCCESS, 1) {
            assert_eq!(
                call_host(TENSOR_DEVICE_STATUS, &[6]),
                (HOST_STATUS_SUCCESS, 1)
            );
            return;
        }
        assert_eq!(
            call_host(TENSOR_DEVICE_STATUS, &[6]),
            (HOST_STATUS_SUCCESS, 0)
        );

        let one = 1.0f64.to_bits() as i64;
        let two = 2.0f64.to_bits() as i64;
        let (status, a) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, b) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, two]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_DEVICE, &[a]), (HOST_STATUS_SUCCESS, 6));

        let (status, added) = call_host(TENSOR_ADD, &[a, b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, relu) = call_host(TENSOR_RELU, &[added]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, sum_bits) = call_host(TENSOR_SUM_F, &[relu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(sum_bits as u64) - 12.0).abs() < 1e-5);

        let (status, left) = call_host(TENSOR_RESHAPE, &[a, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, right) = call_host(TENSOR_RESHAPE, &[b, 2, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, product) = call_host(TENSOR_MATMUL, &[left, right]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, p00_bits) = call_host(TENSOR_GET_F, &[product, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(p00_bits as u64) - 4.0).abs() < 1e-5);

        let (status, input) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, kernel) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[1, two]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bias) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[1, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, conv) = call_host(ML_CONV2D, &[input, kernel, bias, 1, 1, 2, 2, 1, 1, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, conv0_bits) = call_host(TENSOR_GET_F, &[conv, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(conv0_bits as u64) - 3.0).abs() < 1e-5);

        let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
        let (status, backward_base) = call_host(
            TENSOR_TO_DEVICE,
            &[call_host(TENSOR_FULL_F, &[4, one]).1, 6],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_source) = call_host(TENSOR_REQUIRES_GRAD, &[backward_base, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, backward_relu) = call_host(TENSOR_RELU, &[grad_source]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, loss) = call_host(TENSOR_SUM_T, &[backward_relu]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(TENSOR_BACKWARD, &[loss]).0, HOST_STATUS_SUCCESS);
        let (status, grad) = call_host(TENSOR_GRAD, &[grad_source]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, grad_sum_bits) = call_host(TENSOR_SUM_F, &[grad]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((f64::from_bits(grad_sum_bits as u64) - 4.0).abs() < 1e-5);

        let (status, transfers) = call_host(TENSOR_STATS_DEVICE_TRANSFERS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(transfers >= 5);
        let (status, gpu_ops) = call_host(TENSOR_STATS_GPU_KERNEL_OPS, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(gpu_ops >= 5);
        assert_eq!(call_host(TENSOR_SYNC, &[conv]).0, HOST_STATUS_SUCCESS);
    }

    #[test]
    fn tensor_runtime_float_distributions_and_activations() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();
        let _ = call_host(TENSOR_FREE_ALL, &[]);

        let zero = 0.0f64.to_bits() as i64;
        let one = 1.0f64.to_bits() as i64;
        let (status, values) = call_host(TENSOR_UNIFORM_F, &[16, zero, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, mean_bits) = call_host(TENSOR_MEAN_F, &[values]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let mean = f64::from_bits(mean_bits as u64);
        assert!((0.0..1.0).contains(&mean));

        let (status, normal) = call_host(TENSOR_NORMAL_F, &[8, zero, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, len) = call_host(TENSOR_LEN, &[normal]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(len, 8);

        let (status, sigmoid) = call_host(TENSOR_SIGMOID_F, &[normal]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, sigmoid_first_bits) = call_host(TENSOR_GET_F, &[sigmoid, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let sigmoid_first = f64::from_bits(sigmoid_first_bits as u64);
        assert!((0.0..=1.0).contains(&sigmoid_first));

        let (status, bernoulli) = call_host(TENSOR_BERNOULLI, &[16, one]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, bernoulli_sum) = call_host(TENSOR_SUM, &[bernoulli]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(bernoulli_sum, 16);

        let (status, weights) = call_host(TENSOR_FULL, &[3, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, _) = call_host(TENSOR_SET, &[weights, 2, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, categories) = call_host(TENSOR_CATEGORICAL, &[10, weights]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, category_sum) = call_host(TENSOR_SUM, &[categories]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(category_sum, 20);

        let half = 0.5f64.to_bits() as i64;
        let (status, fair) = call_host(TENSOR_BERNOULLI, &[1000, half]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, fair_sum) = call_host(TENSOR_SUM, &[fair]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((350..650).contains(&fair_sum));

        let (status, broad_uniform) = call_host(TENSOR_UNIFORM, &[1000, 0, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, broad_sum) = call_host(TENSOR_SUM, &[broad_uniform]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!((3500..5500).contains(&broad_sum));

        let (status, freed) = call_host(TENSOR_FREE_ALL, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert!(freed >= 4);
    }

    #[test]
    fn concurrent_host_calls_cover_tasks_channels_counters_and_pipeline() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(CONCURRENT_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let (status, task) = call_host(CONCURRENT_TASK_SPAWN, &[42]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(CONCURRENT_TASK_IS_DONE, &[task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(CONCURRENT_TASK_JOIN, &[task]),
            (HOST_STATUS_SUCCESS, 42)
        );
        assert_eq!(
            call_host(CONCURRENT_TASK_IS_DONE, &[task]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(CONCURRENT_STATS_TASKS_SPAWNED, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, channel) = call_host(CONCURRENT_CHANNEL_NEW, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_SEND, &[channel, 7]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_SEND, &[channel, 9]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_LEN, &[channel]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_RECV, &[channel]),
            (HOST_STATUS_SUCCESS, 7)
        );
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_RECV, &[channel]),
            (HOST_STATUS_SUCCESS, 9)
        );
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_RECV, &[channel]),
            (HOST_STATUS_SUCCESS, -1)
        );
        assert_eq!(
            call_host(CONCURRENT_CHANNEL_CLOSE, &[channel]).0,
            HOST_STATUS_SUCCESS
        );

        let (status, counter) = call_host(CONCURRENT_COUNTER_NEW, &[5]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(CONCURRENT_COUNTER_ADD, &[counter, 4]),
            (HOST_STATUS_SUCCESS, 9)
        );
        assert_eq!(
            call_host(CONCURRENT_COUNTER_GET, &[counter]),
            (HOST_STATUS_SUCCESS, 9)
        );
        assert_eq!(
            call_host(CONCURRENT_PIPELINE_SUM, &[1, 100, 4]),
            (HOST_STATUS_SUCCESS, 5050)
        );
        assert_eq!(
            call_host(CONCURRENT_STATS_CHANNELS, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );
    }

    #[test]
    fn async_task_host_calls_cover_ready_poll_result_and_cancellation() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(ASYNC_TASK_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let (status, task) = call_host(ASYNC_TASK_READY, &[42]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_POLL, &[task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[task]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(ASYNC_TASK_RESULT, &[task]),
            (HOST_STATUS_SUCCESS, 42)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[task]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[task]),
            (HOST_STATUS_SUCCESS, 42)
        );

        let (status, cancelled_task) = call_host(ASYNC_TASK_READY, &[99]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_CANCEL, &[cancelled_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[cancelled_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_RESULT, &[cancelled_task]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[cancelled_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[cancelled_task]),
            (HOST_STATUS_SUCCESS, -1)
        );
    }

    #[test]
    fn async_task_ready_batch_creates_sequential_ready_tasks() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(ASYNC_TASK_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let (status, first_task) = call_host(ASYNC_TASK_READY_BATCH, &[5, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        for offset in 0..5 {
            let task = first_task + offset;
            assert_eq!(
                call_host(ASYNC_TASK_POLL, &[task]),
                (HOST_STATUS_SUCCESS, 1)
            );
            assert_eq!(
                call_host(ASYNC_TASK_RESULT, &[task]),
                (HOST_STATUS_SUCCESS, 10 + offset)
            );
        }
        assert_eq!(
            call_host(ASYNC_TASK_BATCH_CHECKSUM, &[first_task, 5]),
            (HOST_STATUS_SUCCESS, 60)
        );
        assert_eq!(
            call_host(ASYNC_TASK_READY_BATCH, &[0, 1]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(ASYNC_TASK_BATCH_CHECKSUM, &[first_task, 0]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
    }

    #[test]
    fn async_structured_concurrency_host_calls_cover_cascade_timeout_and_join_order() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(ASYNC_TASK_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let (status, parent_scope) = call_host(ASYNC_SCOPE_NEW, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, child_scope) = call_host(ASYNC_SCOPE_CHILD, &[parent_scope]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, parent_task) = call_host(ASYNC_SCOPE_SPAWN_READY, &[parent_scope, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, child_task) = call_host(ASYNC_SCOPE_SPAWN_READY, &[child_scope, 20]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        assert_eq!(
            call_host(ASYNC_SCOPE_CANCEL, &[parent_scope]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[parent_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[child_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_SCOPE_JOIN, &[parent_scope]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_SCOPE_JOINED_COUNT, &[parent_scope]),
            (HOST_STATUS_SUCCESS, 2)
        );

        let (status, task) = call_host(ASYNC_TASK_READY, &[55]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, timed_task) = call_host(ASYNC_TASK_WITH_TIMEOUT, &[task, 5]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[timed_task]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(ASYNC_SCHEDULER_ADVANCE_TIME, &[5]),
            (HOST_STATUS_SUCCESS, 5)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[timed_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_RESULT, &[timed_task]).0,
            HOST_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[timed_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[timed_task]),
            (HOST_STATUS_SUCCESS, -1)
        );

        let (status, scoped_join) = call_host(ASYNC_SCOPE_NEW, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, first) = call_host(ASYNC_SCOPE_SPAWN_READY, &[scoped_join, 100]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, second) = call_host(ASYNC_SCOPE_SPAWN_READY, &[scoped_join, 200]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, handle) = call_host(ASYNC_TASK_CANCEL_HANDLE, &[first]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_CANCEL_HANDLE_CANCEL, &[handle]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[first]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_FAIL, &[second]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[second]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[second]),
            (HOST_STATUS_SUCCESS, -2)
        );
        assert_eq!(
            call_host(ASYNC_SCOPE_JOIN, &[scoped_join]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ASYNC_SCOPE_JOINED_COUNT, &[scoped_join]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ASYNC_SCOPE_FAILURES, &[scoped_join]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_ORDER, &[first]),
            (HOST_STATUS_SUCCESS, 3)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_ORDER, &[second]),
            (HOST_STATUS_SUCCESS, 4)
        );
    }

    #[test]
    fn async_stream_host_calls_cover_adaptors_backpressure_done_and_cancellation() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(ASYNC_TASK_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let (status, source) = call_host(ASYNC_STREAM_NEW, &[8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        for value in 1..=5 {
            assert_eq!(
                call_host(ASYNC_STREAM_PUSH, &[source, value]),
                (HOST_STATUS_SUCCESS, 1)
            );
        }
        assert_eq!(
            call_host(ASYNC_STREAM_DONE, &[source]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, mapped) = call_host(ASYNC_STREAM_MAP, &[source, 1, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, filtered) = call_host(ASYNC_STREAM_FILTER, &[mapped, 3, 12]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, skipped) = call_host(ASYNC_STREAM_SKIP, &[filtered, 1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, taken) = call_host(ASYNC_STREAM_TAKE, &[skipped, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        let (status, first_task) = call_host(ASYNC_STREAM_NEXT, &[taken]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_NEXT_STATUS, &[taken]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[first_task]),
            (HOST_STATUS_SUCCESS, 14)
        );

        let (status, second_task) = call_host(ASYNC_STREAM_NEXT, &[taken]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[second_task]),
            (HOST_STATUS_SUCCESS, 15)
        );

        let (status, done_task) = call_host(ASYNC_STREAM_NEXT, &[taken]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_NEXT_STATUS, &[taken]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[done_task]),
            (HOST_STATUS_SUCCESS, -1)
        );

        let (status, chunk_source) = call_host(ASYNC_STREAM_NEW, &[8]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        for value in 1..=5 {
            assert_eq!(
                call_host(ASYNC_STREAM_PUSH, &[chunk_source, value]),
                (HOST_STATUS_SUCCESS, 1)
            );
        }
        assert_eq!(
            call_host(ASYNC_STREAM_DONE, &[chunk_source]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, chunks) = call_host(ASYNC_STREAM_CHUNKS, &[chunk_source, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        for expected in [3, 7, 5] {
            let (status, task) = call_host(ASYNC_STREAM_NEXT, &[chunks]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            assert_eq!(
                call_host(ASYNC_TASK_JOIN, &[task]),
                (HOST_STATUS_SUCCESS, expected)
            );
        }
        let (status, chunk_done) = call_host(ASYNC_STREAM_NEXT, &[chunks]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[chunk_done]),
            (HOST_STATUS_SUCCESS, -1)
        );

        let (status, fold_source) = call_host(ASYNC_STREAM_NEW, &[4]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        for value in 1..=4 {
            assert_eq!(
                call_host(ASYNC_STREAM_PUSH, &[fold_source, value]),
                (HOST_STATUS_SUCCESS, 1)
            );
        }
        assert_eq!(
            call_host(ASYNC_STREAM_DONE, &[fold_source]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, fold_task) = call_host(ASYNC_STREAM_FOLD, &[fold_source, 0, 0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[fold_task]),
            (HOST_STATUS_SUCCESS, 10)
        );

        let (status, fuse_source) = call_host(ASYNC_STREAM_NEW, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_DONE, &[fuse_source]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, fused) = call_host(ASYNC_STREAM_FUSE, &[fuse_source]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        for _ in 0..2 {
            let (status, task) = call_host(ASYNC_STREAM_NEXT, &[fused]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            assert_eq!(
                call_host(ASYNC_TASK_JOIN, &[task]),
                (HOST_STATUS_SUCCESS, -1)
            );
            assert_eq!(
                call_host(ASYNC_STREAM_NEXT_STATUS, &[fused]),
                (HOST_STATUS_SUCCESS, 2)
            );
        }

        let (status, backpressure) = call_host(ASYNC_STREAM_NEW, &[2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_CAPACITY, &[backpressure]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(ASYNC_STREAM_PUSH, &[backpressure, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_STREAM_PUSH, &[backpressure, 2]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_STREAM_PUSH, &[backpressure, 3]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(ASYNC_STREAM_LEN, &[backpressure]),
            (HOST_STATUS_SUCCESS, 2)
        );
        let (status, consumed) = call_host(ASYNC_STREAM_NEXT, &[backpressure]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[consumed]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_STREAM_PUSH, &[backpressure, 3]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, fast_consumer) = call_host(ASYNC_STREAM_NEW, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, pending_task) = call_host(ASYNC_STREAM_NEXT, &[fast_consumer]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_NEXT_STATUS, &[fast_consumer]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(ASYNC_TASK_POLL, &[pending_task]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(ASYNC_STREAM_PUSH, &[fast_consumer, 44]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_POLL, &[pending_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[pending_task]),
            (HOST_STATUS_SUCCESS, 44)
        );

        let (status, cancellable) = call_host(ASYNC_STREAM_NEW, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, waiting) = call_host(ASYNC_STREAM_NEXT, &[cancellable]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_CANCEL, &[cancellable]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[waiting]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, cancelled_next) = call_host(ASYNC_STREAM_NEXT, &[cancellable]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_STREAM_NEXT_STATUS, &[cancellable]),
            (HOST_STATUS_SUCCESS, 4)
        );
        assert_eq!(
            call_host(ASYNC_TASK_IS_CANCELLED, &[cancelled_next]),
            (HOST_STATUS_SUCCESS, 1)
        );
    }

    #[test]
    fn async_stdlib_host_calls_cover_fs_tcp_udp_channels_and_cancellation() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        assert_eq!(call_host(ASYNC_TASK_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let dir = std::env::temp_dir().join(format!(
            "spectra_r2107_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create async stdlib temp dir");
        let file = dir.join("payload.txt");
        let file_arg = test_string(file.to_string_lossy().as_ref());
        let payload_arg = test_string("async-payload");

        let (status, write_task) = call_host(ASYNC_FS_WRITE, &[file_arg, payload_arg]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[write_task]),
            (HOST_STATUS_SUCCESS, 13)
        );
        let (status, read_task) = call_host(ASYNC_FS_READ, &[file_arg]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, read_ptr) = call_host(ASYNC_TASK_JOIN, &[read_task]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let read_back = unsafe { read_spectra_string(read_ptr) }.expect("async fs read string");
        assert_eq!(read_back, "async-payload");

        let (status, cancelled_write) =
            call_host(ASYNC_FS_WRITE, &[file_arg, test_string("cancel")]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_CANCEL, &[cancelled_write]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[cancelled_write]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, listener) = call_host(ASYNC_TCP_LISTEN, &[0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, port) = call_host(ASYNC_TCP_LISTENER_PORT, &[listener]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, accept_task) = call_host(ASYNC_TCP_ACCEPT, &[listener]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[accept_task]),
            (HOST_STATUS_SUCCESS, 3)
        );
        let (status, connect_task) = call_host(ASYNC_TCP_CONNECT, &[port]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, client_stream) = call_host(ASYNC_TASK_JOIN, &[connect_task]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, server_stream) = call_host(ASYNC_TASK_JOIN, &[accept_task]);
        assert_eq!(status, HOST_STATUS_SUCCESS);

        let (status, pending_read) = call_host(ASYNC_TCP_READ, &[server_stream]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[pending_read]),
            (HOST_STATUS_SUCCESS, 3)
        );
        let (status, write_byte) = call_host(ASYNC_TCP_WRITE, &[client_stream, 65]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[write_byte]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[pending_read]),
            (HOST_STATUS_SUCCESS, 65)
        );

        let (status, cancelled_tcp_read) = call_host(ASYNC_TCP_READ, &[server_stream]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_CANCEL, &[cancelled_tcp_read]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TCP_WRITE, &[client_stream, 66]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[cancelled_tcp_read]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TCP_CLOSE, &[client_stream]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ASYNC_TCP_CLOSE, &[server_stream]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(ASYNC_TCP_CLOSE, &[listener]).0,
            HOST_STATUS_SUCCESS
        );

        let (status, udp_a) = call_host(ASYNC_UDP_BIND, &[0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, udp_b) = call_host(ASYNC_UDP_BIND, &[0]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, udp_b_port) = call_host(ASYNC_UDP_PORT, &[udp_b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, udp_recv) = call_host(ASYNC_UDP_RECV, &[udp_b]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[udp_recv]),
            (HOST_STATUS_SUCCESS, 3)
        );
        let (status, udp_send) = call_host(ASYNC_UDP_SEND_TO, &[udp_a, udp_b_port, 77]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[udp_send]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[udp_recv]),
            (HOST_STATUS_SUCCESS, 77)
        );
        assert_eq!(call_host(ASYNC_UDP_CLOSE, &[udp_a]).0, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(ASYNC_UDP_CLOSE, &[udp_b]).0, HOST_STATUS_SUCCESS);

        let (status, channel) = call_host(ASYNC_CHANNEL_NEW, &[1]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, waiting_recv) = call_host(ASYNC_CHANNEL_RECV, &[channel]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[waiting_recv]),
            (HOST_STATUS_SUCCESS, 3)
        );
        let (status, send_task) = call_host(ASYNC_CHANNEL_SEND, &[channel, 91]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[send_task]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[waiting_recv]),
            (HOST_STATUS_SUCCESS, 91)
        );
        assert_eq!(
            call_host(ASYNC_CHANNEL_SEND, &[channel, 1]).0,
            HOST_STATUS_SUCCESS
        );
        let (status, pending_send) = call_host(ASYNC_CHANNEL_SEND, &[channel, 2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN_STATUS, &[pending_send]),
            (HOST_STATUS_SUCCESS, 3)
        );
        assert_eq!(
            call_host(ASYNC_TASK_CANCEL, &[pending_send]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, first_recv) = call_host(ASYNC_CHANNEL_RECV, &[channel]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[first_recv]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, closing_recv) = call_host(ASYNC_CHANNEL_RECV, &[channel]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(ASYNC_CHANNEL_CLOSE, &[channel]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_TASK_JOIN, &[closing_recv]),
            (HOST_STATUS_SUCCESS, -1)
        );
        assert_eq!(
            call_host(ASYNC_CHANNEL_LEN, &[channel]),
            (HOST_STATUS_SUCCESS, 0)
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn async_reactor_host_calls_cover_backend_wake_timer_and_io() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(ASYNC_TASK_RESET, &[]).0, HOST_STATUS_SUCCESS);
        assert_eq!(call_host(ASYNC_REACTOR_RESET, &[]).0, HOST_STATUS_SUCCESS);

        let (status, backend) = call_host(ASYNC_REACTOR_BACKEND, &[]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        #[cfg(target_os = "linux")]
        assert_eq!(backend, 1);
        #[cfg(target_os = "windows")]
        assert_eq!(backend, 2);
        #[cfg(target_os = "macos")]
        assert_eq!(backend, 3);

        assert_eq!(
            call_host(ASYNC_REACTOR_WAKE, &[101]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_REACTOR_IO_REGISTER, &[202, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_REACTOR_IO_NOTIFY, &[202, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_REACTOR_TIMER, &[303, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let mut kinds = Vec::new();
        for _ in 0..3 {
            let (status, token) = call_host(ASYNC_REACTOR_POLL, &[100]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            assert_ne!(token, -1);
            let (status, kind) = call_host(ASYNC_REACTOR_LAST_KIND, &[]);
            assert_eq!(status, HOST_STATUS_SUCCESS);
            kinds.push(kind);
        }

        assert!(kinds.contains(&1));
        assert!(kinds.contains(&2));
        assert!(kinds.contains(&3));
        assert_eq!(
            call_host(ASYNC_REACTOR_STATS_TASK_WAKEUPS, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_REACTOR_STATS_TIMER_EVENTS, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_REACTOR_STATS_IO_EVENTS, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(ASYNC_REACTOR_STATS_IO_REGISTRATIONS, &[]),
            (HOST_STATUS_SUCCESS, 1)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_async_reactor_host_calls_handle_10k_task_wakeups() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(ASYNC_REACTOR_RESET, &[]).0, HOST_STATUS_SUCCESS);

        for task in 0..10_000 {
            assert_eq!(
                call_host(ASYNC_REACTOR_WAKE, &[task]),
                (HOST_STATUS_SUCCESS, 1)
            );
        }

        let mut drained = 0usize;
        while call_host(ASYNC_REACTOR_POLL, &[0]).1 != -1 {
            drained += 1;
        }

        assert_eq!(drained, 10_000);
        assert_eq!(
            call_host(ASYNC_REACTOR_STATS_TASK_WAKEUPS, &[]),
            (HOST_STATUS_SUCCESS, 10_000)
        );
    }

    #[test]
    fn serve_host_calls_cover_warmup_batching_cancellation_and_benchmark() {
        let _lock = test_guard();
        clear_host_functions();
        register();

        assert_eq!(call_host(SERVE_RESET, &[]).0, HOST_STATUS_SUCCESS);
        let (status, server) = call_host(SERVE_SERVER_NEW, &[3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_RESIDENT_MODEL, &[server]),
            (HOST_STATUS_SUCCESS, 3)
        );
        assert_eq!(
            call_host(SERVE_SERVER_IS_WARM, &[server]),
            (HOST_STATUS_SUCCESS, 0)
        );

        let (status, first) = call_host(SERVE_SERVER_ENQUEUE, &[server, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_PROCESS_BATCH, &[server, 1]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(SERVE_SERVER_WARMUP, &[server]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_PROCESS_BATCH, &[server, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, first]),
            (HOST_STATUS_SUCCESS, 30)
        );

        let (status, second) = call_host(SERVE_SERVER_ENQUEUE, &[server, 20]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_CANCEL, &[server, second]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_PENDING, &[server]),
            (HOST_STATUS_SUCCESS, 0)
        );
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, second]),
            (HOST_STATUS_SUCCESS, -1)
        );

        assert_eq!(
            call_host(SERVE_SERVER_BENCHMARK, &[server, 8, 3]),
            (HOST_STATUS_SUCCESS, 8)
        );
    }

    #[test]
    fn serve_host_calls_cover_guardrails_rate_limit_fallback_and_audit() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        assert_eq!(call_host(SERVE_RESET, &[]).0, HOST_STATUS_SUCCESS);
        let (status, server) = call_host(SERVE_SERVER_NEW, &[3]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_SET_FALLBACK, &[server, -999]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_SET_INPUT_POLICY, &[server, 0, 100]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_SET_OUTPUT_POLICY, &[server, 0, 200]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_SET_RATE_LIMIT, &[server, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_WARMUP, &[server]),
            (HOST_STATUS_SUCCESS, 1)
        );

        let (status, ok_request) = call_host(SERVE_SERVER_ENQUEUE, &[server, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_PROCESS_BATCH, &[server, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, ok_request]),
            (HOST_STATUS_SUCCESS, 30)
        );

        let (status, rate_limited) = call_host(SERVE_SERVER_ENQUEUE, &[server, 11]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, rate_limited]),
            (HOST_STATUS_SUCCESS, -999)
        );
        let (status, diagnostic_ptr) = call_host(SERVE_SERVER_LAST_DIAGNOSTIC, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let diagnostic = unsafe { read_spectra_string(diagnostic_ptr) }.expect("diagnostic");
        assert!(diagnostic.contains("\"policy\":\"rate_limit\""));

        assert_eq!(
            call_host(SERVE_SERVER_SET_RATE_LIMIT, &[server, 10]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, input_blocked) = call_host(SERVE_SERVER_ENQUEUE, &[server, 101]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, input_blocked]),
            (HOST_STATUS_SUCCESS, -999)
        );
        let (status, diagnostic_ptr) = call_host(SERVE_SERVER_LAST_DIAGNOSTIC, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let diagnostic = unsafe { read_spectra_string(diagnostic_ptr) }.expect("diagnostic");
        assert!(diagnostic.contains("\"stage\":\"input\""));
        assert!(diagnostic.contains("\"policy\":\"range\""));

        let (status, output_blocked) = call_host(SERVE_SERVER_ENQUEUE, &[server, 90]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_PROCESS_BATCH, &[server, 1]),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, output_blocked]),
            (HOST_STATUS_SUCCESS, -999)
        );
        let (status, diagnostic_ptr) = call_host(SERVE_SERVER_LAST_DIAGNOSTIC, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let diagnostic = unsafe { read_spectra_string(diagnostic_ptr) }.expect("diagnostic");
        assert!(diagnostic.contains("\"stage\":\"output\""));

        let (status, audit_ptr) = call_host(SERVE_SERVER_AUDIT_LOG, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let audit = unsafe { read_spectra_string(audit_ptr) }.expect("audit log");
        assert!(audit.contains("spectra.serve.audit.v1"));
        assert!(audit.contains("\"event\":\"blocked\""));
        assert!(audit.contains("\"event\":\"policy_attached\""));
    }

    #[test]
    fn serve_host_calls_cover_monitoring_drift_and_export() {
        let _lock = test_guard();
        clear_host_functions();
        register();
        crate::ffi::spectra_rt_manual_clear();

        assert_eq!(call_host(SERVE_RESET, &[]).0, HOST_STATUS_SUCCESS);
        let (status, server) = call_host(SERVE_SERVER_NEW, &[2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(
                SERVE_SERVER_SET_MODEL_VERSION,
                &[server, test_string("model-v1")]
            ),
            (HOST_STATUS_SUCCESS, 1)
        );
        assert_eq!(
            call_host(SERVE_SERVER_WARMUP, &[server]),
            (HOST_STATUS_SUCCESS, 1)
        );
        let (status, first) = call_host(SERVE_SERVER_ENQUEUE, &[server, 10]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let (status, second) = call_host(SERVE_SERVER_ENQUEUE, &[server, 20]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_PROCESS_BATCH, &[server, 2]),
            (HOST_STATUS_SUCCESS, 2)
        );
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, first]),
            (HOST_STATUS_SUCCESS, 20)
        );
        assert_eq!(
            call_host(SERVE_SERVER_RESULT, &[server, second]),
            (HOST_STATUS_SUCCESS, 40)
        );

        let (status, snapshot_ptr) = call_host(SERVE_SERVER_MONITORING_SNAPSHOT, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let snapshot = unsafe { read_spectra_string(snapshot_ptr) }.expect("snapshot");
        assert!(snapshot.contains("spectra.serve.monitoring_snapshot.v1"));
        assert!(snapshot.contains("\"model_version\":\"model-v1\""));
        assert!(snapshot.contains("\"requests\":2"));

        let (status, reference_ptr) = call_host(SERVE_SERVER_DISTRIBUTION_SUMMARY, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let reference = unsafe { read_spectra_string(reference_ptr) }.expect("reference");
        assert!(reference.contains("spectra.serve.distribution_summary.v1"));

        let (status, live_server) = call_host(SERVE_SERVER_NEW, &[2]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        assert_eq!(
            call_host(SERVE_SERVER_WARMUP, &[live_server]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(SERVE_SERVER_ENQUEUE, &[live_server, 110]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(SERVE_SERVER_ENQUEUE, &[live_server, 120]).0,
            HOST_STATUS_SUCCESS
        );
        assert_eq!(
            call_host(SERVE_SERVER_PROCESS_BATCH, &[live_server, 2]),
            (HOST_STATUS_SUCCESS, 2)
        );
        let (status, live_ptr) = call_host(SERVE_SERVER_DISTRIBUTION_SUMMARY, &[live_server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let live = unsafe { read_spectra_string(live_ptr) }.expect("live");

        let (status, drift_ptr) = call_host(
            SERVE_DRIFT_CHECK,
            &[test_string(&reference), test_string(&live), 100],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let drift = unsafe { read_spectra_string(drift_ptr) }.expect("drift");
        assert!(drift.contains("spectra.serve.drift_check.v1"));
        assert!(drift.contains("\"drifted\":true"));

        let (status, audit_ptr) = call_host(SERVE_SERVER_AUDIT_LOG, &[server]);
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let audit = unsafe { read_spectra_string(audit_ptr) }.expect("audit");
        let dir = std::env::temp_dir().join(format!(
            "spectra_r1903_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let path = dir.join("monitoring.json");
        let (status, export_ptr) = call_host(
            SERVE_EXPORT_MONITORING,
            &[
                server,
                test_string(path.to_string_lossy().as_ref()),
                test_string(&reference),
                test_string(&drift),
                test_string(&audit),
            ],
        );
        assert_eq!(status, HOST_STATUS_SUCCESS);
        let export_path = unsafe { read_spectra_string(export_ptr) }.expect("export path");
        let exported = std::fs::read_to_string(&export_path).expect("monitoring export");
        assert!(exported.contains("spectra.serve.monitoring_export.v1"));
        assert!(exported.contains("\"snapshot\""));
        assert!(exported.contains("\"drift\""));
        std::fs::remove_dir_all(&dir).ok();
    }
}
