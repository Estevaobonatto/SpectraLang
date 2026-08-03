// Phase 31: cpu-fibs (Rust)

fn main() {
    let iters: i64 = 200_000;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let (mut a, mut b): (i64, i64) = (0, 1);
        for _ in 0..40 {
            let c = a + b;
            a = b;
            b = c;
        }
        total += a;
    }
    if total != 20_466_831_000_000 {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
