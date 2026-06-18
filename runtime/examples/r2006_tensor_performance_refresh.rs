use spectra_runtime::ffi::{
    lookup_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_SUCCESS,
};
use spectra_runtime::stdlib;
use std::ptr;
use std::time::{Duration, Instant};

const TENSOR_ADD: &str = "spectra.std.tensor.add";
const TENSOR_ARANGE: &str = "spectra.std.tensor.arange";
const TENSOR_BACKWARD: &str = "spectra.std.tensor.backward";
const TENSOR_FLATTEN: &str = "spectra.std.tensor.flatten";
const TENSOR_FREE: &str = "spectra.std.tensor.free";
const TENSOR_FREE_ALL: &str = "spectra.std.tensor.free_all";
const TENSOR_FULL_F: &str = "spectra.std.tensor.full_f";
const TENSOR_GET: &str = "spectra.std.tensor.get";
const TENSOR_GRAD: &str = "spectra.std.tensor.grad";
const TENSOR_LEN: &str = "spectra.std.tensor.len";
const TENSOR_MATMUL: &str = "spectra.std.tensor.matmul";
const TENSOR_MUL: &str = "spectra.std.tensor.mul";
const TENSOR_RELU: &str = "spectra.std.tensor.relu";
const TENSOR_RESHAPE: &str = "spectra.std.tensor.reshape";
const TENSOR_REQUIRES_GRAD: &str = "spectra.std.tensor.requires_grad";
const TENSOR_RESET_STATS: &str = "spectra.std.tensor.reset_stats";
const TENSOR_SET_GRAD_ENABLED: &str = "spectra.std.tensor.set_grad_enabled";
const TENSOR_STATS_ALLOCATIONS: &str = "spectra.std.tensor.stats_allocations";
const TENSOR_STATS_KERNEL_ELEMENTS: &str = "spectra.std.tensor.stats_kernel_elements";
const TENSOR_STATS_KERNEL_OPS: &str = "spectra.std.tensor.stats_kernel_ops";
const TENSOR_STATS_PEAK_BYTES: &str = "spectra.std.tensor.stats_peak_bytes";
const TENSOR_STATS_POOL_HITS: &str = "spectra.std.tensor.stats_pool_hits";
const TENSOR_STATS_POOL_MISSES: &str = "spectra.std.tensor.stats_pool_misses";
const TENSOR_STATS_REUSED_BUFFERS: &str = "spectra.std.tensor.stats_reused_buffers";
const TENSOR_STATS_REUSE_RATE_PER_MILLE: &str = "spectra.std.tensor.stats_reuse_rate_per_mille";
const TENSOR_STATS_SCRATCH_REUSES: &str = "spectra.std.tensor.stats_scratch_reuses";
const TENSOR_SUM_F: &str = "spectra.std.tensor.sum_f";
const TENSOR_SUM_T: &str = "spectra.std.tensor.sum_t";
const TENSOR_TRANSPOSE: &str = "spectra.std.tensor.transpose";

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

