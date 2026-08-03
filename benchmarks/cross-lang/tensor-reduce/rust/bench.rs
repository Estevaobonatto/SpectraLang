// Phase 31: tensor-reduce (Rust)

fn main() {
    let iters: usize = 50;
    let mut total: f64 = 0.0;
    for _ in 0..iters {
        let t: Vec<f64> = vec![1.0_f64; 100_000];
        let s: f64 = t.iter().sum();
        total += s;
    }
    if total < 4_999_999.0 || total > 5_000_001.0 {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
