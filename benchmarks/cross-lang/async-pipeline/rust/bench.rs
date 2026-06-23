// Phase 31: async-pipeline (Rust)

use std::sync::mpsc;
use std::thread;

fn main() {
    let iters: usize = 5;
    let n: usize = 200;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let (tx, rx) = mpsc::sync_channel::<usize>(16);
        let producer = thread::spawn(move || {
            for i in 0..n {
                tx.send(i).unwrap();
            }
        });
        let consumer = thread::spawn(move || {
            let mut s: i64 = 0;
            for v in rx {
                s += v as i64;
            }
            s
        });
        producer.join().unwrap();
        let s = consumer.join().unwrap();
        total += s;
    }
    let expected: i64 = 19_900 * (iters as i64);
    if total < expected || total > expected + (iters as i64) {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
