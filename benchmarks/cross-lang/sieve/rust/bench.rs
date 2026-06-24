// Phase 31: sieve (Rust)

fn main() {
    let iters: i64 = 2_000;
    let n: usize = 200;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut sieve = vec![0i64; n + 1];
        let mut p: usize = 2;
        while p * p <= n {
            if sieve[p] == 0 {
                let mut multiple = p * p;
                while multiple <= n {
                    if sieve[multiple] == 0 {
                        sieve[multiple] = 1;
                    }
                    multiple += p;
                }
            }
            p += 1;
        }
        let mut count: i64 = 0;
        for k in 2..=n {
            if sieve[k] == 0 {
                count += 1;
            }
        }
        total += count;
    }
    if total != 46 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
