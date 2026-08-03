// Phase 31: mergesort (Rust)

fn merge_in_place(arr: &mut [i64; 64], scratch: &mut [i64; 64], lo: usize, mid: usize, hi: usize) {
    for i in lo..hi {
        scratch[i] = arr[i];
    }
    let (mut l, mut r, mut k) = (lo, mid, lo);
    while l < mid {
        if r >= hi {
            while l < mid {
                arr[k] = scratch[l];
                k += 1;
                l += 1;
            }
        } else {
            if scratch[l] <= scratch[r] {
                arr[k] = scratch[l];
                k += 1;
                l += 1;
            } else {
                arr[k] = scratch[r];
                k += 1;
                r += 1;
            }
        }
    }
    while r < hi {
        arr[k] = scratch[r];
        k += 1;
        r += 1;
    }
}

fn main() {
    let iters: i64 = 30_000;
    let src: [i64; 64] = [
        5, 16, 27, 38, 49, 60, 71, 82, 93, 7, 18, 29, 40, 51, 62, 73,
        84, 95, 9, 20, 31, 42, 53, 64, 75, 86, 0, 11, 22, 33, 44, 55,
        66, 77, 88, 2, 13, 24, 35, 46, 57, 68, 79, 90, 4, 15, 26, 37,
        48, 59, 70, 81, 92, 6, 17, 28, 39, 50, 61, 72, 83, 94, 8, 19,
    ];
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut arr: [i64; 64] = src;
        let mut scratch: [i64; 64] = [0; 64];
        let mut w: usize = 1;
        while w < 64 {
            let step = w * 2;
            let mut lo: usize = 0;
            while lo < 64 {
                let mut mid = lo + w;
                let mut hi = lo + step;
                if mid > 64 { mid = 64; }
                if hi > 64 { hi = 64; }
                merge_in_place(&mut arr, &mut scratch, lo, mid, hi);
                lo += step;
            }
            w *= 2;
        }
        let mut checksum: i64 = 0;
        for k in 0..64 {
            checksum += arr[k] * (k as i64 + 1);
        }
        total += checksum;
    }
    if total != 130_926 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
