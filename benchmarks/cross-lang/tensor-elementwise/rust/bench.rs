// Phase 31: tensor-elementwise (Rust)

fn main() {
    let iters: usize = 50;
    let mut checksum: f64 = 0.0;
    for _ in 0..iters {
        let mut t: Vec<f64> = vec![0.5_f64; 100_000];
        for v in t.iter_mut() {
            if *v < 0.0 {
                *v = 0.0;
            }
        }
        checksum += t[0] + t[99_999];
    }
    if checksum <= 0.0 {
        eprintln!("unexpected checksum");
        std::process::exit(1);
    }
}
