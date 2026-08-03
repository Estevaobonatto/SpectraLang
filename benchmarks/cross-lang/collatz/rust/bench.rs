// Phase 31: 3n-plus-1 (Rust)

fn main() {
    let iters: i64 = 1_000;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut acc: i64 = 0;
        for n in 1i64..=1000 {
            let mut x = n;
            let mut steps: i64 = 0;
            while x != 1 {
                if x % 2 == 0 {
                    x /= 2;
                } else {
                    x = 3 * x + 1;
                }
                steps += 1;
            }
            acc += steps;
        }
        total += acc;
    }
    if total != 59_542 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
