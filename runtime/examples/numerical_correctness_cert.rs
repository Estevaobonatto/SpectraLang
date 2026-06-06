use spectra_runtime::ffi::{
    lookup_host_function, SpectraHostCallContext, SpectraHostValue, HOST_STATUS_SUCCESS,
};
use spectra_runtime::stdlib::{self, NUMERICAL_TOLERANCE_ABS, NUMERICAL_TOLERANCE_REL};
use std::ptr;

const TENSOR_ARANGE: &str = "spectra.std.tensor.arange";
const TENSOR_BACKWARD: &str = "spectra.std.tensor.backward";
const TENSOR_FREE_ALL: &str = "spectra.std.tensor.free_all";
const TENSOR_FULL_F: &str = "spectra.std.tensor.full_f";
const TENSOR_GET: &str = "spectra.std.tensor.get";
const TENSOR_GET_F: &str = "spectra.std.tensor.get_f";
const TENSOR_MATMUL: &str = "spectra.std.tensor.matmul";
const TENSOR_RESHAPE: &str = "spectra.std.tensor.reshape";
const TENSOR_REQUIRES_GRAD: &str = "spectra.std.tensor.requires_grad";
const TENSOR_SEED: &str = "spectra.std.tensor.seed";
const TENSOR_SET_DETERMINISTIC_MODE: &str = "spectra.std.tensor.set_deterministic_mode";
const TENSOR_SUM_F: &str = "spectra.std.tensor.sum_f";
const TENSOR_UNIFORM: &str = "spectra.std.tensor.uniform";

const ML_CONV2D: &str = "spectra.std.ml.conv2d";
const ML_LINEAR: &str = "spectra.std.ml.linear";
const ML_MSE_LOSS: &str = "spectra.std.ml.mse_loss";
const ML_SGD_STEP: &str = "spectra.std.ml.sgd_step";

type HostFn = extern "C" fn(*mut SpectraHostCallContext) -> i32;

