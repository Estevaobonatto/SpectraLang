use spectra_runtime::ffi::{
    lookup_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_SUCCESS,
};
use spectra_runtime::stdlib::{self, tensor_bench_kernel_dot_i64, tensor_bench_kernel_matmul_i64};
use std::ptr;
use std::time::{Duration, Instant};

const TENSOR_ARANGE: &str = "spectra.std.tensor.arange";
const TENSOR_DOT: &str = "spectra.std.tensor.dot";
const TENSOR_FREE_ALL: &str = "spectra.std.tensor.free_all";
const TENSOR_MATMUL: &str = "spectra.std.tensor.matmul";
const TENSOR_RESHAPE: &str = "spectra.std.tensor.reshape";
const TENSOR_RESET_STATS: &str = "spectra.std.tensor.reset_stats";
const TENSOR_STATS_POOL_HITS: &str = "spectra.std.tensor.stats_pool_hits";
const TENSOR_STATS_POOL_MISSES: &str = "spectra.std.tensor.stats_pool_misses";
const TENSOR_STATS_SCRATCH_REUSES: &str = "spectra.std.tensor.stats_scratch_reuses";

type HostFn = extern "C" fn(*mut SpectraHostCallContext) -> i32;

fn host(name: &str) -> HostFn {
    lookup_host_function(name).unwrap_or_else(|| panic!("{name} not registered"))
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

fn time_it(iterations: usize, mut work: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        work();
    }
    start.elapsed()
}

fn comparable_or_faster(candidate: Duration, baseline: Duration) -> bool {
    // Treat <=10% overhead as same-speed to absorb normal CI/desktop timing noise.
    candidate.as_nanos() <= baseline.as_nanos().saturating_mul(110) / 100
}

fn naive_dot(values: &[i64]) -> i64 {
    let mut acc = 0;
    let mut idx = 0usize;
    while idx < values.len() {
        let value = std::hint::black_box(values[idx]);
        acc += value * value;
        idx += 1;
    }
    std::hint::black_box(acc)
}

fn naive_matmul_square(values: &[i64], size: usize) -> Vec<i64> {
    let mut out = vec![0; size * size];
    for row in 0..size {
        for col in 0..size {
            let mut acc = 0;
            for idx in 0..size {
                acc += values[row * size + idx] * values[idx * size + col];
            }
            out[row * size + col] = std::hint::black_box(acc);
        }
    }
    out
}

fn main() {
    stdlib::register();
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    let _ = call_host(TENSOR_RESET_STATS, &[]);

    let dot_len = 65_536usize;
    let dot_iterations = 500usize;
    let dot_values = (1..=dot_len as i64).collect::<Vec<_>>();
    let left = call_host(TENSOR_ARANGE, &[1, dot_len as i64 + 1, 1]);
    let right = call_host(TENSOR_ARANGE, &[1, dot_len as i64 + 1, 1]);
    let dot_fn = host(TENSOR_DOT);

    let naive_dot_elapsed = time_it(dot_iterations, || {
        let result = naive_dot(&dot_values);
        std::hint::black_box(result);
    });
    let kernel_dot_elapsed = time_it(dot_iterations, || {
        let result = tensor_bench_kernel_dot_i64(&dot_values, &dot_values);
        std::hint::black_box(result);
    });
    let runtime_dot_elapsed = time_it(dot_iterations, || {
        let result = call_host_fn(TENSOR_DOT, dot_fn, &[left, right]);
        std::hint::black_box(result);
    });

    let mat_size = 32usize;
    let mat_iterations = 120usize;
    let mat_values = (1..=mat_size as i64 * mat_size as i64).collect::<Vec<_>>();
    let flat = call_host(
        TENSOR_ARANGE,
        &[1, mat_size as i64 * mat_size as i64 + 1, 1],
    );
    let matrix = call_host(TENSOR_RESHAPE, &[flat, mat_size as i64, mat_size as i64]);
    let matmul_fn = host(TENSOR_MATMUL);

    let naive_matmul_elapsed = time_it(mat_iterations, || {
        let result = naive_matmul_square(&mat_values, mat_size);
        std::hint::black_box(result);
    });
    let kernel_matmul_elapsed = time_it(mat_iterations, || {
        let result =
            tensor_bench_kernel_matmul_i64(&mat_values, &mat_values, mat_size, mat_size, mat_size);
        std::hint::black_box(result);
    });
    let runtime_matmul_elapsed = time_it(mat_iterations, || {
        let result = call_host_fn(TENSOR_MATMUL, matmul_fn, &[matrix, matrix]);
        std::hint::black_box(result);
    });

    let _ = call_host(TENSOR_FREE_ALL, &[]);
    let scratch = call_host(TENSOR_ARANGE, &[1, 1025, 1]);
    std::hint::black_box(scratch);
    let pool_hits = call_host(TENSOR_STATS_POOL_HITS, &[]);
    let pool_misses = call_host(TENSOR_STATS_POOL_MISSES, &[]);
    let scratch_reuses = call_host(TENSOR_STATS_SCRATCH_REUSES, &[]);
    let dot_pass = comparable_or_faster(kernel_dot_elapsed, naive_dot_elapsed);
    let matmul_pass = comparable_or_faster(kernel_matmul_elapsed, naive_matmul_elapsed);
    let allocation_pass = pool_hits > 0 && pool_misses > 0 && scratch_reuses > 0;
    let passed = dot_pass && matmul_pass && allocation_pass;

    println!(
        "{{\"dot_len\":{},\"dot_iterations\":{},\"dot_naive_ns\":{},\"dot_kernel_ns\":{},\"dot_host_ns\":{},\"dot_pass\":{},\"mat_size\":{},\"mat_iterations\":{},\"matmul_naive_ns\":{},\"matmul_kernel_ns\":{},\"matmul_host_ns\":{},\"matmul_pass\":{},\"pool_hits\":{},\"pool_misses\":{},\"scratch_reuses\":{},\"allocation_pass\":{},\"passed\":{}}}",
        dot_len,
        dot_iterations,
        naive_dot_elapsed.as_nanos(),
        kernel_dot_elapsed.as_nanos(),
        runtime_dot_elapsed.as_nanos(),
        dot_pass,
        mat_size,
        mat_iterations,
        naive_matmul_elapsed.as_nanos(),
        kernel_matmul_elapsed.as_nanos(),
        runtime_matmul_elapsed.as_nanos(),
        matmul_pass,
        pool_hits,
        pool_misses,
        scratch_reuses,
        allocation_pass,
        passed
    );
    if !passed {
        std::process::exit(1);
    }
}
