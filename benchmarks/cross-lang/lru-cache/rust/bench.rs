// Phase 31: lru-cache (Rust)

use std::collections::HashMap;

struct LRU {
    cap: usize,
    m: HashMap<i64, usize>,
    keys: Vec<i64>,
    prev: Vec<i64>,
    next: Vec<i64>,
    head: i64,
    tail: i64,
}

impl LRU {
    fn new(cap: usize) -> Self {
        LRU {
            cap,
            m: HashMap::new(),
            keys: Vec::with_capacity(2000),
            prev: Vec::with_capacity(2000),
            next: Vec::with_capacity(2000),
            head: -1,
            tail: -1,
        }
    }

    fn add_head(&mut self, k: i64) -> usize {
        let nid = self.keys.len();
        self.keys.push(k);
        self.prev.push(-1);
        self.next.push(self.head);
        if self.head != -1 {
            self.prev[self.head as usize] = nid as i64;
        }
        self.head = nid as i64;
        if self.tail == -1 {
            self.tail = nid as i64;
        }
        nid
    }

    fn remove(&mut self, nid: usize) {
        let p = self.prev[nid];
        let n = self.next[nid];
        if p != -1 {
            self.next[p as usize] = n;
        } else {
            self.head = n;
        }
        if n != -1 {
            self.prev[n as usize] = p;
        } else {
            self.tail = p;
        }
    }

    fn get(&mut self, k: i64) -> bool {
        let nid = match self.m.get(&k) {
            Some(&v) => v,
            None => return false,
        };
        if nid as i64 != self.head {
            self.remove(nid);
            let new_nid = self.add_head(k);
            self.m.insert(k, new_nid);
        }
        true
    }

    fn put(&mut self, k: i64) {
        if self.m.contains_key(&k) {
            self.get(k);
            return;
        }
        if self.m.len() >= self.cap {
            let evicted = self.keys[self.tail as usize];
            self.m.remove(&evicted);
            self.remove(self.tail as usize);
        }
        let nid = self.add_head(k);
        self.m.insert(k, nid);
    }
}

fn main() {
    let iters: i64 = 5_000;
    let ops: i64 = 1_000;
    let mut total_hits: i64 = 0;
    for _ in 0..iters {
        let mut lru = LRU::new(16);
        let mut hits = 0;
        for t in 0..ops {
            let k: i64 = if t % 2 == 0 {
                t % 16
            } else {
                16 + (t * 3) % 64
            };
            if lru.get(k) {
                hits += 1;
            } else {
                lru.put(k);
            }
        }
        total_hits += hits;
    }
    if total_hits != 492 * iters {
        eprintln!("unexpected: {}", total_hits);
        std::process::exit(1);
    }
}
