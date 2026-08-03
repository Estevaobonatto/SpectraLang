// Phase 31: quicksort (Rust)

fn partition(arr: &mut [i64; 64], lo: usize, hi: usize) -> usize {
    let pivot = arr[hi];
    let mut i = lo;
    for j in lo..hi {
        if arr[j] < pivot {
            arr.swap(i, j);
            i += 1;
        }
    }
    arr.swap(i, hi);
    i
}

fn qs(arr: &mut [i64; 64], lo: usize, hi: usize) {
    if lo >= hi {
        return;
    }
    let p = partition(arr, lo, hi);
    qs(arr, lo, p - 1);
    qs(arr, p + 1, hi);
}

fn main() {
    let iters: i64 = 50_000;
    let src: [i64; 64] = [
        3, 10, 17, 24, 31, 38, 45, 52, 59, 66, 73, 80, 87, 94, 1, 8,
        15, 22, 29, 36, 43, 50, 57, 64, 71, 78, 85, 92, 99, 6, 13, 20,
        27, 34, 41, 48, 55, 62, 69, 76, 83, 90, 97, 4, 11, 18, 25, 32,
        39, 46, 53, 60, 67, 74, 81, 88, 95, 2, 9, 16, 23, 30, 37, 44,
    ];
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut arr: [i64; 64] = src;
        qs(&mut arr, 0, 63);
        let mut checksum: i64 = 0;
        for k in 0..64 {
            checksum += arr[k] * (k as i64 + 1);
        }
        total += checksum;
    }
    if total != 131_629 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
