// Phase 31: cpu-loop-sum (Rust)
// Sum 1..N inside a tight loop. Baseline integer arithmetic benchmark.

fn main() {
    let outer: i64 = 5;
    let inner: i64 = 200_000;
    let mut acc: i64 = 0;
    for _ in 0..outer {
        let mut local: i64 = 0;
        let mut i: i64 = 1;
        while i <= inner {
            local += i;
            i += 1;
        }
        acc += local;
    }
    if acc != 100_000_500_000 {
        eprintln!("unexpected: {}", acc);
        std::process::exit(1);
    }
}
