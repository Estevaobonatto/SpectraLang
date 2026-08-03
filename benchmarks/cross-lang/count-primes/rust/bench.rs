// Phase 31: count-primes (Rust)

fn main() {
    let iters: i64 = 500;
    let n: i64 = 500;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut count: i64 = 0;
        for i in 2..=n {
            let mut is_prime: i64 = 1;
            let mut d: i64 = 2;
            while d * d <= i {
                if i % d == 0 {
                    is_prime = 0;
                }
                d += 1;
            }
            if is_prime == 1 {
                count += 1;
            }
        }
        total += count;
    }
    if total != 95 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
