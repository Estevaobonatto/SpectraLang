// Phase 31: string-reverse (Rust)

fn main() {
    let iters: i64 = 200_000;
    let text: &str = "The quick brown fox jumps over the lazy dog";
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut chars: Vec<u8> = text.bytes().collect();
        let mut lo: usize = 0;
        let mut hi: usize = chars.len() - 1;
        while lo < hi {
            let tmp = chars[lo];
            chars[lo] = chars[hi];
            chars[hi] = tmp;
            lo += 1;
            hi -= 1;
        }
        let mut checksum: i64 = 0;
        for (i, c) in chars.iter().enumerate() {
            checksum += (*c as i64) * (i as i64 + 1);
        }
        total += checksum;
    }
    if total != 88_994 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
