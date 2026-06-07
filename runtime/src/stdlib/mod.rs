use crate::ffi::{
    register_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_INTERNAL_ERROR,
    HOST_STATUS_INVALID_ARGUMENT, HOST_STATUS_NOT_FOUND, HOST_STATUS_SUCCESS,
};
use crate::initialize;
use crate::memory::ManualBox;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, BufRead, Write};
use std::slice;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
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
    register_tensor();
    register_ml();
    register_concurrent();
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
        if let Some(index) = self.pool.iter().position(|buffer| buffer.capacity() >= len) {
            let mut buffer = self.pool.swap_remove(index);
            buffer.clear();
            buffer.resize(len, 0);
            self.metrics.reused_buffers = self.metrics.reused_buffers.saturating_add(1);
            self.metrics.pool_hits = self.metrics.pool_hits.saturating_add(1);
            buffer
        } else {
            self.metrics.pool_misses = self.metrics.pool_misses.saturating_add(1);
            vec![0; len]
        }
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
    let mut guard = tensor_registry()
        .lock()
        .expect("tensor registry mutex poisoned");
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
    *tensor_grad_enabled()
        .lock()
        .expect("tensor grad mode mutex poisoned")
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
        if args[0] <= 0 {
            return HOST_STATUS_INVALID_ARGUMENT;
        }
        let data = vec![args[1]; args[0] as usize];
        match tensor_alloc(TensorDType::Float, vec![args[0] as usize], data) {
            Ok(handle) => tensor_result(ctx_ref, handle as SpectraHostValue),
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
        *tensor_grad_enabled()
            .lock()
            .expect("tensor grad mode mutex poisoned") = args[0] != 0;
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

struct MlRegistry {
    next_id: usize,
    modules: HashMap<usize, MlModule>,
    datasets: HashMap<usize, MlDataset>,
    loaders: HashMap<usize, MlDataLoader>,
    dataframes: HashMap<usize, MlDataFrame>,
    experiments: HashMap<usize, MlExperiment>,
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
    let mut guard = ml_registry().lock().expect("ml registry mutex poisoned");
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

extern "C" fn std_tensor_seed(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, args)) = tensor_args(ctx, 1) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        *random_state().lock().expect("random mutex poisoned") = args[0] as u64;
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
        let mut state = random_state().lock().expect("random mutex poisoned");
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
        let mut state = random_state().lock().expect("random mutex poisoned");
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
        let mut state = random_state().lock().expect("random mutex poisoned");
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
        *tensor_deterministic_mode()
            .lock()
            .expect("tensor deterministic mode mutex poisoned") = enabled;
        if enabled {
            *random_state().lock().expect("random mutex poisoned") = 0x5350_4543_5452_4131;
        }
        tensor_optional_result(ctx_ref, 0)
    }
}

