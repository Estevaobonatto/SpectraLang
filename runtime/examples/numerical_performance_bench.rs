use spectra_runtime::ffi::{
    lookup_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_SUCCESS,
};
use spectra_runtime::stdlib;
use std::ptr;
use std::time::{Duration, Instant};

const TENSOR_ARANGE: &str = "spectra.std.tensor.arange";
const TENSOR_BACKWARD: &str = "spectra.std.tensor.backward";
const TENSOR_FREE_ALL: &str = "spectra.std.tensor.free_all";
const TENSOR_FULL_F: &str = "spectra.std.tensor.full_f";
const TENSOR_GET_F: &str = "spectra.std.tensor.get_f";
const TENSOR_GRAD: &str = "spectra.std.tensor.grad";
const TENSOR_LEN: &str = "spectra.std.tensor.len";
const TENSOR_MATMUL: &str = "spectra.std.tensor.matmul";
const TENSOR_MUL: &str = "spectra.std.tensor.mul";
const TENSOR_RELU: &str = "spectra.std.tensor.relu";
const TENSOR_RESHAPE: &str = "spectra.std.tensor.reshape";
const TENSOR_REQUIRES_GRAD: &str = "spectra.std.tensor.requires_grad";
const TENSOR_SET_GRAD_ENABLED: &str = "spectra.std.tensor.set_grad_enabled";
const TENSOR_SUM_F: &str = "spectra.std.tensor.sum_f";
const TENSOR_SUM_T: &str = "spectra.std.tensor.sum_t";

const ML_CONV2D: &str = "spectra.std.ml.conv2d";
const ML_DATALOADER_BATCH_COUNT: &str = "spectra.std.ml.dataloader_batch_count";
const ML_DATALOADER_BATCH_FEATURES: &str = "spectra.std.ml.dataloader_batch_features";
const ML_DATALOADER_BATCH_LABELS: &str = "spectra.std.ml.dataloader_batch_labels";
const ML_DATALOADER_NEW: &str = "spectra.std.ml.dataloader_new";
const ML_DATASET_FROM_TENSORS: &str = "spectra.std.ml.dataset_from_tensors";
const ML_DATASET_LEN: &str = "spectra.std.ml.dataset_len";
const ML_LINEAR: &str = "spectra.std.ml.linear";
const ML_MSE_LOSS: &str = "spectra.std.ml.mse_loss";
const ML_SGD_STEP: &str = "spectra.std.ml.sgd_step";

type HostFn = extern "C" fn(*mut SpectraHostCallContext) -> i32;

#[derive(Clone)]
struct BenchResult {
    id: &'static str,
    category: &'static str,
    iterations: usize,
    elapsed: Duration,
    correctness_passed: bool,
    detail: String,
}

impl BenchResult {
    fn ns_per_iter(&self) -> u128 {
        if self.iterations == 0 {
            return self.elapsed.as_nanos();
        }
        self.elapsed.as_nanos() / self.iterations as u128
    }
}

fn f64_arg(value: f64) -> SpectraHostValue {
    value.to_bits() as SpectraHostValue
}

fn f64_result(value: SpectraHostValue) -> f64 {
    f64::from_bits(value as u64)
}

fn host(name: &str) -> HostFn {
    lookup_host_function(name).unwrap_or_else(|| panic!("{name} is not registered"))
}

fn call_host_fn(name: &str, func: HostFn, args: &[SpectraHostValue]) -> SpectraHostValue {
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
    assert_eq!(status, HOST_STATUS_SUCCESS, "{name} failed with {status}");
    results[0]
}

fn call_host(name: &str, args: &[SpectraHostValue]) -> SpectraHostValue {
    call_host_fn(name, host(name), args)
}

