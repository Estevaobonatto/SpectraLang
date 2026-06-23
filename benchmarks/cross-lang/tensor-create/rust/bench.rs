// Phase 31: tensor-create (Rust)

fn main() {
    let iters: usize = 20;
    let mut total: usize = 0;
    for _ in 0..iters {
        let t: Vec<f64> = vec![1.0_f64; 1_048_576];
        total += t.len();
        drop(t);
    }
    if total != 20_971_520 {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
