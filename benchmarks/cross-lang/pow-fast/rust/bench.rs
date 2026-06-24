// Phase 31: pow-fast (Rust)

fn main() {
    let iters: i64 = 50_000;
    let bases: [i64; 10] = [2, 3, 5, 7, 10, 13, 2, 4, 6, 8];
    let exps: [i64; 10] = [10, 8, 6, 5, 4, 3, 20, 15, 12, 10];
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut acc: i64 = 0;
        for p in 0..10 {
            let mut base = bases[p];
            let mut exp = exps[p];
            let mut result: i64 = 1;
            while exp > 0 {
                if exp % 2 == 1 {
                    result *= base;
                }
                base *= base;
                exp /= 2;
            }
            acc += result;
        }
        total += acc;
    }
    if total != 4_325_366_774 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
