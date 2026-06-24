// Phase 31: concurrent-fanout (Java)

import java.util.concurrent.*;

public class Bench {
    static long sumSq(long lo, long hi) {
        long s = 0L;
        for (long i = lo; i < hi; i++) {
            s += i * i;
        }
        return s;
    }

    public static void main(String[] args) throws Exception {
        final int iters = 1_000;
        long total = 0L;
        ExecutorService exec = Executors.newFixedThreadPool(8);
        try {
            for (int it = 0; it < iters; it++) {
                Future<Long>[] futures = new Future[8];
                for (int k = 0; k < 8; k++) {
                    final long lo = (long) k * 1000;
                    final long hi = (long) (k + 1) * 1000;
                    futures[k] = exec.submit(() -> sumSq(lo, hi));
                }
                long acc = 0L;
                for (int k = 0; k < 8; k++) {
                    acc += futures[k].get();
                }
                total += acc;
            }
        } finally {
            exec.shutdown();
        }
        if (total != 170634668000L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
