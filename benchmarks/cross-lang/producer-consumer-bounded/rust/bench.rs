// Phase 31: producer-consumer-bounded (Rust)

use std::sync::mpsc;
use std::thread;

fn process(n: i64) -> i64 {
    let v = n * n * n;
    v - (v / 1000) * 1000
}

fn main() {
    let iters: i64 = 200;
    let mut total: i64 = 0;
    for _ in 0..iters {
        let (tx, rx) = mpsc::sync_channel::<i64>(4); // bounded capacity 4
        let producer = thread::spawn(move || {
            for i in 0i64..500 {
                tx.send(i).unwrap();
            }
        });
        let consumer = thread::spawn(move || {
            let mut acc: i64 = 0;
            for _ in 0..500 {
                let v = rx.recv().unwrap();
                acc += process(v);
            }
            acc
        });
        producer.join().unwrap();
        let acc = consumer.join().unwrap();
        total += acc;
    }
    if total != 228_500 * iters {
        eprintln!("unexpected: {}", total);
        std::process::exit(1);
    }
}
