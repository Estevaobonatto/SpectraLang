// Phase 31: concurrent-fanout (Rust)

use std::thread;

fn sum_sq(lo: i64, hi: i64) -> i64 {
    let mut s: i64 = 0;
    for i in lo..hi {
        s += i * i;
    }
    s
}

fn main() {
    let iters: i64 = 1_000;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut handles = Vec::with_capacity(8);
        for k in 0..8 {
            let lo: i64 = k as i64 * 1000;
            let hi: i64 = (k as i64 + 1) * 1000;
            handles.push(thread::spawn(move || sum_sq(lo, hi)));
        }
        let mut acc: i64 = 0;
        for h in handles {
            acc += h.join().unwrap();
        }
        total += acc;
    }
    if total != 170_634_668_000 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
