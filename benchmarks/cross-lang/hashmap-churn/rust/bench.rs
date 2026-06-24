// Phase 31: hashmap-churn (Rust)

use std::collections::HashMap;

fn main() {
    let iters: i64 = 2_000;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut m: HashMap<i64, i64> = HashMap::new();
        for i in 0..500 {
            m.insert(i, i * 2);
        }
        for k in 0..500 {
            if k % 2 == 1 {
                m.remove(&k);
            }
        }
        for j in 0..250 {
            m.insert(500 + j, (500 + j) * 2);
        }
        let mut acc: i64 = 0;
        for x in 0..750 {
            if m.contains_key(&x) {
                acc += x;
            }
        }
        total += acc;
    }
    if total != 218_375 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
