// Phase 31: gcd (Rust)

fn main() {
    let iters: i64 = 1_000_000;
    let a_vals: [i64; 10] = [48, 56, 1071, 1024, 270, 816, 462, 100, 75, 999];
    let b_vals: [i64; 10] = [36, 42, 462, 768, 192, 204, 330, 75, 125, 333];
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut acc: i64 = 0;
        for p in 0..10 {
            let mut a = a_vals[p];
            let mut b = b_vals[p];
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            acc += a;
        }
        total += acc;
    }
    if total != 962 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
