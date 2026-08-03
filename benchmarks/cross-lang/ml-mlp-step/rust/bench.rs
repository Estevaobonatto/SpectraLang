// Phase 31: ml-mlp-step (Rust)

fn main() {
    let iters: usize = 50;
    let n: usize = 64;
    let x: Vec<f64> = vec![1.0_f64; n];
    let y: Vec<f64> = vec![2.0_f64; n];
    let mut w: f64 = 0.0;
    let mut b: f64 = 0.0;
    for _ in 0..iters {
        let mut dw = 0.0_f64;
        let mut db = 0.0_f64;
        for i in 0..n {
            let p = w * x[i] + b;
            let diff = p - y[i];
            dw += diff * x[i];
            db += diff;
        }
        let nf = n as f64;
        dw /= nf;
        db /= nf;
        w -= 0.1 * dw;
        b -= 0.1 * db;
    }
    if w.is_nan() {
        eprintln!("unexpected");
        std::process::exit(1);
    }
}