#[derive(Clone)]
struct Check {
    id: &'static str,
    category: &'static str,
    observed: f64,
    expected: f64,
    passed: bool,
    detail: String,
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

fn f64_arg(value: f64) -> SpectraHostValue {
    value.to_bits() as SpectraHostValue
}

fn f64_result(value: SpectraHostValue) -> f64 {
    f64::from_bits(value as u64)
}

fn close_enough(observed: f64, expected: f64) -> bool {
    let diff = (observed - expected).abs();
    diff <= NUMERICAL_TOLERANCE_ABS || diff <= NUMERICAL_TOLERANCE_REL * expected.abs().max(1.0)
}

fn check(
    id: &'static str,
    category: &'static str,
    observed: f64,
    expected: f64,
    detail: impl Into<String>,
) -> Check {
    Check {
        id,
        category,
        observed,
        expected,
        passed: close_enough(observed, expected),
        detail: detail.into(),
    }
}

fn rng_check() -> Check {
    let _ = call_host(TENSOR_SET_DETERMINISTIC_MODE, &[1]);
    let _ = call_host(TENSOR_SEED, &[20260606]);
    let first = call_host(TENSOR_UNIFORM, &[8, 0, 100]);
    let mut fingerprint = 0i64;
    for idx in 0..8 {
        fingerprint = fingerprint
            .wrapping_mul(131)
            .wrapping_add(call_host(TENSOR_GET, &[first, idx]) as i64)
            % 1_000_000_007;
    }
    let _ = call_host(TENSOR_SEED, &[20260606]);
    let second = call_host(TENSOR_UNIFORM, &[8, 0, 100]);
    let mut second_fingerprint = 0i64;
    for idx in 0..8 {
        second_fingerprint = second_fingerprint
            .wrapping_mul(131)
            .wrapping_add(call_host(TENSOR_GET, &[second, idx]) as i64)
            % 1_000_000_007;
    }
    check(
        "rng_uniform_seeded_fingerprint",
        "rng",
        fingerprint as f64,
        second_fingerprint as f64,
        format!("fingerprint={fingerprint}"),
    )
}

fn reduction_check() -> Check {
    let values = call_host(TENSOR_FULL_F, &[32, f64_arg(2.0)]);
    let observed = f64_result(call_host(TENSOR_SUM_F, &[values]));
    check(
        "reduction_sum_f",
        "reductions",
        observed,
        64.0,
        "sum_f(full_f(32,2))",
    )
}

fn matmul_check() -> Check {
    let flat = call_host(TENSOR_ARANGE, &[1, 5, 1]);
    let matrix = call_host(TENSOR_RESHAPE, &[flat, 2, 2]);
    let product = call_host(TENSOR_MATMUL, &[matrix, matrix]);
    let observed = call_host(TENSOR_GET, &[product, 0]) as f64
        + call_host(TENSOR_GET, &[product, 1]) as f64
        + call_host(TENSOR_GET, &[product, 2]) as f64
        + call_host(TENSOR_GET, &[product, 3]) as f64;
    check(
        "matmul_2x2_sum",
        "matmul",
        observed,
        54.0,
        "sum([[1,2],[3,4]]^2)",
    )
}

fn convolution_check() -> Check {
    let input = call_host(TENSOR_FULL_F, &[4, f64_arg(1.0)]);
    let kernel = call_host(TENSOR_FULL_F, &[1, f64_arg(1.0)]);
    let bias = call_host(TENSOR_FULL_F, &[1, f64_arg(0.0)]);
    let conv = call_host(ML_CONV2D, &[input, kernel, bias, 1, 1, 2, 2, 1, 1, 1]);
    let observed = f64_result(call_host(TENSOR_SUM_F, &[conv]));
    check(
        "conv2d_unit_kernel_sum",
        "convolution",
        observed,
        4.0,
        "valid 1x1 conv",
    )
}

fn optimizer_check() -> Check {
    let x = call_host(
        TENSOR_RESHAPE,
        &[call_host(TENSOR_FULL_F, &[4, f64_arg(1.0)]), 4, 1],
    );
    let target = call_host(TENSOR_FULL_F, &[4, f64_arg(2.0)]);
    let w0 = call_host(TENSOR_FULL_F, &[1, f64_arg(0.0)]);
    let w = call_host(
        TENSOR_RESHAPE,
        &[call_host(TENSOR_REQUIRES_GRAD, &[w0, 1]), 1, 1],
    );
    let b = call_host(
        TENSOR_REQUIRES_GRAD,
        &[call_host(TENSOR_FULL_F, &[1, f64_arg(0.0)]), 1],
    );
    let pred = call_host(ML_LINEAR, &[x, w, b]);
    let loss = call_host(ML_MSE_LOSS, &[pred, target]);
    let _ = call_host(TENSOR_BACKWARD, &[loss]);
    let _ = call_host(ML_SGD_STEP, &[w, f64_arg(0.1)]);
    let _ = call_host(ML_SGD_STEP, &[b, f64_arg(0.1)]);
    let new_pred = call_host(ML_LINEAR, &[x, w, b]);
    let new_loss = call_host(ML_MSE_LOSS, &[new_pred, target]);
    let observed = f64_result(call_host(TENSOR_GET_F, &[new_loss, 0]));
    check(
        "optimizer_sgd_linear_loss",
        "optimizer",
        observed,
        1.44,
        "one SGD step",
    )
}

fn json_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn print_json(checks: &[Check]) {
    let passed = checks.iter().all(|check| check.passed);
    println!("{{");
    println!("  \"schema\": \"spectra.r1503.correctness.v1\",");
    println!("  \"platform\": \"{}\",", std::env::consts::OS);
    println!("  \"arch\": \"{}\",", std::env::consts::ARCH);
    println!("  \"abs_tolerance\": {},", NUMERICAL_TOLERANCE_ABS);
    println!("  \"rel_tolerance\": {},", NUMERICAL_TOLERANCE_REL);
    println!("  \"passed\": {passed},");
    println!("  \"checks\": [");
    for (idx, check) in checks.iter().enumerate() {
        let comma = if idx + 1 == checks.len() { "" } else { "," };
        println!(
            "    {{\"id\":\"{}\",\"category\":\"{}\",\"observed\":{},\"expected\":{},\"passed\":{},\"detail\":\"{}\"}}{}",
            check.id,
            check.category,
            check.observed,
            check.expected,
            check.passed,
            json_escape(&check.detail),
            comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn main() {
    stdlib::register();
    let _ = call_host(TENSOR_FREE_ALL, &[]);
    let checks = vec![
        rng_check(),
        reduction_check(),
        matmul_check(),
        convolution_check(),
        optimizer_check(),
    ];
    print_json(&checks);
    if checks.iter().any(|check| !check.passed) {
        std::process::exit(1);
    }
}
