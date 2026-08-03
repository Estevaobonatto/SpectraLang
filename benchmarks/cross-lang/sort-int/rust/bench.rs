// Phase 31: sort-int (Rust)

fn main() {
    let iters: i64 = 50_000;
    let n: usize = 16;
    let base: [i64; 16] = [9, 1, 5, 3, 7, 2, 8, 4, 0, 6, 11, 10, 15, 13, 14, 12];
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut values = base;
        for outer in 0..n {
            for inner in 0..(n - 1) {
                if values[inner] > values[inner + 1] {
                    let tmp = values[inner];
                    values[inner] = values[inner + 1];
                    values[inner + 1] = tmp;
                }
            }
        }
        let mut checksum: i64 = 0;
        for k in 0..n {
            checksum += values[k] * (k as i64 + 1);
        }
        total += checksum;
    }
    if total != 1360 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
