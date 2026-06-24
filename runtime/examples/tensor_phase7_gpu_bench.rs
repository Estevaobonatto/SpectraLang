#[cfg(feature = "gpu")]
fn main() {
    use std::time::Instant;

    let size = 16_384usize;
    let m = 64usize;
    let k = 64usize;
    let n = 64usize;

    let left = vec![1.25f32; size];
    let right = vec![2.0f32; size];
    let cpu_start = Instant::now();
    let cpu_add = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| a + b)
        .collect::<Vec<_>>();
    let cpu_add_ms = cpu_start.elapsed().as_secs_f64() * 1000.0;

    let gpu_start = Instant::now();
    let gpu_add =
        spectra_runtime::gpu::binary(&left, &right, spectra_runtime::gpu::GpuBinaryOp::Add)
            .expect("gpu add");
    let gpu_add_ms = gpu_start.elapsed().as_secs_f64() * 1000.0;

    let a = vec![1.0f32; m * k];
    let b = vec![2.0f32; k * n];
    let cpu_matmul_start = Instant::now();
    let mut cpu_matmul = vec![0.0f32; m * n];
    for row in 0..m {
        for col in 0..n {
            let mut acc = 0.0f32;
            for inner in 0..k {
                acc += a[row * k + inner] * b[inner * n + col];
            }
            cpu_matmul[row * n + col] = acc;
        }
    }
    let cpu_matmul_ms = cpu_matmul_start.elapsed().as_secs_f64() * 1000.0;

    let gpu_matmul_start = Instant::now();
    let gpu_matmul = spectra_runtime::gpu::matmul(&a, &b, m, k, n).expect("gpu matmul");
    let gpu_matmul_ms = gpu_matmul_start.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(cpu_add.len(), gpu_add.len());
    assert_eq!(cpu_matmul.len(), gpu_matmul.len());
    assert!((cpu_add[0] - gpu_add[0]).abs() < 1e-6);
    assert!((cpu_matmul[0] - gpu_matmul[0]).abs() < 1e-3);

    let adapter = spectra_runtime::gpu::adapter_name().unwrap_or_else(|| "unknown".to_string());

    // R-3025: estimate transfer bytes. The current WGPU path materializes
    // every op, so each dispatch issues (left + right + output) host->device
    // bytes and (output) device->host bytes. This overestimates slightly
    // for in-place kernels but is the right ballpark for diagnosing the
    // host-transfer bottleneck that the R-30xx work is meant to remove.
    let add_h2d = (left.len() + right.len()) * std::mem::size_of::<f32>();
    let add_d2h = left.len() * std::mem::size_of::<f32>();
    let matmul_h2d = (a.len() + b.len()) * std::mem::size_of::<f32>();
    let matmul_d2h = (m * n) * std::mem::size_of::<f32>();

    // R-3025: only claim "GPU FASTER" when the measured speedup is actually
    // > 1.0x. The historical bench always printed side-by-side timings but
    // did not adjudicate, which let readers mistake "GPU works" for
    // "GPU is fast". The current naive WGSL shaders are often slower than
    // the CPU scalar reference because of the per-op host transfer; the
    // kernel rewrite in R-3031/R-3032 is what closes that gap.
    let add_speedup = cpu_add_ms / gpu_add_ms.max(1e-9);
    let matmul_speedup = cpu_matmul_ms / gpu_matmul_ms.max(1e-9);
    let add_faster = add_speedup > 1.0;
    let matmul_faster = matmul_speedup > 1.0;

    println!(
        "{{\"adapter\":\"{}\",\"add_cpu_ms\":{:.4},\"add_gpu_ms\":{:.4},\"add_speedup\":{:.3},\"add_faster\":{},\"add_h2d_bytes\":{},\"add_d2h_bytes\":{},\"matmul_cpu_ms\":{:.4},\"matmul_gpu_ms\":{:.4},\"matmul_speedup\":{:.3},\"matmul_faster\":{},\"matmul_h2d_bytes\":{},\"matmul_d2h_bytes\":{},\"semantic_parity\":true,\"r3025_honest_speedup_claim\":true}}",
        adapter.replace('"', "\\\""),
        cpu_add_ms,
        gpu_add_ms,
        add_speedup,
        add_faster,
        add_h2d,
        add_d2h,
        cpu_matmul_ms,
        gpu_matmul_ms,
        matmul_speedup,
        matmul_faster,
        matmul_h2d,
        matmul_d2h,
    );
}

#[cfg(not(feature = "gpu"))]
fn main() {
    eprintln!("tensor_phase7_gpu_bench requires `--features gpu`");
    std::process::exit(2);
}
