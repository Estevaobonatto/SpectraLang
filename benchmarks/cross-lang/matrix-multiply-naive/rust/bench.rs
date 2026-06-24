// Phase 31: matrix-multiply-naive (Rust)

fn main() {
    let iters: i64 = 20_000;
    let n: usize = 16;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut a: [i64; 256] = [0; 256];
        let mut b: [i64; 256] = [0; 256];
        let mut c: [i64; 256] = [0; 256];
        for i in 0..n {
            for j in 0..n {
                let v = (i + j) as i64;
                a[i * n + j] = v - (v / 100) * 100;
            }
        }
        for i in 0..n {
            for j in 0..n {
                let v = (i * 2 + j) as i64;
                b[i * n + j] = v - (v / 100) * 100;
            }
        }
        for i in 0..n {
            for k in 0..n {
                let aik = a[i * n + k];
                for j in 0..n {
                    c[i * n + j] += aik * b[k * n + j];
                }
            }
        }
        let mut checksum: i64 = 0;
        for i in 0..n {
            for j in 0..n {
                checksum += c[i * n + j] * (i * n + j + 1) as i64;
            }
        }
        total += checksum;
    }
    if total != 232_647_680 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
