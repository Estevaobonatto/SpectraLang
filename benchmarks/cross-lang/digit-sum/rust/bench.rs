// Phase 31: digit-sum (Rust)

fn main() {
    let iters: i64 = 200;
    let n: i64 = 10_000;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut acc: i64 = 0;
        for i in 1..=n {
            let mut x = i;
            let mut ds: i64 = 0;
            while x > 0 {
                ds += x % 10;
                x /= 10;
            }
            acc += ds;
        }
        total += acc;
    }
    if total != 180_001 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
