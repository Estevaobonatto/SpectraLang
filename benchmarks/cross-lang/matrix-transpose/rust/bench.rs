// Phase 31: matrix-transpose (Rust)

fn main() {
    let iters: i64 = 20_000;
    let rows: usize = 16;
    let cols: usize = 16;
    let mut total: i64 = 0;
    let mut m: Vec<i64> = vec![0; rows * cols];
    for _ in 0..iters {
        for r in 0..rows {
            for c in 0..cols {
                m[r * cols + c] = (r * cols + c) as i64;
            }
        }
        let mut t_checksum: i64 = 0;
        for r in 0..rows {
            for c in 0..cols {
                let val = m[r * cols + c];
                let t_pos = c * rows + r;
                t_checksum += val * (t_pos as i64 + 1);
            }
        }
        total += t_checksum;
    }
    if total != 4_368_320 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
