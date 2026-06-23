// Phase 31: tensor-matmul (Rust)

fn main() {
    let n: usize = 64;
    let iters: usize = 20;
    let mut checksum: f64 = 0.0;
    for _ in 0..iters {
        let a: Vec<f64> = vec![0.5_f64; n * n];
        let b: Vec<f64> = vec![0.25_f64; n * n];
        let mut c: Vec<f64> = vec![0.0_f64; n * n];
        for r in 0..n {
            for col in 0..n {
                let mut s = 0.0_f64;
                for k in 0..n {
                    s += a[r * n + k] * b[k * n + col];
                }
                c[r * n + col] = s;
            }
        }
        checksum += c[0] + c[n * n - 1];
    }
    if checksum <= 0.0 {
        eprintln!("unexpected checksum");
        std::process::exit(1);
    }
}