#[derive(Clone, Copy)]
struct TensorMetrics {
    allocations: SpectraHostValue,
    peak_bytes: SpectraHostValue,
    reused_buffers: SpectraHostValue,
    pool_hits: SpectraHostValue,
    pool_misses: SpectraHostValue,
    scratch_reuses: SpectraHostValue,
    reuse_rate_per_mille: SpectraHostValue,
    kernel_ops: SpectraHostValue,
    kernel_elements: SpectraHostValue,
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

fn free_tensor(handle: SpectraHostValue) {
    let _ = call_host(TENSOR_FREE, &[handle]);
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

fn tensor_metrics() -> TensorMetrics {
    TensorMetrics {
        allocations: call_host(TENSOR_STATS_ALLOCATIONS, &[]),
        peak_bytes: call_host(TENSOR_STATS_PEAK_BYTES, &[]),
        reused_buffers: call_host(TENSOR_STATS_REUSED_BUFFERS, &[]),
        pool_hits: call_host(TENSOR_STATS_POOL_HITS, &[]),
        pool_misses: call_host(TENSOR_STATS_POOL_MISSES, &[]),
        scratch_reuses: call_host(TENSOR_STATS_SCRATCH_REUSES, &[]),
        reuse_rate_per_mille: call_host(TENSOR_STATS_REUSE_RATE_PER_MILLE, &[]),
        kernel_ops: call_host(TENSOR_STATS_KERNEL_OPS, &[]),
        kernel_elements: call_host(TENSOR_STATS_KERNEL_ELEMENTS, &[]),
    }
}

fn bench_materialization_view_chain() -> BenchResult {
    let flat = call_host(TENSOR_ARANGE, &[1, 1025, 1]);
    let base = call_host(TENSOR_RESHAPE, &[flat, 32, 32]);
    let transpose = host(TENSOR_TRANSPOSE);
    let flatten = host(TENSOR_FLATTEN);
    let sum_f = host(TENSOR_SUM_F);
    let warm_transposed = call_host(TENSOR_TRANSPOSE, &[base]);
    let warm_flat = call_host(TENSOR_FLATTEN, &[warm_transposed]);
    let warm_sum = f64_result(call_host(TENSOR_SUM_F, &[warm_flat]));
    free_tensor(warm_flat);
    free_tensor(warm_transposed);

    let iterations = 96;
    let result = measure(
        "tensor_materialization_view_chain",
        "materialization",
        iterations,
        || {
            let transposed = call_host_fn(TENSOR_TRANSPOSE, transpose, &[base]);
            let flat = call_host_fn(TENSOR_FLATTEN, flatten, &[transposed]);
            let sum = call_host_fn(TENSOR_SUM_F, sum_f, &[flat]);
            std::hint::black_box(sum);
            free_tensor(flat);
            free_tensor(transposed);
        },
        approx_eq(warm_sum, 524800.0, 1e-9),
        format!("view_chain_sum={warm_sum}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_elementwise_chain() -> BenchResult {
    let input = call_host(TENSOR_FULL_F, &[2048, f64_arg(2.0)]);
    let add = host(TENSOR_ADD);
    let mul = host(TENSOR_MUL);
    let relu = host(TENSOR_RELU);
    let sum_f = host(TENSOR_SUM_F);

    let doubled = call_host(TENSOR_ADD, &[input, input]);
    let multiplied = call_host(TENSOR_MUL, &[doubled, input]);
    let activated = call_host(TENSOR_RELU, &[multiplied]);
    let observed_sum = f64_result(call_host(TENSOR_SUM_F, &[activated]));
    free_tensor(activated);
    free_tensor(multiplied);
    free_tensor(doubled);

    let iterations = 96;
    let result = measure(
        "tensor_elementwise_chain_add_mul_relu",
        "elementwise_chains",
        iterations,
        || {
            let doubled = call_host_fn(TENSOR_ADD, add, &[input, input]);
            let multiplied = call_host_fn(TENSOR_MUL, mul, &[doubled, input]);
            let activated = call_host_fn(TENSOR_RELU, relu, &[multiplied]);
            let sum = call_host_fn(TENSOR_SUM_F, sum_f, &[activated]);
            std::hint::black_box(sum);
            free_tensor(activated);
            free_tensor(multiplied);
            free_tensor(doubled);
        },
        approx_eq(observed_sum, 16384.0, 1e-9),
        format!("chain_sum={observed_sum}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_reduction_sum_f_refresh() -> BenchResult {
    let input = call_host(TENSOR_FULL_F, &[8192, f64_arg(1.5)]);
    let sum_f = host(TENSOR_SUM_F);
    let observed_sum = f64_result(call_host(TENSOR_SUM_F, &[input]));
    let iterations = 384;
    let result = measure(
        "tensor_reduction_sum_f_refresh",
        "reductions",
        iterations,
        || {
            let sum = call_host_fn(TENSOR_SUM_F, sum_f, &[input]);
            std::hint::black_box(sum);
        },
        approx_eq(observed_sum, 12288.0, 1e-9),
        format!("sum_f={observed_sum}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_matmul_refresh() -> BenchResult {
    let flat = call_host(TENSOR_ARANGE, &[1, 1025, 1]);
    let matrix = call_host(TENSOR_RESHAPE, &[flat, 32, 32]);
    let matmul = host(TENSOR_MATMUL);
    let get = host(TENSOR_GET);
    let warm_product = call_host(TENSOR_MATMUL, &[matrix, matrix]);
    let warm_len = call_host(TENSOR_LEN, &[warm_product]);
    let warm_p00 = call_host_fn(TENSOR_GET, get, &[warm_product, 0]);
    free_tensor(warm_product);

    let iterations = 80;
    let result = measure(
        "tensor_matmul_32x32_refresh",
        "matmul",
        iterations,
        || {
            let out = call_host_fn(TENSOR_MATMUL, matmul, &[matrix, matrix]);
            let first = call_host_fn(TENSOR_GET, get, &[out, 0]);
            std::hint::black_box(first);
            free_tensor(out);
        },
        warm_len == 1024 && warm_p00 == 349712,
        format!("output_len={warm_len}, p00={warm_p00}"),
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn bench_autodiff_refresh() -> BenchResult {
    let _ = call_host(TENSOR_SET_GRAD_ENABLED, &[1]);
    let base = call_host(TENSOR_FULL_F, &[256, f64_arg(3.0)]);
    let trainable = call_host(TENSOR_REQUIRES_GRAD, &[base, 1]);
    let mul = host(TENSOR_MUL);
    let sum_t = host(TENSOR_SUM_T);
    let backward = host(TENSOR_BACKWARD);
    let iterations = 48;
    let result = measure(
        "tensor_autodiff_square_sum_refresh",
        "autodiff",
        iterations,
        || {
            let squared = call_host_fn(TENSOR_MUL, mul, &[trainable, trainable]);
            let loss = call_host_fn(TENSOR_SUM_T, sum_t, &[squared]);
            let _ = call_host_fn(TENSOR_BACKWARD, backward, &[loss]);
            std::hint::black_box(loss);
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

fn bench_buffer_reuse_refresh() -> BenchResult {
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    let full_f = host(TENSOR_FULL_F);
    let free = host(TENSOR_FREE);
    let warm = call_host(TENSOR_FULL_F, &[4096, f64_arg(1.0)]);
    free_tensor(warm);

    let iterations = 160;
    let result = measure(
        "tensor_buffer_reuse_full_refresh",
        "buffer_reuse",
        iterations,
        || {
            let handle = call_host_fn(TENSOR_FULL_F, full_f, &[4096, f64_arg(1.0)]);
            std::hint::black_box(handle);
            let _ = call_host_fn(TENSOR_FREE, free, &[handle]);
        },
        true,
        "repeated full_f/free cycle should hit the tensor buffer pool",
    );
    let metrics = tensor_metrics();
    let mut result = result;
    result.correctness_passed =
        metrics.pool_hits > 0 && metrics.reused_buffers > 0 && metrics.reuse_rate_per_mille > 0;
    result.detail = format!(
        "pool_hits={}, pool_misses={}, reused_buffers={}, reuse_rate_per_mille={}",
        metrics.pool_hits,
        metrics.pool_misses,
        metrics.reused_buffers,
        metrics.reuse_rate_per_mille
    );
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    result
}

fn json_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn print_json(results: &[BenchResult], metrics: TensorMetrics) {
    let passed = results.iter().all(|result| result.correctness_passed)
        && metrics.allocations > 0
        && metrics.peak_bytes > 0
        && metrics.reused_buffers > 0
        && metrics.pool_hits > 0
        && metrics.scratch_reuses > 0
        && metrics.kernel_ops > 0;
    let total_elapsed_ns = results.iter().fold(0u128, |acc, result| {
        acc.saturating_add(result.elapsed.as_nanos())
    });
    println!("{{");
    println!("  \"schema\": \"spectra.r2006.performance_refresh.v1\",");
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
    println!("  \"memory\": {{");
    println!("    \"allocations\": {},", metrics.allocations);
    println!("    \"peak_bytes\": {},", metrics.peak_bytes);
    println!("    \"reused_buffers\": {},", metrics.reused_buffers);
    println!("    \"pool_hits\": {},", metrics.pool_hits);
    println!("    \"pool_misses\": {},", metrics.pool_misses);
    println!("    \"scratch_reuses\": {},", metrics.scratch_reuses);
    println!(
        "    \"reuse_rate_per_mille\": {},",
        metrics.reuse_rate_per_mille
    );
    println!("    \"kernel_ops\": {},", metrics.kernel_ops);
    println!("    \"kernel_elements\": {}", metrics.kernel_elements);
    println!("  }},");
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
    let _ = call_host(TENSOR_RESET_STATS, &[]);
    let results = vec![
        bench_materialization_view_chain(),
        bench_elementwise_chain(),
        bench_reduction_sum_f_refresh(),
        bench_matmul_refresh(),
        bench_autodiff_refresh(),
        bench_buffer_reuse_refresh(),
    ];
    let metrics = tensor_metrics();
    print_json(&results, metrics);
    if cfg!(debug_assertions)
        || results.iter().any(|result| !result.correctness_passed)
        || metrics.allocations == 0
        || metrics.peak_bytes == 0
        || metrics.reused_buffers == 0
        || metrics.pool_hits == 0
        || metrics.scratch_reuses == 0
        || metrics.kernel_ops == 0
    {
        std::process::exit(1);
    }
}
