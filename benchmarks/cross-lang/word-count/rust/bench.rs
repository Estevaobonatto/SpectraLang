// Phase 31: word-count (Rust)

fn main() {
    let iters: i64 = 200_000;
    let text: &str = "The quick brown fox jumps over the lazy dog and runs away";
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut count: i64 = 0;
        let mut in_word: i64 = 0;
        for c in text.bytes() {
            let is_space: i64 = if c == b' ' { 1 } else { 0 };
            if is_space == 0 {
                if in_word == 0 {
                    count += 1;
                    in_word = 1;
                }
            } else {
                in_word = 0;
            }
        }
        total += count;
    }
    if total != 12 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