extern "C" fn std_tensor_deterministic_mode(ctx: *mut SpectraHostCallContext) -> i32 {
    unsafe {
        let Ok((ctx_ref, _args)) = tensor_args(ctx, 0) else {
            return HOST_STATUS_INVALID_ARGUMENT;
        };
        let enabled = *tensor_deterministic_mode()
            .lock()
            .expect("tensor deterministic mode mutex poisoned");
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
        let mut state = random_state().lock().expect("random mutex poisoned");
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
        let mut state = random_state().lock().expect("random mutex poisoned");
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
        let f = random_unit_f64(&mut *random_state().lock().expect("random mutex poisoned"));
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

struct ConcurrentTask {
    handle: Option<JoinHandle<SpectraHostValue>>,
    result: Option<SpectraHostValue>,
}

struct ConcurrentChannel {
    queue: VecDeque<SpectraHostValue>,
    closed: bool,
}

struct ConcurrentRegistry {
    next_task: SpectraHostValue,
    next_channel: SpectraHostValue,
    next_counter: SpectraHostValue,
    tasks_spawned: SpectraHostValue,
    tasks: HashMap<SpectraHostValue, ConcurrentTask>,
    channels: HashMap<SpectraHostValue, ConcurrentChannel>,
    counters: HashMap<SpectraHostValue, SpectraHostValue>,
}

impl ConcurrentRegistry {
    fn new() -> Self {
        Self {
            next_task: 1,
            next_channel: 1,
            next_counter: 1,
            tasks_spawned: 0,
            tasks: HashMap::new(),
            channels: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
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
    let handle = thread::spawn(move || value);

    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let task_id = registry.next_task;
    registry.next_task += 1;
    registry.tasks_spawned += 1;
    registry.tasks.insert(
        task_id,
        ConcurrentTask {
            handle: Some(handle),
            result: None,
        },
    );
    results[0] = task_id;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_task_join(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let task_id = args[0];
    let handle = {
        let mut registry = match lock_concurrent_registry() {
            Ok(registry) => registry,
            Err(status) => return status,
        };
        let Some(task) = registry.tasks.get_mut(&task_id) else {
            return HOST_STATUS_NOT_FOUND;
        };
        if let Some(result) = task.result {
            results[0] = result;
            return HOST_STATUS_SUCCESS;
        }
        task.handle.take()
    };

    let Some(handle) = handle else {
        return HOST_STATUS_INTERNAL_ERROR;
    };
    let result = match handle.join() {
        Ok(value) => value,
        Err(_) => return HOST_STATUS_INTERNAL_ERROR,
    };

    let mut registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(task) = registry.tasks.get_mut(&task_id) else {
        return HOST_STATUS_NOT_FOUND;
    };
    task.result = Some(result);
    results[0] = result;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_concurrent_task_is_done(ctx: *mut SpectraHostCallContext) -> i32 {
    let (args, results) = match host_call_args(ctx, 1) {
        Ok(parts) => parts,
        Err(status) => return status,
    };
    let registry = match lock_concurrent_registry() {
        Ok(registry) => registry,
        Err(status) => return status,
    };
    let Some(task) = registry.tasks.get(&args[0]) else {
        return HOST_STATUS_NOT_FOUND;
    };
    let done = task.result.is_some()
        || task
            .handle
            .as_ref()
            .map(|handle| handle.is_finished())
            .unwrap_or(true);
    results[0] = i64::from(done);
    HOST_STATUS_SUCCESS
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
    warm: bool,
    timeout: SpectraHostValue,
    queue: VecDeque<SpectraHostValue>,
    requests: HashMap<SpectraHostValue, (SpectraHostValue, ServeRequestState)>,
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
            warm: false,
            timeout: 1,
            queue: VecDeque::new(),
            requests: HashMap::new(),
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
    server
        .requests
        .insert(request_id, (args[1], ServeRequestState::Pending));
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

    let mut processed = 0;
    for _ in 0..max_batch {
        let Some(request_id) = server.queue.pop_front() else {
            break;
        };
        let Some((input, state)) = server.requests.get_mut(&request_id) else {
            continue;
        };
        if *state != ServeRequestState::Pending {
            continue;
        }
        if server.timeout == 0 {
            *state = ServeRequestState::Cancelled;
            continue;
        }
        *state = ServeRequestState::Complete(*input * server.model);
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
        let mut processed = 0;
        for _ in 0..batch {
            let Some(request_id) = server.queue.pop_front() else {
                break;
            };
            let Some((input, state)) = server.requests.get_mut(&request_id) else {
                continue;
            };
            if *state != ServeRequestState::Pending {
                continue;
            }
            if server.timeout == 0 {
                *state = ServeRequestState::Cancelled;
                continue;
            }
            *state = ServeRequestState::Complete(*input * server.model);
            processed += 1;
        }
        processed_total += processed;
    }
    results[0] = processed_total;
    HOST_STATUS_SUCCESS
}

extern "C" fn std_serve_reset(ctx: *mut SpectraHostCallContext) -> i32 {
    let _ = match host_call_void_args(ctx, 0) {
        Ok(args) => args,
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
            call_host(CONCURRENT_TASK_JOIN, &[task]),
            (HOST_STATUS_SUCCESS, 42)
        );
        assert_eq!(
            call_host(CONCURRENT_TASK_IS_DONE, &[task]),
            (HOST_STATUS_SUCCESS, 1)
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
}
