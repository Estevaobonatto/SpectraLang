// Phase 31: json-parse (Rust)

fn main() {
    let iters: i64 = 100_000;
    let doc = r#"{"a":1,"b":[2,3,4],"c":true,"d":"hi","e":-7,"f":[],"g":{}}"#;
    let bytes = doc.as_bytes();
    let n = bytes.len();
    let mut total: i64 = 0;
    for _ in 0..iters {
        let mut i: usize = 0;
        let mut tokens: i64 = 0;
        let mut intsum: i64 = 0;
        while i < n {
            let c = bytes[i] as char;
            match c {
                '{' | '}' | '[' | ']' | ',' | ':' => {
                    tokens += 1;
                    i += 1;
                }
                '"' => {
                    tokens += 1;
                    i += 1;
                    while i < n && bytes[i] != b'"' {
                        i += 1;
                    }
                    if i < n {
                        i += 1;
                    }
                }
                '-' => {
                    tokens += 1;
                    i += 1;
                    let mut neg: i64 = 0;
                    while i < n && bytes[i] >= b'0' && bytes[i] <= b'9' {
                        neg = neg * 10 + (bytes[i] - b'0') as i64;
                        i += 1;
                    }
                    intsum += -neg;
                }
                '0'..='9' => {
                    tokens += 1;
                    let mut pos: i64 = 0;
                    while i < n && bytes[i] >= b'0' && bytes[i] <= b'9' {
                        pos = pos * 10 + (bytes[i] - b'0') as i64;
                        i += 1;
                    }
                    intsum += pos;
                }
                't' => {
                    tokens += 1;
                    i += 4;
                }
                'f' => {
                    tokens += 1;
                    i += 5;
                }
                'n' => {
                    tokens += 1;
                    i += 4;
                }
                _ => {
                    i += 1;
                }
            }
        }
        total += tokens * 1000 + intsum;
    }
    if total != 37_003 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
