// Phase 31: lru-cache (Java)

import java.util.HashMap;

public class Bench {
    static class LRU {
        int cap;
        HashMap<Long, Integer> m = new HashMap<>();
        long[] keys = new long[2000];
        int[] prev = new int[2000];
        int[] next = new int[2000];
        int head = -1;
        int tail = -1;

        LRU(int cap) { this.cap = cap; }

        int addHead(long k) {
            int nid = m.size() + countAllocs;
            keys[nid] = k;
            prev[nid] = -1;
            next[nid] = head;
            if (head != -1) prev[head] = nid;
            head = nid;
            if (tail == -1) tail = nid;
            return nid;
        }
        // dummy counter to ensure unique nid even with removes (since map.size()
        // decreases on remove). We track allocations with a static counter.
        // Simpler: use a static int allocator for the whole program.
        static int countAllocs = 0;
    }

    public static void main(String[] args) {
        final int iters = 5_000;
        final int cap = 16;
        final int ops = 1_000;
        long totalHits = 0L;
        for (int it = 0; it < iters; it++) {
            HashMap<Long, Integer> m = new HashMap<>();
            long[] keys = new long[2000];
            int[] prev = new int[2000];
            int[] next = new int[2000];
            int head = -1;
            int tail = -1;
            int nidCounter = 0;
            int hits = 0;
            for (int t = 0; t < ops; t++) {
                long k;
                if (t % 2 == 0) {
                    k = t % 16;
                } else {
                    k = 16 + ((t * 3) % 64);
                }
                if (m.containsKey(k)) {
                    hits++;
                    int oldNid = m.get(k);
                    int p = prev[oldNid];
                    int nx = next[oldNid];
                    if (p != -1) next[p] = nx;
                    if (nx != -1) prev[nx] = p;
                    if (oldNid == head) head = nx;
                    if (oldNid == tail) tail = p;
                    int newNid = nidCounter++;
                    keys[newNid] = k;
                    prev[newNid] = -1;
                    next[newNid] = head;
                    if (head != -1) prev[head] = newNid;
                    head = newNid;
                    if (tail == -1) tail = newNid;
                    m.put(k, newNid);
                } else {
                    if (m.size() >= cap) {
                        long evictedKey = keys[tail];
                        m.remove(evictedKey);
                        int p = prev[tail];
                        int nx = next[tail];
                        if (p != -1) next[p] = nx;
                        if (nx != -1) prev[nx] = p;
                        if (tail == head) head = nx;
                        tail = p;
                    }
                    int newNid = nidCounter++;
                    keys[newNid] = k;
                    prev[newNid] = -1;
                    next[newNid] = head;
                    if (head != -1) prev[head] = newNid;
                    head = newNid;
                    if (tail == -1) tail = newNid;
                    m.put(k, newNid);
                }
            }
            totalHits += hits;
        }
        if (totalHits != 492L * (long) iters) {
            System.err.println("unexpected: " + totalHits);
            System.exit(1);
        }
    }
}