fn approx_eq(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

fn measure(
    id: &'static str,
    category: &'static str,
    iterations: usize,
    mut work: impl FnMut(),
    correctness_passed: bool,
    detail: impl Into<String>,
) -> BenchResult {
    let start = Instant::now();
    for _ in 0..iterations {
        work();
    }
    BenchResult {
        id,
        category,
        iterations,
        elapsed: start.elapsed(),
        correctness_passed,
        detail: detail.into(),
    }
}

fn bench_tensor_creation() -> BenchResult {
    let full_f = host(TENSOR_FULL_F);
    let free_all = host(TENSOR_FREE_ALL);
    let iterations = 128;
    let result = measure(
        "tensor_creation_full_f",
        "tensor_creation",
        iterations,
        || {
            let handle = call_host_fn(TENSOR_FULL_F, full_f, &[1024, f64_arg(1.25)]);
            std::hint::black_box(handle);
        },
        true,
        "full_f creates rank1 float tensors",
    );
    let _ = call_host_fn(TENSOR_FREE_ALL, free_all, &[]);
    result
}

fn bench_unary_ops() -> BenchResult {
    let input = call_host(TENSOR_FULL_F, &[8192, f64_arg(2.0)]);
    let relu = host(TENSOR_RELU);
    let iterations = 256;
    let result = measure(
        "tensor_unary_relu",
        "unary_ops",
        iterations,
        || {
            let out = call_host_fn(TENSOR_RELU, relu, &[input]);
            std::hint::black_box(out);
        },
        true,
        "relu over float tensor",
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_reductions() -> BenchResult {
    let input = call_host(TENSOR_FULL_F, &[4096, f64_arg(2.0)]);
    let sum_f = host(TENSOR_SUM_F);
    let expected = 8192.0;
    let observed = f64_result(call_host(TENSOR_SUM_F, &[input]));
    let iterations = 512;
    let result = measure(
        "tensor_reduction_sum_f",
        "reductions",
        iterations,
        || {
            let sum = call_host_fn(TENSOR_SUM_F, sum_f, &[input]);
            std::hint::black_box(sum);
        },
        approx_eq(observed, expected, 1e-9),
        format!("expected_sum={expected}, observed_sum={observed}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_matmul() -> BenchResult {
    let flat = call_host(TENSOR_ARANGE, &[1, 1025, 1]);
    let matrix = call_host(TENSOR_RESHAPE, &[flat, 32, 32]);
    let matmul = host(TENSOR_MATMUL);
    let len = call_host(TENSOR_LEN, &[call_host(TENSOR_MATMUL, &[matrix, matrix])]);
    let iterations = 96;
    let result = measure(
        "tensor_matmul_32x32",
        "matmul",
        iterations,
        || {
            let out = call_host_fn(TENSOR_MATMUL, matmul, &[matrix, matrix]);
            std::hint::black_box(out);
        },
        len == 1024,
        format!("output_len={len}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_convolution() -> BenchResult {
    let input = call_host(TENSOR_FULL_F, &[64, f64_arg(1.0)]);
    let kernel = call_host(TENSOR_FULL_F, &[9, f64_arg(0.25)]);
    let bias = call_host(TENSOR_FULL_F, &[1, f64_arg(0.0)]);
    let conv2d = host(ML_CONV2D);
    let args = [input, kernel, bias, 1, 1, 8, 8, 1, 3, 3];
    let len = call_host(TENSOR_LEN, &[call_host(ML_CONV2D, &args)]);
    let iterations = 96;
    let result = measure(
        "ml_conv2d_8x8_k3",
        "convolution",
        iterations,
        || {
            let out = call_host_fn(ML_CONV2D, conv2d, &args);
            std::hint::black_box(out);
        },
        len == 36,
        format!("output_len={len}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_autodiff() -> BenchResult {
    let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
    let base = call_host(TENSOR_FULL_F, &[256, f64_arg(3.0)]);
    let trainable = call_host(TENSOR_REQUIRES_GRAD, &[base, 1]);
    let mul = host(TENSOR_MUL);
    let sum_t = host(TENSOR_SUM_T);
    let backward = host(TENSOR_BACKWARD);
    let iterations = 64;
    let result = measure(
        "autodiff_square_sum_backward",
        "autodiff",
        iterations,
        || {
            let squared = call_host_fn(TENSOR_MUL, mul, &[trainable, trainable]);
            let loss = call_host_fn(TENSOR_SUM_T, sum_t, &[squared]);
            let _ = call_host_fn(TENSOR_BACKWARD, backward, &[loss]);
        },
        true,
        "square-sum backward over trainable tensor",
    );
    let grad = call_host(TENSOR_GRAD, &[trainable]);
    let grad_sum = f64_result(call_host(TENSOR_SUM_F, &[grad]));
    let mut result = result;
    result.correctness_passed = grad_sum.is_finite() && grad_sum > 0.0;
    result.detail = format!("grad_sum={grad_sum}");
    let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[0]);
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_optimizer_step() -> BenchResult {
    let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
    let x = call_host(
        TENSOR_RESHAPE,
        &[call_host(TENSOR_FULL_F, &[64, f64_arg(1.0)]), 64, 1],
    );
    let target = call_host(TENSOR_FULL_F, &[64, f64_arg(2.0)]);
    let w0 = call_host(TENSOR_FULL_F, &[1, f64_arg(0.0)]);
    let w = call_host(
        TENSOR_RESHAPE,
        &[call_host(TENSOR_REQUIRES_GRAD, &[w0, 1]), 1, 1],
    );
    let b = call_host(
        TENSOR_REQUIRES_GRAD,
        &[call_host(TENSOR_FULL_F, &[1, f64_arg(0.0)]), 1],
    );
    let linear = host(ML_LINEAR);
    let mse_loss = host(ML_MSE_LOSS);
    let backward = host(TENSOR_BACKWARD);
    let sgd_step = host(ML_SGD_STEP);
    let iterations = 48;
    let result = measure(
        "ml_optimizer_sgd_linear",
        "optimizer_steps",
        iterations,
        || {
            let pred = call_host_fn(ML_LINEAR, linear, &[x, w, b]);
            let loss = call_host_fn(ML_MSE_LOSS, mse_loss, &[pred, target]);
            let _ = call_host_fn(TENSOR_BACKWARD, backward, &[loss]);
            let _ = call_host_fn(ML_SGD_STEP, sgd_step, &[w, f64_arg(0.01)]);
            let _ = call_host_fn(ML_SGD_STEP, sgd_step, &[b, f64_arg(0.01)]);
        },
        true,
        "linear mse backward plus sgd updates",
    );
    let pred = call_host(ML_LINEAR, &[x, w, b]);
    let loss = call_host(ML_MSE_LOSS, &[pred, target]);
    let final_loss = f64_result(call_host(TENSOR_GET_F, &[loss, 0]));
    let mut result = result;
    result.correctness_passed = final_loss.is_finite() && final_loss >= 0.0;
    result.detail = format!("final_loss={final_loss}");
    let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[0]);
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_data_loading() -> BenchResult {
    let features = call_host(
        TENSOR_RESHAPE,
        &[call_host(TENSOR_FULL_F, &[128, f64_arg(1.0)]), 64, 2],
    );
    let labels = call_host(TENSOR_FULL_F, &[64, f64_arg(1.0)]);
    let dataset = call_host(ML_DATASET_FROM_TENSORS, &[features, labels, 64]);
    let loader = call_host(ML_DATALOADER_NEW, &[dataset, 8, 42]);
    let batch_features = host(ML_DATALOADER_BATCH_FEATURES);
    let batch_labels = host(ML_DATALOADER_BATCH_LABELS);
    let len = call_host(ML_DATASET_LEN, &[dataset]);
    let batches = call_host(ML_DATALOADER_BATCH_COUNT, &[loader]);
    let iterations = 256;
    let result = measure(
        "ml_dataloader_batches",
        "data_loading",
        iterations,
        || {
            let idx = (std::hint::black_box(3usize) % batches as usize) as SpectraHostValue;
            let x = call_host_fn(ML_DATALOADER_BATCH_FEATURES, batch_features, &[loader, idx]);
            let y = call_host_fn(ML_DATALOADER_BATCH_LABELS, batch_labels, &[loader, idx]);
            std::hint::black_box((x, y));
        },
        len == 64 && batches == 8,
        format!("dataset_len={len}, batch_count={batches}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn json_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn print_json(results: &[BenchResult]) {
    let passed = results.iter().all(|result| result.correctness_passed);
    let total_elapsed_ns = results.iter().fold(0u128, |acc, result| {
        acc.saturating_add(result.elapsed.as_nanos())
    });
    println!("{{");
    println!("  \"schema\": \"spectra.r1501.benchmark.v1\",");
    println!(
        "  \"profile\": \"{}\",",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("  \"passed\": {passed},");
    println!("  \"total_elapsed_ns\": {total_elapsed_ns},");
    println!("  \"benchmarks\": [");
    for (index, result) in results.iter().enumerate() {
        let comma = if index + 1 == results.len() { "" } else { "," };
        println!(
            "    {{\"id\":\"{}\",\"category\":\"{}\",\"iterations\":{},\"elapsed_ns\":{},\"ns_per_iter\":{},\"correctness_passed\":{},\"detail\":\"{}\"}}{}",
            result.id,
            result.category,
            result.iterations,
            result.elapsed.as_nanos(),
            result.ns_per_iter(),
            result.correctness_passed,
            json_escape(&result.detail),
            comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn main() {
    stdlib::register();
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    let results = vec![
        bench_tensor_creation(),
        bench_unary_ops(),
        bench_reductions(),
        bench_matmul(),
        bench_convolution(),
        bench_autodiff(),
        bench_optimizer_step(),
        bench_data_loading(),
    ];
    print_json(&results);
    if cfg!(debug_assertions) || results.iter().any(|result| !result.correctness_passed) {
        std::process::exit(1);
    }
}
