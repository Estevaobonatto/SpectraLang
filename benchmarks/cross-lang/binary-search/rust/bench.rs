// Phase 31: binary-search (Rust)

fn main() {
    let iters: i64 = 1_000_000;
    let values: [i64; 16] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30];
    let n: usize = 16;
    let targets: [i64; 4] = [14, 3, 28, 100];
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut acc: i64 = 0;
        for ti in 0..4 {
            let target = targets[ti];
            let mut low: usize = 0;
            let mut high: usize = n - 1;
            let mut found: i64 = -1;
            while low <= high {
                let mid = (low + high) / 2;
                if values[mid] == target {
                    found = mid as i64;
                    low = high + 1;
                } else if values[mid] < target {
                    low = mid + 1;
                } else {
                    high = mid - 1;
                }
            }
            acc += found;
        }
        total += acc;
    }
    if total != 19 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
