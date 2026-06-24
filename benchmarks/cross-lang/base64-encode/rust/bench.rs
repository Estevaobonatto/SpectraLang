// Phase 31: base64-encode (Rust)

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode96() -> [u8; 128] {
    let mut out = [0u8; 128];
    let mut i = 0;
    while i < 96 {
        let b0 = i as u32;
        let b1 = (i + 1) as u32;
        let b2 = (i + 2) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        let g = (i / 3) * 4;
        out[g] = ALPHABET[((n >> 18) & 63) as usize];
        out[g + 1] = ALPHABET[((n >> 12) & 63) as usize];
        if i + 1 < 96 {
            out[g + 2] = ALPHABET[((n >> 6) & 63) as usize];
        } else {
            out[g + 2] = b'=';
        }
        if i + 2 < 96 {
            out[g + 3] = ALPHABET[(n & 63) as usize];
        } else {
            out[g + 3] = b'=';
        }
        i += 3;
    }
    out
}

fn main() {
    let iters: i64 = 50_000;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let out = encode96();
        let mut checksum: i64 = 0;
        for k in 0..128 {
            checksum += out[k] as i64 * (k as i64 + 1);
        }
        total += checksum;
    }
    if total != 690_549 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
